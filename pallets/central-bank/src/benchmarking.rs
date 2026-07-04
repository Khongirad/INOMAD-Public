//! Benchmarking suite for pallet-central-bank.
//!
//! Measures worst-case costs for each of the 8 extrinsics.
//!
//! **Note for runtime integration:** `BankingOrigin` in the benchmark runtime
//! must be configured as `EnsureRoot` so that `RawOrigin::Root` passes all
//! `T::BankingOrigin::ensure_origin()` guards. This is the standard FRAME
//! benchmark pattern for governance-gated extrinsics.

use super::*;

#[allow(unused)]
use crate::Pallet as CentralBank;
use frame_benchmarking::v2::*;
use frame_support::traits::Currency;
use frame_system::RawOrigin;

const UNIT: u128 = 1_000_000_000_000u128;

use sp_runtime::traits::Saturating;

#[benchmarks(where BalanceOf<T>: From<u128> + Saturating + PartialOrd + Copy)]
mod benchmarks {
    use super::*;

    // =========================================================================
    // grant_operator_license — storage write to LicensedOperators
    // =========================================================================

    /// Worst case: operator is NOT yet licensed (requires storage write).
    #[benchmark]
    fn grant_operator_license() {
        let operator: T::AccountId = account("operator", 0, 0);

        #[extrinsic_call]
        grant_operator_license(RawOrigin::Root, operator.clone());

        assert!(LicensedOperators::<T>::get(&operator));
    }

    // =========================================================================
    // revoke_operator_license — storage update in LicensedOperators
    // =========================================================================

    /// Worst case: operator IS licensed (requires read + write).
    #[benchmark]
    fn revoke_operator_license() {
        let operator: T::AccountId = account("operator", 0, 0);
        LicensedOperators::<T>::insert(&operator, true);

        #[extrinsic_call]
        revoke_operator_license(RawOrigin::Root, operator.clone());

        assert!(!LicensedOperators::<T>::get(&operator));
    }

    // =========================================================================
    // mint_to_operator — balance creation + counter increment + storage write
    // =========================================================================

    /// Worst case: operator is licensed, minting non-zero amount.
    /// Measures `deposit_creating` + `NextTrancheId` + `TotalEmitted` mutations.
    #[benchmark]
    fn mint_to_operator() {
        let operator: T::AccountId = account("operator", 0, 0);
        LicensedOperators::<T>::insert(&operator, true);
        let amount: BalanceOf<T> = (100u128 * UNIT).into();

        #[extrinsic_call]
        mint_to_operator(RawOrigin::Root, operator.clone(), amount);

        assert_eq!(TotalEmitted::<T>::get(), amount);
        assert_eq!(NextTrancheId::<T>::get(), 1u32);
    }

    // =========================================================================
    // burn — balance withdrawal + TotalBurned update
    // =========================================================================

    /// Worst case: operator has sufficient balance to burn.
    #[benchmark]
    fn burn() {
        let operator: T::AccountId = account("operator", 0, 0);
        LicensedOperators::<T>::insert(&operator, true);
        let amount: BalanceOf<T> = (10u128 * UNIT).into();
        T::Currency::make_free_balance_be(&operator, (100u128 * UNIT).into());

        #[extrinsic_call]
        burn(RawOrigin::Root, operator.clone(), amount);

        assert_eq!(TotalBurned::<T>::get(), amount);
    }

    // =========================================================================
    // set_current_rate — epoch read + mutate
    // =========================================================================

    /// Worst case: epoch exists in storage (requires read + write to Epochs map).
    #[benchmark]
    fn set_current_rate() {
        // Epoch 1 is created in genesis_build; ensure it exists
        let epoch_id = CurrentEpochId::<T>::get();
        assert!(
            Epochs::<T>::contains_key(epoch_id),
            "epoch must exist for benchmark"
        );

        #[extrinsic_call]
        set_current_rate(RawOrigin::Root, 1000u32);

        let epoch = Epochs::<T>::get(epoch_id).unwrap();
        assert_eq!(epoch.key_rate, 1000u32);
    }

    // =========================================================================
    // set_next_epoch_rate — single StorageValue write
    // =========================================================================

    /// Cheapest governance extrinsic — single StorageValue put.
    #[benchmark]
    fn set_next_epoch_rate() {
        #[extrinsic_call]
        set_next_epoch_rate(RawOrigin::Root, 900u32);

        assert_eq!(NextEpochKeyRate::<T>::get(), 900u32);
    }

    // =========================================================================
    // request_credit — mint to citizen + epoch write + CitizenDebt write
    // =========================================================================

    /// Worst case without epoch rollover: single-epoch credit issuance.
    /// Measures: `deposit_creating` + 2 storage mutations (Epochs, CitizenDebt).
    #[benchmark]
    fn request_credit() {
        let caller: T::AccountId = whitelisted_caller();
        let amount: BalanceOf<T> = (1u128 * UNIT).into();

        // Ensure epoch 1 is initialized
        let epoch_id = CurrentEpochId::<T>::get();
        assert!(Epochs::<T>::contains_key(epoch_id));

        #[extrinsic_call]
        request_credit(RawOrigin::Signed(caller.clone()), amount);

        assert_eq!(CitizenDebt::<T>::get(&caller, epoch_id), amount);
    }

    // =========================================================================
    // repay_credit — balance burn + CitizenDebt reduce + Epochs total_repaid
    // =========================================================================

    /// Worst case: citizen has debt and sufficient balance.
    /// Measures: 1 balance withdrawal + 2 storage mutations.
    #[benchmark]
    fn repay_credit() {
        let caller: T::AccountId = whitelisted_caller();
        let amount: BalanceOf<T> = (1u128 * UNIT).into();
        T::Currency::make_free_balance_be(&caller, (10u128 * UNIT).into());

        let epoch_id = CurrentEpochId::<T>::get();
        CitizenDebt::<T>::insert(&caller, epoch_id, amount);
        Epochs::<T>::mutate(epoch_id, |maybe| {
            if let Some(e) = maybe {
                e.total_issued = e.total_issued.saturating_add(amount);
            }
        });

        #[extrinsic_call]
        repay_credit(RawOrigin::Signed(caller.clone()), epoch_id, amount);

        assert_eq!(
            CitizenDebt::<T>::get(&caller, epoch_id),
            BalanceOf::<T>::default()
        );
    }

    // =========================================================================
    // Test suite wiring
    // =========================================================================
    impl_benchmark_test_suite!(CentralBank, crate::mock::new_test_ext(), crate::mock::Test);
}
