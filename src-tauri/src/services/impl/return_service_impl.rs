use std::collections::HashMap;
use std::sync::Arc;

use crate::core::constants::EPS;
use crate::core::dtos::{validate_date_range, BillEditGuard, DeleteReturnResult,
    CustomerReturnPayload, PageRequest, Paginated, RecordRefundPayload, ReturnDetailView, ReturnItemDetail,
    ReturnResult, ReturnsFilter, VendorReturnPayload,
};
use crate::core::entities::{Item, ReturnItem, ReturnTransaction};
use crate::core::error::AppError;
use crate::core::money::{check_entry_date_now, round2, MONEY_EPS};
use crate::core::value_objects::{
    LotSource, OutflowSourceType, ReturnLineType, ReturnType, SettlementType, StockCondition,
    TransactionType,
};
use crate::data::contracts::unit_of_work::{TransactionContext, UnitOfWork};
use crate::services::contracts::accounting_service::AccountingService;
use crate::services::contracts::inventory_service::InventoryService;
use crate::services::contracts::payment_service::PaymentService;
use crate::services::contracts::return_service::ReturnService;
use crate::services::r#impl::bill_guard::{check_return_deletable, evaluate_return_guard};
use crate::services::r#impl::bill_math::net_unit_price;


#[derive(Clone)]
pub struct ReturnServiceImpl<
    I: InventoryService = Arc<dyn InventoryService>,
    P: PaymentService = Arc<dyn PaymentService>,
    A: AccountingService = Arc<dyn AccountingService>,
    U: UnitOfWork = Arc<dyn UnitOfWork>,
> {
    pub inventory_service: I,
    pub payment_service: P,
    pub accounting_service: A,
    pub uow: U,
}

impl<I: InventoryService, P: PaymentService, A: AccountingService, U: UnitOfWork>
    ReturnServiceImpl<I, P, A, U>
{
    pub fn new(inventory_service: I, payment_service: P, accounting_service: A, uow: U) -> Self {
        Self { inventory_service, payment_service, accounting_service, uow }
    }

    /// Credits dues first, then books the cash remainder (DD-03). `linked_value` is credited to
    /// `bill_id` only; `unlinked_value` is applied FIFO across the party's open bills (walk-in: none).
    #[allow(clippy::too_many_arguments)]
    fn settle(
        &self,
        tx: &mut dyn TransactionContext,
        tx_type: TransactionType,
        party_id: Option<i64>,
        bill_id: Option<i64>,
        return_id: i64,
        date: &str,
        linked_value: f64,
        unlinked_value: f64,
        note: &str,
    ) -> Result<(f64, f64), AppError> {
        let total = round2(linked_value + unlinked_value);
        let mut credited = 0.0;
        if linked_value > MONEY_EPS {
            credited += self.accounting_service.apply_return_credit(
                tx, tx_type, party_id, bill_id, return_id, date, linked_value,
            )?;
        }
        if unlinked_value > MONEY_EPS {
            credited += self.accounting_service.apply_return_credit(
                tx, tx_type, party_id, None, return_id, date, unlinked_value,
            )?;
        }
        let credited = round2(credited);
        let cash = round2((total - credited).max(0.0));
        if cash > MONEY_EPS {
            self.payment_service.record_refund(
                tx,
                RecordRefundPayload {
                    party_id,
                    transaction_type: tx_type,
                    transaction_id: bill_id,
                    return_id: Some(return_id),
                    refund_date: date.to_string(),
                    amount: cash,
                    payment_mode: Some("CASH".to_string()),
                    notes: Some(note.to_string()),
                },
            )?;
        }
        Ok((credited, cash))
    }
}

fn validation(field: &str, message: impl Into<String>) -> AppError {
    AppError::ValidationError { field: field.to_string(), message: message.into() }
}

fn check_qty(field: &str, q: f64) -> Result<(), AppError> {
    if !q.is_finite() || q <= 0.0 {
        return Err(validation(field, format!("Quantity must be a positive finite number, got {q}")));
    }
    Ok(())
}

