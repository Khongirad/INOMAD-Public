#![cfg(feature = "runtime-benchmarks")]
use super::*;
use frame_benchmarking::v2::*;
use frame_support::pallet_prelude::*;
use frame_system::RawOrigin;

fn fund<T: Config>(who: &T::AccountId) {
    use frame_support::traits::Currency as _;
    let amount: BalanceOf<T> = (1_000_000_000_000u128 * 100_000u128)
        .try_into()
        .unwrap_or_default();
    <T as Config>::Currency::make_free_balance_be(who, amount);
}

#[benchmarks]
mod benchmarks {
    use super::*;

    // authorize_ministry(ministry, tag, nation_id) — 3 args, Root
    #[benchmark]
    fn authorize_ministry() {
        let ministry: T::AccountId = account("ministry", 0, 0);
        let tag: BoundedVec<u8, ConstU32<64>> = b"Ministry of Resources"
            .to_vec()
            .try_into()
            .unwrap_or_default();

        #[extrinsic_call]
        authorize_ministry(RawOrigin::Root, ministry.clone(), tag, None);

        assert!(AuthorizedMinistries::<T>::contains_key(&ministry));
    }

    // submit_license_application(org_account, license_type, region_id, nation_id, audit_hash, requested_duration)
    #[benchmark]
    fn submit_license_application() {
        let ministry: T::AccountId = whitelisted_caller();
        let org: T::AccountId = account("org", 0, 0);
        fund::<T>(&ministry);

        let tag: BoundedVec<u8, ConstU32<64>> = b"Ministry of Resources"
            .to_vec()
            .try_into()
            .unwrap_or_default();
        AuthorizedMinistries::<T>::insert(
            &ministry,
            MinistryRecord {
                tag,
                nation_id: Some(1u32),
                is_active: true,
            },
        );

        let audit_hash = sp_core::H256::from([1u8; 32]);

        #[extrinsic_call]
        submit_license_application(
            RawOrigin::Signed(ministry.clone()),
            org.clone(),
            LicenseType::MineralExtraction,
            1u8,  // region_id
            1u32, // nation_id
            audit_hash,
            0u32, // requested_duration — 0 = use default
        );

        assert_eq!(NextLicenseAppId::<T>::get(), 1);
    }

    impl_benchmark_test_suite!(Pallet, sp_io::TestExternalities::default(), T);
}
