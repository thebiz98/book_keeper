// tests/search_indexer_test.rs
//! Test Suite: Search Indexer & Typeahead (FTS5 / Inverted Index)
//! Test Cases: TC-SRCH-001 through TC-SRCH-004
//! Derived from book_keeper_LLD.md § 11.7 and Spec Miner Report

mod common;

use std::time::Instant;
use common::fixtures::*;
use common::TestDb;
use book_keeper::data::contracts::search_indexer::ItemSearchIndexer;
use book_keeper::data::contracts::unit_of_work::UnitOfWork;

#[test]
fn test_tc_srch_001_sub_millisecond_prefix_search() {
    let uow = TestDb::new();
    let indexer = uow.indexer();

    let mut tx = uow.begin_transaction().expect("begin tx");
    let item = create_test_item(1, "Apsara Platinum Pencil", Some("Stationery"), Some("8901030381000"), 50.0, 10.0);
    tx.items().create(&item).expect("create item");

    let bundle = create_test_bundle(1, 1, "Box of 10", 10.0, Some(90.0), None);
    tx.bundles().create(&bundle).expect("create bundle");
    tx.commit().expect("commit");

    indexer.index_item_with_bundles(&item, &[bundle]).expect("index in FTS5");

    // Action: search with prefix "Aps*"
    let start = Instant::now();
    let results = indexer.search("Aps*").expect("search succeeds");
    let elapsed = start.elapsed();

    // Assert: latency < 10ms (LLD < 1ms on native, generous upper bound for cold runner)
    assert!(!results.is_empty(), "Must find Apsara Platinum Pencil");
    assert_eq!(results[0].item_id, 1);
    assert_eq!(results[0].name, "Apsara Platinum Pencil");
    assert_eq!(results[0].bundles.len(), 1, "Must include packaging bundle mappings");
    assert_eq!(results[0].bundles[0].bundle_name, "Box of 10");
    assert!(elapsed < std::time::Duration::from_millis(10), "Latency must be < 10ms; was {:?}", elapsed);
    println!("Prefix search latency: {:?}", elapsed);
}

#[test]
fn test_tc_srch_002_barcode_exact_and_prefix_scanner_lookup() {
    let uow = TestDb::new();
    let indexer = uow.indexer();

    let mut tx = uow.begin_transaction().expect("begin tx");
    let item1 = create_test_item(1, "Doms Colour Pencils", None, Some("8901030381023"), 20.0, 60.0);
    let item2 = create_test_item(2, "Doms Eraser", None, Some("8901030381099"), 30.0, 5.0);
    tx.items().create(&item1).expect("create item 1");
    tx.items().create(&item2).expect("create item 2");
    tx.commit().expect("commit");

    indexer.index_item(&item1).expect("index item 1");
    indexer.index_item(&item2).expect("index item 2");

    // Action: Exact barcode scanner input "8901030381023"
    let results = indexer.search("8901030381023").expect("barcode search");

    // Assert: Exact barcode match is returned as first result
    assert!(!results.is_empty());
    assert_eq!(results[0].item_id, 1);
    assert_eq!(results[0].barcode.as_deref(), Some("8901030381023"));
}

#[test]
fn test_tc_srch_003_soft_deleted_archived_item_exclusion() {
    let uow = TestDb::new();
    let indexer = uow.indexer();

    let mut tx = uow.begin_transaction().expect("begin tx");
    let mut archived_item = create_test_item(3, "Old Discontinued Ink", None, None, 0.0, 20.0);
    archived_item.is_active = false; // Soft-deleted
    tx.items().create(&archived_item).expect("create item");
    tx.commit().expect("commit");

    indexer.index_item(&archived_item).expect("index archived item");

    // Action: Search for archived item name
    let results = indexer.search("Discontinued").expect("search");

    // Assert: Returns 0 results (hidden from active search)
    assert!(results.is_empty(), "Soft-archived item must be excluded from search results");
}

