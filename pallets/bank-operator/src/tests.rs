//! Unit tests for pallet-bank-operator.
//!
//! Covers:
//! - CDP 9x multiplier enforcement (constitutional invariant)
//! - Credit rating tier resolution (FICO-inspired, score 300–850)
//! - Interest rate tiers (0% Excellent → 15% Poor)
//! - Deflationary principal burn on repayment
//! - Credit score updates (+20 on full repay, −100 on default)
//! - Error paths: zero collateral, missing bank account, wrong owner, etc.

use super::*;
use crate::mock::*;
use frame_support::{assert_noop, assert_ok};

// =========================================================================
// Helpers
// =========================================================================

/// Lock `amount` ALTAN as collateral for `citizen` (bank account must be set).
/// Returns the collateral contract ID.
fn lock(citizen: AccountId, amount: Balance) -> u32 {
    let id = BankOperator::next_collateral_id();
    assert_ok!(BankOperator::lock_collateral(
        RuntimeOrigin::signed(citizen),
        amount,
        AssetType::AltanCoin,
    ));
    id
}

/// Request credit → issue → return (collateral_id, credit_id).
fn full_cycle(citizen: AccountId, collateral: Balance) -> (u32, u32) {
    let col_id = lock(citizen, collateral);
    let credit_id = BankOperator::next_credit_id();
    assert_ok!(BankOperator::request_credit(
        RuntimeOrigin::signed(citizen),
        col_id
    ));
    assert_ok!(BankOperator::issue_credit(RuntimeOrigin::root(), col_id));
    (col_id, credit_id)
}

// =========================================================================
// 1. Bank account configuration
// =========================================================================

#[test]
fn set_bank_account_requires_root() {
    new_test_ext().execute_with(|| {
        assert_noop!(
            BankOperator::set_bank_account(RuntimeOrigin::signed(1), BANK),
            frame_support::error::BadOrigin,
        );
    });
}

#[test]
fn set_bank_account_stores_value() {
    new_test_ext().execute_with(|| {
        // new_test_ext already calls set_bank_account
        assert_eq!(BankOperator::bank_special_account(), Some(BANK));
    });
}

// =========================================================================
// 2. Collateral locking
// =========================================================================

#[test]
fn lock_collateral_transfers_full_amount_to_bank() {
    new_test_ext().execute_with(|| {
        let alice_before = Balances::free_balance(1);
        let bank_before = Balances::free_balance(BANK);
        lock(1, 100 * UNIT);
        assert_eq!(Balances::free_balance(1), alice_before - 100 * UNIT);
        assert_eq!(Balances::free_balance(BANK), bank_before + 100 * UNIT);
    });
}

#[test]
fn lock_collateral_stores_10pct_fee_and_90pct_net() {
    new_test_ext().execute_with(|| {
        lock(1, 100 * UNIT);
        let c = BankOperator::collateral_contracts(0).unwrap();
        assert_eq!(c.bank_fee, 10 * UNIT);
        assert_eq!(c.collateral_net, 90 * UNIT);
    });
}

#[test]
fn lock_zero_collateral_fails() {
    new_test_ext().execute_with(|| {
        assert_noop!(
            BankOperator::lock_collateral(RuntimeOrigin::signed(1), 0, AssetType::AltanCoin),
            Error::<Test>::ZeroCollateral,
        );
    });
}

#[test]
fn lock_fails_without_bank_account_configured() {
    new_test_ext().execute_with(|| {
        // Override: clear bank account
        BankSpecialAccount::<Test>::kill();
        assert_noop!(
            BankOperator::lock_collateral(
                RuntimeOrigin::signed(1),
                100 * UNIT,
                AssetType::AltanCoin
            ),
            Error::<Test>::BankAccountNotConfigured,
        );
    });
}

// =========================================================================
// 3. CDP 9x — CONSTITUTIONAL INVARIANT
// =========================================================================

#[test]
fn issue_credit_gives_9x_collateral_net() {
    new_test_ext().execute_with(|| {
        // col = 100 ALTAN; fee = 10; net = 90; credit = 90 × 9 = 810
        let (_col_id, credit_id) = full_cycle(1, 100 * UNIT);
        let credit = BankOperator::credit_contracts(credit_id).unwrap();
        assert_eq!(credit.credit_amount, 810 * UNIT);
        assert_eq!(credit.outstanding, 810 * UNIT);
    });
}

