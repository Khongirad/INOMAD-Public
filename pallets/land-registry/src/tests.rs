//! # pallet-land-registry: Unit Tests
//!
//! Tests the Non-Alienation Law (Article V) and the Active Citizen Guard.

use crate::mock::*;
use crate::pallet::{Error, Event};
use frame_support::{assert_noop, assert_ok};
// CitizenStatus and CitizenshipStatus come from pallet-inomad-identity (re-exported by mock)
use pallet_inomad_identity::{CitizenStatus, CitizenshipStatus};

// ─────────────────────────────────────────────────────────────────────────────
// transfer_land — success cases
// ─────────────────────────────────────────────────────────────────────────────

/// Seller (Indigenous) can transfer to an Indigenous citizen.
#[test]
fn transfer_to_indigenous_citizen_succeeds() {
    new_test_ext().execute_with(|| {
        insert_citizen(SELLER, CitizenshipStatus::Indigenous, CitizenStatus::Active);
        insert_citizen(
            CITIZEN_BUYER,
            CitizenshipStatus::Indigenous,
            CitizenStatus::Active,
        );
        insert_parcel(0, SELLER);

        assert_ok!(LandRegistry::transfer_land(
            RuntimeOrigin::signed(SELLER),
            0u64,
            CITIZEN_BUYER,
        ));

        // Ownership updated.
        let parcel = LandRegistry::land_parcels(0).unwrap();
        assert_eq!(parcel.owner, CITIZEN_BUYER);

        // Reverse index updated.
        assert!(LandRegistry::parcels_by_owner(CITIZEN_BUYER, 0).is_some());
        assert!(LandRegistry::parcels_by_owner(SELLER, 0).is_none());

        System::assert_last_event(
            Event::LandTransferred {
                parcel_id: 0,
                from: SELLER,
                to: CITIZEN_BUYER,
            }
            .into(),
        );
    });
}

/// Seller (Indigenous) can transfer to a Naturalized citizen.
#[test]
fn transfer_to_naturalized_citizen_succeeds() {
    new_test_ext().execute_with(|| {
        insert_citizen(SELLER, CitizenshipStatus::Indigenous, CitizenStatus::Active);
        insert_citizen(
            CITIZEN_BUYER,
            CitizenshipStatus::Naturalized,
            CitizenStatus::Active,
        );
        insert_parcel(0, SELLER);

        assert_ok!(LandRegistry::transfer_land(
            RuntimeOrigin::signed(SELLER),
            0u64,
            CITIZEN_BUYER,
        ));
    });
}

// ─────────────────────────────────────────────────────────────────────────────
// transfer_land — foreign ownership blocked (Article V)
// ─────────────────────────────────────────────────────────────────────────────

/// Transferring to a Foreigner fails with ForeignLandOwnershipForbidden.
#[test]
fn transfer_to_foreigner_fails_with_forbidden_error() {
    new_test_ext().execute_with(|| {
        insert_citizen(SELLER, CitizenshipStatus::Indigenous, CitizenStatus::Active);
        insert_citizen(
            FOREIGNER_BUYER,
            CitizenshipStatus::Foreigner,
            CitizenStatus::Active,
        );
        insert_parcel(0, SELLER);

        assert_noop!(
            LandRegistry::transfer_land(RuntimeOrigin::signed(SELLER), 0u64, FOREIGNER_BUYER,),
            Error::<Test>::ForeignLandOwnershipForbidden
        );

        // Ownership unchanged.
        assert_eq!(LandRegistry::land_parcels(0).unwrap().owner, SELLER);
    });
}

// ─────────────────────────────────────────────────────────────────────────────
// transfer_land — frozen/exiled buyer blocked (Active Citizen Guard)
// ─────────────────────────────────────────────────────────────────────────────

/// A Frozen Indigenous citizen cannot acquire land — pre-trial hold guard.
#[test]
fn transfer_to_frozen_citizen_fails() {
    new_test_ext().execute_with(|| {
        insert_citizen(SELLER, CitizenshipStatus::Indigenous, CitizenStatus::Active);
        insert_citizen(
            FROZEN_BUYER,
            CitizenshipStatus::Indigenous,
            CitizenStatus::Frozen,
        );
        insert_parcel(0, SELLER);

        assert_noop!(
            LandRegistry::transfer_land(RuntimeOrigin::signed(SELLER), 0u64, FROZEN_BUYER,),
            Error::<Test>::BuyerAccountFrozenOrExiled
        );
    });
}

/// An Exiled citizen cannot acquire land.
#[test]
fn transfer_to_exiled_citizen_fails() {
    new_test_ext().execute_with(|| {
        insert_citizen(SELLER, CitizenshipStatus::Indigenous, CitizenStatus::Active);
        insert_citizen(
            FROZEN_BUYER,
            CitizenshipStatus::Indigenous,
            CitizenStatus::Exiled,
        );
        insert_parcel(0, SELLER);

        assert_noop!(
            LandRegistry::transfer_land(RuntimeOrigin::signed(SELLER), 0u64, FROZEN_BUYER,),
            Error::<Test>::BuyerAccountFrozenOrExiled
        );
    });
}

// ─────────────────────────────────────────────────────────────────────────────
// transfer_land — other guards
// ─────────────────────────────────────────────────────────────────────────────

/// Non-owner cannot transfer another person's parcel.
#[test]
fn non_owner_cannot_transfer_parcel() {
    new_test_ext().execute_with(|| {
        insert_citizen(SELLER, CitizenshipStatus::Indigenous, CitizenStatus::Active);
        insert_citizen(
            CITIZEN_BUYER,
            CitizenshipStatus::Indigenous,
            CitizenStatus::Active,
        );
        insert_citizen(
            FOREIGNER_BUYER,
            CitizenshipStatus::Indigenous,
            CitizenStatus::Active,
        );
        insert_parcel(0, SELLER);

        assert_noop!(
            LandRegistry::transfer_land(
                RuntimeOrigin::signed(CITIZEN_BUYER), // not the owner
                0u64,
                FOREIGNER_BUYER,
            ),
            Error::<Test>::NotParcelOwner
        );
    });
}

/// Self-transfer is rejected.
#[test]
fn self_transfer_fails() {
    new_test_ext().execute_with(|| {
        insert_citizen(SELLER, CitizenshipStatus::Indigenous, CitizenStatus::Active);
        insert_parcel(0, SELLER);

        assert_noop!(
            LandRegistry::transfer_land(RuntimeOrigin::signed(SELLER), 0u64, SELLER),
            Error::<Test>::SelfTransfer
        );
    });
}

/// Buyer must be a registered citizen.
#[test]
fn unregistered_buyer_fails() {
    new_test_ext().execute_with(|| {
        insert_citizen(SELLER, CitizenshipStatus::Indigenous, CitizenStatus::Active);
        insert_parcel(0, SELLER);
        // CITIZEN_BUYER not inserted → not registered

        assert_noop!(
            LandRegistry::transfer_land(RuntimeOrigin::signed(SELLER), 0u64, CITIZEN_BUYER,),
            Error::<Test>::BuyerNotRegistered
        );
    });
}