/// Resolves the effective party: defaults to the bill's party and must match it when given.
fn resolve_party(
    tx: &mut dyn TransactionContext,
    payload_party: Option<i64>,
    bill_party: Option<i64>,
    has_bill: bool,
) -> Result<(Option<i64>, Option<String>), AppError> {
    let party_id = if has_bill {
        if let Some(p) = payload_party {
            if bill_party != Some(p) {
                return Err(validation("party_id", "party_id does not match the original bill's party"));
            }
        }
        bill_party
    } else {
        payload_party
    };
    let party_name = match party_id {
        Some(pid) => Some(
            tx.parties()
                .find_by_id(pid)?
                .ok_or_else(|| AppError::not_found("Party", pid.to_string()))?
                .name,
        ),
        None => None,
    };
    Ok((party_id, party_name))
}

fn active_item(tx: &mut dyn TransactionContext, item_id: i64) -> Result<Item, AppError> {
    let item = tx
        .items()
        .find_by_id(item_id)?
        .ok_or_else(|| AppError::not_found("Item", item_id.to_string()))?;
    if !item.is_active {
        return Err(validation("item_id", format!("Item '{}' is inactive", item.name)));
    }
    Ok(item)
}

fn active_item_name(tx: &mut dyn TransactionContext, item_id: i64) -> Result<String, AppError> {
    Ok(active_item(tx, item_id)?.name)
}

