//! Test Suite: FR-41 imports (items with services and components, service components CSV,
//! parties with opening balances, expenses) on real SQLite with the production services.
//! X7: `validate_*` never writes; `execute_*` is all-or-nothing.

mod common;

use std::sync::Arc;
use book_keeper::core::dtos::*;
use book_keeper::core::entities::*;
use book_keeper::core::error::AppError;
use book_keeper::core::value_objects::*;
use book_keeper::services::contracts::accounting_service::AccountingService;
use book_keeper::services::contracts::expense_service::ExpenseService;
use book_keeper::services::contracts::item_import_service::ItemImportService;
use book_keeper::services::contracts::item_service::ItemService;
use book_keeper::services::contracts::party_service::PartyService;
use book_keeper::services::contracts::record_import_service::RecordImportService;
use book_keeper::services::r#impl::import_template_writer::write_import_template;
use common::ledger::*;
use common::*;

type Rec = book_keeper::services::RecordImportServiceImpl<TestDb>;
type ItemsImp = ItemImportServiceImpl<InventoryServiceImpl<FifoStrategy>>;

struct Env {
    db: TestDb,
    rec: Rec,
    items_imp: ItemsImp,
    items: ItemServiceImpl<InventoryServiceImpl<FifoStrategy>>,
    parties: PartyServiceImpl,
    expenses: ExpenseServiceImpl,
}

fn env() -> Env {
    let db = TestDb::new();
    let inv = InventoryServiceImpl::new(FifoStrategy, db.clone());
    Env {
        rec: Rec::new(db.clone(), Arc::new(db.party_index()), Arc::new(db.expense_type_index())),
        items_imp: ItemImportServiceImpl::new(db.clone(), inv.clone()).with_indexer(Arc::new(db.indexer())),
        items: ItemServiceImpl::new(db.clone(), Arc::new(db.indexer()), inv),
        parties: PartyServiceImpl::new(db.clone(), Arc::new(db.party_index())),
        expenses: ExpenseServiceImpl::new(db.clone(), Arc::new(db.expense_type_index())),
        db,
    }
}

impl Env {
    fn count(&self, sql: &str) -> i64 {
        self.db.pool.get().unwrap().query_row(sql, [], |r| r.get(0)).unwrap()
    }
    fn total_rows(&self) -> i64 {
        ["items", "service_components", "parties", "expenses", "expense_types", "sale_transactions", "purchase_transactions"]
            .iter()
            .map(|t| self.count(&format!("SELECT COUNT(*) FROM {t}")))
            .sum()
    }
    fn item_id(&self, name: &str) -> i64 {
        self.count(&format!("SELECT id FROM items WHERE name = '{name}' COLLATE NOCASE"))
    }
    fn components_of(&self, service: &str) -> Vec<(String, f64)> {
        let id = self.item_id(service);
        self.items.get_service_components(id).unwrap().into_iter().map(|c| (c.name, c.quantity_per_unit)).collect()
    }
}

fn file(ext: &str, content: &str) -> String {
    let p = std::env::temp_dir().join(format!("bk_rec_{}.{ext}", uuid::Uuid::new_v4()));
    std::fs::write(&p, content).unwrap();
    p.to_str().unwrap().to_string()
}

fn csv(content: &str) -> String {
    file("csv", content)
}

fn today() -> String {
    chrono::Local::now().format("%Y-%m-%d").to_string()
}

fn tomorrow() -> String {
    (chrono::Local::now() + chrono::Duration::days(1)).format("%Y-%m-%d").to_string()
}

fn has_error(rep: &ImportValidationReport, field: &str, needle: &str) -> bool {
    rep.rows_with_errors.iter().any(|e| e.field == field && e.message.contains(needle))
}

// ---------------------------------------------------------------------------------------------
// Items: SERVICE rows
// ---------------------------------------------------------------------------------------------

const ITEM_HEADER: &str = "Item Name,Base Unit,Sale Price,Purchase Cost,Opening Stock,Packaging Bundle,Units Per Bundle,Item Type,Service Cost";

