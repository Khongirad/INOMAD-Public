//! Unit tests for pallet-black-book.
//!
//! Covers:
//!  1. condemn_and_issue_warrant: confiscation, WallOfShame entry, bounty seeding
//!  2. [VECTOR 3] Anti-Tyranny Guard: Academician & Delegate protection
//!  3. donate_to_bounty: donor mechanics, AtLarge check, Captured guard
//!  4. [VECTOR 1] register_capture_and_payout: lock period, duplicate lock prevention
//!  5. claim_bounty_payout: time-lock enforcement, successful payout
//!  6. cancel_bounty_payout: Root veto of suspicious payout
//!  7. Edge: condemn without fugitive flag (no bounty pool)

#![cfg(test)]

use crate::mock::*;
use crate::pallet::{CrimeCategory, Error, FugitiveStatus};
use frame_support::{assert_noop, assert_ok, traits::Currency};
use sp_core::H256;

fn free_bal(who: AccountId) -> Balance {
    <pallet_balances::Pallet<Test> as Currency<AccountId>>::free_balance(&who)
}

fn verdict() -> H256 {
    H256::repeat_byte(0xAB)
}

// ─── condemn_and_issue_warrant ────────────────────────────────────────────

#[test]
fn condemn_exiles_citizen_and_confiscates_balance() {
    new_test_ext().execute_with(|| {
        let alice_before = free_bal(ALICE);
        let treasury_before = free_bal(ROOT_TREASURY);

        assert_ok!(BlackBook::condemn_and_issue_warrant(
            RuntimeOrigin::root(),
            ALICE,
            CrimeCategory::Corruption,
            verdict(),
            false, // not a fugitive
            0,     // no bounty
        ));

        assert!(is_exiled(ALICE));
        let record = BlackBook::wall_of_shame(ALICE).expect("must be in WallOfShame");
        assert_eq!(record.category, CrimeCategory::Corruption);
        assert!(record.fugitive_status.is_none());

        // ALICE's balance confiscated to treasury
        assert_eq!(free_bal(ALICE), 0);
        assert_eq!(free_bal(ROOT_TREASURY), treasury_before + alice_before);

        clear_mocks();
    });
}

#[test]
fn condemn_as_fugitive_seeds_bounty_pool() {
    new_test_ext().execute_with(|| {
        let initial_bounty = 100 * UNIT;
        let treasury_before = free_bal(ROOT_TREASURY);

        assert_ok!(BlackBook::condemn_and_issue_warrant(
            RuntimeOrigin::root(),
            ALICE,
            CrimeCategory::HighTreason,
            verdict(),
            true, // fugitive = AtLarge
            initial_bounty,
        ));

        let record = BlackBook::wall_of_shame(ALICE).unwrap();
        assert_eq!(record.fugitive_status, Some(FugitiveStatus::AtLarge));

        let pool = BlackBook::bounty_pools(ALICE).expect("pool must exist");
        assert_eq!(pool, initial_bounty);

        // Treasury paid for bounty (ALICE's confiscated balance was also added,
        // but Alice's own balance went to treasury first, then initial_bounty came back)
        // Net: treasury_before - initial_bounty + alice_confiscated
        assert_eq!(
            free_bal(ROOT_TREASURY),
            treasury_before - initial_bounty + 10_000 * UNIT,
        );

        clear_mocks();
    });
}

#[test]
fn condemn_requires_root_origin() {
    new_test_ext().execute_with(|| {
        assert_noop!(
            BlackBook::condemn_and_issue_warrant(
                RuntimeOrigin::signed(ALICE),
                ALICE,
                CrimeCategory::GrandTheft,
                verdict(),
                false,
                0,
            ),
            sp_runtime::DispatchError::BadOrigin,
        );
    });
}

// ─── [VECTOR 3] Anti-Tyranny Guard ────────────────────────────────────────

#[test]
fn academician_cannot_be_condemned_directly() {
    new_test_ext().execute_with(|| {
        register_academician(ACADEMICIAN);

        assert_noop!(
            BlackBook::condemn_and_issue_warrant(
                RuntimeOrigin::root(),
                ACADEMICIAN,
                CrimeCategory::Corruption,
                verdict(),
                false,
                0,
            ),
            Error::<Test>::RequiresImpeachment,
        );

        // Academician NOT exiled
        assert!(!is_exiled(ACADEMICIAN));
        assert!(BlackBook::wall_of_shame(ACADEMICIAN).is_none());

        clear_mocks();
    });
}

#[test]
fn khural_delegate_cannot_be_condemned_directly() {
    new_test_ext().execute_with(|| {
        register_delegate(DELEGATE);

        assert_noop!(
            BlackBook::condemn_and_issue_warrant(
                RuntimeOrigin::root(),
                DELEGATE,
                CrimeCategory::HighTreason,
                verdict(),
                false,
                0,
            ),
            Error::<Test>::RequiresImpeachment,
        );

        assert!(!is_exiled(DELEGATE));

        clear_mocks();
    });
}

// ─── donate_to_bounty ───────────────────────────────────────────────────

#[test]
fn citizen_can_donate_to_at_large_bounty() {
    new_test_ext().execute_with(|| {
        // Setup fugitive
        assert_ok!(BlackBook::condemn_and_issue_warrant(
            RuntimeOrigin::root(),
            ALICE,
            CrimeCategory::Corruption,
            verdict(),
            true,
            0,
        ));

        let donor_before = free_bal(DONOR);
        let donation = 50 * UNIT;

        assert_ok!(BlackBook::donate_to_bounty(
            RuntimeOrigin::signed(DONOR),
            ALICE,
            donation,
        ));

        assert_eq!(free_bal(DONOR), donor_before - donation);
        let pool = BlackBook::bounty_pools(ALICE).unwrap();
        assert_eq!(pool, donation);

        clear_mocks();
    });
}

