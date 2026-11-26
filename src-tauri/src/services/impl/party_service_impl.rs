use std::sync::Arc;
use crate::core::dtos::{DeleteMode, DeletePartyResult, DeletedPartyRow, PageRequest, Paginated, PartyDeletePreview};
use crate::core::entities::{
    CreatePartyPayload, OpeningBalanceInput, Party, PartyListRow, PurchaseTransaction, SaleTransaction,
    UpdatePartyPayload,
};
use crate::core::error::AppError;
use crate::core::money::{check_entry_date_now, round2, MONEY_EPS};
use crate::core::value_objects::{LotSource, PartyType, PaymentStatus, TransactionType};
use crate::data::contracts::search_indexer::FtsIndex;
use crate::data::contracts::unit_of_work::{TransactionContext, UnitOfWork};
use crate::data::sqlite::fts::sync_after_commit;
use crate::services::contracts::party_service::PartyService;
use crate::services::r#impl::payment_service_impl::recompute_bill;

fn validation(field: &str, message: impl Into<String>) -> AppError {
    AppError::ValidationError { field: field.to_string(), message: message.into() }
}

/// `500` for whole rupees, `500.50` otherwise.
fn rupees(v: f64) -> String {
    if (v - v.round()).abs() < MONEY_EPS { format!("{}", v.round() as i64) } else { format!("{v:.2}") }
}

/// Validated opening-balance input: `(amount, as_of_date)` (FR-35).
fn check_opening(input: &OpeningBalanceInput) -> Result<(f64, String), AppError> {
    if !input.amount.is_finite() {
        return Err(validation("opening_balance.amount", "Opening balance must be a finite number"));
    }
    let amount = round2(input.amount);
    if amount < 0.0 {
        return Err(validation("opening_balance.amount", "Opening balance cannot be negative"));
    }
    let date = input.as_of_date.trim().to_string();
    check_entry_date_now("opening_balance.as_of_date", &date)?;
    Ok((amount, date))
}

/// Creates, updates or removes the party's opening bill inside the caller's transaction (FR-35).
/// The bill has no lines and no lots; `recompute_bill` sets paid / balance / status from the ledger.
pub(crate) fn apply_opening_balance(
    tx: &mut dyn TransactionContext,
    party: &Party,
    input: &OpeningBalanceInput,
) -> Result<(), AppError> {
    let (amount, date) = check_opening(input)?;
    let now = chrono::Utc::now().to_rfc3339();
    match party.party_type {
        PartyType::Customer => {
            let existing = tx.sales().find_opening_for_party(party.id)?;
            let bill_id = match existing {
                None => {
                    if amount <= MONEY_EPS {
                        return Ok(());
                    }
                    let h = SaleTransaction {
                        id: 0,
                        transaction_code: tx.sales().next_opening_code()?,
                        transaction_date: date,
                        party_id: Some(party.id),
                        party_name: Some(party.name.clone()),
                        payment_mode: None,
                        payment_status: PaymentStatus::Unpaid,
                        total_amount: amount,
                        paid_amount: 0.0,
                        balance_due: amount,
                        notes: None,
                        created_at: now,
                        items_subtotal: amount,
                        charges_total: 0.0,
                        discounts_total: 0.0,
                        updated_at: None,
                        is_opening_balance: true,
                    };
                    tx.sales().create_transaction(&h)?
                }
                Some(mut h) => {
                    check_not_below_applied(tx, TransactionType::Sale, h.id, amount)?;
                    if amount <= MONEY_EPS {
                        tx.sales().delete_transaction(h.id)?;
                        return Ok(());
                    }
                    h.transaction_date = date;
                    h.party_name = Some(party.name.clone());
                    h.items_subtotal = amount;
                    h.total_amount = amount;
                    h.updated_at = Some(now);
                    tx.sales().update_header_for_edit(&h)?;
                    h.id
                }
            };
            recompute_bill(tx, TransactionType::Sale, bill_id)
        }
        PartyType::Vendor => {
            let existing = tx.purchases().find_opening_for_party(party.id)?;
            let bill_id = match existing {
                None => {
                    if amount <= MONEY_EPS {
                        return Ok(());
                    }
                    let h = PurchaseTransaction {
                        id: 0,
                        transaction_code: tx.purchases().next_opening_code()?,
                        source: LotSource::Purchase,
                        supplier_invoice_ref: None,
                        transaction_date: date,
                        party_id: Some(party.id),
                        party_name: Some(party.name.clone()),
                        payment_mode: None,
                        payment_status: PaymentStatus::Unpaid,
                        total_amount: amount,
                        paid_amount: 0.0,
                        balance_due: amount,
                        notes: None,
                        created_at: now,
                        items_subtotal: amount,
                        charges_total: 0.0,
                        discounts_total: 0.0,
                        updated_at: None,
                        is_opening_balance: true,
                    };
                    tx.purchases().create_transaction(&h)?
                }
                Some(mut h) => {
                    check_not_below_applied(tx, TransactionType::Purchase, h.id, amount)?;
                    if amount <= MONEY_EPS {
                        tx.purchases().delete_transaction(h.id)?;
                        return Ok(());
                    }
                    h.transaction_date = date;
                    h.party_name = Some(party.name.clone());
                    h.items_subtotal = amount;
                    h.total_amount = amount;
                    h.updated_at = Some(now);
                    tx.purchases().update_header_for_edit(&h)?;
                    h.id
                }
            };
            recompute_bill(tx, TransactionType::Purchase, bill_id)
        }
    }
}