#[test]
fn items_service_rows_row_errors() {
    let e = env();
    let f = csv(&format!(
        "{ITEM_HEADER}\n\
         Bad Type,PCS,1,1,,,,GADGET,\n\
         Svc Stock,PCS,10,,5,,,SERVICE,\n\
         Svc Bundle,PCS,10,,,Box,10,SERVICE,\n\
         Phys Cost,PCS,10,1,,,,PHYSICAL,3\n\
         Svc Neg,PCS,10,,,,,SERVICE,-1\n"
    ));
    let rep = e.items_imp.validate_items_import(&f).unwrap();
    assert_eq!(rep.valid_count, 0, "{rep:?}");
    assert!(has_error(&rep, "Item Type", "not valid"));
    assert!(has_error(&rep, "Opening Stock", "SERVICE row cannot have Opening Stock"));
    assert!(has_error(&rep, "Packaging Bundle", "SERVICE row cannot have a Packaging Bundle"));
    assert!(has_error(&rep, "Service Cost", "only for rows with Item Type SERVICE"));
    assert!(has_error(&rep, "Service Cost", "0 or more"));
    assert_eq!(e.total_rows(), 0);
    assert!(e.items_imp.execute_items_import(&f, ConflictStrategy::Skip).is_err());
    assert_eq!(e.total_rows(), 0);
}

#[test]
fn items_service_row_creates_a_service_without_stock() {
    let e = env();
    let f = csv(&format!("{ITEM_HEADER}\nPhotocopy,PCS,2,,,,,service,1.5\nPen,PCS,10,6,20,,,,\n"));
    let rep = e.items_imp.validate_items_import(&f).unwrap();
    assert_eq!((rep.valid_count, rep.lots_to_create), (2, 1), "{rep:?}");
    assert!(rep.info.iter().any(|i| i.contains("1 new service")));
    assert_eq!(e.total_rows(), 0, "dry run writes nothing");
    let sum = e.items_imp.execute_items_import(&f, ConflictStrategy::Skip).unwrap();
    assert_eq!((sum.created, sum.stock_lots_created), (2, 1));
    let svc = e.items.get_item(e.item_id("Photocopy")).unwrap();
    assert_eq!((svc.item_type, svc.service_cost, svc.current_stock, svc.is_low_stock), (ItemType::Service, Some(1.5), 0.0, false));
    assert_eq!(e.items.get_item(e.item_id("Pen")).unwrap().item_type, ItemType::Physical);
}

#[test]
fn items_type_mismatch_with_existing_is_a_row_error() {
    let e = env();
    e.items_imp.execute_items_import(&csv(&format!("{ITEM_HEADER}\nCopy,PCS,2,,,,,SERVICE,\nPen,PCS,1,1,,,,,\n")), ConflictStrategy::Skip).unwrap();
    let f = csv(&format!("{ITEM_HEADER}\nCopy,PCS,2,1,,,,PHYSICAL,\nPen,PCS,1,1,,,,SERVICE,\n"));
    let rep = e.items_imp.validate_items_import(&f).unwrap();
    assert!(has_error(&rep, "Item Type", "already exists as a service"), "{rep:?}");
    assert!(has_error(&rep, "Item Type", "already exists as a physical item"));
}

// ---------------------------------------------------------------------------------------------
// Items: Components sheet (XLSX) and the service components CSV
// ---------------------------------------------------------------------------------------------

fn xlsx_with(items: &[&[&str]], comps: Option<&[&[&str]]>) -> String {
    use rust_xlsxwriter::Workbook;
    let mut wb = Workbook::new();
    let ws = wb.add_worksheet().set_name("Items").unwrap();
    for (c, h) in ["Item Name", "Base Unit", "Sale Price", "Purchase Cost", "Opening Stock", "Item Type"].iter().enumerate() {
        ws.write_string(0, c as u16, *h).unwrap();
    }
    for (r, row) in items.iter().enumerate() {
        for (c, v) in row.iter().enumerate() {
            ws.write_string(1 + r as u32, c as u16, *v).unwrap();
        }
    }
    if let Some(comps) = comps {
        let ws = wb.add_worksheet().set_name("Components").unwrap();
        for (c, h) in ["Service Name", "Component Name", "Quantity Per Unit"].iter().enumerate() {
            ws.write_string(0, c as u16, *h).unwrap();
        }
        for (r, row) in comps.iter().enumerate() {
            for (c, v) in row.iter().enumerate() {
                ws.write_string(1 + r as u32, c as u16, *v).unwrap();
            }
        }
    }
    let p = std::env::temp_dir().join(format!("bk_rec_{}.xlsx", uuid::Uuid::new_v4()));
    wb.save(&p).unwrap();
    p.to_str().unwrap().to_string()
}

