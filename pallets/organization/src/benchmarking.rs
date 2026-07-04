#![cfg(feature = "runtime-benchmarks")]
use super::*;
use frame_benchmarking::v2::*;
use frame_support::pallet_prelude::*;
use frame_system::{pallet_prelude::BlockNumberFor, RawOrigin};

fn fund_org_bank<T: Config>(bank: &T::AccountId) {
    use frame_support::traits::Currency;
    let amount: BalanceOf<T> = (1_000_000_000_000u128 * 100_000u128)
        .try_into()
        .unwrap_or_default();
    T::Currency::make_free_balance_be(bank, amount);
}

fn seed_active_org<T: Config>(org_id: u32, director: &T::AccountId) -> T::AccountId {
    let bank = Pallet::<T>::derive_org_account(org_id);
    fund_org_bank::<T>(&bank);
    let now: BlockNumberFor<T> = 0u32.into();
    let name: BoundedVec<u8, T::MaxNameLength> =
        b"BenchOrg".to_vec().try_into().unwrap_or_default();
    Organizations::<T>::insert(
        org_id,
        Organization::<T> {
            name,
            hq_region: 1u32,
            status: OrgStatus::Active,
            bank_account_id: bank.clone(),
            last_tax_period_paid: now,
            registered_at: now,
        },
    );
    CrewMembers::<T>::insert(
        org_id,
        director,
        CrewMember::<T> {
            role: OrgRole::Director,
            region_assigned: 1u32,
            active_since: now,
        },
    );
    NextOrgId::<T>::put(org_id + 1);
    bank
}

#[benchmarks]
mod benchmarks {
    use super::*;

    // register_organization(name, hq_region, required_founders) — 3 args
    #[benchmark]
    fn register_organization() {
        let caller: T::AccountId = whitelisted_caller();

        #[extrinsic_call]
        register_organization(
            RawOrigin::Signed(caller.clone()),
            b"BenchOrg".to_vec(),
            1u32, // hq_region
            1u32, // required_founders
        );

        assert!(NextOrgId::<T>::get() > 0);
    }

    // add_crew_member(org_id, member, role, region_assigned) — 4 args
    #[benchmark]
    fn add_crew_member() {
        let director: T::AccountId = whitelisted_caller();
        let member: T::AccountId = account("member", 0, 0);
        let org_id: u32 = 0;
        seed_active_org::<T>(org_id, &director);

        #[extrinsic_call]
        add_crew_member(
            RawOrigin::Signed(director.clone()),
            org_id,
            member.clone(),
            OrgRole::Employee,
            1u32,
        );

        assert!(CrewMembers::<T>::contains_key(org_id, &member));
    }

    impl_benchmark_test_suite!(Pallet, sp_io::TestExternalities::default(), T);
}