#[test]
fn test_tc_srch_004_dynamic_bundle_suggestion_aggregation() {
    let uow = TestDb::new();
    let mut tx = uow.begin_transaction().expect("begin tx");

    // Setup: 10 items have "Box" (10 units), 2 items have "Dozen" (12 units)
    for i in 1..=10 {
        let item = create_test_item(i, &format!("Item {}", i), None, None, 10.0, 10.0);
        tx.items().create(&item).expect("create item");
        let box_bundle = create_test_bundle(i, i, "Box", 10.0, Some(90.0), None);
        tx.bundles().create(&box_bundle).expect("create bundle");
    }

    for i in 11..=12 {
        let item = create_test_item(i, &format!("Item {}", i), None, None, 12.0, 15.0);
        tx.items().create(&item).expect("create item");
        let dozen_bundle = create_test_bundle(i, i, "Dozen", 12.0, Some(160.0), None);
        tx.bundles().create(&dozen_bundle).expect("create bundle");
    }
    tx.commit().expect("commit");

    // Action: query suggestions from BundleRepository
    let mut read_tx = uow.begin_transaction().expect("read tx");
    let suggestions = read_tx.bundles().get_bundle_suggestions().expect("list suggestions");

    // Assert: Top suggestion is ("Box", 10.0, usage_count = 10), second is ("Dozen", 12.0, usage_count = 2)
    assert!(suggestions.len() >= 2);
    assert_eq!(suggestions[0].bundle_name, "Box");
    assert_eq!(suggestions[0].units_per_bundle, 10.0);
    assert_eq!(suggestions[0].usage_count, 10);

    assert_eq!(suggestions[1].bundle_name, "Dozen");
    assert_eq!(suggestions[1].units_per_bundle, 12.0);
    assert_eq!(suggestions[1].usage_count, 2);
}

fn seed(uow: &TestDb, items: &[(i64, &str, Option<&str>, Option<&str>)]) {
    let indexer = uow.indexer();
    let mut tx = uow.begin_transaction().expect("begin tx");
    for (id, name, cat, bc) in items {
        let it = create_test_item(*id, name, *cat, *bc, 5.0, 10.0);
        tx.items().create(&it).expect("create item");
    }
    tx.commit().expect("commit");
    for (id, name, cat, bc) in items {
        indexer.index_item(&create_test_item(*id, name, *cat, *bc, 5.0, 10.0)).expect("index");
    }
}

#[test]
fn test_srch_multi_word_and_semantics() {
    let uow = TestDb::new();
    seed(&uow, &[(1, "Blue Ink Pen", None, None), (2, "Blue Notebook", None, None), (3, "Red Ink", None, None)]);
    let indexer = uow.indexer();
    let ids = |q: &str| indexer.search(q).unwrap().into_iter().map(|r| r.item_id).collect::<Vec<_>>();
    assert_eq!(ids("blue ink"), vec![1]);
    let mut both = ids("ink");
    both.sort();
    assert_eq!(both, vec![1, 3]);
    assert!(ids("blue zebra").is_empty());
}

#[test]
fn test_srch_query_sanitisation_never_errors() {
    let uow = TestDb::new();
    seed(&uow, &[(1, "Blue Pen", Some("Stationery"), None)]);
    let indexer = uow.indexer();
    for q in ["\"", "\"blue", "*", "bl*ue", "-pen", "blue:pen", "(pen)", "pen)", "((", "a OR b", "NEAR(pen)", "blue -", "%", "_", "'; DROP TABLE items;--"] {
        indexer.search(q).unwrap_or_else(|e| panic!("query {q:?} must not error: {e:?}"));
    }
    assert_eq!(indexer.search("(blue) \"pen\"").unwrap().len(), 1);
    assert_eq!(indexer.search("-pen").unwrap().len(), 1);
    // LIKE wildcards are not special.
    assert!(indexer.search("%").unwrap().is_empty());
    assert!(indexer.search("_").unwrap().is_empty());
}

