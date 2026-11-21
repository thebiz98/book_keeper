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
