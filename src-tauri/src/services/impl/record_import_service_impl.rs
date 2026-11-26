//! FR-41 imports of service components, parties and expenses (LLD section 26). Rows are
//! parsed by the shared core and written through the same code the screens use: the FR-35 party
//! opening-balance code, the expense / expense-type code and the item service's component rules.

use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use crate::core::dtos::{
    ConflictStrategy, ExpenseFilter, ExpensePayload, HeaderIssue, ImportDuplicate, ImportRowError, ImportRowWarning,
    ImportSummary, ImportValidationReport,
};
use crate::core::entities::{OpeningBalanceInput, Party};
use crate::core::error::AppError;
use crate::core::money::{round2, MONEY_EPS};
use crate::core::value_objects::{clean_name, clean_optional, PartyType};
use crate::data::contracts::search_indexer::FtsIndex;
use crate::data::contracts::unit_of_work::{TransactionContext, UnitOfWork};
use crate::data::sqlite::fts::sync_after_commit;
use crate::services::contracts::record_import_service::RecordImportService;
use super::expense_service_impl::{check_payload, create_expense_in_tx, get_or_create_type_in_tx, valid_type_name};
use super::import_core as core;
use super::import_core::{ColumnSpec, Parsed, RawRow};
use super::item_import_service_impl::{
    apply_component_plan, component_specs, parse_component_grid, plan_components, ParsedComponents,
};
use super::party_service_impl::{apply_opening_balance, check_opening_update};

#[derive(Clone)]
pub struct RecordImportServiceImpl<U: UnitOfWork = Arc<dyn UnitOfWork>> {
    pub uow: U,
    pub party_index: Arc<dyn FtsIndex>,
    pub expense_type_index: Arc<dyn FtsIndex>,
}

impl<U: UnitOfWork> RecordImportServiceImpl<U> {
    pub fn new(uow: U, party_index: Arc<dyn FtsIndex>, expense_type_index: Arc<dyn FtsIndex>) -> Self {
        Self { uow, party_index, expense_type_index }
    }
}

// ---------------------------------------------------------------------------------------------
// Template columns
// ---------------------------------------------------------------------------------------------

const P_NAME: &str = "name";
const P_TYPE: &str = "type";
const P_PHONE: &str = "phone";
const P_NOTES: &str = "notes";
const P_BALANCE: &str = "opening balance";
const P_DATE: &str = "as of date";

pub(crate) static PARTY_COLUMNS: [ColumnSpec; 6] = [
    ColumnSpec { key: P_NAME, display: "Name", required: true, numeric: false,
        rule: "Text. A customer or vendor is the same name and type, ignoring case.", example: "Ravi Traders" },
    ColumnSpec { key: P_TYPE, display: "Type", required: true, numeric: false,
        rule: "CUSTOMER or VENDOR.", example: "CUSTOMER" },
    ColumnSpec { key: P_PHONE, display: "Phone", required: false, numeric: false,
        rule: "Text. Optional.", example: "9876543210" },
    ColumnSpec { key: P_NOTES, display: "Notes", required: false, numeric: false,
        rule: "Text. Optional.", example: "Wholesale" },
    ColumnSpec { key: P_BALANCE, display: "Opening Balance", required: false, numeric: true,
        rule: "Number, 0 or more. What the customer owes you (or you owe the vendor) before you started. Creates an OB- opening bill. Empty = none.", example: "5000" },
    ColumnSpec { key: P_DATE, display: "As Of Date", required: false, numeric: false,
        rule: "Date of the balance: YYYY-MM-DD or DD-MM-YYYY, not in the future. Empty = today.", example: "2026-11-01" },
];

const E_DATE: &str = "date";
const E_TYPE: &str = "expense type";
const E_AMOUNT: &str = "amount";
const E_NOTES: &str = "notes";

pub(crate) static EXPENSE_COLUMNS: [ColumnSpec; 4] = [
    ColumnSpec { key: E_DATE, display: "Date", required: true, numeric: false,
        rule: "YYYY-MM-DD or DD-MM-YYYY, not in the future.", example: "2026-11-20" },
    ColumnSpec { key: E_TYPE, display: "Expense Type", required: true, numeric: false,
        rule: "Text. An existing type (case does not matter) or a new one, which is created.", example: "Electricity" },
    ColumnSpec { key: E_AMOUNT, display: "Amount", required: true, numeric: true,
        rule: "Number greater than 0.", example: "1250.50" },
    ColumnSpec { key: E_NOTES, display: "Notes", required: false, numeric: false,
        rule: "Text, up to 500 characters. Optional.", example: "October bill" },
];