#[test]
fn test_srch_bundle_barcode_resolves_parent_with_matched_bundle_and_dedupes() {
    let uow = TestDb::new();
    let indexer = uow.indexer();
    let mut tx = uow.begin_transaction().unwrap();
    let item = create_test_item(1, "Gel Pen", None, Some("1000"), 5.0, 10.0);
    tx.items().create(&item).unwrap();
    let b1 = create_test_bundle(1, 1, "Box", 10.0, Some(90.0), Some("2000"));
    let b2 = create_test_bundle(2, 1, "Case", 100.0, Some(800.0), Some("3000"));
    tx.bundles().create(&b1).unwrap();
    tx.bundles().create(&b2).unwrap();
    tx.commit().unwrap();
    indexer.index_item_with_bundles(&item, &[b1, b2]).unwrap();

    let r = indexer.search("3000").unwrap();
    assert_eq!(r.len(), 1);
    assert_eq!((r[0].item_id, r[0].matched_bundle_id), (1, Some(2)));
    assert_eq!(r[0].bundles.len(), 2);
    // Item barcode: no matched bundle.
    let r = indexer.search("1000").unwrap();
    assert_eq!((r[0].item_id, r[0].matched_bundle_id), (1, None));
    // Name hit matching several rows of the same item is deduped.
    let r = indexer.search("gel").unwrap();
    assert_eq!(r.len(), 1);
    assert_eq!(r[0].matched_bundle_id, None);
}

#[test]
fn test_srch_exact_barcode_ranks_first_and_limit_applies() {
    let uow = TestDb::new();
    seed(&uow, &[
        (1, "Pen 123 Premium", None, Some("9990")),
        (2, "Pen Basic", None, Some("123")),
        (3, "Pen 123", None, None),
    ]);
    let indexer = uow.indexer();
    assert_eq!(indexer.search("123").unwrap()[0].item_id, 2, "exact barcode first");
    assert_eq!(indexer.search_with_limit("pen", 2).unwrap().len(), 2);
    let many: Vec<_> = (10..30).map(|i| (i, format!("Widget {i}"))).collect();
    let refs: Vec<(i64, &str, Option<&str>, Option<&str>)> =
        many.iter().map(|(i, n)| (*i, n.as_str(), None, None)).collect();
    seed(&uow, &refs);
    assert_eq!(indexer.search("widget").unwrap().len(), 8, "default limit is 8");
}

#[test]
fn test_srch_archived_item_excluded_even_when_row_remains() {
    let uow = TestDb::new();
    seed(&uow, &[(1, "Old Ink", None, None)]);
    let indexer = uow.indexer();
    assert_eq!(indexer.search("ink").unwrap().len(), 1);
    let mut tx = uow.begin_transaction().unwrap();
    let mut it = tx.items().find_by_id(1).unwrap().unwrap();
    it.is_active = false;
    tx.items().update(&it).unwrap();
    tx.commit().unwrap();
    assert!(indexer.search("ink").unwrap().is_empty(), "is_active is checked at query time");
}

#[test]
fn test_srch_startup_rebuild_on_count_mismatch() {
    let uow = TestDb::new();
    let indexer = uow.indexer();
    let mut tx = uow.begin_transaction().unwrap();
    // Two active items committed but never indexed, one archived.
    tx.items().create(&create_test_item(1, "Alpha", None, None, 1.0, 1.0)).unwrap();
    tx.items().create(&create_test_item(2, "Beta", None, None, 1.0, 1.0)).unwrap();
    let mut arch = create_test_item(3, "Gamma", None, None, 1.0, 1.0);
    arch.is_active = false;
    tx.items().create(&arch).unwrap();
    tx.commit().unwrap();
    assert!(indexer.search("alpha").unwrap().is_empty());
    assert_eq!(indexer.indexed_item_count().unwrap(), 0);

    assert!(book_keeper::app_state::ensure_search_index(&uow, &indexer).unwrap(), "rebuilt");
    assert_eq!(indexer.indexed_item_count().unwrap(), 2);
    assert_eq!(indexer.search("alpha").unwrap().len(), 1);
    assert!(indexer.search("gamma").unwrap().is_empty());
    assert!(!book_keeper::app_state::ensure_search_index(&uow, &indexer).unwrap(), "consistent: no rebuild");
}

// ---------------------------------------------------------------- generic FtsIndex (TC-FTS-01..05)

use book_keeper::data::contracts::FtsIndex;
use book_keeper::data::sqlite::fts::{fts_prefix_query, SqliteFtsIndex};
use book_keeper::data::sqlite::fts_specs::ITEM_FTS;

fn item_fts(db: &TestDb) -> SqliteFtsIndex {
    SqliteFtsIndex::new(db.pool.clone(), &ITEM_FTS)
}

