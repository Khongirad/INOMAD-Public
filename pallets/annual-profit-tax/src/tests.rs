//! Unit tests for pallet-annual-profit-tax (v2).
//!
//! Constitutional mandates covered:
//!
//! 1.  Standard rate:  10% of annual profit
//! 2.  Large family:   5% of annual profit (3+ children)
//! 3.  On-time window: Jan 1 – Apr 15 (no penalties)
//! 4.  Штраф:         5% of base tax (one-time, if late)
//! 5.  Пени:          5% / 365 per day from Apr 16
//! 6.  Max late days:  259 (Apr 16 – Dec 31)
//! 7.  Dec 31 deadline: filing blocked after year-end
//! 8.  70/30 split:    to planck precision
//! 9.  Double-filing prevention
//! 10. Zero profit rejected
//! 11. Future year blocked, pre-genesis year blocked
//! 12. Total issuance conserved (pure routing, no mint/burn)

use super::*;
use crate::mock::*;
use frame_support::{assert_noop, assert_ok};

const YEAR: TaxYear = 2026;

// ============================================================================
// 1. Standard 10% tax rate
// ============================================================================

#[test]
fn standard_rate_is_10_percent() {
    new_test_ext().execute_with(|| {
        // Jan 15, 2026 — on-time
        set_time(JAN1_2026 + 14 * 86_400);

        let profit = 1_000_000 * UNIT;

        let org_before = Balances::free_balance(ORG);

        assert_ok!(AnnualProfitTax::declare_annual_profit(
            RuntimeOrigin::signed(ORG),
            YEAR,
            profit,
            false, // standard rate
        ));

        let total_paid = org_before - Balances::free_balance(ORG);
        let expected = profit / 10; // 10% = 100,000 ALTAN

        assert_eq!(total_paid, expected, "Standard tax must be exactly 10%");

        let record = AnnualProfitTax::profit_declarations(ORG, YEAR).unwrap();
        assert_eq!(record.base_tax, expected);
        assert_eq!(record.shtraf, 0);
        assert_eq!(record.peni, 0);
        assert_eq!(record.days_late, 0);
        assert!(!record.large_family_rate);
    });
}

// ============================================================================
// 2. Large family 5% reduced rate
// ============================================================================

#[test]
fn large_family_rate_is_5_percent() {
    new_test_ext().execute_with(|| {
        // Jan 15, 2026 — on-time
        set_time(JAN1_2026 + 14 * 86_400);

        let profit = 1_000_000 * UNIT;
        let expected_tax = profit / 20; // 5% = 50,000 ALTAN

        let fam_before = Balances::free_balance(FAMILY);

        assert_ok!(AnnualProfitTax::declare_annual_profit(
            RuntimeOrigin::signed(FAMILY),
            YEAR,
            profit,
            true, // large family — 3+ children
        ));

        let total_paid = fam_before - Balances::free_balance(FAMILY);
        assert_eq!(total_paid, expected_tax, "Large family tax must be 5%");

        let record = AnnualProfitTax::profit_declarations(FAMILY, YEAR).unwrap();
        assert!(record.large_family_rate, "Record must mark large_family_rate=true");
        assert_eq!(record.base_tax, expected_tax);
        assert_eq!(record.shtraf, 0);
        assert_eq!(record.peni, 0);
    });
}

// ============================================================================
// 3. On-time filing — no penalties (Apr 15 last second)
// ============================================================================

#[test]
fn no_penalties_within_filing_window() {
    new_test_ext().execute_with(|| {
        // Apr 15, 2026 23:59:59 — last valid second
        set_time(APR15_2026_END);

        assert_ok!(AnnualProfitTax::declare_annual_profit(
            RuntimeOrigin::signed(ORG),
            YEAR,
            500_000 * UNIT,
            false,
        ));

        let record = AnnualProfitTax::profit_declarations(ORG, YEAR).unwrap();
        assert_eq!(record.shtraf, 0, "No штраф on Apr 15");
        assert_eq!(record.peni, 0, "No пени on Apr 15");
        assert_eq!(record.days_late, 0);
    });
}

// ============================================================================
// 4. Late filing — штраф (day 1 = April 16)
// ============================================================================

#[test]
fn shtraf_is_5_percent_on_day_1_late() {
    new_test_ext().execute_with(|| {
        // Apr 16, 2026 00:00:00 — first late day
        set_time(APR16_2026);

        let profit = 1_000_000 * UNIT;
        let base_tax = profit / 10; // 100,000 ALTAN

        assert_ok!(AnnualProfitTax::declare_annual_profit(
            RuntimeOrigin::signed(ORG),
            YEAR,
            profit,
            false,
        ));

        let record = AnnualProfitTax::profit_declarations(ORG, YEAR).unwrap();

        // Штраф = 5% of base_tax = 5,000 ALTAN
        let expected_shtraf = base_tax * 5 / 100;
        assert_eq!(record.shtraf, expected_shtraf, "Штраф must be 5% of base tax");
        assert_eq!(record.days_late, 1, "Day 1 of late period");
    });
}

