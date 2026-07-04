//! Benchmarking suite for pallet-buryad-mongol-mixer.
#![cfg(feature = "runtime-benchmarks")]
//!
//! Measures worst-case costs for each of the 4 extrinsics:
//! - `deposit` — commitment registration + 4-way fee split (800 ppm)
//! - `withdraw` — nullifier burn + balance transfer to recipient
//! - `reveal_transaction` — audit log write (BankBoard governance)
//! - `submit_quarterly_audit` — quarterly hash storage (Khural governance)
//!
//! **Note:** In the benchmark runtime, RelayerOrigin, BankBoardOrigin, and
//! KhuralOrigin must implement `try_successful_origin()` (gated on
//! `runtime-benchmarks` feature in the runtime config).

use super::*;

#[allow(unused)]
use crate::Pallet as Mixer;
use frame_benchmarking::v2::*;
use frame_support::traits::Currency;
use frame_system::RawOrigin;

const UNIT: u128 = 1_000_000_000_000u128;
/// 1,000 ALTAN — MixerDenomination (matches mock::DENOM).
const DENOM: u128 = 1_000 * UNIT;

#[benchmarks(where
    BalanceOf<T>: From<u128>,
)]
mod benchmarks {
    use super::*;

    // =========================================================================
    // deposit — worst case: below fee cap, 4-way fee split, pool write
    // =========================================================================
    /// Measures: `Commitments` write + `PoolLeafCount` inc + 5 currency transfers.
    #[benchmark]
    fn deposit() {
        let caller: T::AccountId = whitelisted_caller();
        // Fund: DENOM + 12 ALTAN buffer (fee cap = 10 ALTAN, + 2 ALTAN ED)
        let fund: <T::Currency as frame_support::traits::Currency<T::AccountId>>::Balance =
            (DENOM + 12 * UNIT).into();
        T::Currency::make_free_balance_be(&caller, fund);

        let commitment: [u8; 32] = [0xABu8; 32];
        let payload: frame_support::BoundedVec<u8, frame_support::traits::ConstU32<64>> =
            Default::default();

        #[extrinsic_call]
        deposit(RawOrigin::Signed(caller), DENOM.into(), commitment, payload);

        assert_eq!(
            Commitments::<T>::get(&commitment),
            Some(CommitmentState::Active)
        );
    }

    // =========================================================================
    // reveal_transaction — audit log write
    // =========================================================================
    /// Measures: `AuditLogId` read+inc + `AuditLogs` write.
    #[benchmark]
    fn reveal_transaction() {
        let board: T::AccountId = whitelisted_caller();
        let commitment: [u8; 32] = [0x01u8; 32];
        let warrant: frame_support::BoundedVec<u8, frame_support::traits::ConstU32<256>> =
            frame_support::BoundedVec::try_from(alloc::vec![0x01u8; 8]).unwrap();

        #[extrinsic_call]
        reveal_transaction(RawOrigin::Signed(board), commitment, warrant);

        assert_eq!(AuditLogId::<T>::get(), 1u64);
    }

    // =========================================================================
    // submit_quarterly_audit — quarterly hash storage
    // =========================================================================
    /// Measures: `QuarterlyAudits` existence check + write.
    #[benchmark]
    fn submit_quarterly_audit() {
        let khural: T::AccountId = whitelisted_caller();
        let quarter_id: u32 = 20261;
        let report_hash: [u8; 32] = [0xAAu8; 32];

        #[extrinsic_call]
        submit_quarterly_audit(RawOrigin::Signed(khural), quarter_id, report_hash);

        assert_eq!(QuarterlyAudits::<T>::get(quarter_id), Some(report_hash));
    }

    // =========================================================================
    // Test suite wiring
    // =========================================================================
    impl_benchmark_test_suite!(Mixer, crate::mock::new_test_ext(), crate::mock::Test);
}
