// tests/party_delete_test.rs
//! Test Suite: delete and restore parties (FR-46, DD-74, DD-75, DD-77, DD-80).
//! Production services on real SQLite.

mod common;

use std::sync::Arc;

use book_keeper::core::dtos::*;
use book_keeper::core::entities::*;
use book_keeper::core::error::AppError;
use book_keeper::core::value_objects::*;
use book_keeper::data::contracts::search_indexer::FtsIndex;
use book_keeper::data::contracts::unit_of_work::UnitOfWork;
use book_keeper::services::contracts::accounting_service::AccountingService;
use book_keeper::services::contracts::party_service::PartyService;
use book_keeper::services::contracts::purchase_service::PurchaseService;
use book_keeper::services::contracts::record_import_service::RecordImportService;
use book_keeper::services::contracts::return_service::ReturnService;
use book_keeper::services::contracts::sale_service::SaleService;
use common::ledger::*;
use common::*;

type Inv = InventoryServiceImpl<FifoStrategy>;
type Acct = AccountingServiceImpl<PaymentServiceImpl>;
type Rec = book_keeper::services::RecordImportServiceImpl<TestDb>;

const TODAY: &str = "2026-11-20";

struct E {
    db: TestDb,
    parties: PartyServiceImpl,
    acct: Acct,
    sales: SaleServiceImpl<Inv, PaymentServiceImpl>,
    purchases: PurchaseServiceImpl<Inv, PaymentServiceImpl>,
    rets: ReturnServiceImpl<Inv, PaymentServiceImpl, Acct>,
    rec: Rec,
}

fn env() -> E {
    let db = TestDb::new();
    let inv = InventoryServiceImpl::new(FifoStrategy, db.clone());
    let acct = AccountingServiceImpl::new(PaymentServiceImpl, db.clone());
    seed_stocked_item(&db, &inv, 1, 100.0, 5.0, 10.0);
    E {
        parties: PartyServiceImpl::new(db.clone(), Arc::new(db.party_index())),
        sales: SaleServiceImpl::new(inv.clone(), PaymentServiceImpl, db.clone()),
        purchases: PurchaseServiceImpl::new(inv.clone(), PaymentServiceImpl, db.clone()),
        rets: ReturnServiceImpl::new(inv, PaymentServiceImpl, acct.clone(), db.clone()),
        rec: Rec::new(db.clone(), Arc::new(db.party_index()), Arc::new(db.expense_type_index())),
        acct,
        db,
    }
}

fn ob(amount: f64) -> Option<OpeningBalanceInput> {
    Some(OpeningBalanceInput { amount, as_of_date: TODAY.to_string() })
}

impl E {
    fn party(&self, name: &str, t: PartyType, phone: Option<&str>, opening: Option<OpeningBalanceInput>) -> i64 {
        self.parties
            .create_party(CreatePartyPayload {
                name: name.into(),
                phone: phone.map(String::from),
                party_type: t,
                notes: None,
                opening_balance: opening,
            })
            .expect("party")
    }
    fn update(&self, id: i64, name: &str, t: PartyType, opening: Option<OpeningBalanceInput>) -> Result<(), AppError> {
        self.parties.update_party(
            id,
            UpdatePartyPayload { name: name.into(), phone: None, party_type: t, notes: None, opening_balance: opening },
        )
    }
    fn count(&self, sql: &str) -> i64 {
        self.db.pool.get().unwrap().query_row(sql, [], |r| r.get(0)).unwrap()
    }
    fn parties_total(&self) -> i64 {
        self.count("SELECT COUNT(*) FROM parties")
    }
    /// A customer bill of `10 x qty` with `paid` upfront.
    fn sale(&self, party: Option<i64>, paid: f64) -> i64 {
        let status = if paid >= 20.0 { PaymentStatus::Paid } else if paid <= 0.0 { PaymentStatus::Unpaid } else { PaymentStatus::Partial };
        self.sales
            .create_sale(sale_payload(party, status, paid, &[(1, None, 2.0, 10.0)]))
            .expect("sale")
            .sale_id
    }
    fn purchase(&self, party: Option<i64>, paid: f64) -> i64 {
        let status = if paid >= 10.0 { PaymentStatus::Paid } else if paid <= 0.0 { PaymentStatus::Unpaid } else { PaymentStatus::Partial };
        self.purchases
            .create_purchase(purchase_payload(party, status, paid, &[(1, None, 2.0, 5.0)]))
            .expect("purchase")
            .purchase_id
    }
    fn search(&self, q: &str) -> Vec<i64> {
        self.parties.search_parties(q, None).unwrap().into_iter().map(|r| r.party.id).collect()
    }
    fn deleted(&self, search: Option<&str>) -> Paginated<DeletedPartyRow> {
        self.parties.list_deleted_parties(search.map(String::from), None, None).unwrap()
    }
}