/// Dry-run twin of `apply_opening_balance` for imports (FR-41): the same amount / date checks
/// and, for a party that already has an opening bill, the "not below what is settled" check.
/// Reads only. `party` is `None` for a party that does not exist yet.
pub(crate) fn check_opening_update(
    tx: &mut dyn TransactionContext,
    party: Option<&Party>,
    input: &OpeningBalanceInput,
) -> Result<(), AppError> {
    let (amount, _) = check_opening(input)?;
    let Some(party) = party else { return Ok(()) };
    let (tx_type, bill_id) = match party.party_type {
        PartyType::Customer => (TransactionType::Sale, tx.sales().find_opening_for_party(party.id)?.map(|h| h.id)),
        PartyType::Vendor => (TransactionType::Purchase, tx.purchases().find_opening_for_party(party.id)?.map(|h| h.id)),
    };
    match bill_id {
        Some(id) => check_not_below_applied(tx, tx_type, id, amount),
        None => Ok(()),
    }
}

/// The new amount cannot go below what settlements and return credits already applied.
fn check_not_below_applied(
    tx: &mut dyn TransactionContext,
    tx_type: TransactionType,
    bill_id: i64,
    amount: f64,
) -> Result<(), AppError> {
    let totals = tx.settlements().sum_by_kind_for_transaction(tx_type, bill_id)?;
    let applied = round2(totals.settlement + totals.return_credit);
    if amount + MONEY_EPS < applied {
        return Err(validation(
            "opening_balance.amount",
            format!("₹{} is already settled against the opening balance", rupees(applied)),
        ));
    }
    Ok(())
}

/// FR-46 / Y3: every write to a deleted party is refused except restore.
fn ensure_party_active(party: &Party) -> Result<(), AppError> {
    if party.is_active {
        Ok(())
    } else {
        Err(validation("party_id", format!("'{}' is deleted. Restore it first.", party.name)))
    }
}

fn plural(n: i64, one: &str, many: &str) -> String {
    format!("{n} {}", if n == 1 { one } else { many })
}