pub(crate) fn party_specs() -> Vec<&'static ColumnSpec> {
    PARTY_COLUMNS.iter().collect()
}

pub(crate) fn expense_specs() -> Vec<&'static ColumnSpec> {
    EXPENSE_COLUMNS.iter().collect()
}

fn suggest_with(specs: &[&'static ColumnSpec], header: &str, words: &[(&[&str], &str)]) -> Option<&'static str> {
    if let Some(s) = core::suggest_by_spelling(specs, header) {
        return Some(s);
    }
    let sq = core::squash(header);
    if sq.is_empty() {
        return None;
    }
    words
        .iter()
        .find(|(ws, _)| ws.iter().any(|w| sq.contains(w)))
        .and_then(|(_, key)| specs.iter().find(|c| c.key == *key).map(|c| c.display))
}

fn suggest_party_column(header: &str) -> Option<&'static str> {
    suggest_with(
        &party_specs(),
        header,
        &[
            (&["balance", "due", "outstanding", "opening", "amount"], P_BALANCE),
            (&["asof", "balancedate", "date"], P_DATE),
            (&["phone", "mobile", "contact", "cell"], P_PHONE),
            (&["note", "remark", "comment"], P_NOTES),
            (&["type", "kind", "category", "role"], P_TYPE),
            (&["name", "party", "customer", "vendor", "supplier"], P_NAME),
        ],
    )
}

fn suggest_expense_column(header: &str) -> Option<&'static str> {
    suggest_with(
        &expense_specs(),
        header,
        &[
            (&["amount", "value", "cost", "paid", "rupee"], E_AMOUNT),
            (&["date", "day"], E_DATE),
            (&["type", "category", "head", "kind", "expense"], E_TYPE),
            (&["note", "remark", "comment", "description", "particular"], E_NOTES),
        ],
    )
}

fn fmt_money(v: f64) -> String {
    if (v - v.round()).abs() < MONEY_EPS { format!("{}", v.round() as i64) } else { format!("{v:.2}") }
}

// ---------------------------------------------------------------------------------------------
// Parties
// ---------------------------------------------------------------------------------------------

#[derive(Debug, Clone)]
struct PartyRow {
    row: usize,
    name: String,
    party_type: PartyType,
    phone: Option<String>,
    notes: Option<String>,
    opening: Option<OpeningBalanceInput>,
}

fn parse_party_rows(raws: &[RawRow], errors: &mut Vec<ImportRowError>, warnings: &mut Vec<ImportRowWarning>) -> Vec<PartyRow> {
    let specs = party_specs();
    let today = core::today();
    let mut out = Vec::new();
    let mut seen: HashMap<(String, PartyType), usize> = HashMap::new();
    for raw in raws {
        let before = errors.len();
        let name = clean_name(&raw.text(P_NAME));
        let nm = Some(name.as_str()).filter(|n| !n.is_empty());
        if name.is_empty() {
            core::push_error(errors, raw.row, None, "Name", "Name is empty. Every row needs a name (or delete the row).");
        }
        let type_text = raw.text(P_TYPE);
        let party_type = match type_text.trim().to_uppercase().as_str() {
            "CUSTOMER" => Some(PartyType::Customer),
            "VENDOR" => Some(PartyType::Vendor),
            "" => {
                core::push_error(errors, raw.row, nm, "Type", "Type is empty. Enter CUSTOMER or VENDOR.");
                None
            }
            _ => {
                core::push_error(errors, raw.row, nm, "Type", format!("Type \"{type_text}\" is not valid. Enter CUSTOMER or VENDOR."));
                None
            }
        };
        let balance = core::parse_number(&specs, raw, nm, P_BALANCE, false, errors).map(round2);
        let date_text = raw.text(P_DATE);
        let mut as_of = today.clone();
        if !date_text.is_empty() {
            match core::parse_date_text(&date_text) {
                Some(d) => as_of = d,
                None => core::push_error(
                    errors,
                    raw.row,
                    nm,
                    "As Of Date",
                    format!("As Of Date \"{date_text}\" is not a date. {}", core::date_hint()),
                ),
            }
        }
        let opening = balance.filter(|b| *b > MONEY_EPS).map(|amount| OpeningBalanceInput { amount, as_of_date: as_of.clone() });
        if opening.is_none() && !date_text.is_empty() {
            warnings.push(ImportRowWarning {
                row: raw.row,
                name: name.clone(),
                message: "As Of Date is ignored because there is no Opening Balance.".into(),
            });
        }
        if let (Some(t), false) = (party_type, name.is_empty()) {
            let key = (name.to_lowercase(), t);
            if let Some(first) = seen.get(&key) {
                core::push_error(
                    errors,
                    raw.row,
                    nm,
                    "Name",
                    format!("\"{name}\" ({}) is repeated (first seen in row {first}). Each name and type may appear once.", type_text.trim().to_uppercase()),
                );
            } else {
                seen.insert(key, raw.row);
            }
        }
        if errors.len() == before {
            if let Some(party_type) = party_type {
                out.push(PartyRow {
                    row: raw.row,
                    name,
                    party_type,
                    phone: Some(raw.text(P_PHONE).trim().to_string()).filter(|p| !p.is_empty()),
                    notes: clean_optional(Some(&raw.text(P_NOTES))),
                    opening,
                });
            }
        }
    }
    out
}

