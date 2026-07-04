//! Unit tests for pallet-shielded-vaults.
//!
//! Validates:
//! - TransparentStateGuard: state accounts CANNOT enter shielded pool
//! - Citizens CAN shield/unshield freely
//! - Commitment lifecycle: shield → active, spend → false
//! - Nullifier double-spend protection
//! - `unshield_to_account`: materializes balance + spent nullifier
//! - `org_unshield_tax_payment`: 7/10 regional + 3/10 confederation split
//! - `shielded_transfer`: spends input nullifier, creates new commitment

use super::*;
use crate::mock::*;
use frame_support::{assert_noop, assert_ok};

// No ZkTaxProof arg — org_unshield_tax_payment args: org_id, nullifier, commitment, amount_claimed

// =========================================================================
// 1. TransparentStateGuard — constitutional core
// =========================================================================

#[test]
fn state_account_cannot_shield_funds() {
    new_test_ext().execute_with(|| {
        // CENTRAL_BANK is in state account blocklist
        assert_noop!(
            Vaults::shield_funds(
                RuntimeOrigin::signed(CENTRAL_BANK),
                100 * UNIT, // amount FIRST
                commitment(1),
                None,
            ),
            Error::<Test>::StateFundsMustRemainPublic,
        );
    });
}

#[test]
fn state_account_balance_unchanged_after_blocked_shield() {
    new_test_ext().execute_with(|| {
        let balance_before = Balances::free_balance(CENTRAL_BANK);
        let _ = Vaults::shield_funds(
            RuntimeOrigin::signed(CENTRAL_BANK),
            100 * UNIT,
            commitment(1),
            None,
        );
        assert_eq!(Balances::free_balance(CENTRAL_BANK), balance_before);
    });
}

#[test]
fn citizen_can_shield_funds() {
    new_test_ext().execute_with(|| {
        assert_ok!(Vaults::shield_funds(
            RuntimeOrigin::signed(ALICE),
            100 * UNIT,
            commitment(1),
            None,
        ));
    });
}

#[test]
fn shield_funds_deducts_from_public_balance() {
    new_test_ext().execute_with(|| {
        let before = Balances::free_balance(ALICE);
        assert_ok!(Vaults::shield_funds(
            RuntimeOrigin::signed(ALICE),
            100 * UNIT,
            commitment(1),
            None,
        ));
        assert_eq!(Balances::free_balance(ALICE), before - 100 * UNIT);
    });
}

#[test]
fn shield_funds_registers_active_commitment() {
    new_test_ext().execute_with(|| {
        assert_ok!(Vaults::shield_funds(
            RuntimeOrigin::signed(ALICE),
            100 * UNIT,
            commitment(1),
            None,
        ));
        assert_eq!(Vaults::shielded_commitments(commitment(1)), true);
    });
}

#[test]
fn shield_funds_duplicate_commitment_fails() {
    new_test_ext().execute_with(|| {
        assert_ok!(Vaults::shield_funds(
            RuntimeOrigin::signed(ALICE),
            100 * UNIT,
            commitment(1),
            None,
        ));
        assert_noop!(
            Vaults::shield_funds(
                RuntimeOrigin::signed(BOB),
                50 * UNIT,
                commitment(1), // same commitment hash
                None,
            ),
            Error::<Test>::CommitmentAlreadySpent,
        );
    });
}

#[test]
fn any_additional_state_account_blocked() {
    new_test_ext().execute_with(|| {
        block_state_account(BOB);
        assert_noop!(
            Vaults::shield_funds(RuntimeOrigin::signed(BOB), 10 * UNIT, commitment(5), None,),
            Error::<Test>::StateFundsMustRemainPublic,
        );
        unblock_state_account(BOB);
    });
}

// =========================================================================
// 2. Nullifier double-spend protection
// =========================================================================

