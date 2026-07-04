#![cfg(feature = "runtime-benchmarks")]
use super::*;
use frame_benchmarking::v2::*;
use frame_support::pallet_prelude::*;
use frame_system::RawOrigin;

fn fund<T: Config>(who: &T::AccountId) {
    use frame_support::traits::Currency;
    let amount: BalanceOf<T> = (1_000_000_000_000u128 * 100_000u128)
        .try_into()
        .unwrap_or_default();
    T::Currency::make_free_balance_be(who, amount);
}

fn fresh_commitment(seed: u8) -> [u8; 32] {
    let mut c = [0u8; 32];
    c[0] = seed;
    c
}

fn fresh_nullifier(seed: u8) -> [u8; 32] {
    let mut n = [1u8; 32];
    n[0] = seed;
    n
}

#[benchmarks]
mod benchmarks {
    use super::*;

    // shield_funds(amount, commitment, org_id) — Currency-based, no TransparentStateGuard
    #[benchmark]
    fn shield_funds() {
        let caller: T::AccountId = whitelisted_caller();
        fund::<T>(&caller);
        let amount: BalanceOf<T> = (1_000_000_000_000u128 * 10u128)
            .try_into()
            .unwrap_or_default();
        let commitment = fresh_commitment(1u8);

        #[extrinsic_call]
        shield_funds(RawOrigin::Signed(caller.clone()), amount, commitment, None);

        assert!(ShieldedCommitments::<T>::get(&commitment));
    }

    // shielded_transfer(nullifier, input_commitment, new_commitment, org_id)
    #[benchmark]
    fn shielded_transfer() {
        let caller: T::AccountId = whitelisted_caller();
        let input_commitment = fresh_commitment(2u8);
        let new_commitment = fresh_commitment(3u8);
        let nullifier = fresh_nullifier(2u8);

        // Seed: input commitment active
        ShieldedCommitments::<T>::insert(&input_commitment, true);
        TotalShielded::<T>::put(10_000_000_000_000u128);

        #[extrinsic_call]
        shielded_transfer(
            RawOrigin::Signed(caller.clone()),
            nullifier,
            input_commitment,
            new_commitment,
            None,
        );

        assert!(ShieldedCommitments::<T>::get(&new_commitment));
        assert!(!ShieldedCommitments::<T>::get(&input_commitment));
    }

    // unshield_to_account(nullifier, input_commitment, amount, recipient, org_id)
    #[benchmark]
    fn unshield_to_account() {
        let caller: T::AccountId = whitelisted_caller();
        let recipient: T::AccountId = account("recipient", 0, 0);
        let input_commitment = fresh_commitment(4u8);
        let nullifier = fresh_nullifier(4u8);
        let amount: u128 = 1_000_000_000_000u128;

        ShieldedCommitments::<T>::insert(&input_commitment, true);
        TotalShielded::<T>::put(amount * 2);

        #[extrinsic_call]
        unshield_to_account(
            RawOrigin::Signed(caller.clone()),
            nullifier,
            input_commitment,
            amount,
            recipient.clone(),
            None,
        );

        assert!(!ShieldedCommitments::<T>::get(&input_commitment));
    }

    // org_unshield_tax_payment(org_id, nullifier, input_commitment, amount_claimed)
    #[benchmark]
    fn org_unshield_tax_payment() {
        let caller: T::AccountId = whitelisted_caller();
        let org_id: u32 = 1;
        let input_commitment = fresh_commitment(5u8);
        let nullifier = fresh_nullifier(5u8);
        let amount: u128 = 1_000_000_000_000u128;

        ShieldedCommitments::<T>::insert(&input_commitment, true);
        OrgVaultBalance::<T>::insert(org_id, amount * 2);
        TotalShielded::<T>::put(amount * 2);

        #[extrinsic_call]
        org_unshield_tax_payment(
            RawOrigin::Signed(caller.clone()),
            org_id,
            nullifier,
            input_commitment,
            amount,
        );

        assert!(!ShieldedCommitments::<T>::get(&input_commitment));
        assert!(SpentNullifiers::<T>::contains_key(&nullifier));
    }

    impl_benchmark_test_suite!(Pallet, sp_io::TestExternalities::default(), T);
}