struct PartyChecked {
    rows: Vec<(PartyRow, Option<Party>)>,
    errors: Vec<ImportRowError>,
    warnings: Vec<ImportRowWarning>,
    duplicates: Vec<ImportDuplicate>,
}

fn check_parties(
    tx: &mut dyn TransactionContext,
    rows: Vec<PartyRow>,
    mut errors: Vec<ImportRowError>,
    mut warnings: Vec<ImportRowWarning>,
    strategy: ConflictStrategy,
) -> Result<PartyChecked, AppError> {
    let mut known: HashMap<(String, PartyType), Party> = HashMap::new();
    // FR-46: duplicates are matched among active parties only.
    for p in tx.parties().list(None, true)? {
        known.insert((p.name.to_lowercase(), p.party_type), p);
    }
    let update = strategy == ConflictStrategy::Overwrite;
    let mut out = Vec::new();
    let mut duplicates = Vec::new();
    for row in rows {
        let existing = known.get(&(row.name.to_lowercase(), row.party_type)).cloned();
        if let Some(e) = &existing {
            duplicates.push(ImportDuplicate { row: row.row, name: row.name.clone(), existing_item_id: e.id });
            if !update {
                warnings.push(ImportRowWarning { row: row.row, name: row.name.clone(), message: "Already exists; skipped.".into() });
            }
        }
        // Same checks the party screen applies on save (FR-35, FR-28, below what is settled).
        let check = match (&row.opening, &existing) {
            (Some(o), None) => check_opening_update(tx, None, o),
            (Some(o), Some(e)) if update => check_opening_update(tx, Some(e), o),
            _ => Ok(()),
        };
        if let Err(AppError::ValidationError { message, .. }) = check {
            core::push_error(&mut errors, row.row, Some(&row.name), "Opening Balance", message);
            continue;
        }
        check?;
        out.push((row, existing));
    }
    errors.sort_by_key(|e| e.row);
    Ok(PartyChecked { rows: out, errors, warnings, duplicates })
}

type PartyParse = (Vec<PartyRow>, Vec<ImportRowError>, Vec<ImportRowWarning>);

fn parties_prepare(path: &str) -> Result<Result<PartyParse, Vec<HeaderIssue>>, AppError> {
    let raws = match core::parse_raw(path, &party_specs(), &suggest_party_column)? {
        Parsed::Rows(r) => r,
        Parsed::HeaderIssues(i) => return Ok(Err(i)),
    };
    let mut errors = Vec::new();
    let mut warnings = Vec::new();
    let rows = parse_party_rows(&raws, &mut errors, &mut warnings);
    Ok(Ok((rows, errors, warnings)))
}

// ---------------------------------------------------------------------------------------------
// Expenses
// ---------------------------------------------------------------------------------------------

#[derive(Debug, Clone)]
struct ExpenseRow {
    row: usize,
    date: String,
    type_name: String,
    amount: f64,
    notes: Option<String>,
}

impl ExpenseRow {
    fn key(&self) -> (String, String, i64, String) {
        (self.date.clone(), self.type_name.to_lowercase(), (self.amount * 100.0).round() as i64, self.notes.clone().unwrap_or_default())
    }
}