#[test]
fn items_components_sheet_resolves_file_and_db_names_and_replaces_on_update() {
    let e = env();
    // "Ink" exists in the DB; "Paper" is new in the file.
    e.items_imp.execute_items_import(&csv("Item Name,Base Unit,Sale Price,Purchase Cost\nInk,ML,1,1\n"), ConflictStrategy::Skip).unwrap();
    let f = xlsx_with(
        &[&["Paper", "PCS", "1", "0.5", "10", ""], &["Copy", "PCS", "2", "", "", "SERVICE"]],
        Some(&[&["Copy", "Paper", "1"], &["copy", "INK", "0.25"]]),
    );
    let before = e.total_rows();
    let rep = e.items_imp.validate_items_import(&f).unwrap();
    assert!(rep.rows_with_errors.is_empty() && rep.header_issues.is_empty(), "{rep:?}");
    assert!(rep.info.iter().any(|i| i.contains("1 service component list")), "{:?}", rep.info);
    assert_eq!(e.total_rows(), before, "dry run writes nothing");

    let sum = e.items_imp.execute_items_import(&f, ConflictStrategy::Skip).unwrap();
    assert_eq!(sum.created, 2);
    assert_eq!(e.components_of("Copy"), vec![("Paper".to_string(), 1.0), ("Ink".to_string(), 0.25)]);

    // Skip leaves an existing service's list alone; Overwrite (UPDATE) replaces it.
    let f2 = xlsx_with(&[&["Copy", "PCS", "3", "", "", "SERVICE"]], Some(&[&["Copy", "Ink", "2"]]));
    e.items_imp.execute_items_import(&f2, ConflictStrategy::Skip).unwrap();
    assert_eq!(e.components_of("Copy").len(), 2);
    e.items_imp.execute_items_import(&f2, ConflictStrategy::Overwrite).unwrap();
    assert_eq!(e.components_of("Copy"), vec![("Ink".to_string(), 2.0)]);
}

#[test]
fn items_components_rules_are_the_item_services() {
    let e = env();
    e.items_imp.execute_items_import(&csv("Item Name,Base Unit,Sale Price,Purchase Cost\nInk,ML,1,1\n"), ConflictStrategy::Skip).unwrap();
    let f = xlsx_with(
        &[&["Paper", "PCS", "1", "0.5", "", ""], &["Copy", "PCS", "2", "", "", "SERVICE"], &["Scan", "PCS", "2", "", "", "SERVICE"]],
        Some(&[
            &["Copy", "Paper", "1"],
            &["Copy", "paper", "2"],  // listed twice
            &["Copy", "Scan", "1"],   // a service as component
            &["Copy", "Copy", "1"],   // itself
            &["Copy", "Ghost", "1"],  // unknown
            &["Paper", "Ink", "1"],   // Paper is not a service
            &["Nobody", "Ink", "1"],  // unknown service
            &["Scan", "Ink", "0"],    // quantity 0
        ]),
    );
    let before = e.total_rows();
    let rep = e.items_imp.validate_items_import(&f).unwrap();
    let msgs: Vec<_> = rep.rows_with_errors.iter().map(|r| r.message.clone()).collect();
    let any = |needle: &str| msgs.iter().any(|m| m.contains(needle));
    assert!(any("is listed more than once"), "{msgs:?}");
    assert!(any("is a service; only physical items can be components"));
    assert!(any("A service cannot be its own component"));
    assert!(any("Component \"Ghost\" was not found"));
    assert!(any("is a physical item, not a service"));
    assert!(any("Service \"Nobody\" was not found"));
    assert!(any("must be greater than 0"));
    assert!(msgs.iter().all(|m| m.starts_with("Components sheet, row ")), "{msgs:?}");
    // All-or-nothing: the whole file is refused and nothing is written.
    assert!(e.items_imp.execute_items_import(&f, ConflictStrategy::Skip).is_err());
    assert_eq!(e.total_rows(), before);
}

