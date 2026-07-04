//! Unit tests for pallet-inheritance.
//!
//! Covers:
//!  1. draft_will: basic drafting, share validation, MaxHeirs guard
//!  2. notarize_will: notary-only gate, self-notarization prevention
//!  3. execute_will: heir distribution, dust to last heir, fallback to treasury
//!  4. [SECURITY VECTOR 1] CDP dead-debt liquidation before distribution
//!  5. execute_will when deceased has zero reserved balance
//!  6. Re-draft clears notarization

#![cfg(test)]

use crate::mock::*;
use crate::pallet::Error;
use frame_support::{
    assert_noop, assert_ok,
    traits::{Currency, ReservableCurrency},
};

/// Helper: free balance.
fn free_bal(who: AccountId) -> Balance {
    <pallet_balances::Pallet<Test> as Currency<AccountId>>::free_balance(&who)
}

/// Helper: reserved balance.
#[allow(dead_code)]
fn reserved_bal(who: AccountId) -> Balance {
    <pallet_balances::Pallet<Test> as ReservableCurrency<AccountId>>::reserved_balance(&who)
}

/// Helper: reserve funds for ALICE (simulates `register_death` freezing balance).
fn freeze_alice(amount: Balance) {
    <pallet_balances::Pallet<Test> as ReservableCurrency<AccountId>>::reserve(&ALICE, amount)
        .expect("alice has enough");
}

// ─── draft_will ────────────────────────────────────────────────────────────

#[test]
fn draft_will_stores_heirs_and_shares() {
    new_test_ext().execute_with(|| {
        assert_ok!(Inheritance::draft_will(
            RuntimeOrigin::signed(ALICE),
            vec![(BOB, 60), (CHARLIE, 40)],
        ));
        let will = Inheritance::active_wills(ALICE).expect("will must exist");
        assert_eq!(will.heirs.len(), 2);
        assert!(!will.is_notarized);
        assert!(will.notary.is_none());
    });
}

#[test]
fn draft_will_single_heir_100_percent() {
    new_test_ext().execute_with(|| {
        assert_ok!(Inheritance::draft_will(
            RuntimeOrigin::signed(ALICE),
            vec![(BOB, 100)],
        ));
        let will = Inheritance::active_wills(ALICE).unwrap();
        assert_eq!(will.heirs.len(), 1);
        assert_eq!(will.heirs[0].1, 100);
    });
}

#[test]
fn draft_will_shares_not_100_fails() {
    new_test_ext().execute_with(|| {
        assert_noop!(
            Inheritance::draft_will(
                RuntimeOrigin::signed(ALICE),
                vec![(BOB, 60), (CHARLIE, 30)], // sum = 90
            ),
            Error::<Test>::HeirsShareNot100,
        );
    });
}

#[test]
fn draft_will_no_heirs_fails() {
    new_test_ext().execute_with(|| {
        assert_noop!(
            Inheritance::draft_will(RuntimeOrigin::signed(ALICE), vec![]),
            Error::<Test>::NoHeirsDeclared,
        );
    });
}

#[test]
fn redraft_clears_notarization() {
    new_test_ext().execute_with(|| {
        // Draft and notarize
        assert_ok!(Inheritance::draft_will(
            RuntimeOrigin::signed(ALICE),
            vec![(BOB, 100)],
        ));
        assert_ok!(Inheritance::notarize_will(
            RuntimeOrigin::signed(NOTARY),
            ALICE,
            0, // zero fee for simplicity
        ));
        let notarized = Inheritance::active_wills(ALICE).unwrap();
        assert!(notarized.is_notarized);

        // Redraft — should reset notarization
        assert_ok!(Inheritance::draft_will(
            RuntimeOrigin::signed(ALICE),
            vec![(CHARLIE, 100)],
        ));
        let redrafted = Inheritance::active_wills(ALICE).unwrap();
        assert!(!redrafted.is_notarized);
        assert!(redrafted.notary.is_none());
    });
}

// ─── notarize_will ────────────────────────────────────────────────────────

