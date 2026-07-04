#![cfg(feature = "runtime-benchmarks")]
use super::*;
use frame_benchmarking::v2::*;
use frame_support::pallet_prelude::*;
use frame_system::RawOrigin;

fn fund<T: Config>(who: &T::AccountId) {
    use frame_support::traits::Currency;
    let amount = (1_000_000_000_000u128 * 100_000u128)
        .try_into()
        .unwrap_or_default();
    <T as Config>::Currency::make_free_balance_be(who, amount);
}

#[benchmarks(
    where
        T::NftIssuerOrigin: frame_support::traits::EnsureOriginWithArg<T::RuntimeOrigin, (), Success = T::AccountId>
)]
mod benchmarks {
    use super::*;

    // award_achievement(origin, holder, kind, event_ref)
    #[benchmark]
    fn award_achievement() {
        let issuer: T::AccountId = whitelisted_caller();
        fund::<T>(&issuer);
        let holder: T::AccountId = account("holder", 0, 0);

        #[extrinsic_call]
        award_achievement(
            RawOrigin::Root, // NftIssuerOrigin maps Root in mock
            holder.clone(),
            AchievementKind::Verifier,
            None,
        );

        assert!(NextAchievementId::<T>::get() > 0);
    }

    // issue_reward_nft(origin, holder, kind, altan_amount, event_ref)
    #[benchmark]
    fn issue_reward_nft() {
        let treasury = T::ConfederationTreasury::get();
        fund::<T>(&treasury);
        let holder: T::AccountId = account("holder", 0, 0);
        let reward: BalanceOf<T> = (1_000_000_000_000u128 * 10u128)
            .try_into()
            .unwrap_or_default();

        #[extrinsic_call]
        issue_reward_nft(
            RawOrigin::Root,
            holder.clone(),
            AchievementKind::VerifierReward,
            reward,
            None,
        );

        assert!(NextAchievementId::<T>::get() > 0);
    }

    // redeem_reward_nft(origin, token_id)
    #[benchmark]
    fn redeem_reward_nft() {
        let holder: T::AccountId = whitelisted_caller();
        let treasury = T::ConfederationTreasury::get();
        fund::<T>(&treasury);
        let altan: BalanceOf<T> = (1_000_000_000_000u128 * 10u128)
            .try_into()
            .unwrap_or_default();

        // Reserve ALTAN in treasury (mirrors issue_reward_nft)
        use frame_support::traits::ReservableCurrency;
        T::Currency::reserve(&treasury, altan).ok();

        let token_id: u64 = 0;
        NextAchievementId::<T>::put(1u64);
        Achievements::<T>::insert(
            token_id,
            AchievementToken::<
                T::AccountId,
                frame_system::pallet_prelude::BlockNumberFor<T>,
                BalanceOf<T>,
            > {
                token_id,
                holder: holder.clone(),
                category: AchievementCategory::Reward,
                kind: AchievementKind::VerifierReward,
                altan_value: altan,
                issued_at: 0u32.into(),
                redeemed: false,
                issued_by: holder.clone(),
                event_ref: None,
            },
        );

        #[extrinsic_call]
        redeem_reward_nft(RawOrigin::Signed(holder.clone()), token_id);

        let token = Achievements::<T>::get(token_id).expect("exists");
        assert!(token.redeemed);
    }

    impl_benchmark_test_suite!(Pallet, sp_io::TestExternalities::default(), T);
}