#[test]
fn items_components_sheet_header_problem_is_a_header_issue() {
    let e = env();
    use rust_xlsxwriter::Workbook;
    let mut wb = Workbook::new();
    let ws = wb.add_worksheet().set_name("Items").unwrap();
    for (c, h) in ["Item Name", "Base Unit", "Sale Price", "Purchase Cost"].iter().enumerate() {
        ws.write_string(0, c as u16, *h).unwrap();
    }
    let ws = wb.add_worksheet().set_name("Components").unwrap();
    for (c, h) in ["Service Name", "Component Name", "Qty Per Unt"].iter().enumerate() {
        ws.write_string(0, c as u16, *h).unwrap();
    }
    let p = std::env::temp_dir().join(format!("bk_rec_{}.xlsx", uuid::Uuid::new_v4()));
    wb.save(&p).unwrap();
    let rep = e.items_imp.validate_items_import(p.to_str().unwrap()).unwrap();
    assert!(rep.header_issues.iter().any(|i| i.message.starts_with("Components sheet:")), "{rep:?}");
}

#[test]
fn service_components_csv_sets_and_replaces_lists() {
    let e = env();
    e.items_imp
        .execute_items_import(
            &csv(&format!("{ITEM_HEADER}\nPaper,PCS,1,1,,,,,\nInk,ML,1,1,,,,,\nCopy,PCS,2,,,,,SERVICE,\nPlain,PCS,2,1,,,,,\n")),
            ConflictStrategy::Skip,
        )
        .unwrap();
    let f = csv("Service Name,Component Name,Quantity Per Unit\nCopy,Paper,1\nCopy,Ink,0.5\n");
    let rep = e.rec.validate_service_components_import(&f).unwrap();
    assert_eq!(rep.valid_count, 2, "{rep:?}");
    assert_eq!(e.count("SELECT COUNT(*) FROM service_components"), 0, "dry run writes nothing");
    let sum = e.rec.execute_service_components_import(&f).unwrap();
    assert_eq!(sum.updated, 1);
    assert_eq!(e.components_of("Copy").len(), 2);

    let f2 = csv("Service Name,Component Name,Quantity Per Unit\nCopy,Ink,3\n");
    e.rec.execute_service_components_import(&f2).unwrap();
    assert_eq!(e.components_of("Copy"), vec![("Ink".to_string(), 3.0)]);

    // Existing services only; physical rows and unknown names are row errors; nothing changes.
    let bad = csv("Service Name,Component Name,Quantity Per Unit\nCopy,Paper,1\nPlain,Paper,1\nNew Svc,Paper,1\nCopy,Copy,1\nCopy,Paper,x\n");
    let rep = e.rec.validate_service_components_import(&bad).unwrap();
    assert!(has_error(&rep, "Service Name", "is a physical item, not a service"), "{rep:?}");
    assert!(has_error(&rep, "Service Name", "\"New Svc\" was not found"));
    assert!(has_error(&rep, "Quantity Per Unit", "is not a number"));
    assert!(e.rec.execute_service_components_import(&bad).is_err());
    assert_eq!(e.components_of("Copy"), vec![("Ink".to_string(), 3.0)]);

    // Header problems come back in the report.
    let hdr = csv("Service,Component Name\nCopy,Ink\n");
    let rep = e.rec.validate_service_components_import(&hdr).unwrap();
    assert!(rep.header_issues.iter().any(|i| i.kind == HeaderIssueKind::Missing && i.column == "Quantity Per Unit"));
    assert!(rep.header_issues.iter().any(|i| i.suggestion.as_deref() == Some("Service Name")));
    assert!(e.rec.execute_service_components_import(&hdr).is_err());
}

