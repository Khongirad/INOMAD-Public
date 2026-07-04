//! Unit tests for pallet-steppe-offline.
//!
//! Validates:
//! - `lock_funds`: reserves balance, updates shadow ledger
//! - `settle_iou` (honest path): transfers from reserved, updates pocket
//! - `settle_iou` (ARMAGEDDON): amount > pocket → drain + slash + freeze
//! - Replay prevention: same (sender, nonce) rejected
//! - Zero-pocket ARMAGEDDON: sender with nothing locked
//! - Total issuance invariant: supply unchanged throughout all paths

use super::*;
use crate::mock::*;
use frame_support::{assert_noop, assert_ok};

// =========================================================================
// Helpers
// =========================================================================

/// Lock `amount` ALTAN from ALICE into her offline pocket.
fn lock(amount: Balance) {
    assert_ok!(Steppe::lock_funds(RuntimeOrigin::signed(ALICE), amount));
}

/// Settle an IOU where ALICE → BOB. Uses MockSignature (always valid).
fn settle(amount: Balance, nonce: u64) -> frame_support::dispatch::DispatchResult {
    Steppe::settle_iou(
        RuntimeOrigin::signed(BOB), // receiver/merchant brings it on-chain
        ALICE,
        iou(amount, nonce),
        MockSignature,
    )
}

// =========================================================================
// 1. lock_funds
// =========================================================================

#[test]
fn lock_funds_moves_free_to_reserved() {
    new_test_ext().execute_with(|| {
        let free_before = Balances::free_balance(ALICE);
        lock(100 * UNIT);
        assert_eq!(Balances::free_balance(ALICE), free_before - 100 * UNIT);
        assert_eq!(Balances::reserved_balance(ALICE), 100 * UNIT);
    });
}

#[test]
fn lock_funds_credits_shadow_ledger() {
    new_test_ext().execute_with(|| {
        lock(200 * UNIT);
        assert_eq!(Steppe::offline_pockets(ALICE), 200 * UNIT);
    });
}

#[test]
fn lock_funds_does_not_change_total_issuance() {
    new_test_ext().execute_with(|| {
        let issuance_before = Balances::total_issuance();
        lock(500 * UNIT);
        assert_eq!(Balances::total_issuance(), issuance_before);
    });
}

#[test]
fn lock_funds_emits_pocket_funded_event() {
    new_test_ext().execute_with(|| {
        assert_ok!(Steppe::lock_funds(RuntimeOrigin::signed(ALICE), 50 * UNIT));
        System::assert_last_event(
            Event::PocketFunded {
                who: ALICE,
                amount: 50 * UNIT,
            }
            .into(),
        );
    });
}

#[test]
fn lock_funds_insufficient_balance_fails() {
    new_test_ext().execute_with(|| {
        // Alice has 1000 UNIT — try locking more than free balance
        assert_noop!(
            Steppe::lock_funds(RuntimeOrigin::signed(ALICE), 2_000 * UNIT),
            Error::<Test>::InsufficientFreeBalance,
        );
    });
}

#[test]
fn lock_funds_can_be_accumulated_multiple_times() {
    new_test_ext().execute_with(|| {
        lock(100 * UNIT);
        lock(50 * UNIT);
        assert_eq!(Steppe::offline_pockets(ALICE), 150 * UNIT);
        assert_eq!(Balances::reserved_balance(ALICE), 150 * UNIT);
    });
}

// =========================================================================
// 2. settle_iou — Honest path
// =========================================================================

#[test]
fn settle_iou_honest_transfers_to_receiver() {
    new_test_ext().execute_with(|| {
        lock(100 * UNIT);
        let bob_before = Balances::free_balance(BOB);

        assert_ok!(settle(100 * UNIT, 0));

        // BOB received 100 UNIT
        assert_eq!(Balances::free_balance(BOB), bob_before + 100 * UNIT);
        // ALICE has 0 reserved
        assert_eq!(Balances::reserved_balance(ALICE), 0);
        // Shadow ledger cleared
        assert_eq!(Steppe::offline_pockets(ALICE), 0);
    });
}

#[test]
fn settle_iou_honest_does_not_change_total_issuance() {
    new_test_ext().execute_with(|| {
        lock(100 * UNIT);
        let issuance_before = Balances::total_issuance();
        assert_ok!(settle(100 * UNIT, 0));
        assert_eq!(Balances::total_issuance(), issuance_before);
    });
}

#[test]
fn settle_iou_partial_amount_deducts_correct_pocket() {
    new_test_ext().execute_with(|| {
        lock(200 * UNIT);
        assert_ok!(settle(80 * UNIT, 0));

        // 120 UNIT remain in pocket
        assert_eq!(Steppe::offline_pockets(ALICE), 120 * UNIT);
        assert_eq!(Balances::reserved_balance(ALICE), 120 * UNIT);
    });
}