impl<I: InventoryService, P: PaymentService, A: AccountingService, U: UnitOfWork>
    ReturnServiceImpl<I, P, A, U>
{
    fn create_customer_return_in_tx(&self, tx: &mut dyn TransactionContext, payload: &CustomerReturnPayload) -> Result<ReturnResult, AppError> {
        // ---- Payload-only validation -------------------------------------------------------
        check_entry_date_now("return_date", &payload.return_date)?;
        if payload.items.is_empty() {
            return Err(validation("items", "A return needs at least one returned line"));
        }
        for it in &payload.items {
            check_qty("quantity", it.quantity)?;
            if !it.unit_price.is_finite() || it.unit_price < 0.0 {
                return Err(validation("unit_price", "unit_price must be a non-negative finite number"));
            }
            if let Some(c) = it.unit_cost_override {
                if !c.is_finite() || c <= 0.0 {
                    return Err(validation("unit_cost_override", "unit_cost_override must be positive"));
                }
            }
            if it.sale_item_id.is_none() && payload.settlement_type != SettlementType::Refund {
                return Err(validation("sale_item_id", "Replacement returns require every returned line to be linked"));
            }
            if it.sale_item_id.is_some() && payload.original_sale_id.is_none() {
                return Err(validation("original_sale_id", "original_sale_id is required for linked lines"));
            }
        }
        match payload.settlement_type {
            SettlementType::Refund => {
                if !payload.replacement_items.is_empty() {
                    return Err(validation("replacement_items", "replacement_items must be empty for a Refund"));
                }
            }
            SettlementType::Replacement => {
                if payload.original_sale_id.is_none() {
                    return Err(validation("original_sale_id", "A Replacement requires the original sale"));
                }
                if payload.replacement_items.is_empty() {
                    return Err(validation("replacement_items", "A Replacement needs at least one issued item"));
                }
                for r in &payload.replacement_items {
                    check_qty("replacement_items.quantity", r.quantity)?;
                }
            }
        }

        // ---- Validation against stored data (read-only, before any write) -------------------
        let sale = match payload.original_sale_id {
            Some(sid) => Some(
                tx.sales()
                    .find_transaction_by_id(sid)?
                    .ok_or_else(|| AppError::not_found("SaleTransaction", sid.to_string()))?,
            ),
            None => None,
        };
        if sale.as_ref().is_some_and(|s| s.is_opening_balance) {
            return Err(validation("original_sale_id", "An opening balance has no items to return"));
        }
        let (party_id, party_name) = resolve_party(
            tx,
            payload.party_id,
            sale.as_ref().and_then(|s| s.party_id),
            sale.is_some(),
        )?;

        let mut per_line_qty: HashMap<i64, f64> = HashMap::new();
        let mut item_names: Vec<String> = Vec::with_capacity(payload.items.len());
        for it in &payload.items {
            match it.sale_item_id {
                Some(sid) => {
                    let line = tx
                        .sales()
                        .find_sale_item_by_id(sid)?
                        .ok_or_else(|| AppError::not_found("SaleItem", sid.to_string()))?;
                    if Some(line.sale_transaction_id) != payload.original_sale_id {
                        return Err(validation("sale_item_id", format!("Sale line {sid} does not belong to the original sale")));
                    }
                    if line.item_id != it.item_id {
                        return Err(validation("item_id", format!("Item does not match sale line {sid}")));
                    }
                    // Refund cap = the sold rate net of the bill-level discount share (FR-19).
                    let s = sale.as_ref().expect("a linked line implies the original sale");
                    let cap = net_unit_price(line.unit_price, s.items_subtotal, s.discounts_total);
                    if it.unit_price > cap + MONEY_EPS {
                        return Err(validation(
                            "unit_price",
                            format!("unit_price {} exceeds the net sold rate {:.4} (after bill discounts)", it.unit_price, cap),
                        ));
                    }
                    *per_line_qty.entry(sid).or_insert(0.0) += it.quantity;
                    item_names.push(line.item_name);
                }
                None => item_names.push(active_item_name(tx, it.item_id)?),
            }
        }
        for (sid, qty) in &per_line_qty {
            let line = tx.sales().find_sale_item_by_id(*sid)?.ok_or_else(|| AppError::not_found("SaleItem", sid.to_string()))?;
            let returned = tx.sales().get_returned_quantity_for_line(*sid)?;
            if *qty > line.quantity - returned + EPS {
                return Err(validation(
                    "quantity",
                    format!("Cannot return {qty} of sale line {sid}: sold {}, already returned {returned}", line.quantity),
                ));
            }
        }
        let mut issue_names: Vec<String> = Vec::new();
        let mut issued_value = 0.0;
        for r in &payload.replacement_items {
            let item = active_item(tx, r.item_id)?;
            issued_value += r.quantity * item.default_sale_price;
            issue_names.push(item.name);
        }

        // ---- Writes -----------------------------------------------------------------------
        let line_totals: Vec<f64> =
            payload.items.iter().map(|i| round2(i.quantity * i.unit_price)).collect();
        let total_amount = round2(payload.items.iter().map(|i| i.quantity * i.unit_price).sum());
        let return_code = tx.returns().get_next_return_code()?;
        let return_id = tx.returns().create_return_transaction(&ReturnTransaction {
            id: 0,
            return_code: return_code.clone(),
            return_date: payload.return_date.clone(),
            return_type: ReturnType::Customer,
            party_id,
            party_name,
            original_transaction_id: payload.original_sale_id,
            settlement_type: payload.settlement_type,
            total_amount,
            reason: payload.reason.clone(),
            notes: payload.notes.clone(),
            created_at: chrono::Utc::now().to_rfc3339(),
        })?;

        let (mut linked_value, mut unlinked_value) = (0.0, 0.0);
        for (idx, it) in payload.items.iter().enumerate() {
            let restock = it.condition == StockCondition::Restockable;
            let mut return_lot: Option<i64> = None;
            let cost = match it.sale_item_id {
                Some(sid) => {
                    linked_value += line_totals[idx];
                    self.inventory_service.reverse_sale_line(tx, sid, it.item_id, it.quantity, restock)?
                }
                None => {
                    unlinked_value += line_totals[idx];
                    if restock {
                        let lot_id = self.inventory_service.create_source_lot(
                            tx,
                            LotSource::Return,
                            &payload.return_date,
                            it.item_id,
                            it.quantity,
                            it.unit_cost_override,
                            Some(format!("Return {return_code}")),
                        )?;
                        let lot = tx
                            .purchases()
                            .find_purchase_item_by_id(lot_id)?
                            .ok_or_else(|| AppError::not_found("PurchaseItem", lot_id.to_string()))?;
                        return_lot = Some(lot_id); // FR-20: lets a return delete find its lot
                        it.quantity * lot.unit_cost
                    } else {
                        0.0
                    }
                }
            };
            tx.returns().create_return_item(&ReturnItem {
                id: 0,
                return_transaction_id: return_id,
                line_type: ReturnLineType::Returned,
                item_id: it.item_id,
                item_name: item_names[idx].clone(),
                sale_item_id: it.sale_item_id,
                purchase_item_id: return_lot,
                quantity: it.quantity,
                unit_price: it.unit_price,
                total_amount: line_totals[idx],
                cost_amount: round2(cost),
                condition: Some(it.condition),
            })?;
        }

        let (mut credited, mut cash) = (0.0, 0.0);
        match payload.settlement_type {
            SettlementType::Refund => {
                (credited, cash) = self.settle(
                    tx,
                    TransactionType::Sale,
                    party_id,
                    payload.original_sale_id,
                    return_id,
                    &payload.return_date,
                    linked_value,
                    unlinked_value,
                    "Customer return refund",
                )?;
            }
            SettlementType::Replacement => {
                // Stock check after the returned units are restocked, so swapping a just-sold
                // last unit works; any shortfall rolls the whole transaction back.
                let mut reqs: Vec<(i64, f64)> = Vec::new();
                for r in &payload.replacement_items {
                    match reqs.iter_mut().find(|(i, _)| *i == r.item_id) {
                        Some(e) => e.1 += r.quantity,
                        None => reqs.push((r.item_id, r.quantity)),
                    }
                }
                self.inventory_service.validate_stock_availability(tx, &reqs)?;
                for (idx, r) in payload.replacement_items.iter().enumerate() {
                    let cost = self.inventory_service.issue_stock_outflow(
                        tx,
                        OutflowSourceType::Replacement,
                        return_id,
                        r.item_id,
                        r.quantity,
                    )?;
                    tx.returns().create_return_item(&ReturnItem {
                        id: 0,
                        return_transaction_id: return_id,
                        line_type: ReturnLineType::ReplacementIssued,
                        item_id: r.item_id,
                        item_name: issue_names[idx].clone(),
                        sale_item_id: None,
                        purchase_item_id: None,
                        quantity: r.quantity,
                        unit_price: 0.0,
                        total_amount: 0.0,
                        cost_amount: round2(cost),
                        condition: None,
                    })?;
                }
            }
        }
        Ok(ReturnResult { return_id, return_code, credited_amount: credited, cash_refunded: cash, total_amount, issued_value: round2(issued_value) })
    }

    fn create_vendor_return_in_tx(&self, tx: &mut dyn TransactionContext, payload: &VendorReturnPayload) -> Result<ReturnResult, AppError> {
        check_entry_date_now("return_date", &payload.return_date)?;
        if payload.settlement_type == SettlementType::Replacement {
            return Err(validation("settlement_type", "Replacement is customer-only; vendor returns are always Refund"));
        }
        if payload.items.is_empty() {
            return Err(validation("items", "A return needs at least one line"));
        }
        for it in &payload.items {
            check_qty("quantity", it.quantity)?;
        }

        let purchase = match payload.original_purchase_id {
            Some(pid) => {
                let p = tx
                    .purchases()
                    .find_transaction_by_id(pid)?
                    .ok_or_else(|| AppError::not_found("PurchaseTransaction", pid.to_string()))?;
                if p.is_opening_balance {
                    return Err(validation("original_purchase_id", "An opening balance has no items to return"));
                }
                if p.source != LotSource::Purchase {
                    return Err(validation("original_purchase_id", "Only a PURCHASE bill can be returned to a vendor"));
                }
                Some(p)
            }
            None => None,
        };
        let (party_id, party_name) = resolve_party(
            tx,
            payload.party_id,
            purchase.as_ref().and_then(|p| p.party_id),
            purchase.is_some(),
        )?;

        let mut per_lot: HashMap<i64, f64> = HashMap::new();
        let mut fifo_reqs: Vec<(i64, f64)> = Vec::new();
        let mut item_names: Vec<String> = Vec::new();
        for it in &payload.items {
            item_names.push(
                tx.items()
                    .find_by_id(it.item_id)?
                    .ok_or_else(|| AppError::not_found("Item", it.item_id.to_string()))?
                    .name,
            );
            match it.purchase_item_id {
                Some(lid) => {
                    let lot = tx
                        .purchases()
                        .find_purchase_item_by_id(lid)?
                        .ok_or_else(|| AppError::not_found("PurchaseItem", lid.to_string()))?;
                    let header = tx
                        .purchases()
                        .find_transaction_by_id(lot.purchase_transaction_id)?
                        .ok_or_else(|| AppError::not_found("PurchaseTransaction", lot.purchase_transaction_id.to_string()))?;
                    if header.source != LotSource::Purchase {
                        return Err(validation("purchase_item_id", "Only PURCHASE-source lots can be returned to a vendor"));
                    }
                    if lot.item_id != it.item_id {
                        return Err(validation("item_id", format!("Item does not match lot {lid}")));
                    }
                    if let Some(pid) = payload.original_purchase_id {
                        if lot.purchase_transaction_id != pid {
                            return Err(validation("purchase_item_id", format!("Lot {lid} does not belong to the original purchase")));
                        }
                    }
                    *per_lot.entry(lid).or_insert(0.0) += it.quantity;
                }
                None => match fifo_reqs.iter_mut().find(|(i, _)| *i == it.item_id) {
                    Some(e) => e.1 += it.quantity,
                    None => fifo_reqs.push((it.item_id, it.quantity)),
                },
            }
        }
        for (lid, qty) in &per_lot {
            let lot = tx.purchases().find_purchase_item_by_id(*lid)?.ok_or_else(|| AppError::not_found("PurchaseItem", lid.to_string()))?;
            if *qty > lot.remaining_quantity + EPS {
                return Err(validation(
                    "quantity",
                    format!("Cannot return {qty} from lot {lid}: only {} remaining", lot.remaining_quantity),
                ));
            }
        }
        if !fifo_reqs.is_empty() {
            self.inventory_service.validate_stock_availability(tx, &fifo_reqs)?;
        }

        // ---- Writes -----------------------------------------------------------------------
        let return_code = tx.returns().get_next_return_code()?;
        let return_id = tx.returns().create_return_transaction(&ReturnTransaction {
            id: 0,
            return_code: return_code.clone(),
            return_date: payload.return_date.clone(),
            return_type: ReturnType::Vendor,
            party_id,
            party_name,
            original_transaction_id: payload.original_purchase_id,
            settlement_type: SettlementType::Refund,
            total_amount: 0.0,
            reason: payload.reason.clone(),
            notes: payload.notes.clone(),
            created_at: chrono::Utc::now().to_rfc3339(),
        })?;
        let mut total = 0.0;
        for (idx, it) in payload.items.iter().enumerate() {
            // DD-11: the lot cost removed (named lot or FIFO) is the line's value and cost.
            let value = self.inventory_service.remove_stock_for_vendor_return(
                tx,
                return_id,
                it.item_id,
                it.purchase_item_id,
                it.quantity,
            )?;
            total += value;
            tx.returns().create_return_item(&ReturnItem {
                id: 0,
                return_transaction_id: return_id,
                line_type: ReturnLineType::Returned,
                item_id: it.item_id,
                item_name: item_names[idx].clone(),
                sale_item_id: None,
                purchase_item_id: it.purchase_item_id,
                quantity: it.quantity,
                unit_price: value / it.quantity,
                total_amount: round2(value),
                cost_amount: round2(value),
                condition: None, // condition applies to customer lines only
            })?;
        }
        let total = round2(total);
        tx.returns().update_return_total(return_id, total)?;
        let (credited, cash) = self.settle(
            tx,
            TransactionType::Purchase,
            party_id,
            payload.original_purchase_id,
            return_id,
            &payload.return_date,
            total,
            0.0,
            "Vendor return refund",
        )?;
        Ok(ReturnResult { return_id, return_code, credited_amount: credited, cash_refunded: cash, total_amount: total, issued_value: 0.0 })
    }
}