fn parse_expense_rows(raws: &[RawRow], errors: &mut Vec<ImportRowError>) -> Vec<ExpenseRow> {
    let specs = expense_specs();
    let mut out = Vec::new();
    for raw in raws {
        let before = errors.len();
        let type_raw = raw.text(E_TYPE);
        let type_name = clean_name(&type_raw);
        let nm = Some(type_name.as_str()).filter(|n| !n.is_empty());
        let date_text = raw.text(E_DATE);
        let mut date = String::new();
        if date_text.is_empty() {
            core::push_error(errors, raw.row, nm, "Date", "Date is empty. Enter the expense date.");
        } else {
            match core::parse_date_text(&date_text) {
                Some(d) => date = d,
                None => core::push_error(errors, raw.row, nm, "Date", format!("Date \"{date_text}\" is not a date. {}", core::date_hint())),
            }
        }
        if let Err(e) = valid_type_name(&type_raw) {
            let message = match &e {
                AppError::ValidationError { message, .. } => message.clone(),
                other => other.to_string(),
            };
            core::push_error(errors, raw.row, nm, "Expense Type", message);
        }
        let amount = core::parse_number(&specs, raw, nm, E_AMOUNT, true, errors);
        let notes = clean_optional(Some(&raw.text(E_NOTES)));
        // The expense service's own rules (date not in the future, amount > 0, notes length).
        if errors.len() == before {
            let payload = ExpensePayload { expense_date: date.clone(), expense_type_id: 0, amount: amount.unwrap_or(0.0), notes: notes.clone() };
            match check_payload(&payload) {
                Ok(c) => out.push(ExpenseRow { row: raw.row, date: c.date, type_name, amount: c.amount, notes: c.notes }),
                Err(AppError::ValidationError { field, message }) => {
                    let label = match field.as_str() {
                        "expense_date" => "Date",
                        "amount" => "Amount",
                        "notes" => "Notes",
                        other => other,
                    };
                    core::push_error(errors, raw.row, nm, label, message);
                }
                Err(e) => core::push_error(errors, raw.row, nm, "Row", e.to_string()),
            }
        }
    }
    out
}

struct ExpenseChecked {
    rows: Vec<ExpenseRow>,
    warnings: Vec<ImportRowWarning>,
    new_types: Vec<String>,
}

fn check_expenses(tx: &mut dyn TransactionContext, rows: Vec<ExpenseRow>) -> Result<ExpenseChecked, AppError> {
    let types: HashSet<String> = tx.expenses().list_types(false)?.into_iter().map(|t| t.name.to_lowercase()).collect();
    let mut new_types: Vec<String> = Vec::new();
    let mut warnings = Vec::new();
    let mut seen: HashMap<(String, String, i64, String), usize> = HashMap::new();
    let mut db_keys: HashSet<(String, String, i64, String)> = HashSet::new();
    let mut dates_loaded: HashSet<String> = HashSet::new();
    for r in &rows {
        if !types.contains(&r.type_name.to_lowercase()) && !new_types.iter().any(|t| t.eq_ignore_ascii_case(&r.type_name)) {
            new_types.push(r.type_name.clone());
        }
        if dates_loaded.insert(r.date.clone()) {
            let filter = ExpenseFilter { start_date: Some(r.date.clone()), end_date: Some(r.date.clone()), ..Default::default() };
            let (stored, _, _) = tx.expenses().list(&filter, 0, 500)?;
            for e in stored {
                db_keys.insert((e.expense_date, e.expense_type_name.to_lowercase(), (e.amount * 100.0).round() as i64, e.notes.unwrap_or_default()));
            }
        }
        let key = r.key();
        if let Some(first) = seen.get(&key) {
            warnings.push(ImportRowWarning {
                row: r.row,
                name: r.type_name.clone(),
                message: format!("Identical to row {first} (same date, type, amount and notes). It will still be imported; delete the row if it is a repeat."),
            });
        } else if db_keys.contains(&key) {
            warnings.push(ImportRowWarning {
                row: r.row,
                name: r.type_name.clone(),
                message: "An expense with the same date, type, amount and notes is already saved. It will still be imported; delete the row if it is a repeat.".into(),
            });
        }
        seen.entry(key).or_insert(r.row);
    }
    Ok(ExpenseChecked { rows, warnings, new_types })
}

// ---------------------------------------------------------------------------------------------
// Service
// ---------------------------------------------------------------------------------------------

