//! Test Suite: FTS-backed party search (TC-PSRCH-01..08) on real SQLite.

mod common;

use std::sync::Arc;
use book_keeper::app_state::ensure_fts_index;
use book_keeper::core::entities::{CreatePartyPayload, UpdatePartyPayload};
use book_keeper::core::value_objects::PartyType;
use book_keeper::data::contracts::FtsIndex;
use book_keeper::services::contracts::party_service::PartyService;
use book_keeper::services::PartyServiceImpl;
use common::TestDb;

type Svc = PartyServiceImpl<TestDb>;

fn setup() -> (TestDb, Svc) {
    let db = TestDb::new();
    let svc = PartyServiceImpl::new(db.clone(), Arc::new(db.party_index()));
    (db, svc)
}

fn add(svc: &Svc, name: &str, phone: Option<&str>, t: PartyType) -> i64 {
    svc.create_party(CreatePartyPayload { name: name.into(), phone: phone.map(Into::into), party_type: t, notes: None, opening_balance: None })
        .unwrap()
}

fn names(svc: &Svc, q: &str, t: Option<PartyType>) -> Vec<String> {
    svc.search_parties(q, t).unwrap().into_iter().map(|r| r.party.name).collect()
}

#[test]
fn psrch_01_create_is_searchable_immediately() {
    let (_db, svc) = setup();
    add(&svc, "Ramesh Kumar", None, PartyType::Customer);
    assert_eq!(names(&svc, "ram", None), vec!["Ramesh Kumar"]);
}

#[test]
fn psrch_02_rename_replaces_the_index_row() {
    let (_db, svc) = setup();
    let id = add(&svc, "Ramesh Kumar", None, PartyType::Customer);
    svc.update_party(
        id,
        UpdatePartyPayload { name: "Suresh Kumar".into(), phone: None, party_type: PartyType::Customer, notes: None, opening_balance: None },
    )
    .unwrap();
    assert!(names(&svc, "ram", None).is_empty());
    assert_eq!(names(&svc, "sur", None), vec!["Suresh Kumar"]);
}

#[test]
fn psrch_03_multi_token_is_anded() {
    let (_db, svc) = setup();
    add(&svc, "Ramesh Kumar", Some("98765-43210"), PartyType::Customer);
    add(&svc, "Ramesh Gupta", Some("11111"), PartyType::Customer);
    assert_eq!(names(&svc, "ram 987", None), vec!["Ramesh Kumar"]);
}

#[test]
fn psrch_04_digits_without_separators_find_formatted_phone() {
    let (_db, svc) = setup();
    add(&svc, "Ramesh Kumar", Some("98765-43210"), PartyType::Customer);
    assert_eq!(names(&svc, "9876543210", None), vec!["Ramesh Kumar"]);
}

#[test]
fn psrch_05_last_digits_fallback_needs_three_digits() {
    let (_db, svc) = setup();
    add(&svc, "Ramesh Kumar", Some("98765-43210"), PartyType::Customer);
    assert_eq!(names(&svc, "3210", None), vec!["Ramesh Kumar"]);
    assert!(names(&svc, "32", None).is_empty());
}

#[test]
fn psrch_06_blank_lists_all_and_symbol_only_matches_nothing() {
    let (_db, svc) = setup();
    add(&svc, "A One", None, PartyType::Customer);
    add(&svc, "B Two", None, PartyType::Vendor);
    assert_eq!(names(&svc, "", None).len(), 2);
    assert!(names(&svc, "%", None).is_empty());
    assert!(names(&svc, "!!", None).is_empty());
}

#[test]
fn psrch_07_type_filter_and_balance_columns() {
    let (_db, svc) = setup();
    add(&svc, "Shared Name", None, PartyType::Customer);
    add(&svc, "Shared Name Two", None, PartyType::Vendor);
    let rows = svc.search_parties("shared", Some(PartyType::Vendor)).unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!((rows[0].outstanding_balance, rows[0].transaction_count), (0.0, 0));
}

#[test]
fn psrch_08_ensure_fts_index_repairs_missing_rows() {
    let (db, svc) = setup();
    add(&svc, "Ramesh Kumar", None, PartyType::Customer);
    db.pool.get().unwrap().execute("DELETE FROM party_search_fts", []).unwrap();
    assert!(names(&svc, "ram", None).is_empty());
    let idx = db.party_index();
    assert!(ensure_fts_index(&idx).unwrap());
    assert_eq!(names(&svc, "ram", None), vec!["Ramesh Kumar"]);
    assert_eq!(idx.indexed_count().unwrap(), idx.source_count().unwrap());
    assert!(!ensure_fts_index(&idx).unwrap());
}
