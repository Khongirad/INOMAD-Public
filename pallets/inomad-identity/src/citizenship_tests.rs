//! # pallet-inomad-identity: Citizenship Upgrade Tests
//!
//! Covers `claim_repatriation` and `claim_birthright` extrinsics.

use crate::mock::*;
use crate::pallet::{CitizenStatus, CitizenshipStatus, Error, Event};
use frame_support::{assert_noop, assert_ok};

// Since mock uses NoConstitutionHash (returns None), we can pass any hash
// and it will be accepted.
const DUMMY_CONSTITUTION_HASH: [u8; 32] = [0u8; 32];

// ─────────────────────────────────────────────────────────────────────────────
// claim_repatriation (Root-gated Jus Sanguinis upgrade)
// ─────────────────────────────────────────────────────────────────────────────

/// Root can upgrade a Foreign active citizen to Indigenous.
#[test]
fn claim_repatriation_root_succeeds() {
    new_test_ext().execute_with(|| {
        insert_citizen(ALICE, CitizenshipStatus::Foreigner, CitizenStatus::Active);
        let proof = [0xAB; 32];

        assert_ok!(InomadIdentity::claim_repatriation(
            RuntimeOrigin::root(),
            ALICE,
            proof,
            DUMMY_CONSTITUTION_HASH,
        ));

        let record = crate::Citizens::<Test>::get(ALICE).unwrap();
        assert_eq!(record.citizenship_status, CitizenshipStatus::Indigenous);
        assert!(record.is_indigenous);

        System::assert_last_event(
            Event::CitizenshipRepatriated {
                citizen: ALICE,
                lineage_proof: proof,
                status: CitizenshipStatus::Indigenous,
            }
            .into(),
        );
    });
}

/// Non-root signed origin is rejected with `BadOrigin`.
#[test]
fn claim_repatriation_non_root_fails() {
    new_test_ext().execute_with(|| {
        insert_citizen(ALICE, CitizenshipStatus::Foreigner, CitizenStatus::Active);

        assert_noop!(
            InomadIdentity::claim_repatriation(
                RuntimeOrigin::signed(BOB), // non-root
                ALICE,
                [0u8; 32],
                DUMMY_CONSTITUTION_HASH,
            ),
            sp_runtime::DispatchError::BadOrigin
        );
    });
}

/// Repatriation fails if the target citizen is not registered.
#[test]
fn claim_repatriation_not_registered_fails() {
    new_test_ext().execute_with(|| {
        assert_noop!(
            InomadIdentity::claim_repatriation(
                RuntimeOrigin::root(),
                ALICE,
                [0u8; 32],
                DUMMY_CONSTITUTION_HASH,
            ),
            Error::<Test>::NotRegistered
        );
    });
}

/// Repatriation fails if the citizen is not Active (e.g. Frozen).
#[test]
fn claim_repatriation_frozen_citizen_fails() {
    new_test_ext().execute_with(|| {
        insert_citizen(ALICE, CitizenshipStatus::Foreigner, CitizenStatus::Frozen);

        assert_noop!(
            InomadIdentity::claim_repatriation(
                RuntimeOrigin::root(),
                ALICE,
                [0u8; 32],
                DUMMY_CONSTITUTION_HASH,
            ),
            Error::<Test>::CitizenInactive
        );
    });
}

// ─────────────────────────────────────────────────────────────────────────────
// claim_birthright (Self-signed Jus Soli upgrade)
// ─────────────────────────────────────────────────────────────────────────────

/// A Foreigner can self-upgrade to Naturalized via claim_birthright.
#[test]
fn claim_birthright_foreigner_succeeds() {
    new_test_ext().execute_with(|| {
        insert_citizen(ALICE, CitizenshipStatus::Foreigner, CitizenStatus::Active);

        assert_ok!(InomadIdentity::claim_birthright(
            RuntimeOrigin::signed(ALICE),
            DUMMY_CONSTITUTION_HASH,
        ));

        let record = crate::Citizens::<Test>::get(ALICE).unwrap();
        assert_eq!(record.citizenship_status, CitizenshipStatus::Naturalized);
        // is_indigenous stays false — Naturalized ≠ Indigenous.
        assert!(!record.is_indigenous);

        System::assert_last_event(
            Event::CitizenshipNaturalized {
                citizen: ALICE,
                status: CitizenshipStatus::Naturalized,
            }
            .into(),
        );
    });
}

/// Calling claim_birthright when already Naturalized returns AlreadyCitizen.
#[test]
fn claim_birthright_already_naturalized_fails() {
    new_test_ext().execute_with(|| {
        insert_citizen(ALICE, CitizenshipStatus::Naturalized, CitizenStatus::Active);

        assert_noop!(
            InomadIdentity::claim_birthright(
                RuntimeOrigin::signed(ALICE),
                DUMMY_CONSTITUTION_HASH,
            ),
            Error::<Test>::AlreadyCitizen
        );
    });
}

/// Calling claim_birthright when already Indigenous returns AlreadyCitizen.
#[test]
fn claim_birthright_already_indigenous_fails() {
    new_test_ext().execute_with(|| {
        insert_citizen(ALICE, CitizenshipStatus::Indigenous, CitizenStatus::Active);

        assert_noop!(
            InomadIdentity::claim_birthright(
                RuntimeOrigin::signed(ALICE),
                DUMMY_CONSTITUTION_HASH,
            ),
            Error::<Test>::AlreadyCitizen
        );
    });
}

/// Frozen citizens cannot claim birthright.
#[test]
fn claim_birthright_frozen_citizen_fails() {
    new_test_ext().execute_with(|| {
        insert_citizen(ALICE, CitizenshipStatus::Foreigner, CitizenStatus::Frozen);

        assert_noop!(
            InomadIdentity::claim_birthright(
                RuntimeOrigin::signed(ALICE),
                DUMMY_CONSTITUTION_HASH,
            ),
            Error::<Test>::CitizenInactive
        );
    });
}

/// Unregistered account cannot call claim_birthright.
#[test]
fn claim_birthright_not_registered_fails() {
    new_test_ext().execute_with(|| {
        assert_noop!(
            InomadIdentity::claim_birthright(RuntimeOrigin::signed(BOB), DUMMY_CONSTITUTION_HASH,),
            Error::<Test>::NotRegistered
        );
    });
}