// ---------------------------------------------------------------------------------------------
// Parties
// ---------------------------------------------------------------------------------------------

const PARTY_HEADER: &str = "Name,Type,Phone,Notes,Opening Balance,As Of Date";

fn party_csv(rows: &[String]) -> String {
    csv(&format!("{PARTY_HEADER}\n{}\n", rows.join("\n")))
}

#[test]
fn parties_header_and_row_errors() {
    let e = env();
    let rep = e.rec.validate_parties_import(&csv("Nme,Type\nA,CUSTOMER\n"), ConflictStrategy::Skip).unwrap();
    assert!(rep.header_issues.iter().any(|i| i.column == "Nme" && i.suggestion.as_deref() == Some("Name")), "{rep:?}");
    assert_eq!(rep.expected_columns.len(), 6);

    let f = party_csv(&[
        ",CUSTOMER,,,,".into(),
        "A,SUPPLIER,,,,".into(),
        "B,CUSTOMER,,,-5,".into(),
        format!("C,CUSTOMER,,,100,{}", tomorrow()),
        "D,CUSTOMER,,,100,31-02-2026".into(),
        "E,CUSTOMER,,,1,".into(),
        "e,customer,,,,".into(),
    ]);
    let rep = e.rec.validate_parties_import(&f, ConflictStrategy::Skip).unwrap();
    assert!(has_error(&rep, "Name", "Name is empty"), "{rep:?}");
    assert!(has_error(&rep, "Type", "not valid"));
    assert!(has_error(&rep, "Opening Balance", "0 or more"));
    assert!(has_error(&rep, "Opening Balance", ""), "future date comes from the party service rule");
    assert!(has_error(&rep, "As Of Date", "not a date"));
    assert!(has_error(&rep, "Name", "is repeated (first seen in row 7)"));
    assert_eq!(rep.valid_count, 1, "only E is valid: C is future-dated and e repeats E");
    assert_eq!(e.total_rows(), 0);
    assert!(e.rec.execute_parties_import(&f, ConflictStrategy::Skip).is_err());
    assert_eq!(e.total_rows(), 0);
}

#[test]
fn parties_dry_run_then_execute_creates_parties_and_opening_bills() {
    let e = env();
    let f = party_csv(&[
        "Ravi Traders,CUSTOMER,98765,Wholesale,5000,2026-10-01".into(),
        "Ravi Traders,VENDOR,,,300,".into(),
        "Cash Guy,customer,,,0,2026-10-01".into(),
    ]);
    let rep = e.rec.validate_parties_import(&f, ConflictStrategy::Skip).unwrap();
    assert!(rep.rows_with_errors.is_empty(), "{rep:?}");
    assert_eq!(rep.valid_count, 3);
    assert!(rep.info.iter().any(|i| i.contains("2 opening balance bill(s) totalling 5300")), "{:?}", rep.info);
    assert!(rep.warnings.iter().any(|w| w.name == "Cash Guy" && w.message.contains("As Of Date is ignored")));
    assert_eq!(e.total_rows(), 0, "dry run writes nothing");

    let sum = e.rec.execute_parties_import(&f, ConflictStrategy::Skip).unwrap();
    assert_eq!((sum.created, sum.updated, sum.skipped), (3, 0, 0));
    let dir = e.parties.list_parties(Some(PartyType::Customer), Some(true)).unwrap();
    let ravi = dir.iter().find(|p| p.party.name == "Ravi Traders").unwrap();
    assert_eq!(ravi.party.phone.as_deref(), Some("98765"));
    assert_eq!(ravi.outstanding_balance, 5000.0);
    let ob = e.parties.get_party(ravi.party.id).unwrap().opening_balance.unwrap();
    assert_eq!((ob.amount, ob.as_of_date.as_str()), (5000.0, "2026-10-01"));
    assert_eq!(e.count("SELECT COUNT(*) FROM sale_transactions WHERE is_opening_balance = 1 AND transaction_code LIKE 'OB-%'"), 1);
    assert_eq!(e.count("SELECT COUNT(*) FROM purchase_transactions WHERE is_opening_balance = 1"), 1);
    let vendor = e.parties.list_parties(Some(PartyType::Vendor), Some(true)).unwrap();
    assert_eq!(vendor[0].outstanding_balance, 300.0);
    assert_eq!(vendor[0].party.opening_balance.as_ref().map(|o| o.as_of_date.clone()), Some(today()));
    assert_eq!(e.parties.list_parties(Some(PartyType::Customer), Some(true)).unwrap().len(), 2);
    // FTS: the new party is searchable.
    assert_eq!(e.parties.search_parties("Ravi", None).unwrap().len(), 2);
}