// ============================================================================
// 5. Пени — daily accrual (5%/year / 365)
// ============================================================================

#[test]
fn peni_accrues_daily_from_apr16() {
    new_test_ext().execute_with(|| {
        // Apr 16, 2026: day 1 late
        let profit = 1_000_000 * UNIT;
        let base_tax = profit / 10; // 100,000 ALTAN

        // Day 1
        set_time(APR16_2026);
        assert_ok!(AnnualProfitTax::declare_annual_profit(
            RuntimeOrigin::signed(ORG),
            YEAR,
            profit,
            false,
        ));
        let record_d1 = AnnualProfitTax::profit_declarations(ORG, YEAR).unwrap();
        let peni_d1 = record_d1.peni;

        // Expected peni day 1: base_tax × 5 × 1 / 36_500
        let expected_d1 = base_tax * 5 * 1 / 36_500;
        assert_eq!(peni_d1, expected_d1, "Пени day 1 = base_tax × 5/36500");

        // Day 76 (≈ July 1, 2026)
        // Reset state by using FAMILY account for a different filing
        set_time(APR16_2026 + 75 * 86_400); // 75 complete days after Apr16 = day 76
        assert_ok!(AnnualProfitTax::declare_annual_profit(
            RuntimeOrigin::signed(FAMILY),
            YEAR,
            profit,
            false,
        ));
        let record_d76 = AnnualProfitTax::profit_declarations(FAMILY, YEAR).unwrap();
        let peni_d76 = record_d76.peni;

        let expected_d76 = base_tax * 5 * 76 / 36_500;
        assert_eq!(peni_d76, expected_d76, "Пени day 76 = base_tax × 5 × 76 / 36500");

        // Verify peni grows with days
        assert!(peni_d76 > peni_d1, "Peni must grow as days increase");
    });
}

// ============================================================================
// 6. Full late payment example — Jul 1 (76 days late, standard rate)
// ============================================================================

#[test]
fn full_late_payment_jul1_example() {
    new_test_ext().execute_with(|| {
        // July 1, 2026 = Apr 16 + 75 full days = day 76 of late period
        set_time(APR16_2026 + 75 * 86_400);

        let profit = 1_000_000 * UNIT; // 1,000,000 ALTAN
        let base_tax = profit / 10;    // 100,000 ALTAN

        let org_before = Balances::free_balance(ORG);

        assert_ok!(AnnualProfitTax::declare_annual_profit(
            RuntimeOrigin::signed(ORG),
            YEAR,
            profit,
            false,
        ));

        let record = AnnualProfitTax::profit_declarations(ORG, YEAR).unwrap();

        // Штраф = 100,000 × 5% = 5,000 ALTAN
        assert_eq!(record.shtraf, 5_000 * UNIT);

        // Пени = 100,000 × 5 × 76 / 36,500 = 1,041.09... → truncated in integer math
        let expected_peni = base_tax * 5 * 76 / 36_500;
        assert_eq!(record.peni, expected_peni);

        // Total = base + shtraf + peni
        let expected_total = base_tax + 5_000 * UNIT + expected_peni;
        assert_eq!(record.total_paid, expected_total);

        // Verify actual transfer
        let total_paid = org_before - Balances::free_balance(ORG);
        assert_eq!(total_paid, expected_total);

        assert_eq!(record.days_late, 76);
    });
}

// ============================================================================
// 7. Maximum penalty — Day 259 (December 31)
// ============================================================================

#[test]
fn max_late_days_is_259_dec31() {
    new_test_ext().execute_with(|| {
        // Dec 31, 2026 23:59:59 — last possible second
        set_time(DEC31_2026_END);

        let profit = 1_000_000 * UNIT;
        let base_tax = profit / 10; // 100,000 ALTAN

        assert_ok!(AnnualProfitTax::declare_annual_profit(
            RuntimeOrigin::signed(ORG),
            YEAR,
            profit,
            false,
        ));

        let record = AnnualProfitTax::profit_declarations(ORG, YEAR).unwrap();

        assert_eq!(record.days_late, 259, "Dec 31 = 259 days late (max)");

        // Max пени = base_tax × 5 × 259 / 36_500
        let expected_max_peni = base_tax * 5 * 259 / 36_500;
        assert_eq!(record.peni, expected_max_peni);

        // Total = base + 5% shtraf + max peni
        let expected_total = base_tax + base_tax * 5 / 100 + expected_max_peni;
        assert_eq!(record.total_paid, expected_total);
    });
}

// ============================================================================
// 8. Filing after December 31 is blocked
// ============================================================================

#[test]
fn filing_after_dec31_is_blocked() {
    new_test_ext().execute_with(|| {
        // Jan 1, 2027 — tax authority takes over
        set_time(JAN1_2027);

        assert_noop!(
            AnnualProfitTax::declare_annual_profit(
                RuntimeOrigin::signed(ORG),
                YEAR,
                100_000 * UNIT,
                false,
            ),
            Error::<Test>::FilingDeadlineExpired
        );
    });
}

// ============================================================================
// 9. 70/30 split — planck precision
// ============================================================================