fn report(
    valid_count: usize,
    errors: Vec<ImportRowError>,
    duplicates: Vec<ImportDuplicate>,
    mut warnings: Vec<ImportRowWarning>,
    specs: &[&ColumnSpec],
    info: Vec<String>,
) -> ImportValidationReport {
    warnings.sort_by_key(|w| w.row);
    ImportValidationReport {
        valid_count,
        rows_with_errors: errors,
        duplicates,
        header_issues: Vec::new(),
        warnings,
        lots_to_create: 0,
        expected_columns: core::expected_columns_of(specs),
        info,
    }
}

struct ComponentsFile {
    rows: Vec<super::item_import_service_impl::ComponentRow>,
    errors: Vec<ImportRowError>,
}

fn components_prepare(path: &str) -> Result<Result<ComponentsFile, Vec<HeaderIssue>>, AppError> {
    let grid = core::read_grid(path)?;
    let mut errors = Vec::new();
    Ok(match parse_component_grid(grid, &mut errors)? {
        ParsedComponents::Rows(rows) => Ok(ComponentsFile { rows, errors }),
        ParsedComponents::HeaderIssues(i) => Err(i),
    })
}

impl<U: UnitOfWork> RecordImportService for RecordImportServiceImpl<U> {
    fn validate_service_components_import(&self, file_path: &str) -> Result<ImportValidationReport, AppError> {
        let specs = component_specs();
        let f = match components_prepare(file_path)? {
            Ok(f) => f,
            Err(i) => return Ok(core::header_report(i, &specs)),
        };
        let mut tx = self.uow.begin_read()?;
        let mut errors = f.errors;
        let plans = plan_components(&mut *tx, &f.rows, None, &HashSet::new(), &mut errors)?;
        errors.sort_by_key(|e| e.row);
        let lines: usize = plans.iter().map(|p| p.components.len()).sum();
        let info = if plans.is_empty() { Vec::new() } else { vec![format!("{} service(s) will get a new component list (the old list is replaced).", plans.len())] };
        Ok(report(lines, errors, Vec::new(), Vec::new(), &specs, info))
    }

    fn execute_service_components_import(&self, file_path: &str) -> Result<ImportSummary, AppError> {
        let f = match components_prepare(file_path)? {
            Ok(f) => f,
            Err(i) => return Err(core::header_error(&i)),
        };
        let mut tx = self.uow.begin_transaction()?;
        let mut errors = f.errors;
        let plans = plan_components(&mut *tx, &f.rows, None, &HashSet::new(), &mut errors)?;
        if let Some(e) = core::rows_have_errors(&errors) {
            return Err(e);
        }
        for p in &plans {
            apply_component_plan(&mut *tx, p)?;
        }
        tx.commit()?;
        Ok(ImportSummary { created: 0, updated: plans.len(), skipped: 0, stock_lots_created: 0 })
    }

    fn validate_parties_import(&self, file_path: &str, strategy: ConflictStrategy) -> Result<ImportValidationReport, AppError> {
        let specs = party_specs();
        let (rows, errors, warnings) = match parties_prepare(file_path)? {
            Ok(v) => v,
            Err(i) => return Ok(core::header_report(i, &specs)),
        };
        let mut tx = self.uow.begin_read()?;
        let c = check_parties(&mut *tx, rows, errors, warnings, strategy)?;
        let update = strategy == ConflictStrategy::Overwrite;
        let mut bills = 0usize;
        let mut total = 0.0;
        for (r, e) in &c.rows {
            if r.opening.is_some() && (e.is_none() || update) {
                bills += 1;
                total += r.opening.as_ref().map(|o| o.amount).unwrap_or(0.0);
            }
        }
        let mut info = Vec::new();
        let new_count = c.rows.iter().filter(|(_, e)| e.is_none()).count();
        if new_count > 0 {
            info.push(format!("{new_count} new {} will be created.", if new_count == 1 { "party" } else { "parties" }));
        }
        if bills > 0 {
            info.push(format!("{bills} opening balance bill(s) totalling {} will be created or updated.", fmt_money(total)));
        }
        Ok(report(c.rows.len(), c.errors, c.duplicates, c.warnings, &specs, info))
    }