#[test]
fn parties_strategy_skip_and_update_and_below_settled() {
    let e = env();
    let id = e
        .parties
        .create_party(CreatePartyPayload {
            name: "Ravi".into(),
            phone: Some("111".into()),
            party_type: PartyType::Customer,
            notes: Some("old".into()),
            opening_balance: Some(OpeningBalanceInput { amount: 1000.0, as_of_date: "2026-09-01".into() }),
        })
        .unwrap();
    let acct = AccountingServiceImpl::new(PaymentServiceImpl, e.db.clone());
    acct.settle_bill_payment(settle_payload(id, TransactionType::Sale, None, 600.0)).unwrap();

    // Skip: the existing party is untouched and the dry run does not complain about the balance.
    let low = party_csv(&["RAVI,Customer,222,,500,".into(), "Fresh,VENDOR,,,,".into()]);
    let rep = e.rec.validate_parties_import(&low, ConflictStrategy::Skip).unwrap();
    assert!(rep.rows_with_errors.is_empty(), "{rep:?}");
    assert_eq!(rep.duplicates.len(), 1);
    assert!(rep.warnings.iter().any(|w| w.message.contains("skipped")));
    let sum = e.rec.execute_parties_import(&low, ConflictStrategy::Skip).unwrap();
    assert_eq!((sum.created, sum.skipped), (1, 1));
    assert_eq!(e.parties.get_party(id).unwrap().phone.as_deref(), Some("111"));

    // Update: 500 is below the 600 already settled -> the party service's message, as a row error.
    let rep = e.rec.validate_parties_import(&low, ConflictStrategy::Overwrite).unwrap();
    assert!(has_error(&rep, "Opening Balance", "₹600 is already settled against the opening balance"), "{rep:?}");
    let before = e.total_rows();
    assert!(e.rec.execute_parties_import(&low, ConflictStrategy::Overwrite).is_err());
    assert_eq!(e.total_rows(), before);

    // Update with a valid amount: phone and notes change when given, balance recomputes.
    let ok = party_csv(&["Ravi,CUSTOMER,222,,800,2026-09-15".into()]);
    let sum = e.rec.execute_parties_import(&ok, ConflictStrategy::Overwrite).unwrap();
    assert_eq!((sum.created, sum.updated), (0, 1));
    let p = e.parties.get_party(id).unwrap();
    assert_eq!((p.phone.as_deref(), p.notes.as_deref()), (Some("222"), Some("old")));
    let ob = p.opening_balance.unwrap();
    assert_eq!((ob.amount, ob.settled_amount, ob.as_of_date.as_str()), (800.0, 600.0, "2026-09-15"));
    // AddStock is not a party strategy.
    assert!(e.rec.execute_parties_import(&ok, ConflictStrategy::AddStock).is_err());
}

#[test]
fn parties_execute_is_all_or_nothing() {
    let e = env();
    // Passes the dry-run checks but fails inside the transaction? Use a row error instead: the
    // whole file, including the valid first row, is refused.
    let f = party_csv(&["Good,CUSTOMER,,,100,".into(), "Bad,NOPE,,,,".into()]);
    assert!(e.rec.execute_parties_import(&f, ConflictStrategy::Skip).is_err());
    assert_eq!(e.count("SELECT COUNT(*) FROM parties"), 0);
    assert_eq!(e.count("SELECT COUNT(*) FROM sale_transactions"), 0);
}

// ---------------------------------------------------------------------------------------------
// Expenses
// ---------------------------------------------------------------------------------------------

const EXP_HEADER: &str = "Date,Expense Type,Amount,Notes";