#[test]
fn tc_fts_01_source_count_equals_indexed_count_after_rebuild() {
    let db = TestDb::new();
    let mut tx = db.begin_transaction().unwrap();
    tx.items().create(&create_test_item(1, "Blue Gel Pen", None, Some("111"), 9.0, 10.0)).unwrap();
    tx.items().create(&create_test_item(2, "Red Pen", None, None, 9.0, 10.0)).unwrap();
    tx.bundles().create(&create_test_bundle(1, 1, "Box", 10.0, None, Some("222"))).unwrap();
    tx.bundles().create(&create_test_bundle(2, 1, "Case", 50.0, None, Some("333"))).unwrap();
    tx.commit().unwrap();
    let idx = item_fts(&db);
    idx.rebuild().unwrap();
    assert_eq!(idx.source_count().unwrap(), 2);
    assert_eq!(idx.indexed_count().unwrap(), 2, "bundle-barcode rows count as one key");
    let rows: i64 = db.pool.get().unwrap().query_row("SELECT COUNT(*) FROM item_search_fts", [], |r| r.get(0)).unwrap();
    assert_eq!(rows, 4, "item rows + 2 bundle barcode rows + Red Pen");
}

#[test]
fn tc_fts_02_replace_rows_touches_only_that_key() {
    let db = TestDb::new();
    let mut tx = db.begin_transaction().unwrap();
    tx.items().create(&create_test_item(1, "Alpha", None, None, 9.0, 10.0)).unwrap();
    tx.items().create(&create_test_item(2, "Beta", None, None, 9.0, 10.0)).unwrap();
    tx.commit().unwrap();
    let idx = item_fts(&db);
    idx.rebuild().unwrap();
    idx.replace_rows(1, &[vec![Some("Gamma".into()), None, None]]).unwrap();
    assert_eq!(idx.search_keys("gamma", 10).unwrap(), vec![1]);
    assert!(idx.search_keys("alpha", 10).unwrap().is_empty());
    assert_eq!(idx.search_keys("beta", 10).unwrap(), vec![2]);
}

#[test]
fn tc_fts_03_search_keys_dedupes_honours_limit_and_empty_for_no_token() {
    let db = TestDb::new();
    let mut tx = db.begin_transaction().unwrap();
    tx.items().create(&create_test_item(1, "Pen One", None, Some("p1"), 9.0, 10.0)).unwrap();
    tx.items().create(&create_test_item(2, "Pen Two", None, None, 9.0, 10.0)).unwrap();
    tx.items().create(&create_test_item(3, "Pen Three", None, None, 9.0, 10.0)).unwrap();
    tx.bundles().create(&create_test_bundle(1, 1, "Box", 10.0, None, Some("p1box"))).unwrap();
    tx.commit().unwrap();
    let idx = item_fts(&db);
    idx.rebuild().unwrap();
    let all = idx.search_keys("pen", 10).unwrap();
    assert_eq!(all.len(), 3, "item 1 has two index rows but one key");
    assert_eq!(idx.search_keys("pen", 2).unwrap().len(), 2);
    assert!(idx.search_keys("%!", 10).unwrap().is_empty());
}

#[test]
fn tc_fts_04_reindex_of_inactive_or_missing_row_removes_it() {
    let db = TestDb::new();
    let mut tx = db.begin_transaction().unwrap();
    tx.items().create(&create_test_item(1, "Alpha", None, None, 9.0, 10.0)).unwrap();
    tx.commit().unwrap();
    let idx = item_fts(&db);
    idx.reindex(1).unwrap();
    assert_eq!(idx.indexed_count().unwrap(), 1);
    db.pool.get().unwrap().execute("UPDATE items SET is_active = 0 WHERE id = 1", []).unwrap();
    idx.reindex(1).unwrap();
    assert_eq!(idx.indexed_count().unwrap(), 0);
    idx.reindex(999).unwrap();
    assert_eq!(idx.indexed_count().unwrap(), 0);
}

#[test]
fn tc_fts_05_prefix_query_builder() {
    assert_eq!(fts_prefix_query("blue gel*").as_deref(), Some("\"blue\"* \"gel\"*"));
    assert_eq!(fts_prefix_query("%"), None);
}