fn file(content: &str) -> String {
    let p = std::env::temp_dir().join(format!("bk_pdel_{}.csv", uuid::Uuid::new_v4()));
    std::fs::write(&p, content).unwrap();
    p.to_str().unwrap().to_string()
}

fn validation_msg<T: std::fmt::Debug>(r: Result<T, AppError>) -> String {
    match r {
        Err(AppError::ValidationError { message, .. }) => message,
        other => panic!("expected ValidationError, got {other:?}"),
    }
}

/// DD-90: the `DeletedReference(PARTY)` message, checking the entity and id.
fn deleted_msg<T: std::fmt::Debug>(r: Result<T, AppError>, party: i64) -> String {
    match r {
        Err(AppError::DeletedReference { entity, id, message, .. }) => {
            assert_eq!((entity.as_str(), id), ("PARTY", party));
            message
        }
        other => panic!("expected DeletedReference, got {other:?}"),
    }
}

#[test]
fn unreferenced_party_is_removed_permanently_with_its_search_rows() {
    let e = env();
    let id = e.party("Bad Row", PartyType::Customer, Some("98765 43210"), None);
    let keep = e.party("Keeper", PartyType::Customer, None, None);
    assert_eq!(e.search("Bad"), vec![id]);
    let idx = e.db.party_index();
    assert_eq!(idx.indexed_count().unwrap(), 2);

    let pv = e.parties.preview_party_delete(id).unwrap();
    assert_eq!(pv.mode, DeleteMode::Hard);
    assert!(pv.can_delete && pv.reasons_used.is_empty() && pv.open_bills_count == 0);

    assert_eq!(e.parties.delete_party(id).unwrap().mode, DeleteMode::Hard);
    assert_eq!(e.parties_total(), 1);
    assert!(matches!(e.parties.get_party(id), Err(AppError::NotFound { .. })));
    assert!(e.search("Bad").is_empty());
    assert_eq!(idx.indexed_count().unwrap(), 1);
    assert_eq!(e.search("Keeper"), vec![keep]);
    assert_eq!(e.deleted(None).total, 0, "a removed party is not in the Deleted list");
}

#[test]
fn party_with_any_history_is_only_soft_deleted() {
    let e = env();
    // a paid bill
    let c = e.party("Cust", PartyType::Customer, None, None);
    e.sale(Some(c), 20.0);
    let pv = e.parties.preview_party_delete(c).unwrap();
    assert_eq!(pv.mode, DeleteMode::Soft);
    assert_eq!(pv.reasons_used, vec!["1 bill".to_string(), "1 payment".to_string()]);
    assert_eq!(e.parties.delete_party(c).unwrap().mode, DeleteMode::Soft);
    assert!(!e.parties.get_party(c).unwrap().is_active);

    // a paid vendor bill
    let v = e.party("Vend", PartyType::Vendor, None, None);
    e.purchase(Some(v), 10.0);
    assert_eq!(e.parties.delete_party(v).unwrap().mode, DeleteMode::Soft);

    // a return alone is history
    let r = e.party("Returner", PartyType::Customer, None, None);
    {
        let mut tx = e.db.begin_transaction().unwrap();
        seed_return(&mut *tx, 900, ReturnType::Customer, Some(r), None, 10.0, TODAY);
        tx.commit().unwrap();
    }
    let pv = e.parties.preview_party_delete(r).unwrap();
    assert_eq!((pv.mode, pv.reasons_used), (DeleteMode::Soft, vec!["1 return".to_string()]));
    assert_eq!(e.parties.delete_party(r).unwrap().mode, DeleteMode::Soft);

    // a settlement alone is history (bill fully settled)
    let s = e.party("Settler", PartyType::Customer, None, None);
    let bill = e.sale(Some(s), 5.0);
    e.acct.settle_bill_payment(settle_payload(s, TransactionType::Sale, Some(bill), 15.0)).unwrap();
    let pv = e.parties.preview_party_delete(s).unwrap();
    assert_eq!(pv.mode, DeleteMode::Soft);
    assert!(pv.reasons_used.contains(&"2 payments".to_string()), "{:?}", pv.reasons_used);
    assert_eq!(e.parties.delete_party(s).unwrap().mode, DeleteMode::Soft);
    assert_eq!(e.parties_total(), 4, "nothing was physically removed");
}