#[test]
fn citizen_receives_credit_amount_after_issue() {
    new_test_ext().execute_with(|| {
        let alice_before_lock = Balances::free_balance(1);
        lock(1, 100 * UNIT); // Alice pays 100 to bank
        let alice_after_lock = Balances::free_balance(1);
        assert_eq!(alice_after_lock, alice_before_lock - 100 * UNIT);

        let _credit_id = BankOperator::next_credit_id();
        assert_ok!(BankOperator::request_credit(RuntimeOrigin::signed(1), 0));
        assert_ok!(BankOperator::issue_credit(RuntimeOrigin::root(), 0));

        // Alice receives 810 ALTAN newly minted
        assert_eq!(Balances::free_balance(1), alice_after_lock + 810 * UNIT);
    });
}

#[test]
fn issue_credit_without_request_fails() {
    new_test_ext().execute_with(|| {
        lock(1, 100 * UNIT);
        assert_noop!(
            BankOperator::issue_credit(RuntimeOrigin::root(), 0),
            Error::<Test>::CreditNotRequested,
        );
    });
}

// =========================================================================
// 4. Credit Rating Tier Resolution (FICO-inspired)
// =========================================================================

#[test]
fn tier_excellent_score_range_and_zero_rate() {
    assert_eq!(CreditTier::from_score(850), CreditTier::Excellent);
    assert_eq!(CreditTier::from_score(750), CreditTier::Excellent);
    assert_eq!(CreditTier::Excellent.annual_rate_bps(), 0);
}

#[test]
fn tier_good_score_range() {
    assert_eq!(CreditTier::from_score(749), CreditTier::Good);
    assert_eq!(CreditTier::from_score(700), CreditTier::Good);
    assert_eq!(CreditTier::Good.annual_rate_bps(), 300);
}

#[test]
fn tier_fair_is_default_at_650() {
    let record = CreditScoreRecord::new_default();
    assert_eq!(record.score, 650);
    assert_eq!(record.tier(), CreditTier::Fair);
    assert_eq!(CreditTier::Fair.annual_rate_bps(), 700);
}

#[test]
fn tier_below_fair_range() {
    assert_eq!(CreditTier::from_score(649), CreditTier::BelowFair);
    assert_eq!(CreditTier::from_score(600), CreditTier::BelowFair);
    assert_eq!(CreditTier::BelowFair.annual_rate_bps(), 1_000);
}

#[test]
fn tier_poor_range_and_max_rate() {
    assert_eq!(CreditTier::from_score(599), CreditTier::Poor);
    assert_eq!(CreditTier::from_score(300), CreditTier::Poor);
    assert_eq!(CreditTier::Poor.annual_rate_bps(), 1_500);
}

#[test]
fn new_credit_uses_fair_tier_by_default() {
    new_test_ext().execute_with(|| {
        let (_col, credit_id) = full_cycle(1, 100 * UNIT);
        let credit = BankOperator::credit_contracts(credit_id).unwrap();
        assert_eq!(credit.tier, CreditTier::Fair);
    });
}

// =========================================================================
// 5. Credit Score Updates
// =========================================================================

#[test]
fn full_repayment_increments_score_by_20() {
    new_test_ext().execute_with(|| {
        let (_col, credit_id) = full_cycle(1, 100 * UNIT);

        // Full repayment: pay the entire 810 ALTAN outstanding
        let outstanding = BankOperator::credit_contracts(credit_id)
            .unwrap()
            .outstanding;
        assert_ok!(BankOperator::repay_credit(
            RuntimeOrigin::signed(1),
            credit_id,
            outstanding,
        ));

        let score = BankOperator::credit_score(&1u64).unwrap();
        assert_eq!(score.score, 650 + 20, "Full repayment: +20 pts");
        assert_eq!(score.credits_repaid, 1);
    });
}

#[test]
fn default_decrements_score_by_100() {
    new_test_ext().execute_with(|| {
        let (_col, credit_id) = full_cycle(1, 100 * UNIT);
        assert_ok!(BankOperator::declare_default(
            RuntimeOrigin::root(),
            credit_id
        ));

        let score = BankOperator::credit_score(&1u64).unwrap();
        assert_eq!(score.score, 650 - 100, "Default: −100 pts");
        assert_eq!(score.credits_defaulted, 1);
    });
}