/// Why a party is SOFT: what references it ("3 bills", "1 payment"). Empty = HARD.
fn usage_reasons(tx: &mut dyn TransactionContext, id: i64) -> Result<Vec<String>, AppError> {
    let (sales, purchases, returns, payments) = tx.parties().reference_breakdown(id)?;
    let mut out = Vec::new();
    if sales + purchases > 0 {
        out.push(plural(sales + purchases, "bill", "bills"));
    }
    if returns > 0 {
        out.push(plural(returns, "return", "returns"));
    }
    if payments > 0 {
        out.push(plural(payments, "payment", "payments"));
    }
    Ok(out)
}

fn dues_block_message(total: f64, count: i64) -> String {
    format!("₹{} due on {}. Settle them first.", rupees(total), plural(count, "bill", "bills"))
}

fn party_not_found(id: i64) -> AppError {
    AppError::NotFound { entity: "Party".to_string(), id: id.to_string() }
}

#[derive(Clone)]
pub struct PartyServiceImpl<U: UnitOfWork = Arc<dyn UnitOfWork>> {
    pub uow: U,
    pub search_index: Arc<dyn FtsIndex>,
}

impl<U: UnitOfWork> PartyServiceImpl<U> {
    pub fn new(uow: U, search_index: Arc<dyn FtsIndex>) -> Self {
        Self { uow, search_index }
    }
}

impl<U: UnitOfWork> PartyService for PartyServiceImpl<U> {
    fn create_party(&self, payload: CreatePartyPayload) -> Result<i64, AppError> {
        if payload.name.trim().is_empty() {
            return Err(AppError::ValidationError {
                field: "name".to_string(),
                message: "Party name cannot be empty".to_string(),
            });
        }

        let mut tx = self.uow.begin_transaction()?;
        let party = Party {
            id: 0,
            name: payload.name.trim().to_string(),
            phone: payload.phone.map(|p| p.trim().to_string()).filter(|p| !p.is_empty()),
            party_type: payload.party_type,
            notes: payload.notes,
            is_active: true,
            created_at: chrono::Utc::now().to_rfc3339(),
            updated_at: chrono::Utc::now().to_rfc3339(),
            opening_balance: None,
        };

        let party_id = tx.parties().create(&party)?;
        if let Some(input) = payload.opening_balance.as_ref() {
            let created = Party { id: party_id, ..party };
            apply_opening_balance(&mut *tx, &created, input)?;
        }
        tx.commit()?;
        sync_after_commit(&*self.search_index, party_id, "party");
        Ok(party_id)
    }

    fn update_party(&self, id: i64, payload: UpdatePartyPayload) -> Result<(), AppError> {
        if payload.name.trim().is_empty() {
            return Err(AppError::ValidationError {
                field: "name".to_string(),
                message: "Party name cannot be empty".to_string(),
            });
        }

        let mut tx = self.uow.begin_transaction()?;
        let mut party = tx
            .parties()
            .find_by_id(id)?
            .ok_or_else(|| AppError::NotFound {
                entity: "Party".to_string(),
                id: id.to_string(),
            })?;

        ensure_party_active(&party)?;
        if party.opening_balance.is_some() && payload.party_type != party.party_type {
            return Err(validation(
                "party_type",
                "This party has an opening balance. Remove it before changing the party type",
            ));
        }
        party.name = payload.name.trim().to_string();
        party.phone = payload.phone.map(|p| p.trim().to_string()).filter(|p| !p.is_empty());
        party.party_type = payload.party_type;
        party.notes = payload.notes;
        party.updated_at = chrono::Utc::now().to_rfc3339();

        tx.parties().update(&party)?;
        if let Some(input) = payload.opening_balance.as_ref() {
            apply_opening_balance(&mut *tx, &party, input)?;
        }
        tx.commit()?;
        sync_after_commit(&*self.search_index, id, "party");
        Ok(())
    }

    fn get_party(&self, id: i64) -> Result<Party, AppError> {
        let mut tx = self.uow.begin_read()?;
        tx.parties()
            .find_by_id(id)?
            .ok_or_else(|| AppError::NotFound {
                entity: "Party".to_string(),
                id: id.to_string(),
            })
    }

