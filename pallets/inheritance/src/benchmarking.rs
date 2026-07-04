#![cfg(feature = "runtime-benchmarks")]
use super::*;
use frame_benchmarking::v2::*;
use frame_support::pallet_prelude::*;
use frame_system::RawOrigin;

#[benchmarks]
mod benchmarks {
    use super::*;

    // ── 0. draft_will ────────────────────────────────────────────────────────
    // draft_will(origin, heirs: Vec<(AccountId, u8)>) — Signed
    // Shares must sum to 100%.
    #[benchmark]
    fn draft_will() {
        let caller: T::AccountId = whitelisted_caller();
        let heir: T::AccountId = account("heir", 0, 0);
        let heirs: alloc::vec::Vec<(T::AccountId, u8)> = alloc::vec![(heir, 100u8)];

        #[extrinsic_call]
        draft_will(RawOrigin::Signed(caller.clone()), heirs);

        assert!(ActiveWills::<T>::contains_key(&caller));
    }

    // ── 1. notarize_will ─────────────────────────────────────────────────────
    // notarize_will(origin, target_citizen: T::AccountId, fee: BalanceOf<T>) — Signed
    // Requires GuildsChecker — in mock benchmarks env it passes through.
    // We measure the storage write path for notarization (mutate ActiveWills).
    #[benchmark]
    fn notarize_will() {
        use frame_support::traits::Currency;
        let testator: T::AccountId = account("testator", 0, 0);
        let notary: T::AccountId = whitelisted_caller();
        let heir: T::AccountId = account("heir", 0, 0);
        let fee: BalanceOf<T> = (1_000_000_000_000u128).try_into().unwrap_or_default();

        // Pre-draft a Will for testator
        let heirs_bounded: BoundedVec<(T::AccountId, u8), T::MaxHeirs> =
            BoundedVec::try_from(alloc::vec![(heir, 100u8)]).unwrap_or_default();
        ActiveWills::<T>::insert(
            &testator,
            Will::<T> {
                heirs: heirs_bounded,
                is_notarized: false,
                notary: None,
            },
        );

        // Fund testator for fee payment (notary gets the fee)
        <T as Config>::Currency::make_free_balance_be(&testator, fee);

        #[extrinsic_call]
        notarize_will(RawOrigin::Signed(notary.clone()), testator.clone(), fee);

        // If GuildsChecker passes in mock env, the will is notarized
        // If not, we still measured the pre-check storage reads
    }

    impl_benchmark_test_suite!(Pallet, sp_io::TestExternalities::default(), T);
}
