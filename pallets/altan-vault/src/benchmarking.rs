#![cfg(feature = "runtime-benchmarks")]
use super::*;
use frame_benchmarking::v2::*;
use frame_system::RawOrigin;
use sp_runtime::traits::Zero;

#[benchmarks]
mod benchmarks {
    use super::*;

    // ── 0. create_vault ───────────────────────────────────────────────────────
    // create_vault(origin, vault_index: u16) — Signed
    #[benchmark]
    fn create_vault() {
        let caller: T::AccountId = whitelisted_caller();
        let vault_index: u16 = 0;

        #[extrinsic_call]
        create_vault(RawOrigin::Signed(caller.clone()), vault_index);

        assert!(Vaults::<T>::contains_key(&caller, vault_index));
    }

    // ── 1. withdraw_to_owner ──────────────────────────────────────────────────
    // withdraw_to_owner(origin, vault_index: u16, amount: BalanceOf<T>) — Signed
    #[benchmark]
    fn withdraw_to_owner() {
        use frame_support::traits::Currency;
        let caller: T::AccountId = whitelisted_caller();
        let vault_index: u16 = 0;
        let amount: BalanceOf<T> = (1_000_000_000_000u128 * 100u128)
            .try_into()
            .unwrap_or_default();

        // Pre-seed the VaultInfo record
        let now = frame_system::Pallet::<T>::block_number();
        Vaults::<T>::insert(
            &caller,
            vault_index,
            VaultInfo {
                owner: caller.clone(),
                vault_index,
                created_at: now,
                total_deposited: amount,
                total_withdrawn: Zero::zero(),
            },
        );
        VaultCount::<T>::insert(&caller, 1u16);

        // Fund the deterministic vault sub-account
        let vault_account = Pallet::<T>::vault_account(&caller, vault_index);
        <T as Config>::Currency::make_free_balance_be(&vault_account, amount);

        let withdraw_amount: BalanceOf<T> = (1_000_000_000_000u128 * 10u128)
            .try_into()
            .unwrap_or_default();

        #[extrinsic_call]
        withdraw_to_owner(
            RawOrigin::Signed(caller.clone()),
            vault_index,
            withdraw_amount,
        );
    }

    // ── 2. record_inbound ─────────────────────────────────────────────────────
    // record_inbound(origin, owner: T::AccountId, vault_index: u16, amount: BalanceOf<T>)
    #[benchmark]
    fn record_inbound() {
        use frame_support::traits::Currency;
        let owner: T::AccountId = account("owner", 0, 0);
        let sender: T::AccountId = whitelisted_caller();
        let vault_index: u16 = 0;
        let amount: BalanceOf<T> = (1_000_000_000_000u128 * 10u128)
            .try_into()
            .unwrap_or_default();

        // Pre-seed vault
        let now = frame_system::Pallet::<T>::block_number();
        Vaults::<T>::insert(
            &owner,
            vault_index,
            VaultInfo {
                owner: owner.clone(),
                vault_index,
                created_at: now,
                total_deposited: Zero::zero(),
                total_withdrawn: Zero::zero(),
            },
        );
        VaultCount::<T>::insert(&owner, 1u16);
        <T as Config>::Currency::make_free_balance_be(&sender, amount);

        #[extrinsic_call]
        record_inbound(
            RawOrigin::Signed(sender.clone()),
            owner.clone(),
            vault_index,
            amount,
        );

        let vault = Vaults::<T>::get(&owner, vault_index).expect("vault exists");
        assert!(vault.total_deposited > Zero::zero());
    }

    impl_benchmark_test_suite!(Pallet, sp_io::TestExternalities::default(), T);
}
