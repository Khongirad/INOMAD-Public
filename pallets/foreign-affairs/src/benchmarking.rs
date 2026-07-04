#![cfg(feature = "runtime-benchmarks")]
extern crate alloc;
use super::*;
use frame_benchmarking::v2::*;
use frame_support::pallet_prelude::*;
use frame_system::RawOrigin;

#[benchmarks]
mod benchmarks {
    use super::*;

    // register_foreign_state(country_code, iso_alpha2, iso_alpha3, name, status) — Root
    #[benchmark]
    fn register_foreign_state() {
        let country_code: u16 = 840u16; // USA
        let name: BoundedVec<u8, ConstU32<128>> =
            b"United States".to_vec().try_into().unwrap_or_default();

        #[extrinsic_call]
        register_foreign_state(
            RawOrigin::Root,
            country_code,
            *b"US",
            *b"USA",
            name,
            DiplomaticStatus::Active,
        );

        assert!(ForeignStates::<T>::contains_key(country_code));
    }

    // update_diplomatic_status(country_code, new_status) — Root
    #[benchmark]
    fn update_diplomatic_status() {
        let country_code: u16 = 643u16; // Russia
                                        // Seed state
        ForeignStates::<T>::insert(
            country_code,
            ForeignStateRecord {
                country_code,
                iso_alpha2: *b"RU",
                iso_alpha3: *b"RUS",
                status: DiplomaticStatus::Active,
                registered_at: 0u32,
                bank_wallet_count: 10u8,
            },
        );

        #[extrinsic_call]
        update_diplomatic_status(RawOrigin::Root, country_code, DiplomaticStatus::Suspended);

        let record = ForeignStates::<T>::get(country_code).expect("exists");
        assert_eq!(record.status, DiplomaticStatus::Suspended);
    }

    // send_diplomatic_message(country_code, message_type, content_hash)
    // Requires caller to be an authorized council member — seed via DiplomaticCouncil
    #[benchmark]
    fn send_diplomatic_message() {
        let sender: T::AccountId = whitelisted_caller();
        let country_code: u16 = 250u16; // France

        ForeignStates::<T>::insert(
            country_code,
            ForeignStateRecord {
                country_code,
                iso_alpha2: *b"FR",
                iso_alpha3: *b"FRA",
                status: DiplomaticStatus::Active,
                registered_at: 0u32,
                bank_wallet_count: 10u8,
            },
        );

        // Authorize sender as council member
        let council_bv: BoundedVec<T::AccountId, T::MaxDiplomaticCouncil> =
            alloc::vec![sender.clone()].try_into().unwrap_or_default();
        DiplomaticCouncil::<T>::insert(country_code, council_bv);

        let content_hash = [2u8; 32];

        #[extrinsic_call]
        send_diplomatic_message(
            RawOrigin::Signed(sender.clone()),
            country_code,
            ChannelMessageType::DiplomaticNote,
            content_hash,
        );
    }

    impl_benchmark_test_suite!(Pallet, sp_io::TestExternalities::default(), T);
}