fn exp_csv(rows: &[String]) -> String {
    csv(&format!("{EXP_HEADER}\n{}\n", rows.join("\n")))
}

#[test]
fn expenses_header_and_row_errors() {
    let e = env();
    let rep = e.rec.validate_expenses_import(&csv("Dt,Expense Type,Amount\n2026-01-01,Rent,5\n")).unwrap();
    assert!(rep.header_issues.iter().any(|i| i.column == "Dt"), "{rep:?}");
    assert_eq!(rep.expected_columns.len(), 4);

    let f = exp_csv(&[
        ",Rent,100,".into(),
        "2026-01-01,,100,".into(),
        "2026-01-01,Rent,0,".into(),
        "2026-01-01,Rent,abc,".into(),
        "2026-01-01,Rent,-3,".into(),
        format!("{},Rent,100,", tomorrow()),
        "yesterday,Rent,100,".into(),
        format!("2026-01-01,Rent,100,{}", "x".repeat(501)),
        "2026-01-01,Rent,100,fine".into(),
    ]);
    let rep = e.rec.validate_expenses_import(&f).unwrap();
    assert!(has_error(&rep, "Date", "Date is empty"), "{rep:?}");
    assert!(has_error(&rep, "Expense Type", "cannot be empty"));
    assert!(has_error(&rep, "Amount", "greater than 0"));
    assert!(has_error(&rep, "Amount", "is not a number"));
    assert!(has_error(&rep, "Amount", "0 or more"));
    assert!(has_error(&rep, "Date", "not a date"));
    assert!(has_error(&rep, "Date", ""), "future date");
    assert!(has_error(&rep, "Notes", "500 characters"));
    assert_eq!(rep.valid_count, 1);
    assert!(e.rec.execute_expenses_import(&f).is_err());
    assert_eq!(e.total_rows(), 0);
}

#[test]
fn expenses_dry_run_warnings_then_execute_creates_types_and_rows() {
    let e = env();
    let existing = e.expenses.create_expense_type(CreateExpenseTypePayload { name: "Electricity".into() }).unwrap();
    let f = exp_csv(&[
        "2026-10-01,electricity,1250.50,Oct bill".into(),
        "01-10-2026,Tea  Snacks,40,".into(),
        "2026-10-01,TEA SNACKS,40,".into(),
        "2026-10-01,Electricity,1250.50,Oct bill".into(),
    ]);
    // Same row already saved in the DB (row 1 / 4 duplicate it).
    e.expenses
        .create_expense(ExpensePayload { expense_date: "2026-10-01".into(), expense_type_id: existing.id, amount: 1250.5, notes: Some("Oct bill".into()) })
        .unwrap();
    let before = e.total_rows();
    let rep = e.rec.validate_expenses_import(&f).unwrap();
    assert!(rep.rows_with_errors.is_empty(), "{rep:?}");
    assert_eq!(rep.valid_count, 4);
    assert!(rep.info.iter().any(|i| i.contains("1 new expense type(s) will be created: Tea Snacks")), "{:?}", rep.info);
    let w = |row: usize, needle: &str| rep.warnings.iter().any(|x| x.row == row && x.message.contains(needle));
    assert!(w(2, "already saved"), "{:?}", rep.warnings);
    assert!(w(4, "Identical to row 3"));
    assert!(w(5, "Identical to row 2"));
    assert_eq!(e.total_rows(), before, "dry run writes nothing");

    let sum = e.rec.execute_expenses_import(&f).unwrap();
    assert_eq!(sum.created, 4);
    assert_eq!(e.count("SELECT COUNT(*) FROM expenses"), 5);
    // The type is matched NOCASE (no second Electricity) and the new one is created and searchable.
    assert_eq!(e.count("SELECT COUNT(*) FROM expense_types"), 2);
    assert_eq!(e.expenses.search_expense_types("Tea", None).unwrap().len(), 1);
    let codes = e.count("SELECT COUNT(DISTINCT expense_code) FROM expenses");
    assert_eq!(codes, 5, "codes come from the sequence");
    assert_eq!(e.count("SELECT COUNT(*) FROM expenses WHERE expense_date = '2026-10-01' AND amount = 40"), 2);
}