    fn list_parties(
        &self,
        party_type: Option<PartyType>,
        active_only: Option<bool>,
    ) -> Result<Vec<PartyListRow>, AppError> {
        let mut tx = self.uow.begin_read()?;
        tx.parties().list_rows(party_type, active_only.unwrap_or(true), None)
    }

    fn search_parties(
        &self,
        query: &str,
        party_type: Option<PartyType>,
    ) -> Result<Vec<PartyListRow>, AppError> {
        let mut tx = self.uow.begin_read()?;
        tx.parties().list_rows(party_type, true, Some(query))
    }

    fn preview_party_delete(&self, party_id: i64) -> Result<PartyDeletePreview, AppError> {
        let mut tx = self.uow.begin_read()?;
        let party = tx.parties().find_by_id(party_id)?.ok_or_else(|| party_not_found(party_id))?;
        let (open_bills_count, balance_due_total) = tx.parties().open_dues(party_id, party.party_type)?;
        let reasons_used = usage_reasons(&mut *tx, party_id)?;
        let mode = if reasons_used.is_empty() { DeleteMode::Hard } else { DeleteMode::Soft };
        let reason = if !party.is_active && mode == DeleteMode::Soft {
            Some(format!("'{}' is already deleted.", party.name))
        } else if open_bills_count > 0 {
            Some(dues_block_message(balance_due_total, open_bills_count))
        } else {
            None
        };
        Ok(PartyDeletePreview {
            party_id,
            name: party.name,
            party_type: party.party_type,
            mode,
            open_bills_count,
            balance_due_total,
            can_delete: reason.is_none(),
            reason,
            reasons_used,
        })
    }

    fn delete_party(&self, party_id: i64) -> Result<DeletePartyResult, AppError> {
        let mut tx = self.uow.begin_transaction()?;
        let party = tx.parties().find_by_id(party_id)?.ok_or_else(|| party_not_found(party_id))?;
        let used = !usage_reasons(&mut *tx, party_id)?.is_empty();
        if !party.is_active && used {
            return Err(validation("party_id", format!("'{}' is already deleted", party.name)));
        }
        let (count, total) = tx.parties().open_dues(party_id, party.party_type)?;
        if count > 0 {
            return Err(AppError::BillLocked {
                code: "PARTY_HAS_DUES".to_string(),
                message: dues_block_message(total, count),
            });
        }
        let mode = if used {
            tx.parties().set_active(party_id, false, &chrono::Utc::now().to_rfc3339())?;
            DeleteMode::Soft
        } else {
            tx.parties().hard_delete(party_id)?;
            DeleteMode::Hard
        };
        tx.commit()?;
        // DD-75: a deleted party leaves the search index (after commit, DD-24).
        self.search_index.remove(party_id)?;
        Ok(DeletePartyResult { mode })
    }

    fn restore_party(&self, party_id: i64) -> Result<(), AppError> {
        let mut tx = self.uow.begin_transaction()?;
        let party = tx.parties().find_by_id(party_id)?.ok_or_else(|| party_not_found(party_id))?;
        if party.is_active {
            return Err(validation("party_id", format!("'{}' is not deleted", party.name)));
        }
        tx.parties().set_active(party_id, true, &chrono::Utc::now().to_rfc3339())?;
        tx.commit()?;
        sync_after_commit(&*self.search_index, party_id, "party");
        Ok(())
    }

    fn list_deleted_parties(
        &self,
        search: Option<String>,
        page: Option<usize>,
        page_size: Option<usize>,
    ) -> Result<Paginated<DeletedPartyRow>, AppError> {
        let req = PageRequest::new(page, page_size.or(Some(10)))?;
        let mut tx = self.uow.begin_read()?;
        let (rows, total) = tx.parties().list_inactive(search.as_deref().unwrap_or(""), req.offset(), req.limit)?;
        Ok(Paginated::new(rows, total, req.page, req.limit))
    }
}