#[test]
fn score_caps_at_850() {
    let mut r = CreditScoreRecord {
        score: 845,
        credits_repaid: 0,
        credits_defaulted: 0,
    };
    r.apply_full_repayment(); // +20 → 865, but capped
    assert_eq!(r.score, 850);
    r.apply_full_repayment();
    assert_eq!(r.score, 850); // stays at cap
}

#[test]
fn score_floors_at_300() {
    let mut r = CreditScoreRecord {
        score: 310,
        credits_repaid: 0,
        credits_defaulted: 0,
    };
    r.apply_default(); // −100 → 210, but floored
    assert_eq!(r.score, 300);
    r.apply_default();
    assert_eq!(r.score, 300); // stays at floor
}

// =========================================================================
// 6. Deflationary Burn on Principal Repayment
// =========================================================================

#[test]
fn partial_repayment_reduces_total_issuance() {
    new_test_ext().execute_with(|| {
        let (_col, credit_id) = full_cycle(1, 100 * UNIT);

        let issuance_before = Balances::total_issuance();
        // Repay 100 ALTAN of the 810 outstanding
        assert_ok!(BankOperator::repay_credit(
            RuntimeOrigin::signed(1),
            credit_id,
            100 * UNIT,
        ));
        let issuance_after = Balances::total_issuance();
        assert!(
            issuance_after < issuance_before,
            "TotalIssuance must decrease after principal burn"
        );
    });
}

#[test]
fn full_repayment_returns_collateral_net_to_citizen() {
    new_test_ext().execute_with(|| {
        let (col_id, credit_id) = full_cycle(1, 100 * UNIT);

        // Balance after receiving credit
        let alice_before_repay = Balances::free_balance(1);
        let outstanding = BankOperator::credit_contracts(credit_id)
            .unwrap()
            .outstanding;

        assert_ok!(BankOperator::repay_credit(
            RuntimeOrigin::signed(1),
            credit_id,
            outstanding,
        ));

        // Credit should be Repaid
        assert_eq!(
            BankOperator::credit_contracts(credit_id).unwrap().status,
            CreditStatus::Repaid,
        );

        // Collateral net (90 ALTAN) returned
        let collateral_net = BankOperator::collateral_contracts(col_id)
            .unwrap()
            .collateral_net;
        let alice_after = Balances::free_balance(1);
        // Alice paid outstanding (−810 ALTAN) and received net back (+90 ALTAN)
        let expected = alice_before_repay - outstanding + collateral_net;
        assert_eq!(alice_after, expected);
    });
}

// =========================================================================
// 7. Error paths
// =========================================================================

#[test]
fn zero_repayment_fails() {
    new_test_ext().execute_with(|| {
        let (_col, credit_id) = full_cycle(1, 100 * UNIT);
        assert_noop!(
            BankOperator::repay_credit(RuntimeOrigin::signed(1), credit_id, 0),
            Error::<Test>::ZeroRepayment,
        );
    });
}

#[test]
fn repay_by_non_owner_fails() {
    new_test_ext().execute_with(|| {
        let (_col, credit_id) = full_cycle(1, 100 * UNIT);
        // Bob (2) tries to repay Alice's (1) credit
        assert_noop!(
            BankOperator::repay_credit(RuntimeOrigin::signed(2), credit_id, 10 * UNIT),
            Error::<Test>::NotCreditOwner,
        );
    });
}

#[test]
fn total_credit_outstanding_tracks_correctly() {
    new_test_ext().execute_with(|| {
        assert_eq!(BankOperator::total_credit_outstanding(), 0u128.into());

        let (_col, credit_id) = full_cycle(1, 100 * UNIT);
        // 90 × 9 = 810 ALTAN outstanding
        assert_eq!(BankOperator::total_credit_outstanding(), 810 * UNIT,);

        // Partial repayment of 100 ALTAN
        assert_ok!(BankOperator::repay_credit(
            RuntimeOrigin::signed(1),
            credit_id,
            100 * UNIT,
        ));
        assert_eq!(BankOperator::total_credit_outstanding(), 710 * UNIT,);
    });
}