#[test]
fn expenses_execute_rolls_back_created_types_when_a_row_fails() {
    let e = env();
    // The last row is invalid, so the file is refused before anything is written.
    let f = exp_csv(&["2026-10-01,Brand New Type,10,".into(), "2026-10-01,Other,0,".into()]);
    assert!(e.rec.execute_expenses_import(&f).is_err());
    assert_eq!(e.count("SELECT COUNT(*) FROM expense_types"), 0);
    assert_eq!(e.count("SELECT COUNT(*) FROM expenses"), 0);
}

// ---------------------------------------------------------------------------------------------
// Templates
// ---------------------------------------------------------------------------------------------

#[test]
fn templates_of_every_kind_validate_cleanly_with_zero_rows() {
    use calamine::{open_workbook_auto, Reader};
    let e = env();
    let dir = std::env::temp_dir().join(format!("bk_tpl_rec_{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&dir).unwrap();
    for fmt in [TemplateFormat::Csv, TemplateFormat::Xlsx] {
        for kind in [ImportKind::Items, ImportKind::ServiceComponents, ImportKind::Parties, ImportKind::Expenses] {
            let saved = write_import_template(kind, dir.join(format!("{kind:?}.txt")).to_str().unwrap(), fmt).unwrap();
            let rep = match kind {
                ImportKind::Items => e.items_imp.validate_items_import(&saved).unwrap(),
                ImportKind::ServiceComponents => e.rec.validate_service_components_import(&saved).unwrap(),
                ImportKind::Parties => e.rec.validate_parties_import(&saved, ConflictStrategy::Skip).unwrap(),
                ImportKind::Expenses => e.rec.validate_expenses_import(&saved).unwrap(),
            };
            assert!(rep.header_issues.is_empty(), "{kind:?} {fmt:?}: {:?}", rep.header_issues);
            assert_eq!(rep.valid_count, 0);
        }
    }
    // The Excel items template carries the optional columns and the Components sheet.
    let mut wb = open_workbook_auto(dir.join("Items.xlsx")).unwrap();
    assert_eq!(wb.sheet_names(), vec!["Items".to_string(), "Components".to_string(), "Instructions".to_string()]);
    let range = wb.worksheet_range_at(0).unwrap().unwrap();
    let header: Vec<String> = range.rows().next().unwrap().iter().map(|c| c.to_string()).collect();
    assert!(header.ends_with(&["Item Type".to_string(), "Service Cost".to_string()]), "{header:?}");
}

#[test]
fn strategy_update_is_an_alias_of_overwrite() {
    let s: ConflictStrategy = serde_json::from_str("\"UPDATE\"").unwrap();
    assert_eq!(s, ConflictStrategy::Overwrite);
    let s: ConflictStrategy = serde_json::from_str("\"OVERWRITE\"").unwrap();
    assert_eq!(s, ConflictStrategy::Overwrite);
    let _ = AppError::IoError(String::new());
}

#[test]
fn l2_parties_update_with_blank_as_of_keeps_the_existing_opening_date() {
    let e = env();
    let id = e
        .parties
        .create_party(CreatePartyPayload {
            name: "Ravi".into(),
            phone: None,
            party_type: PartyType::Customer,
            notes: None,
            opening_balance: Some(OpeningBalanceInput { amount: 1000.0, as_of_date: "2026-09-01".into() }),
        })
        .unwrap();
    let f = party_csv(&["Ravi,CUSTOMER,,,1500,".into()]);
    let rep = e.rec.validate_parties_import(&f, ConflictStrategy::Overwrite).unwrap();
    assert!(rep.rows_with_errors.is_empty(), "{rep:?}");
    let sum = e.rec.execute_parties_import(&f, ConflictStrategy::Overwrite).unwrap();
    assert_eq!(sum.updated, 1);
    let ob = e.parties.get_party(id).unwrap().opening_balance.unwrap();
    assert_eq!((ob.amount, ob.as_of_date.as_str()), (1500.0, "2026-09-01"), "blank As Of keeps the date");
}