#[test]
fn unshield_marks_nullifier_as_spent() {
    new_test_ext().execute_with(|| {
        assert_ok!(Vaults::shield_funds(
            RuntimeOrigin::signed(ALICE),
            200 * UNIT,
            commitment(2),
            None,
        ));
        assert_ok!(Vaults::unshield_to_account(
            RuntimeOrigin::signed(ALICE),
            nullifier(2),
            commitment(2),
            100 * UNIT,
            ALICE,
            None,
        ));
        assert!(Vaults::spent_nullifiers(nullifier(2)).is_some());
    });
}

#[test]
fn double_spend_nullifier_rejected() {
    new_test_ext().execute_with(|| {
        assert_ok!(Vaults::shield_funds(
            RuntimeOrigin::signed(ALICE),
            200 * UNIT,
            commitment(3),
            None,
        ));
        assert_ok!(Vaults::unshield_to_account(
            RuntimeOrigin::signed(ALICE),
            nullifier(3),
            commitment(3),
            100 * UNIT,
            ALICE,
            None,
        ));
        // Second unshield — same nullifier
        assert_noop!(
            Vaults::unshield_to_account(
                RuntimeOrigin::signed(ALICE),
                nullifier(3),
                commitment(3),
                100 * UNIT,
                ALICE,
                None,
            ),
            Error::<Test>::NullifierAlreadySpent,
        );
    });
}

#[test]
fn shielded_transfer_double_spend_nullifier_rejected() {
    new_test_ext().execute_with(|| {
        assert_ok!(Vaults::shield_funds(
            RuntimeOrigin::signed(ALICE),
            200 * UNIT,
            commitment(4),
            None,
        ));
        assert_ok!(Vaults::shielded_transfer(
            RuntimeOrigin::signed(ALICE),
            nullifier(4),
            commitment(4),
            commitment(9),
            None,
        ));
        assert_noop!(
            Vaults::shielded_transfer(
                RuntimeOrigin::signed(ALICE),
                nullifier(4),
                commitment(4),
                commitment(10),
                None,
            ),
            Error::<Test>::NullifierAlreadySpent,
        );
    });
}

// =========================================================================
// 3. unshield_to_account — materializes public balance
// =========================================================================

#[test]
fn unshield_to_account_restores_recipient_balance() {
    new_test_ext().execute_with(|| {
        assert_ok!(Vaults::shield_funds(
            RuntimeOrigin::signed(ALICE),
            200 * UNIT,
            commitment(5),
            None,
        ));
        let bob_before = Balances::free_balance(BOB);
        assert_ok!(Vaults::unshield_to_account(
            RuntimeOrigin::signed(ALICE),
            nullifier(5),
            commitment(5),
            150 * UNIT,
            BOB,
            None,
        ));
        assert_eq!(Balances::free_balance(BOB), bob_before + 150 * UNIT);
    });
}

#[test]
fn unshield_exceeds_shielded_balance_fails() {
    new_test_ext().execute_with(|| {
        // Shield 700 UNIT (leave 300 free to stay above ED)
        assert_ok!(Vaults::shield_funds(
            RuntimeOrigin::signed(ALICE),
            100 * UNIT, // lock only 100 for ExceedsShieldedBalance check
            commitment(6),
            Some(ORG_ID), // attach to org so vault balance is tracked
        ));
        // Try to unshield more than was shielded from the ORG vault
        assert_noop!(
            Vaults::unshield_to_account(
                RuntimeOrigin::signed(ALICE),
                nullifier(6),
                commitment(6),
                500 * UNIT, // exceeds 100 shielded
                ALICE,
                Some(ORG_ID), // org_id required for balance check
            ),
            Error::<Test>::AmountExceedsShieldedBalance,
        );
    });
}

// =========================================================================
// 4. org_unshield_tax_payment — 7/10 Regional + 3/10 Confederation
// =========================================================================

