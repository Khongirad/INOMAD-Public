#![cfg(feature = "runtime-benchmarks")]
use super::*;
use frame_benchmarking::v2::*;
use frame_support::pallet_prelude::*;
use frame_system::RawOrigin;
use sp_core::H256;

fn fund<T: Config>(who: &T::AccountId) {
    use frame_support::traits::Currency;
    let amount = (1_000_000_000_000u128 * 100_000u128)
        .try_into()
        .unwrap_or_default();
    <T as Config>::Currency::make_free_balance_be(who, amount);
}

/// Seed a fugitive in WallOfShame + open bounty pool.
fn seed_fugitive<T: Config>(target: &T::AccountId) {
    fund::<T>(target); // bounty pool = target account
    WallOfShame::<T>::insert(
        target,
        CrimeRecord {
            category: CrimeCategory::HighTreason,
            verdict_hash: H256::zero(),
            timestamp: 0u32,
            fugitive_status: Some(FugitiveStatus::AtLarge),
        },
    );
    let bounty: BalanceOf<T> = (1_000_000_000_000u128 * 10u128)
        .try_into()
        .unwrap_or_default();
    BountyPools::<T>::insert(target, bounty);
}

#[benchmarks]
mod benchmarks {
    use super::*;

    // condemn_and_issue_warrant(target, category, verdict_hash, is_fugitive, initial_bounty)
    #[benchmark]
    fn condemn_and_issue_warrant() {
        let target: T::AccountId = account("target", 0, 0);
        fund::<T>(&target);
        let treasury = T::StateTreasury::get();
        fund::<T>(&treasury);
        let verdict_hash = H256::zero();
        let bounty: BalanceOf<T> = (1_000_000_000_000u128 * 1u128)
            .try_into()
            .unwrap_or_default();
        #[extrinsic_call]
        condemn_and_issue_warrant(
            RawOrigin::Root,
            target.clone(),
            CrimeCategory::HighTreason,
            verdict_hash,
            false, // not fugitive — avoids treasury transfer
            bounty,
        );
        assert!(WallOfShame::<T>::contains_key(&target));
    }

    // donate_to_bounty(target_fugitive, amount)
    #[benchmark]
    fn donate_to_bounty() {
        let donor: T::AccountId = whitelisted_caller();
        fund::<T>(&donor);
        let fugitive: T::AccountId = account("fugitive", 0, 0);
        seed_fugitive::<T>(&fugitive);
        let donation: BalanceOf<T> = (1_000_000_000_000u128 * 5u128)
            .try_into()
            .unwrap_or_default();
        #[extrinsic_call]
        donate_to_bounty(RawOrigin::Signed(donor.clone()), fugitive.clone(), donation);
    }

    // register_capture_and_payout(target_fugitive, bounty_hunter)
    #[benchmark]
    fn register_capture_and_payout() {
        let hunter: T::AccountId = account("hunter", 0, 0);
        fund::<T>(&hunter);
        let fugitive: T::AccountId = account("fugitive", 0, 0);
        seed_fugitive::<T>(&fugitive);
        #[extrinsic_call]
        register_capture_and_payout(RawOrigin::Root, fugitive.clone(), hunter.clone());
        assert!(!BountyPools::<T>::contains_key(&fugitive));
        assert!(LockedBountyPayouts::<T>::contains_key(&fugitive));
    }

    // claim_bounty_payout(target_fugitive)
    #[benchmark]
    fn claim_bounty_payout() {
        let hunter: T::AccountId = account("hunter", 0, 0);
        fund::<T>(&hunter);
        let fugitive: T::AccountId = account("fugitive", 0, 0);
        fund::<T>(&fugitive); // holds the locked funds
        let amount: BalanceOf<T> = (1_000_000_000_000u128 * 5u128)
            .try_into()
            .unwrap_or_default();
        // Pre-seed a matured payout (unlock_block = 0 → always past)
        LockedBountyPayouts::<T>::insert(
            &fugitive,
            LockedPayout::<T> {
                bounty_hunter: hunter.clone(),
                unlock_block: 0u32,
                amount,
            },
        );
        let caller: T::AccountId = whitelisted_caller();
        #[extrinsic_call]
        claim_bounty_payout(RawOrigin::Signed(caller.clone()), fugitive.clone());
        assert!(!LockedBountyPayouts::<T>::contains_key(&fugitive));
    }

    // cancel_bounty_payout(target_fugitive)
    #[benchmark]
    fn cancel_bounty_payout() {
        let hunter: T::AccountId = account("hunter", 0, 0);
        let fugitive: T::AccountId = account("fugitive", 0, 0);
        let amount: BalanceOf<T> = (1_000_000_000_000u128 * 5u128)
            .try_into()
            .unwrap_or_default();
        LockedBountyPayouts::<T>::insert(
            &fugitive,
            LockedPayout::<T> {
                bounty_hunter: hunter.clone(),
                unlock_block: 999_999u32,
                amount,
            },
        );
        #[extrinsic_call]
        cancel_bounty_payout(RawOrigin::Root, fugitive.clone());
        assert!(!LockedBountyPayouts::<T>::contains_key(&fugitive));
    }

    impl_benchmark_test_suite!(Pallet, sp_io::TestExternalities::default(), T);
}