#[test]
fn settle_iou_emits_iou_settled_event() {
    new_test_ext().execute_with(|| {
        lock(100 * UNIT);
        assert_ok!(settle(100 * UNIT, 42));
        System::assert_last_event(
            Event::IouSettled {
                sender: ALICE,
                receiver: BOB,
                amount: 100 * UNIT,
                nonce: 42,
            }
            .into(),
        );
    });
}

// =========================================================================
// 3. Replay prevention
// =========================================================================

#[test]
fn settle_iou_replay_with_same_nonce_fails() {
    new_test_ext().execute_with(|| {
        lock(200 * UNIT);
        assert_ok!(settle(50 * UNIT, 0));
        // Second settle — same (alice, nonce=0) → rejected
        assert_noop!(settle(50 * UNIT, 0), Error::<Test>::IouAlreadyProcessed);
    });
}

#[test]
fn settle_iou_different_nonces_both_succeed() {
    new_test_ext().execute_with(|| {
        lock(200 * UNIT);
        assert_ok!(settle(50 * UNIT, 0));
        assert_ok!(settle(50 * UNIT, 1));
        assert_eq!(Steppe::offline_pockets(ALICE), 100 * UNIT);
    });
}

#[test]
fn processed_ious_flag_set_after_settlement() {
    new_test_ext().execute_with(|| {
        lock(100 * UNIT);
        assert!(!Steppe::processed_ious(ALICE, 7u64));
        assert_ok!(settle(50 * UNIT, 7));
        assert!(Steppe::processed_ious(ALICE, 7u64));
    });
}

// =========================================================================
// 4. ARMAGEDDON — double-spend fraud
// =========================================================================

#[test]
fn armageddon_drains_pocket_to_receiver() {
    new_test_ext().execute_with(|| {
        // Alice locks 100 UNIT but signs an IOU for 500 UNIT (fraud!)
        lock(100 * UNIT);
        let bob_before = Balances::free_balance(BOB);

        assert_ok!(settle(500 * UNIT, 0)); // amount > pocket → ARMAGEDDON

        // BOB gets whatever Alice had (100 UNIT)
        assert_eq!(Balances::free_balance(BOB), bob_before + 100 * UNIT);
        // ALICE pocket zeroed
        assert_eq!(Steppe::offline_pockets(ALICE), 0);
        assert_eq!(Balances::reserved_balance(ALICE), 0);
    });
}

#[test]
fn armageddon_calls_slash_interface() {
    new_test_ext().execute_with(|| {
        clear_slashed();
        lock(100 * UNIT);
        assert_ok!(settle(999 * UNIT, 0)); // FRAUD → ARMAGEDDON

        // MockSlash should have recorded ALICE
        assert!(was_slashed(ALICE));
    });
}

#[test]
fn armageddon_emits_armageddon_triggered_event() {
    new_test_ext().execute_with(|| {
        lock(100 * UNIT);
        assert_ok!(settle(600 * UNIT, 0));

        let deficit = 600 * UNIT - 100 * UNIT; // 500 UNIT
        System::assert_last_event(
            Event::ArmageddonTriggered {
                sinner: ALICE,
                deficit,
            }
            .into(),
        );
    });
}

#[test]
fn armageddon_does_not_change_total_issuance() {
    new_test_ext().execute_with(|| {
        lock(100 * UNIT);
        let issuance_before = Balances::total_issuance();
        assert_ok!(settle(999 * UNIT, 0));
        // Supply unchanged — no mint/burn occurred
        assert_eq!(Balances::total_issuance(), issuance_before);
    });
}

#[test]
fn armageddon_with_empty_pocket_gives_zero_to_receiver() {
    new_test_ext().execute_with(|| {
        // Alice has NOT locked anything — pocket is 0
        let bob_before = Balances::free_balance(BOB);
        // Fraud: signing 100 UNIT with 0 pocket
        assert_ok!(settle(100 * UNIT, 0));

        // BOB receives nothing (pocket was 0)
        assert_eq!(Balances::free_balance(BOB), bob_before);
        // ALICE still slashed (mathematical fraud is still proven)
        assert!(was_slashed(ALICE));
    });
}

#[test]
fn armageddon_nonce_still_marked_processed() {
    new_test_ext().execute_with(|| {
        lock(50 * UNIT);
        assert_ok!(settle(999 * UNIT, 3)); // ARMAGEDDON via nonce 3
                                           // Nonce 3 blocked — even fraud is non-replayable
        assert_noop!(settle(50 * UNIT, 3), Error::<Test>::IouAlreadyProcessed);
    });
}

// =========================================================================
// 5. Economic invariants
// =========================================================================

#[test]
fn full_cycle_total_supply_unchanged() {
    new_test_ext().execute_with(|| {
        let supply = Balances::total_issuance();

        // Lock → settle → lock again → ARMAGEDDON
        lock(300 * UNIT);
        assert_ok!(settle(100 * UNIT, 0)); // honest
        lock(200 * UNIT);
        assert_ok!(settle(999 * UNIT, 1)); // armageddon

        assert_eq!(Balances::total_issuance(), supply);
    });
}
