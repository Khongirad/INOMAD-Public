//! Benchmarking suite for pallet-annual-profit-tax.
//!
//! Measures the real on-chain cost of each extrinsic under worst-case conditions:
//!
//! | Benchmark | Scenario |
//! |---|---|
//! | `declare_annual_profit_standard` | 10% rate, 1,000,000 ALTAN profit, on-time filing |
//! | `declare_annual_profit_large_family` | 5% rate, 1,000,000 ALTAN profit, on-time filing |
//! | `set_regional_treasury` | Root updates regional treasury account |
//! | `set_confederation_treasury` | Root updates confederation treasury account |
//!
//! All benchmarks pre-seed treasury accounts and fund the caller with exactly
//! the minimum required balance (profit + tax + ED) to simulate real-world storage
//! read/write patterns.

use super::*;

#[allow(unused)]
use crate::Pallet as AnnualProfitTax;
use frame_benchmarking::v2::*;
use frame_support::traits::Currency;
use frame_system::RawOrigin;

/// 1 ALTAN in planck (10^12).
const UNIT: u128 = 1_000_000_000_000u128;

#[benchmarks(
    where
        T: Config,
        BalanceOf<T>: From<u128>,
)]
mod benchmarks {
    use super::*;

    // =========================================================================
    // declare_annual_profit — standard rate (10%), on-time
    // =========================================================================

    /// Worst-case `declare_annual_profit` with the standard 10% rate, on-time filing.
    ///
    /// Scenario:
    ///   - declared_profit = 1,000,000 ALTAN
    ///   - base_tax        = 100,000 ALTAN (10%)
    ///   - no штраф / пени (on-time)
    ///   - 2 treasury transfers (regional 70% + confederation 30%)
    ///   - 1 storage write (ProfitDeclarations DoubleMap)
    ///   - 2 storage reads (RegionalTreasury + ConfederationTreasury)
    #[benchmark]
    fn declare_annual_profit_standard() {
        // ── Seed treasury accounts ─────────────────────────────────────────
        let regional: T::AccountId = account("regional_treasury", 0, 0);
        T::Currency::make_free_balance_be(&regional, UNIT.into());
        RegionalTreasury::<T>::put(&regional);

        let confederation: T::AccountId = account("conf_treasury", 0, 0);
        T::Currency::make_free_balance_be(&confederation, UNIT.into());
        ConfederationTreasury::<T>::put(&confederation);

        // ── Fund caller ────────────────────────────────────────────────────
        // 1,000,000 ALTAN profit + 100,000 ALTAN tax (10%) + 1 ALTAN ED
        let total_needed: BalanceOf<T> = (1_100_001u128 * UNIT).into();
        let caller: T::AccountId = whitelisted_caller();
        T::Currency::make_free_balance_be(&caller, total_needed);

        let profit: BalanceOf<T> = (1_000_000u128 * UNIT).into();
        let tax_year: TaxYear = 2026;

        #[extrinsic_call]
        declare_annual_profit(
            RawOrigin::Signed(caller.clone()),
            tax_year,
            profit,
            false, // standard rate — not large family
        );

        // ── Verify storage written ─────────────────────────────────────────
        assert!(ProfitDeclarations::<T>::get(&caller, tax_year).is_some());
    }

    // =========================================================================
    // declare_annual_profit — large family rate (5%), on-time
    // =========================================================================

    /// `declare_annual_profit` with the large-family 5% rate, on-time filing.
    ///
    /// Scenario:
    ///   - declared_profit = 1,000,000 ALTAN
    ///   - base_tax        = 50,000 ALTAN (5%)
    ///   - no штраф / пени (on-time)
    ///   - same storage access pattern as standard variant
    #[benchmark]
    fn declare_annual_profit_large_family() {
        // ── Seed treasury accounts ─────────────────────────────────────────
        let regional: T::AccountId = account("regional_treasury", 0, 1);
        T::Currency::make_free_balance_be(&regional, UNIT.into());
        RegionalTreasury::<T>::put(&regional);

        let confederation: T::AccountId = account("conf_treasury", 0, 1);
        T::Currency::make_free_balance_be(&confederation, UNIT.into());
        ConfederationTreasury::<T>::put(&confederation);

        // ── Fund caller ────────────────────────────────────────────────────
        // 1,000,000 ALTAN profit + 50,000 ALTAN tax (5%) + 1 ALTAN ED
        let total_needed: BalanceOf<T> = (1_050_001u128 * UNIT).into();
        let caller: T::AccountId = whitelisted_caller();
        T::Currency::make_free_balance_be(&caller, total_needed);

        let profit: BalanceOf<T> = (1_000_000u128 * UNIT).into();
        let tax_year: TaxYear = 2026;

        #[extrinsic_call]
        declare_annual_profit(
            RawOrigin::Signed(caller.clone()),
            tax_year,
            profit,
            true, // large family — 5% reduced rate
        );

        // ── Verify storage written ─────────────────────────────────────────
        assert!(ProfitDeclarations::<T>::get(&caller, tax_year).is_some());
    }

    // =========================================================================
    // set_regional_treasury — Root call
    // =========================================================================

    /// Worst-case `set_regional_treasury`: Root updates the regional treasury account.
    ///
    /// Storage: 1 write (RegionalTreasury StorageValue).
    #[benchmark]
    fn set_regional_treasury() {
        let new_treasury: T::AccountId = account("new_regional", 0, 0);
        T::Currency::make_free_balance_be(&new_treasury, UNIT.into());

        #[extrinsic_call]
        set_regional_treasury(RawOrigin::Root, new_treasury.clone());

        assert_eq!(RegionalTreasury::<T>::get(), Some(new_treasury));
    }

    // =========================================================================
    // set_confederation_treasury — Root call
    // =========================================================================

    /// Worst-case `set_confederation_treasury`: Root updates the confederation treasury.
    ///
    /// Storage: 1 write (ConfederationTreasury StorageValue).
    #[benchmark]
    fn set_confederation_treasury() {
        let new_treasury: T::AccountId = account("new_conf", 0, 0);
        T::Currency::make_free_balance_be(&new_treasury, UNIT.into());

        #[extrinsic_call]
        set_confederation_treasury(RawOrigin::Root, new_treasury.clone());

        assert_eq!(ConfederationTreasury::<T>::get(), Some(new_treasury));
    }

    impl_benchmark_test_suite!(
        AnnualProfitTax,
        crate::mock::new_test_ext(),
        crate::mock::Test,
    );
}
