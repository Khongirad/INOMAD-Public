//! Unit tests for pallet-altan-tax.
//!
//! v2 Constitutional Reform: 54/36/10 split.
//! Covers constitutional fee mandates:
//! - Rate: 0.03% of transfer amount
//! - Cap: 1,000 ALTAN maximum fee
//! - Split: 54% Khural Foundation / 36% INOMAD AG / 10% Validators
//! - Dust always goes to Khural (via subtraction method)
//! - Error: missing treasury accounts
//!
//! REMOVED (v2): direct 10% Creator routing.
//! Creator compensation flows through INOMAD AG (Swiss GmbH) corporate structure.

use super::*;
use crate::mock::*;
use frame_support::{assert_noop, assert_ok};
use sp_runtime::BuildStorage;

// =========================================================================
// Constants (match AGENTS.md v2)
// =========================================================================

/// Fee rate: 0.03% = 3/10_000
const FEE_RATE_NUM: u128 = 3;
const FEE_RATE_DEN: u128 = 10_000;

/// Fee cap: 1,000 ALTAN = 1_000 × 10^12 planck
const FEE_CAP: u128 = 1_000 * UNIT;

/// Threshold at which fee cap kicks in: 1_000 / 0.0003 ≈ 3,333,333.33 ALTAN
const CAP_THRESHOLD: u128 = 3_333_334 * UNIT; // slightly above threshold

fn expected_fee(amount: u128) -> u128 {
    let raw = amount * FEE_RATE_NUM / FEE_RATE_DEN;
    raw.min(FEE_CAP)
}

// =========================================================================
// 1. Fee Rate — 0.03%
// =========================================================================

#[test]
fn fee_is_0_03_percent_of_amount() {
    new_test_ext().execute_with(|| {
        let amount = 100_000 * UNIT; // 100,000 ALTAN
        let expected = expected_fee(amount); // 30 ALTAN

        let ag_before = Balances::free_balance(AG_ACCOUNT);
        let khural_before = Balances::free_balance(KHURAL_ACCOUNT);
        let validator_before = Balances::free_balance(VALIDATOR_ACCOUNT);

        assert_ok!(AltanTax::transfer_with_fee(
            RuntimeOrigin::signed(1),
            2,
            amount
        ));

        let total_fee_collected = (Balances::free_balance(AG_ACCOUNT) - ag_before)
            + (Balances::free_balance(KHURAL_ACCOUNT) - khural_before)
            + (Balances::free_balance(VALIDATOR_ACCOUNT) - validator_before);

        assert_eq!(total_fee_collected, expected);
    });
}

// =========================================================================
// 2. Fee Cap — max 1,000 ALTAN
// =========================================================================

#[test]
fn fee_caps_at_1000_altan_for_large_transfers() {
    new_test_ext().execute_with(|| {
        // Transfer 4M ALTAN (above threshold: 3,333,333.33 ALTAN)
        let amount = 4_000_000 * UNIT;
        assert!(
            amount > CAP_THRESHOLD,
            "Test amount must exceed cap threshold"
        );

        let ag_before = Balances::free_balance(AG_ACCOUNT);
        let khural_before = Balances::free_balance(KHURAL_ACCOUNT);
        let validator_before = Balances::free_balance(VALIDATOR_ACCOUNT);

        assert_ok!(AltanTax::transfer_with_fee(
            RuntimeOrigin::signed(1),
            2,
            amount
        ));

        let total_fee = (Balances::free_balance(AG_ACCOUNT) - ag_before)
            + (Balances::free_balance(KHURAL_ACCOUNT) - khural_before)
            + (Balances::free_balance(VALIDATOR_ACCOUNT) - validator_before);

        // Fee must equal exactly 1,000 ALTAN (cap enforced)
        assert_eq!(total_fee, FEE_CAP, "Fee should be capped at 1,000 ALTAN");
    });
}

#[test]
fn fee_not_capped_below_threshold() {
    new_test_ext().execute_with(|| {
        // Transfer 1M ALTAN (below threshold)
        let amount = 1_000_000 * UNIT;
        let expected = expected_fee(amount); // 300 ALTAN (0.03%)
        assert!(expected < FEE_CAP, "Must be below cap for this test");

        let ag_before = Balances::free_balance(AG_ACCOUNT);
        let khural_before = Balances::free_balance(KHURAL_ACCOUNT);
        let validator_before = Balances::free_balance(VALIDATOR_ACCOUNT);

        assert_ok!(AltanTax::transfer_with_fee(
            RuntimeOrigin::signed(1),
            2,
            amount
        ));

        let total_fee = (Balances::free_balance(AG_ACCOUNT) - ag_before)
            + (Balances::free_balance(KHURAL_ACCOUNT) - khural_before)
            + (Balances::free_balance(VALIDATOR_ACCOUNT) - validator_before);

        assert_eq!(total_fee, expected);
    });
}

// =========================================================================
// 3. Fee Split — 54% / 36% / 10% (v2 Constitutional Reform)
// =========================================================================