#[test]
fn opening_bill_blocks_while_due_then_party_is_hard_after_balance_set_to_zero() {
    let e = env();
    let c = e.party("Imported", PartyType::Customer, None, ob(500.0));
    let pv = e.parties.preview_party_delete(c).unwrap();
    assert!(!pv.can_delete);
    assert_eq!((pv.open_bills_count, pv.balance_due_total), (1, 500.0));
    assert_eq!(pv.reason.as_deref(), Some("\u{20b9}500 due on 1 bill. Settle them first."));
    match e.parties.delete_party(c) {
        Err(AppError::BillLocked { code, message }) => {
            assert_eq!(code, "PARTY_HAS_DUES");
            assert!(message.contains("500") && message.contains("1 bill"), "{message}");
        }
        other => panic!("expected PARTY_HAS_DUES, got {other:?}"),
    }
    assert!(e.parties.get_party(c).unwrap().is_active, "blocked delete writes nothing");

    // opening balance to 0 removes the unsettled OB- bill; nothing references the party
    e.update(c, "Imported", PartyType::Customer, ob(0.0)).unwrap();
    let pv = e.parties.preview_party_delete(c).unwrap();
    assert_eq!(pv.mode, DeleteMode::Hard);
    assert_eq!(e.parties.delete_party(c).unwrap().mode, DeleteMode::Hard);
    assert_eq!(e.parties_total(), 0);

    // vendor side
    let v = e.party("Vendor OB", PartyType::Vendor, None, ob(200.0));
    assert!(matches!(e.parties.delete_party(v), Err(AppError::BillLocked { .. })));
}

#[test]
fn dues_block_customer_and_vendor_and_name_amount_and_count() {
    let e = env();
    let c = e.party("Owes", PartyType::Customer, None, None);
    e.sale(Some(c), 5.0);
    e.sale(Some(c), 0.0);
    let pv = e.parties.preview_party_delete(c).unwrap();
    assert!(!pv.can_delete);
    assert_eq!(pv.open_bills_count, 2);
    assert_eq!(pv.balance_due_total, 35.0);
    assert_eq!(pv.reason.as_deref(), Some("\u{20b9}35 due on 2 bills. Settle them first."));
    assert!(matches!(e.parties.delete_party(c), Err(AppError::BillLocked { code, .. }) if code == "PARTY_HAS_DUES"));

    let v = e.party("Owed", PartyType::Vendor, None, None);
    e.purchase(Some(v), 4.0);
    let pv = e.parties.preview_party_delete(v).unwrap();
    assert_eq!((pv.can_delete, pv.open_bills_count, pv.balance_due_total), (false, 1, 6.0));
    assert!(matches!(e.parties.delete_party(v), Err(AppError::BillLocked { .. })));

    // settling lifts the block
    e.acct.settle_bill_payment(settle_payload(v, TransactionType::Purchase, None, 6.0)).unwrap();
    assert_eq!(e.parties.delete_party(v).unwrap().mode, DeleteMode::Soft);
}

#[test]
fn soft_delete_leaves_search_and_restore_brings_it_back_in_the_one_list() {
    let e = env();
    let c = e.party("Sharma Stores", PartyType::Customer, Some("98765 43210"), None);
    let v = e.party("Gupta Supplies", PartyType::Vendor, Some("91234-56789"), None);
    e.sale(Some(c), 20.0);
    e.purchase(Some(v), 10.0);
    let idx = e.db.party_index();
    assert_eq!(idx.indexed_count().unwrap(), 2);
    assert_eq!(e.search("Sharma"), vec![c]);
    assert_eq!(e.search("98765"), vec![c]);

    e.parties.delete_party(c).unwrap();
    e.parties.delete_party(v).unwrap();
    assert!(e.search("Sharma").is_empty() && e.search("98765").is_empty() && e.search("Gupta").is_empty());
    assert_eq!(idx.indexed_count().unwrap(), 0);
    assert!(e.parties.list_parties(None, None).unwrap().is_empty(), "Parties list shows active only");

    // one list, both types, Type is a column
    let all = e.deleted(None);
    assert_eq!(all.total, 2);
    let types: Vec<PartyType> = all.items.iter().map(|r| r.party_type).collect();
    assert!(types.contains(&PartyType::Customer) && types.contains(&PartyType::Vendor));
    assert_eq!(e.deleted(Some("sharma")).items.iter().map(|r| r.party_id).collect::<Vec<_>>(), vec![c]);
    assert_eq!(e.deleted(Some("91234")).items.iter().map(|r| r.party_id).collect::<Vec<_>>(), vec![v]);
    assert_eq!(e.deleted(Some("nobody")).total, 0);
    assert_eq!(e.parties.list_deleted_parties(None, Some(1), Some(1)).unwrap().items.len(), 1);

    e.parties.restore_party(c).unwrap();
    assert_eq!(e.search("Sharma"), vec![c]);
    assert_eq!(idx.indexed_count().unwrap(), 1);
    assert_eq!(e.deleted(None).total, 1);
    assert!(matches!(e.parties.restore_party(c), Err(AppError::ValidationError { .. })));
    assert!(matches!(e.parties.restore_party(9999), Err(AppError::NotFound { .. })));
    // deleting an already deleted party is refused
    assert!(validation_msg(e.parties.delete_party(v)).contains("already deleted"));
    let pv = e.parties.preview_party_delete(v).unwrap();
    assert!(!pv.can_delete);
    assert_eq!(pv.reason.as_deref(), Some("'Gupta Supplies' is already deleted."));
}