#[test]
fn cannot_donate_to_non_fugitive_criminal() {
    new_test_ext().execute_with(|| {
        assert_ok!(BlackBook::condemn_and_issue_warrant(
            RuntimeOrigin::root(),
            ALICE,
            CrimeCategory::Corruption,
            verdict(),
            false,
            0, // not fugitive
        ));

        assert_noop!(
            BlackBook::donate_to_bounty(RuntimeOrigin::signed(DONOR), ALICE, 10 * UNIT),
            Error::<Test>::NotAtLarge,
        );

        clear_mocks();
    });
}

#[test]
fn cannot_donate_to_unknown_criminal() {
    new_test_ext().execute_with(|| {
        assert_noop!(
            BlackBook::donate_to_bounty(RuntimeOrigin::signed(DONOR), ALICE, 10 * UNIT),
            Error::<Test>::TargetNotCondemned,
        );
    });
}

// ─── [VECTOR 1] register_capture_and_payout ──────────────────────────────

#[test]
fn capture_locks_bounty_for_vesting_period() {
    new_test_ext().execute_with(|| {
        let bounty = 200 * UNIT;
        assert_ok!(BlackBook::condemn_and_issue_warrant(
            RuntimeOrigin::root(),
            ALICE,
            CrimeCategory::HighTreason,
            verdict(),
            true,
            bounty,
        ));

        // Register capture → should lock bounty
        assert_ok!(BlackBook::register_capture_and_payout(
            RuntimeOrigin::root(),
            ALICE,
            HUNTER,
        ));

        // WallOfShame updated to Captured
        let record = BlackBook::wall_of_shame(ALICE).unwrap();
        assert_eq!(record.fugitive_status, Some(FugitiveStatus::Captured));

        // BountyPool cleared
        assert!(BlackBook::bounty_pools(ALICE).is_none());

        // LockedPayout created with correct unlock block
        let locked = BlackBook::locked_bounty_payouts(ALICE).expect("must have locked payout");
        assert_eq!(locked.bounty_hunter, HUNTER);
        assert_eq!(locked.amount, bounty);
        assert_eq!(locked.unlock_block, 1 + 10); // block 1 + BountyLockPeriod=10

        clear_mocks();
    });
}

#[test]
fn cannot_capture_non_fugitive() {
    new_test_ext().execute_with(|| {
        assert_ok!(BlackBook::condemn_and_issue_warrant(
            RuntimeOrigin::root(),
            ALICE,
            CrimeCategory::Corruption,
            verdict(),
            false,
            0,
        ));
        assert_noop!(
            BlackBook::register_capture_and_payout(RuntimeOrigin::root(), ALICE, HUNTER),
            Error::<Test>::NotAtLarge,
        );
        clear_mocks();
    });
}

// ─── claim_bounty_payout ─────────────────────────────────────────────────

#[test]
fn bounty_hunter_claims_payout_after_lock_period() {
    new_test_ext().execute_with(|| {
        let bounty = 300 * UNIT;
        assert_ok!(BlackBook::condemn_and_issue_warrant(
            RuntimeOrigin::root(),
            ALICE,
            CrimeCategory::HighTreason,
            verdict(),
            true,
            bounty,
        ));
        assert_ok!(BlackBook::register_capture_and_payout(
            RuntimeOrigin::root(),
            ALICE,
            HUNTER,
        ));

        // Still locked — try to claim before lock expires
        assert_noop!(
            BlackBook::claim_bounty_payout(RuntimeOrigin::signed(HUNTER), ALICE),
            Error::<Test>::BountyPayoutStillLocked,
        );

        // Advance 10 blocks → unlock_block = 11
        frame_system::Pallet::<Test>::set_block_number(11);

        let hunter_before = free_bal(HUNTER);
        assert_ok!(BlackBook::claim_bounty_payout(
            RuntimeOrigin::signed(HUNTER),
            ALICE
        ));

        assert_eq!(free_bal(HUNTER), hunter_before + bounty);
        assert!(BlackBook::locked_bounty_payouts(ALICE).is_none());

        clear_mocks();
    });
}

// ─── cancel_bounty_payout ────────────────────────────────────────────────

#[test]
fn root_can_cancel_suspicious_payout() {
    new_test_ext().execute_with(|| {
        let bounty = 100 * UNIT;
        assert_ok!(BlackBook::condemn_and_issue_warrant(
            RuntimeOrigin::root(),
            ALICE,
            CrimeCategory::HighTreason,
            verdict(),
            true,
            bounty,
        ));
        assert_ok!(BlackBook::register_capture_and_payout(
            RuntimeOrigin::root(),
            ALICE,
            HUNTER,
        ));

        // Root discovers collusion and cancels
        assert_ok!(BlackBook::cancel_bounty_payout(
            RuntimeOrigin::root(),
            ALICE
        ));
        assert!(BlackBook::locked_bounty_payouts(ALICE).is_none());

        // Hunter cannot claim after cancellation
        assert_noop!(
            BlackBook::claim_bounty_payout(RuntimeOrigin::signed(HUNTER), ALICE),
            Error::<Test>::BountyPayoutNotFound,
        );

        clear_mocks();
    });
}