#[test]
fn tax_unshield_splits_7_10_regional_3_10_confederation() {
    new_test_ext().execute_with(|| {
        // Shield 500 UNIT for this org (leaves 500 free above ED)
        assert_ok!(Vaults::shield_funds(
            RuntimeOrigin::signed(ALICE),
            500 * UNIT,
            commitment(7),
            Some(ORG_ID),
        ));
        let regional_before = Balances::free_balance(REGIONAL_TREASURY);
        let conf_before = Balances::free_balance(CONFEDERATION_TREASURY);

        assert_ok!(Vaults::org_unshield_tax_payment(
            RuntimeOrigin::signed(ALICE),
            ORG_ID,
            nullifier(7),
            commitment(7),
            500 * UNIT,
        ));

        // Constitutional split: 7/10 → regional, 3/10 → confederation
        assert_eq!(
            Balances::free_balance(REGIONAL_TREASURY),
            regional_before + 350 * UNIT, // 7/10 of 500
        );
        assert_eq!(
            Balances::free_balance(CONFEDERATION_TREASURY),
            conf_before + 150 * UNIT, // 3/10 of 500
        );
    });
}

#[test]
fn tax_unshield_double_nullifier_rejected() {
    new_test_ext().execute_with(|| {
        assert_ok!(Vaults::shield_funds(
            RuntimeOrigin::signed(ALICE),
            400 * UNIT, // 400 UNIT shielded
            commitment(8),
            Some(ORG_ID),
        ));
        assert_ok!(Vaults::org_unshield_tax_payment(
            RuntimeOrigin::signed(ALICE),
            ORG_ID,
            nullifier(8),
            commitment(8),
            200 * UNIT,
        ));
        assert_noop!(
            Vaults::org_unshield_tax_payment(
                RuntimeOrigin::signed(ALICE),
                ORG_ID,
                nullifier(8), // same nullifier → REJECTED
                commitment(8),
                200 * UNIT,
            ),
            Error::<Test>::NullifierAlreadySpent,
        );
    });
}

#[test]
fn tax_payment_marks_commitment_as_spent() {
    new_test_ext().execute_with(|| {
        assert_ok!(Vaults::shield_funds(
            RuntimeOrigin::signed(ALICE),
            300 * UNIT,
            commitment(9),
            Some(ORG_ID),
        ));
        assert_ok!(Vaults::org_unshield_tax_payment(
            RuntimeOrigin::signed(ALICE),
            ORG_ID,
            nullifier(9),
            commitment(9),
            300 * UNIT,
        ));
        // After spending, commitment should be false (spent)
        assert_eq!(Vaults::shielded_commitments(commitment(9)), false);
    });
}

// =========================================================================
// 5. shielded_transfer — private in-pool transfer
// =========================================================================

#[test]
fn shielded_transfer_creates_new_commitment() {
    new_test_ext().execute_with(|| {
        assert_ok!(Vaults::shield_funds(
            RuntimeOrigin::signed(ALICE),
            300 * UNIT,
            commitment(10),
            None,
        ));
        let new_comm = commitment(99);
        assert_ok!(Vaults::shielded_transfer(
            RuntimeOrigin::signed(ALICE),
            nullifier(10),
            commitment(10),
            new_comm,
            None,
        ));
        // New commitment is active
        assert_eq!(Vaults::shielded_commitments(new_comm), true);
        // Old commitment is spent
        assert_eq!(Vaults::shielded_commitments(commitment(10)), false);
        // Nullifier marked spent
        assert!(Vaults::spent_nullifiers(nullifier(10)).is_some());
    });
}

#[test]
fn shielded_transfer_commitment_not_found_fails() {
    new_test_ext().execute_with(|| {
        assert_noop!(
            Vaults::shielded_transfer(
                RuntimeOrigin::signed(ALICE),
                nullifier(55),
                commitment(55), // never shielded
                commitment(56),
                None,
            ),
            Error::<Test>::CommitmentNotFound,
        );
    });
}