    fn execute_parties_import(&self, file_path: &str, strategy: ConflictStrategy) -> Result<ImportSummary, AppError> {
        if strategy == ConflictStrategy::AddStock {
            return Err(core::vfail("strategy", "Choose Skip or Update for existing parties."));
        }
        let (rows, errors, warnings) = match parties_prepare(file_path)? {
            Ok(v) => v,
            Err(i) => return Err(core::header_error(&i)),
        };
        let mut tx = self.uow.begin_transaction()?;
        let c = check_parties(&mut *tx, rows, errors, warnings, strategy)?;
        if let Some(e) = core::rows_have_errors(&c.errors) {
            return Err(e);
        }
        let now = chrono::Utc::now().to_rfc3339();
        let mut summary = ImportSummary { created: 0, updated: 0, skipped: 0, stock_lots_created: 0 };
        let mut touched = Vec::new();
        for (row, existing) in c.rows {
            match existing {
                None => {
                    let party = Party {
                        id: 0,
                        name: row.name.clone(),
                        phone: row.phone.clone(),
                        party_type: row.party_type,
                        notes: row.notes.clone(),
                        is_active: true,
                        created_at: now.clone(),
                        updated_at: now.clone(),
                        opening_balance: None,
                    };
                    let id = tx.parties().create(&party)?;
                    if let Some(o) = &row.opening {
                        apply_opening_balance(&mut *tx, &Party { id, ..party }, o)?;
                    }
                    summary.created += 1;
                    touched.push(id);
                }
                Some(_) if strategy != ConflictStrategy::Overwrite => summary.skipped += 1,
                Some(mut party) => {
                    if row.phone.is_some() {
                        party.phone = row.phone.clone();
                    }
                    if row.notes.is_some() {
                        party.notes = row.notes.clone();
                    }
                    party.updated_at = now.clone();
                    tx.parties().update(&party)?;
                    if let Some(o) = &row.opening {
                        apply_opening_balance(&mut *tx, &party, o)?;
                    }
                    summary.updated += 1;
                    touched.push(party.id);
                }
            }
        }
        tx.commit()?;
        for id in touched {
            sync_after_commit(&*self.party_index, id, "party");
        }
        Ok(summary)
    }

    fn validate_expenses_import(&self, file_path: &str) -> Result<ImportValidationReport, AppError> {
        let specs = expense_specs();
        let raws = match core::parse_raw(file_path, &specs, &suggest_expense_column)? {
            Parsed::Rows(r) => r,
            Parsed::HeaderIssues(i) => return Ok(core::header_report(i, &specs)),
        };
        let mut errors = Vec::new();
        let rows = parse_expense_rows(&raws, &mut errors);
        let mut tx = self.uow.begin_read()?;
        let c = check_expenses(&mut *tx, rows)?;
        errors.sort_by_key(|e| e.row);
        let mut info = Vec::new();
        if !c.new_types.is_empty() {
            info.push(format!("{} new expense type(s) will be created: {}.", c.new_types.len(), c.new_types.join(", ")));
        }
        let total: f64 = c.rows.iter().map(|r| r.amount).sum();
        if !c.rows.is_empty() {
            info.push(format!("{} expense(s) totalling {} will be added.", c.rows.len(), fmt_money(round2(total))));
        }
        Ok(report(c.rows.len(), errors, Vec::new(), c.warnings, &specs, info))
    }

    fn execute_expenses_import(&self, file_path: &str) -> Result<ImportSummary, AppError> {
        let specs = expense_specs();
        let raws = match core::parse_raw(file_path, &specs, &suggest_expense_column)? {
            Parsed::Rows(r) => r,
            Parsed::HeaderIssues(i) => return Err(core::header_error(&i)),
        };
        let mut errors = Vec::new();
        let rows = parse_expense_rows(&raws, &mut errors);
        if let Some(e) = core::rows_have_errors(&errors) {
            return Err(e);
        }
        let mut tx = self.uow.begin_transaction()?;
        let mut created_types = Vec::new();
        let mut count = 0;
        for r in &rows {
            let (t, created) = get_or_create_type_in_tx(&mut *tx, &r.type_name)?;
            if created {
                created_types.push(t.id);
            }
            create_expense_in_tx(
                &mut *tx,
                &ExpensePayload { expense_date: r.date.clone(), expense_type_id: t.id, amount: r.amount, notes: r.notes.clone() },
            )?;
            count += 1;
        }
        tx.commit()?;
        for id in created_types {
            sync_after_commit(&*self.expense_type_index, id, "expense type");
        }
        Ok(ImportSummary { created: count, updated: 0, skipped: 0, stock_lots_created: 0 })
    }
}
