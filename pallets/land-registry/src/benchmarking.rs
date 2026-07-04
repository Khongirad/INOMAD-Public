#![cfg(feature = "runtime-benchmarks")]
use super::*;
use frame_benchmarking::v2::*;
use frame_support::pallet_prelude::*;
use frame_system::RawOrigin;

/// Seed a CitizenRecord with Indigenous citizenship so land checks pass.
fn seed_indigenous_citizen<T: Config>(who: &T::AccountId)
where
    T: pallet_inomad_identity::Config,
{
    use pallet_inomad_identity::pallet::{
        CitizenRecord, CitizenRole, CitizenStatus, CitizenshipStatus, PassportType,
        VerificationStatus,
    };
    use pallet_inomad_identity::Citizens;

    // Use a zero H256 — fine for benchmark seeding
    let zero_h256 = sp_core::H256::zero();

    Citizens::<T>::insert(
        who,
        CitizenRecord {
            citizen_id: 1u64,
            nation_id: 1u32,
            naturalized_people_id: None,
            role: CitizenRole::Regular,
            status: CitizenStatus::Active,
            verification: VerificationStatus::Verified,
            vesting_level: None,
            branch: None,
            term_end: None,
            khural_terms_served: 0u8,
            is_indigenous: true,
            citizenship_status: CitizenshipStatus::Indigenous,
            region_id: Some(1u8),
            birth_region_id: Some(1u8),
            passport_type: PassportType::Internal,
            document_hash: zero_h256,
            birth_page_hash: zero_h256,
            email_hash: zero_h256,
        },
    );
}

#[benchmarks(where T: pallet_inomad_identity::Config)]
mod benchmarks {
    use super::*;

    // register_parcel(owner, region, coordinate_hash, area_sqm, resource_rights, description) — Root
    #[benchmark]
    fn register_parcel() {
        let owner: T::AccountId = account("owner", 0, 0);
        seed_indigenous_citizen::<T>(&owner);
        let desc: BoundedVec<u8, T::MaxDescriptionLen> =
            b"Bench parcel".to_vec().try_into().unwrap_or_default();

        #[extrinsic_call]
        register_parcel(
            RawOrigin::Root,
            owner.clone(),
            1u8,
            [0u8; 32],
            1000u64,
            ResourceRights::SurfaceOnly,
            desc,
        );

        assert!(NextParcelId::<T>::get() > 0);
    }

    // transfer_land(parcel_id, buyer) — Signed(owner)
    #[benchmark]
    fn transfer_land() {
        let seller: T::AccountId = whitelisted_caller();
        let buyer: T::AccountId = account("buyer", 0, 0);
        seed_indigenous_citizen::<T>(&seller);
        seed_indigenous_citizen::<T>(&buyer);

        let parcel_id: u64 = 0;
        let desc: BoundedVec<u8, T::MaxDescriptionLen> =
            b"Bench parcel".to_vec().try_into().unwrap_or_default();

        LandParcels::<T>::insert(
            parcel_id,
            LandParcel::<T> {
                owner: seller.clone(),
                region: 1u8,
                coordinate_hash: [0u8; 32],
                area_sqm: 1000u64,
                resource_rights: ResourceRights::SurfaceOnly,
                registered_at: 0u32,
                description: desc,
            },
        );
        ParcelsByOwner::<T>::insert(&seller, parcel_id, ());
        NextParcelId::<T>::put(1u64);

        #[extrinsic_call]
        transfer_land(RawOrigin::Signed(seller.clone()), parcel_id, buyer.clone());

        let parcel = LandParcels::<T>::get(parcel_id).expect("exists");
        assert_eq!(parcel.owner, buyer);
    }

    impl_benchmark_test_suite!(Pallet, sp_io::TestExternalities::default(), T);
}
