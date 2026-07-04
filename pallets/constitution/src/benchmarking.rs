#![cfg(feature = "runtime-benchmarks")]
use super::*;
use frame_benchmarking::v2::*;
use frame_support::pallet_prelude::*;
use frame_system::{pallet_prelude::BlockNumberFor, RawOrigin};

#[benchmarks]
mod benchmarks {
    use super::*;

    // ── 0. set_constitution_hash ──────────────────────────────────────────────
    #[benchmark]
    fn set_constitution_hash() {
        let cid: [u8; 46] = [1u8; 46];

        #[extrinsic_call]
        set_constitution_hash(RawOrigin::Root, cid);

        assert!(CoreRightsHash::<T>::get().is_some());
    }

    // ── 1. register_lockup ────────────────────────────────────────────────────
    #[benchmark]
    fn register_lockup() {
        let citizen: T::AccountId = account("citizen", 0, 0);
        let case_hash: [u8; 32] = [0u8; 32];
        let max_block: BlockNumberFor<T> = 100u32.into();

        #[extrinsic_call]
        register_lockup(RawOrigin::Root, citizen.clone(), max_block, case_hash);

        assert!(HabeasCorpusTimers::<T>::contains_key(&citizen));
    }

    // ── 2. resolve_habeas_corpus ──────────────────────────────────────────────
    #[benchmark]
    fn resolve_habeas_corpus() {
        let prisoner: T::AccountId = account("prisoner", 0, 0);
        let now = frame_system::Pallet::<T>::block_number();
        // Pre-insert a timer so the extrinsic can resolve it
        HabeasCorpusTimers::<T>::insert(
            &prisoner,
            HabeasCorpusTimer {
                max_lockup_block: now + 100u32.into(),
                registered_at: now,
                case_hash: [0u8; 32],
                resolved: false,
            },
        );

        #[extrinsic_call]
        resolve_habeas_corpus(RawOrigin::Root, prisoner.clone());

        assert!(!HabeasCorpusTimers::<T>::contains_key(&prisoner));
    }

    impl_benchmark_test_suite!(Pallet, sp_io::TestExternalities::default(), T);
}