#[test]
fn notarize_will_by_valid_notary_succeeds() {
    new_test_ext().execute_with(|| {
        assert_ok!(Inheritance::draft_will(
            RuntimeOrigin::signed(ALICE),
            vec![(BOB, 100)],
        ));
        assert_ok!(Inheritance::notarize_will(
            RuntimeOrigin::signed(NOTARY),
            ALICE,
            5 * UNIT, // fee
        ));
        let will = Inheritance::active_wills(ALICE).unwrap();
        assert!(will.is_notarized);
        assert_eq!(will.notary, Some(NOTARY));
        // NOTARY received the fee
        assert_eq!(free_bal(NOTARY), 1_005 * UNIT);
        // ALICE paid the fee
        assert_eq!(free_bal(ALICE), 9_995 * UNIT);
    });
}

#[test]
fn notarize_will_non_notary_fails() {
    new_test_ext().execute_with(|| {
        assert_ok!(Inheritance::draft_will(
            RuntimeOrigin::signed(ALICE),
            vec![(BOB, 100)],
        ));
        // BOB is not a notary
        assert_noop!(
            Inheritance::notarize_will(RuntimeOrigin::signed(BOB), ALICE, 0),
            Error::<Test>::NotValidNotary,
        );
    });
}

#[test]
fn notarize_will_self_certification_fails() {
    new_test_ext().execute_with(|| {
        register_notary(ALICE); // ALICE is also a notary
        assert_ok!(Inheritance::draft_will(
            RuntimeOrigin::signed(ALICE),
            vec![(BOB, 100)],
        ));
        // ALICE cannot notarize their own will
        assert_noop!(
            Inheritance::notarize_will(RuntimeOrigin::signed(ALICE), ALICE, 0),
            Error::<Test>::CannotNotarizeSelf,
        );
        clear_mocks();
        register_notary(NOTARY);
    });
}

#[test]
fn notarize_will_no_will_fails() {
    new_test_ext().execute_with(|| {
        assert_noop!(
            Inheritance::notarize_will(RuntimeOrigin::signed(NOTARY), ALICE, 0),
            Error::<Test>::NoActiveWill,
        );
    });
}

// ─── execute_will — heir distribution ─────────────────────────────────────

#[test]
fn execute_will_distributes_to_heirs_proportionally() {
    new_test_ext().execute_with(|| {
        let estate = 1_000 * UNIT;
        // Setup
        assert_ok!(Inheritance::draft_will(
            RuntimeOrigin::signed(ALICE),
            vec![(BOB, 60), (CHARLIE, 40)],
        ));
        assert_ok!(Inheritance::notarize_will(
            RuntimeOrigin::signed(NOTARY),
            ALICE,
            0,
        ));
        register_deceased(ALICE);
        freeze_alice(estate);

        let bob_before = free_bal(BOB);
        let charlie_before = free_bal(CHARLIE);

        assert_ok!(Inheritance::execute_will(RuntimeOrigin::signed(BOB), ALICE,));

        // BOB: 60% of 1000 UNIT = 600 UNIT
        assert_eq!(free_bal(BOB), bob_before + 600 * UNIT);
        // CHARLIE: 40% + dust = 400 UNIT (last heir gets remainder)
        assert_eq!(free_bal(CHARLIE), charlie_before + 400 * UNIT);
        // Will should be removed
        assert!(Inheritance::active_wills(ALICE).is_none());

        clear_mocks();
    });
}

#[test]
fn execute_will_citizen_not_deceased_fails() {
    new_test_ext().execute_with(|| {
        assert_ok!(Inheritance::draft_will(
            RuntimeOrigin::signed(ALICE),
            vec![(BOB, 100)],
        ));
        assert_ok!(Inheritance::notarize_will(
            RuntimeOrigin::signed(NOTARY),
            ALICE,
            0,
        ));
        // ALICE not registered as deceased
        assert_noop!(
            Inheritance::execute_will(RuntimeOrigin::signed(BOB), ALICE),
            Error::<Test>::CitizenNotDeceased,
        );
    });
}