#[test]
fn split_70_30_planck_precision() {
    new_test_ext().execute_with(|| {
        // On-time filing: total_paid = base_tax (no penalties)
        let profit = 1_000 * UNIT; // 1,000 ALTAN → base_tax = 100 ALTAN

        let regional_before = Balances::free_balance(REGIONAL_TREASURY);
        let confed_before = Balances::free_balance(CONFEDERATION_TREASURY);

        assert_ok!(AnnualProfitTax::declare_annual_profit(
            RuntimeOrigin::signed(ORG),
            YEAR,
            profit,
            false,
        ));

        let total_tax = profit / 10; // 100 ALTAN
        let regional_got = Balances::free_balance(REGIONAL_TREASURY) - regional_before;
        let confed_got = Balances::free_balance(CONFEDERATION_TREASURY) - confed_before;

        // Regional: 70% of 100 ALTAN = 70 ALTAN
        assert_eq!(regional_got, total_tax * 70 / 100, "Regional = 70%");
        // Confederation: remainder = 30 ALTAN (absorbs dust)
        assert_eq!(confed_got, total_tax - regional_got, "Confed = 30% + dust");
        // Total: no planck lost
        assert_eq!(regional_got + confed_got, total_tax);
    });
}

// ============================================================================
// 10. Large family + late filing — both apply together
// ============================================================================

#[test]
fn large_family_with_late_filing_uses_5pct_base() {
    new_test_ext().execute_with(|| {
        // Apr 16, 2026 — day 1 late
        set_time(APR16_2026);

        let profit = 1_000_000 * UNIT;
        let base_tax = profit / 20; // 5% for large family = 50,000 ALTAN
        let expected_shtraf = base_tax * 5 / 100; // 2,500 ALTAN
        let expected_peni = base_tax * 5 * 1 / 36_500;
        let expected_total = base_tax + expected_shtraf + expected_peni;

        let fam_before = Balances::free_balance(FAMILY);

        assert_ok!(AnnualProfitTax::declare_annual_profit(
            RuntimeOrigin::signed(FAMILY),
            YEAR,
            profit,
            true, // large family
        ));

        let total_paid = fam_before - Balances::free_balance(FAMILY);
        assert_eq!(total_paid, expected_total);

        let record = AnnualProfitTax::profit_declarations(FAMILY, YEAR).unwrap();
        assert_eq!(record.base_tax, base_tax, "Base must use 5% rate");
        assert_eq!(record.shtraf, expected_shtraf);
        assert_eq!(record.peni, expected_peni);
        assert!(record.large_family_rate);
    });
}

// ============================================================================
// 11. Double-filing prevention
// ============================================================================

#[test]
fn cannot_declare_same_year_twice() {
    new_test_ext().execute_with(|| {
        assert_ok!(AnnualProfitTax::declare_annual_profit(
            RuntimeOrigin::signed(ORG),
            YEAR,
            100_000 * UNIT,
            false,
        ));
        assert_noop!(
            AnnualProfitTax::declare_annual_profit(
                RuntimeOrigin::signed(ORG),
                YEAR,
                200_000 * UNIT,
                false,
            ),
            Error::<Test>::AlreadyDeclared
        );
    });
}

// ============================================================================
// 12. Zero profit rejected
// ============================================================================

#[test]
fn zero_profit_rejected() {
    new_test_ext().execute_with(|| {
        assert_noop!(
            AnnualProfitTax::declare_annual_profit(
                RuntimeOrigin::signed(ORG),
                YEAR,
                0,
                false,
            ),
            Error::<Test>::ZeroProfitDeclared
        );
    });
}

// ============================================================================
// 13. Future year blocked
// ============================================================================

#[test]
fn future_year_blocked() {
    new_test_ext().execute_with(|| {
        set_time(JAN1_2026 + 14 * 86_400); // Jan 15, 2026 → year is 2026

        assert_noop!(
            AnnualProfitTax::declare_annual_profit(
                RuntimeOrigin::signed(ORG),
                2027, // future
                100_000 * UNIT,
                false,
            ),
            Error::<Test>::FutureYearNotAllowed
        );
    });
}

// ============================================================================
// 14. Pre-genesis year blocked
// ============================================================================

#[test]
fn pre_genesis_year_blocked() {
    new_test_ext().execute_with(|| {
        assert_noop!(
            AnnualProfitTax::declare_annual_profit(
                RuntimeOrigin::signed(ORG),
                2025, // before genesis
                100_000 * UNIT,
                false,
            ),
            Error::<Test>::YearBeforeGenesis
        );
    });
}

// ============================================================================
// 15. Total issuance unchanged (no mint/burn — pure routing)
// ============================================================================

#[test]
fn total_issuance_unchanged() {
    new_test_ext().execute_with(|| {
        let before = Balances::total_issuance();

        assert_ok!(AnnualProfitTax::declare_annual_profit(
            RuntimeOrigin::signed(ORG),
            YEAR,
            1_000_000 * UNIT,
            false,
        ));

        assert_eq!(
            Balances::total_issuance(),
            before,
            "No ALTAN minted or burned — pure constitutional routing"
        );
    });
}
