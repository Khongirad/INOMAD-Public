#![cfg(feature = "runtime-benchmarks")]
use super::*;
use frame_benchmarking::v2::*;
use frame_support::{pallet_prelude::*, traits::Currency};
use frame_system::RawOrigin;

fn fund<T: Config>(who: &T::AccountId) {
    let amount: BalanceOf<T> = (1_000_000_000_000u128 * 1_000u128)
        .try_into()
        .unwrap_or_default();
    T::Currency::make_free_balance_be(who, amount);
}

fn setup_bank_account<T: Config>() -> T::AccountId {
    let bank: T::AccountId = account("bank", 0, 0);
    fund::<T>(&bank);
    BankSpecialAccount::<T>::put(bank.clone());
    bank
}

/// Seed a Locked (but not credit_requested) CollateralContract.
fn seed_collateral<T: Config>(citizen: &T::AccountId, bank: &T::AccountId) -> u32 {
    fund::<T>(citizen);
    // Transfer some funds to bank to simulate 10% fee
    let amount: BalanceOf<T> = 10_000_000_000u128.try_into().unwrap_or_default();
    T::Currency::transfer(
        citizen,
        bank,
        amount,
        frame_support::traits::ExistenceRequirement::KeepAlive,
    )
    .ok();

    let bank_fee = amount / BalanceOf::<T>::from(10u32);
    let collateral_net = if amount > bank_fee {
        amount - bank_fee
    } else {
        BalanceOf::<T>::zero()
    };
    let contract_id = NextCollateralId::<T>::get();
    CollateralContracts::<T>::insert(
        contract_id,
        CollateralContract::<T> {
            citizen: citizen.clone(),
            asset_type: AssetType::AltanCoin,
            collateral_amount: amount,
            bank_fee,
            collateral_net,
            created_at: 0u32,
            status: CollateralStatus::Locked,
            credit_requested: false,
        },
    );
    NextCollateralId::<T>::put(contract_id.saturating_add(1));
    contract_id
}

#[benchmarks]
mod benchmarks {
    use super::*;

    // set_bank_account(account) — Root
    #[benchmark]
    fn set_bank_account() {
        let bank: T::AccountId = account("bank", 0, 0);

        #[extrinsic_call]
        set_bank_account(RawOrigin::Root, bank.clone());

        assert_eq!(BankSpecialAccount::<T>::get(), Some(bank));
    }

    // assess_rwa(citizen, asset_type, value_in_altan) — Root
    #[benchmark]
    fn assess_rwa() {
        let citizen: T::AccountId = account("citizen", 0, 0);
        let value: BalanceOf<T> = 1_000_000_000u128.try_into().unwrap_or_default();

        #[extrinsic_call]
        assess_rwa(
            RawOrigin::Root,
            citizen.clone(),
            AssetType::AltanCoin,
            value,
        );
    }

    // lock_collateral(amount, asset_type) — Signed
    #[benchmark]
    fn lock_collateral() {
        let bank = setup_bank_account::<T>();
        let citizen: T::AccountId = whitelisted_caller();
        fund::<T>(&citizen);
        let amount: BalanceOf<T> = 10_000_000_000u128.try_into().unwrap_or_default();

        #[extrinsic_call]
        lock_collateral(
            RawOrigin::Signed(citizen.clone()),
            amount,
            AssetType::AltanCoin,
        );

        assert_eq!(NextCollateralId::<T>::get(), 1);
    }

    // request_credit(collateral_id) — Signed
    #[benchmark]
    fn request_credit() {
        let bank = setup_bank_account::<T>();
        let citizen: T::AccountId = whitelisted_caller();
        let contract_id = seed_collateral::<T>(&citizen, &bank);

        #[extrinsic_call]
        request_credit(RawOrigin::Signed(citizen.clone()), contract_id);

        let col = CollateralContracts::<T>::get(contract_id).unwrap();
        assert!(col.credit_requested);
    }

    // issue_credit(collateral_id) — Root
    #[benchmark]
    fn issue_credit() {
        let bank = setup_bank_account::<T>();
        let citizen: T::AccountId = account("citizen", 0, 0);
        let contract_id = seed_collateral::<T>(&citizen, &bank);
        // Mark as credit_requested
        CollateralContracts::<T>::try_mutate(contract_id, |maybe| -> Result<(), ()> {
            if let Some(c) = maybe {
                c.credit_requested = true;
            }
            Ok(())
        })
        .ok();

        #[extrinsic_call]
        issue_credit(RawOrigin::Root, contract_id);

        assert_eq!(NextCreditId::<T>::get(), 1);
    }

    impl_benchmark_test_suite!(Pallet, sp_io::TestExternalities::default(), T);
}