#[test]
fn every_write_to_a_deleted_party_is_refused() {
    let e = env();
    let c = e.party("Gone", PartyType::Customer, None, None);
    e.sale(Some(c), 20.0);
    e.parties.delete_party(c).unwrap();
    let msg = deleted_msg(e.update(c, "Gone 2", PartyType::Customer, None), c);
    assert_eq!(msg, "'Gone' is deleted. Restore it first.");
    assert!(deleted_msg(e.update(c, "Gone", PartyType::Customer, ob(100.0)), c).contains("is deleted"));
    assert_eq!(e.count("SELECT COUNT(*) FROM sale_transactions WHERE is_opening_balance = 1"), 0);
    e.parties.restore_party(c).unwrap();
    e.update(c, "Gone 2", PartyType::Customer, None).unwrap();
}

#[test]
fn new_bills_and_returns_reject_a_deleted_party() {
    let e = env();
    let c = e.party("Cust", PartyType::Customer, None, None);
    let v = e.party("Vend", PartyType::Vendor, None, None);
    let sale = e.sale(Some(c), 20.0);
    let purchase = e.purchase(Some(v), 10.0);
    e.parties.delete_party(c).unwrap();
    e.parties.delete_party(v).unwrap();

    let m = deleted_msg(e.sales.create_sale(sale_payload(Some(c), PaymentStatus::Paid, 10.0, &[(1, None, 1.0, 10.0)])), c);
    assert_eq!(m, "'Cust' is deleted. Restore it first.");
    let m = deleted_msg(
        e.purchases.create_purchase(purchase_payload(Some(v), PaymentStatus::Paid, 5.0, &[(1, None, 1.0, 5.0)])),
        v,
    );
    assert_eq!(m, "'Vend' is deleted. Restore it first.");

    // moving an existing bill of another party onto the deleted one
    let other = e.party("Other", PartyType::Customer, None, None);
    let other_bill = e.sale(Some(other), 20.0);
    let m = deleted_msg(
        e.sales.update_sale(other_bill, sale_payload(Some(c), PaymentStatus::Paid, 20.0, &[(1, None, 2.0, 10.0)])),
        c,
    );
    assert!(m.contains("is deleted. Restore it first."), "{m}");

    // returns against a bill of a deleted party
    let m = deleted_msg(e.rets.create_customer_return(CustomerReturnPayload {
        original_sale_id: Some(sale),
        party_id: None,
        return_date: TODAY.into(),
        settlement_type: SettlementType::Refund,
        reason: None,
        notes: None,
        items: vec![CustomerReturnItemPayload {
            item_id: 1,
            sale_item_id: None,
            quantity: 1.0,
            unit_price: 10.0,
            condition: StockCondition::Restockable,
            unit_cost_override: None,
        }],
        replacement_items: vec![],
    }), c);
    assert_eq!(m, "'Cust' is deleted. Restore it first.");
    let m = deleted_msg(e.rets.create_vendor_return(VendorReturnPayload {
        original_purchase_id: Some(purchase),
        party_id: None,
        return_date: TODAY.into(),
        settlement_type: SettlementType::Refund,
        reason: None,
        notes: None,
        items: vec![VendorReturnItemPayload { item_id: 1, purchase_item_id: None, quantity: 1.0 }],
    }), v);
    assert_eq!(m, "'Vend' is deleted. Restore it first.");
    assert_eq!(e.count("SELECT COUNT(*) FROM return_transactions"), 0);

    // restoring lifts it
    e.parties.restore_party(c).unwrap();
    e.sales.create_sale(sale_payload(Some(c), PaymentStatus::Paid, 10.0, &[(1, None, 1.0, 10.0)])).unwrap();
}