/// Verify exact constitutional ratios (v2: 54/36/10).
/// Using 1,000 ALTAN fee (capped) for planck-level integer precision.
#[test]
fn fee_split_is_54_36_10_at_cap() {
    new_test_ext().execute_with(|| {
        // Use capped fee scenario: 4M ALTAN transfer → fee = 1,000 ALTAN
        let amount = 4_000_000 * UNIT;
        let fee = FEE_CAP; // 1_000 ALTAN

        let ag_before = Balances::free_balance(AG_ACCOUNT);
        let khural_before = Balances::free_balance(KHURAL_ACCOUNT);
        let validator_before = Balances::free_balance(VALIDATOR_ACCOUNT);

        assert_ok!(AltanTax::transfer_with_fee(
            RuntimeOrigin::signed(1),
            2,
            amount
        ));

        let ag_got = Balances::free_balance(AG_ACCOUNT) - ag_before;
        let khural_got = Balances::free_balance(KHURAL_ACCOUNT) - khural_before;
        let validator_got = Balances::free_balance(VALIDATOR_ACCOUNT) - validator_before;

        // AG: 36% of 1,000 ALTAN = 360 ALTAN (includes Creator compensation via corporate)
        assert_eq!(ag_got, fee * 36 / 100, "INOMAD AG should get 36% (v2)");
        // Validator: 10% = 100 ALTAN
        assert_eq!(validator_got, fee * 10 / 100, "Validators should get 10%");
        // Khural: remainder = 540 ALTAN (54% + dust)
        let khural_expected = fee - ag_got - validator_got;
        assert_eq!(khural_got, khural_expected, "Khural should get 54% + dust");

        // Total must equal fee (54 + 36 + 10 = 100%)
        assert_eq!(ag_got + khural_got + validator_got, fee);
    });
}

/// Khural always absorbs mathematical rounding dust (remainder method).
#[test]
fn khural_gets_all_mathematical_dust() {
    new_test_ext().execute_with(|| {
        // 10,000 ALTAN transfer: fee = 3 ALTAN exactly
        let amount = 10_000 * UNIT;
        let fee = expected_fee(amount); // 3 ALTAN = 3_000_000_000_000 planck

        let ag_before = Balances::free_balance(AG_ACCOUNT);
        let khural_before = Balances::free_balance(KHURAL_ACCOUNT);
        let validator_before = Balances::free_balance(VALIDATOR_ACCOUNT);

        assert_ok!(AltanTax::transfer_with_fee(
            RuntimeOrigin::signed(1),
            2,
            amount
        ));

        let ag_got = Balances::free_balance(AG_ACCOUNT) - ag_before;
        let khural_got = Balances::free_balance(KHURAL_ACCOUNT) - khural_before;
        let validator_got = Balances::free_balance(VALIDATOR_ACCOUNT) - validator_before;

        // Total must equal fee exactly (no planck lost — 54+36+10=100)
        assert_eq!(
            ag_got + khural_got + validator_got,
            fee,
            "No planck should be lost"
        );

        // Khural must receive at least 54% (may receive dust extra)
        let khural_min = fee * 54 / 100;
        assert!(khural_got >= khural_min, "Khural must receive ≥54%");
    });
}

// =========================================================================
// 4. Net amount received by recipient (exclusive fee)
// =========================================================================

#[test]
fn recipient_receives_exact_amount_exclusive_fee() {
    new_test_ext().execute_with(|| {
        let amount = 1_000 * UNIT;

        let bob_before = Balances::free_balance(2);
        assert_ok!(AltanTax::transfer_with_fee(
            RuntimeOrigin::signed(1),
            2,
            amount
        ));
        let bob_after = Balances::free_balance(2);

        assert_eq!(
            bob_after - bob_before,
            amount,
            "Bob receives exactly amount (exclusive fee — sender pays extra)"
        );
    });
}

// =========================================================================
// 5. Error paths
// =========================================================================

#[test]
fn transfer_fails_without_treasury_accounts() {
    let mut t = frame_system::GenesisConfig::<Test>::default()
        .build_storage()
        .unwrap();
    pallet_balances::GenesisConfig::<Test> {
        balances: vec![(1, 1_000_000 * UNIT)],
        dev_accounts: None,
    }
    .assimilate_storage(&mut t)
    .unwrap();
    // No treasury accounts configured in genesis!
    let mut ext = sp_io::TestExternalities::new(t);
    ext.execute_with(|| {
        assert_noop!(
            AltanTax::transfer_with_fee(RuntimeOrigin::signed(1), 2, 1_000 * UNIT),
            Error::<Test>::TreasuriesNotInitialized,
        );
    });
}

// =========================================================================
// 6. Transfer amount conservation (no mint / no burn)
// =========================================================================

#[test]
fn total_issuance_unchanged_after_transfer_with_tax() {
    new_test_ext().execute_with(|| {
        let issuance_before = Balances::total_issuance();
        let amount = 100_000 * UNIT;

        assert_ok!(AltanTax::transfer_with_fee(
            RuntimeOrigin::signed(1),
            2,
            amount
        ));

        let issuance_after = Balances::total_issuance();
        // Transfer-with-tax is pure routing — no burn, no mint.
        assert_eq!(
            issuance_before, issuance_after,
            "TotalIssuance must not change on tax routing"
        );
    });
}
