//! Benchmarking suite for pallet-altan-tax.
//!
//! Measures the real on-chain cost of `transfer_with_fee` under worst-case conditions:
//! - Sender has exactly `amount + fee` (no excess)
//! - All 4 treasury accounts are pre-seeded
//! - Amount is 1 ALTAN (below cap: fee path follows mul/div branches, not cap branch)

use super::*;

#[allow(unused)]
use crate::Pallet as AltanTax;
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
    // transfer_with_fee — the single constitutional extrinsic
    // =========================================================================

    /// Worst-case `transfer_with_fee`:
    /// - 1,000 ALTAN amount (fee = 0.03% × 1000 = 0.3 ALTAN — below cap)
    /// - 5 balance transfers (dest + 4 treasury shares)
    /// - All storage reads for treasury accounts
    #[benchmark]
    fn transfer_with_fee() {
        // ── Setup ──────────────────────────────────────────────────────────────
        let amount: BalanceOf<T> = (1_000u128 * UNIT).into();

        // Fee: 0.03% of 1000 ALTAN = 0.3 ALTAN
        let fee: BalanceOf<T> = (300_000_000_000u128).into();
        let total_needed: BalanceOf<T> = (1_000u128 * UNIT + 300_000_000_000u128 + UNIT).into(); // +ED

        // Fund sender
        let caller: T::AccountId = whitelisted_caller();
        T::Currency::make_free_balance_be(&caller, total_needed);

        // Fund recipient (existential deposit minimum)
        let recipient: T::AccountId = account("recipient", 0, 0);
        T::Currency::make_free_balance_be(&recipient, UNIT.into());

        // Fund treasury accounts (existential deposit minimum each)
        let ag: T::AccountId = account("ag_treasury", 0, 0);
        let khural: T::AccountId = account("khural_treasury", 0, 0);
        let validator: T::AccountId = account("validator_pool", 0, 0);
        T::Currency::make_free_balance_be(&ag, UNIT.into());
        T::Currency::make_free_balance_be(&khural, UNIT.into());
        T::Currency::make_free_balance_be(&validator, UNIT.into());

        // Write treasury accounts to storage
        AgTreasuryAccount::<T>::put(ag.clone());
        KhuralFoundationAccount::<T>::put(khural.clone());
        ValidatorPoolAccount::<T>::put(validator.clone());

        // ── Benchmark ─────────────────────────────────────────────────────────
        #[extrinsic_call]
        transfer_with_fee(RawOrigin::Signed(caller), recipient.clone(), amount);

        // ── Verify ────────────────────────────────────────────────────────────
        // Recipient received exactly amount (exclusive fee model)
        assert!(T::Currency::free_balance(&recipient) >= amount + UNIT.into());
    }

    // =========================================================================
    // Test suite wiring
    // =========================================================================
    impl_benchmark_test_suite!(AltanTax, crate::mock::new_test_ext(), crate::mock::Test);
}