#[test]
fn execute_will_fallback_to_treasury_when_no_notarized_will() {
    new_test_ext().execute_with(|| {
        let estate = 500 * UNIT;
        // ALICE has no will
        register_deceased(ALICE);
        freeze_alice(estate);

        let treasury_before = free_bal(TREASURY);

        assert_ok!(Inheritance::execute_will(RuntimeOrigin::signed(BOB), ALICE,));

        // All estate sent to treasury
        assert_eq!(free_bal(TREASURY), treasury_before + estate);

        clear_mocks();
    });
}

// ─── [SECURITY VECTOR 1] CDP Dead-Debt Liquidation ────────────────────────

#[test]
fn execute_will_liquidates_cdp_debt_before_heir_distribution() {
    new_test_ext().execute_with(|| {
        let estate = 1_000 * UNIT;
        let cdp_debt = 300 * UNIT; // outstanding CDP debt

        assert_ok!(Inheritance::draft_will(
            RuntimeOrigin::signed(ALICE),
            vec![(BOB, 100)],
        ));
        assert_ok!(Inheritance::notarize_will(
            RuntimeOrigin::signed(NOTARY),
            ALICE,
            0,
        ));
        register_deceased(ALICE);
        freeze_alice(estate);
        set_cdp_debt(ALICE, cdp_debt);

        let bob_before = free_bal(BOB);
        let supply_before = pallet_balances::Pallet::<Test>::total_issuance();

        assert_ok!(Inheritance::execute_will(RuntimeOrigin::signed(BOB), ALICE,));

        // BOB receives only the remainder: 1000 - 300 = 700 UNIT
        assert_eq!(free_bal(BOB), bob_before + 700 * UNIT);

        // CDP debt was burned (NegativeImbalance dropped) → TotalIssuance decreased
        let supply_after = pallet_balances::Pallet::<Test>::total_issuance();
        assert_eq!(supply_after, supply_before - cdp_debt);

        clear_mocks();
    });
}

#[test]
fn execute_will_zero_reserved_balance_executes_cleanly() {
    new_test_ext().execute_with(|| {
        assert_ok!(Inheritance::draft_will(
            RuntimeOrigin::signed(ALICE),
            vec![(BOB, 100)],
        ));
        assert_ok!(Inheritance::notarize_will(
            RuntimeOrigin::signed(NOTARY),
            ALICE,
            0,
        ));
        register_deceased(ALICE);
        // No funds reserved — execute_will must not panic
        // Per the pallet: when reserved == 0 with notarized will, it emits WillExecuted
        // with total_distributed=0 and returns Ok (early return without removing will)

        assert_ok!(Inheritance::execute_will(RuntimeOrigin::signed(BOB), ALICE,));
        // The pallet exits early without removing the will when reserved == 0
        // This is intentional — the will is still valid for deferred execution
        // if funds are reserved later before death record is finalized.
        // Just verify no panic and no heir fund leaks.
        assert_eq!(free_bal(BOB), 1_000 * UNIT); // BOB got nothing extra

        clear_mocks();
    });
}

#[test]
fn execute_will_cdp_larger_than_estate_burns_all() {
    new_test_ext().execute_with(|| {
        let estate = 200 * UNIT;
        let cdp_debt = 500 * UNIT; // debt > estate → all burned, heirs get 0

        assert_ok!(Inheritance::draft_will(
            RuntimeOrigin::signed(ALICE),
            vec![(BOB, 100)],
        ));
        assert_ok!(Inheritance::notarize_will(
            RuntimeOrigin::signed(NOTARY),
            ALICE,
            0,
        ));
        register_deceased(ALICE);
        freeze_alice(estate);
        set_cdp_debt(ALICE, cdp_debt);

        let bob_before = free_bal(BOB);

        assert_ok!(Inheritance::execute_will(RuntimeOrigin::signed(BOB), ALICE,));

        // Heirs get nothing — all estate was burned to settle debt
        assert_eq!(free_bal(BOB), bob_before);

        clear_mocks();
    });
}