#[test]
fn old_bill_edit_keeps_its_deleted_party_but_cannot_raise_its_due() {
    let e = env();
    let c = e.party("Old Cust", PartyType::Customer, None, None);
    let bill = e.sale(Some(c), 20.0);
    e.parties.delete_party(c).unwrap();
    assert!(e.acct.get_party_dues_summary(PartyType::Customer).unwrap().is_empty());

    // DD-90: leaving a due on the deleted customer is refused, nothing written
    let m = deleted_msg(
        e.sales.update_sale(bill, sale_payload(Some(c), PaymentStatus::Partial, 5.0, &[(1, None, 2.0, 10.0)])),
        c,
    );
    assert_eq!(m, "'Old Cust' is deleted. Restore it first.");
    assert!(e.acct.get_party_dues_summary(PartyType::Customer).unwrap().is_empty());
    // an edit that keeps it paid still keeps the deleted customer
    e.sales
        .update_sale(bill, sale_payload(Some(c), PaymentStatus::Paid, 10.0, &[(1, None, 1.0, 10.0)]))
        .expect("an edit keeps the bill's own deleted party");
    // no new payment against a deleted party
    deleted_msg(e.acct.settle_bill_payment(settle_payload(c, TransactionType::Sale, None, 15.0)), c);
    // the ledger still reads the party
    let ledger = e.acct.get_party_ledger(c).unwrap();
    assert!(!ledger.party.is_active);
}

#[test]
fn deleted_vendor_bill_edit_cannot_raise_its_due() {
    let e = env();
    let v = e.party("Old Vendor", PartyType::Vendor, None, None);
    let bill = e.purchase(Some(v), 10.0);
    e.parties.delete_party(v).unwrap();
    deleted_msg(
        e.purchases.update_purchase(bill, purchase_payload(Some(v), PaymentStatus::Partial, 4.0, &[(1, None, 2.0, 5.0)])),
        v,
    );
    assert!(e.acct.get_party_dues_summary(PartyType::Vendor).unwrap().is_empty());
}

#[test]
fn parties_import_matches_active_parties_only() {
    let e = env();
    let old = e.party("Ravi Traders", PartyType::Customer, None, None);
    e.sale(Some(old), 20.0);
    e.parties.delete_party(old).unwrap();
    let f = file("Name,Type,Phone,Notes,Opening Balance,As Of Date\nRavi Traders,CUSTOMER,99999,,,\n");
    let rep = e.rec.validate_parties_import(&f, ConflictStrategy::Skip).unwrap();
    assert!(rep.duplicates.is_empty(), "a deleted party is not a duplicate: {rep:?}");
    let s = e.rec.execute_parties_import(&f, ConflictStrategy::Skip).unwrap();
    assert_eq!((s.created, s.skipped), (1, 0));
    assert_eq!(e.count("SELECT COUNT(*) FROM parties WHERE name = 'Ravi Traders'"), 2);

    // now an active one exists: Skip skips it, the deleted one is untouched
    let s = e.rec.execute_parties_import(&f, ConflictStrategy::Skip).unwrap();
    assert_eq!((s.created, s.skipped), (0, 1));
    assert!(!e.parties.get_party(old).unwrap().is_active);
}

#[test]
fn previews_write_nothing() {
    let e = env();
    let a = e.party("A", PartyType::Customer, None, None);
    let b = e.party("B", PartyType::Customer, None, ob(50.0));
    let c = e.party("C", PartyType::Vendor, None, None);
    e.purchase(Some(c), 10.0);
    let snap = |e: &E| {
        (
            e.parties_total(),
            e.count("SELECT COUNT(*) FROM parties WHERE is_active = 1"),
            e.count("SELECT COUNT(*) FROM sale_transactions"),
            e.count("SELECT COUNT(*) FROM purchase_transactions"),
            e.count("SELECT COUNT(*) FROM payment_settlements"),
        )
    };
    let before = snap(&e);
    for id in [a, b, c] {
        e.parties.preview_party_delete(id).unwrap();
    }
    assert_eq!(before, snap(&e));
    assert!(matches!(e.parties.preview_party_delete(9999), Err(AppError::NotFound { .. })));
}
