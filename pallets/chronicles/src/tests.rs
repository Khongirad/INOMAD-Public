//! Unit tests for pallet-chronicles.
//!
//! Covers:
//! - `publish_document`: happy path, duplicate hash guard, CID too long
//! - `donate_to_author`: happy path, document not found, balance transfer
//! - Anti-plagiarism invariant

use crate::mock::*;
use crate::pallet::DocumentCategory;
use frame_support::{assert_noop, assert_ok};
use sp_core::H256;

// ── Helpers ───────────────────────────────────────────────────────────────

fn hash(seed: u8) -> H256 {
    H256::repeat_byte(seed)
}

fn cid(s: &str) -> Vec<u8> {
    s.as_bytes().to_vec()
}

// ── 1. publish_document ───────────────────────────────────────────────────

#[test]
fn publish_document_happy_path() {
    new_test_ext().execute_with(|| {
        System::set_block_number(1);

        assert_ok!(Chronicles::publish_document(
            RuntimeOrigin::signed(ALICE),
            hash(1),
            cid("QmAlicePaper"),
            DocumentCategory::Science,
        ));

        let record =
            crate::pallet::Documents::<Test>::get(hash(1)).expect("document should be stored");
        assert_eq!(record.owner, ALICE);
        assert_eq!(record.category, DocumentCategory::Science);
        assert_eq!(record.content_hash, hash(1));
    });
}

#[test]
fn publish_document_stores_multiple_unique_hashes() {
    new_test_ext().execute_with(|| {
        System::set_block_number(1);

        assert_ok!(Chronicles::publish_document(
            RuntimeOrigin::signed(ALICE),
            hash(1),
            cid("QmDoc1"),
            DocumentCategory::History,
        ));
        assert_ok!(Chronicles::publish_document(
            RuntimeOrigin::signed(BOB),
            hash(2),
            cid("QmDoc2"),
            DocumentCategory::Law,
        ));

        assert!(crate::pallet::Documents::<Test>::contains_key(hash(1)));
        assert!(crate::pallet::Documents::<Test>::contains_key(hash(2)));
    });
}

#[test]
fn publish_document_rejects_duplicate_hash() {
    new_test_ext().execute_with(|| {
        System::set_block_number(1);

        assert_ok!(Chronicles::publish_document(
            RuntimeOrigin::signed(ALICE),
            hash(1),
            cid("QmAlice"),
            DocumentCategory::Science,
        ));

        // Bob tries to re-register the same content hash — anti-plagiarism guard
        assert_noop!(
            Chronicles::publish_document(
                RuntimeOrigin::signed(BOB),
                hash(1),
                cid("QmBob"),
                DocumentCategory::Media,
            ),
            crate::pallet::Error::<Test>::DocumentAlreadyExists
        );
    });
}

#[test]
fn publish_document_rejects_cid_too_long() {
    new_test_ext().execute_with(|| {
        // 65 bytes — exceeds MaxCidLength=64
        let long_cid = vec![b'Q'; 65];

        assert_noop!(
            Chronicles::publish_document(
                RuntimeOrigin::signed(ALICE),
                hash(42),
                long_cid,
                DocumentCategory::Science,
            ),
            crate::pallet::Error::<Test>::CidTooLong
        );
    });
}

#[test]
fn publish_document_maximum_valid_cid_length() {
    new_test_ext().execute_with(|| {
        System::set_block_number(1);
        // Exactly 64 bytes — should succeed
        let exact_cid = vec![b'Q'; 64];

        assert_ok!(Chronicles::publish_document(
            RuntimeOrigin::signed(ALICE),
            hash(7),
            exact_cid,
            DocumentCategory::Law,
        ));
    });
}

#[test]
fn publish_document_author_index_updated() {
    new_test_ext().execute_with(|| {
        System::set_block_number(1);

        assert_ok!(Chronicles::publish_document(
            RuntimeOrigin::signed(ALICE),
            hash(10),
            cid("QmAlice10"),
            DocumentCategory::History,
        ));

        // AuthorDocuments double-map should have (ALICE, hash(10)) entry
        assert!(crate::pallet::AuthorDocuments::<Test>::contains_key(
            ALICE,
            hash(10)
        ));
    });
}

// ── 2. donate_to_author ───────────────────────────────────────────────────

#[test]
fn donate_to_author_transfers_balance() {
    new_test_ext().execute_with(|| {
        System::set_block_number(1);

        let donation = 100 * UNIT;

        assert_ok!(Chronicles::publish_document(
            RuntimeOrigin::signed(ALICE),
            hash(20),
            cid("QmAlice20"),
            DocumentCategory::Science,
        ));

        let alice_before = Balances::free_balance(ALICE);
        let bob_before = Balances::free_balance(BOB);

        assert_ok!(Chronicles::donate_to_author(
            RuntimeOrigin::signed(BOB),
            hash(20),
            donation,
        ));

        assert_eq!(Balances::free_balance(ALICE), alice_before + donation);
        assert_eq!(Balances::free_balance(BOB), bob_before - donation);
    });
}

#[test]
fn donate_to_author_fails_document_not_found() {
    new_test_ext().execute_with(|| {
        assert_noop!(
            Chronicles::donate_to_author(
                RuntimeOrigin::signed(BOB),
                hash(99), // non-existent hash
                10 * UNIT,
            ),
            crate::pallet::Error::<Test>::DocumentNotFound
        );
    });
}

#[test]
fn donate_to_author_allows_self_donation() {
    new_test_ext().execute_with(|| {
        System::set_block_number(1);

        assert_ok!(Chronicles::publish_document(
            RuntimeOrigin::signed(ALICE),
            hash(30),
            cid("QmSelf"),
            DocumentCategory::Media,
        ));

        // Alice donates to herself — should succeed (no rule prevents it)
        assert_ok!(Chronicles::donate_to_author(
            RuntimeOrigin::signed(ALICE),
            hash(30),
            1 * UNIT,
        ));
    });
}