impl<I: InventoryService, P: PaymentService, A: AccountingService, U: UnitOfWork> ReturnService
    for ReturnServiceImpl<I, P, A, U>
{
    fn get_next_return_code(&self) -> Result<String, AppError> {
        // Display-only preview: the code is assigned for real inside the create_* write transaction.
        let mut tx = self.uow.begin_read()?;
        tx.returns().get_next_return_code()
    }

    fn create_customer_return(&self, payload: CustomerReturnPayload) -> Result<ReturnResult, AppError> {
        let mut tx = self.uow.begin_transaction()?;
        let result = self.create_customer_return_in_tx(&mut *tx, &payload)?;
        tx.commit()?;
        Ok(result)
    }

    fn preview_customer_return(&self, payload: CustomerReturnPayload) -> Result<ReturnResult, AppError> {
        let mut tx = self.uow.begin_transaction()?;
        let result = self.create_customer_return_in_tx(&mut *tx, &payload);
        tx.rollback()?; // never committed
        result
    }

    fn create_vendor_return(&self, payload: VendorReturnPayload) -> Result<ReturnResult, AppError> {
        let mut tx = self.uow.begin_transaction()?;
        let result = self.create_vendor_return_in_tx(&mut *tx, &payload)?;
        tx.commit()?;
        Ok(result)
    }

    fn preview_vendor_return(&self, payload: VendorReturnPayload) -> Result<ReturnResult, AppError> {
        let mut tx = self.uow.begin_transaction()?;
        let result = self.create_vendor_return_in_tx(&mut *tx, &payload);
        tx.rollback()?; // never committed
        result
    }

    fn list_returns(&self, filter: ReturnsFilter) -> Result<Paginated<ReturnTransaction>, AppError> {
        validate_date_range(filter.start_date.as_deref(), filter.end_date.as_deref())?;
        let page = PageRequest::new(filter.page, filter.limit)?;
        let mut tx = self.uow.begin_read()?;
        let (rows, total) = tx.returns().list_returns_page(&filter, page.offset(), page.limit)?;
        Ok(Paginated::new(rows, total, page.page, page.limit))
    }

    fn delete_return(&self, return_id: i64) -> Result<DeleteReturnResult, AppError> {
        let mut tx = self.uow.begin_transaction()?;
        let r = tx
            .returns()
            .find_return_by_id(return_id)?
            .ok_or_else(|| AppError::not_found("ReturnTransaction", return_id.to_string()))?;
        let plan = check_return_deletable(&mut *tx, &r, &self.inventory_service)?;
        let (cash, credit) = self.payment_service.delete_return_ledger(&mut *tx, return_id)?;
        tx.returns().delete_return(return_id)?;
        // After the lines are gone: a RETURN lot may only be deleted once nothing references it.
        self.inventory_service.apply_return_undo(&mut *tx, &plan)?;
        tx.commit()?;
        Ok(DeleteReturnResult { removed_cash_amount: cash, removed_credit_amount: credit })
    }

    fn get_return_delete_guard(&self, return_id: i64) -> Result<BillEditGuard, AppError> {
        let mut tx = self.uow.begin_read()?;
        evaluate_return_guard(&mut *tx, return_id, &self.inventory_service)
    }

    fn get_return_details(&self, return_id: i64) -> Result<ReturnDetailView, AppError> {
        let mut tx = self.uow.begin_read()?;
        let ret_tx = tx
            .returns()
            .find_return_by_id(return_id)?
            .ok_or_else(|| AppError::not_found("ReturnTransaction", return_id.to_string()))?;
        let party = match ret_tx.party_id {
            Some(pid) => tx.parties().find_by_id(pid)?,
            None => None,
        };
        let items = tx
            .returns()
            .find_items_by_return_id(return_id)?
            .into_iter()
            .map(|it| ReturnItemDetail { item_name: it.item_name.clone(), return_item: it }) // DD-30 snapshot
            .collect();
        let source = match (ret_tx.return_type, ret_tx.settlement_type) {
            (ReturnType::Vendor, _) => Some(OutflowSourceType::VendorReturn),
            (ReturnType::Customer, SettlementType::Replacement) => Some(OutflowSourceType::Replacement),
            _ => None,
        };
        let allocations = match source {
            Some(s) => tx.outflow_allocations().find_by_source(s, return_id)?,
            None => Vec::new(),
        };
        let ledger = tx.returns().find_ledger_by_return_id(return_id)?;
        Ok(ReturnDetailView { return_tx: ret_tx, party, items, allocations, ledger })
    }
}
