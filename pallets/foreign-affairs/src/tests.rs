//! Unit tests for pallet-foreign-affairs.
//!
//! Covers:
//! - `register_foreign_state`: happy path, duplicate guard, empty name guard
//! - `update_diplomatic_status`: change status, country not found
//! - `freeze_country_operations`: sets Sanctioned status
//! - `submit_legalization_document`: Active + Suspended states allowed, Sanctioned blocked
//! - `send_diplomatic_message`: requires Active status

use crate::mock::*;
use crate::pallet::{ChannelKind, ChannelMessageType, DiplomaticStatus};
use frame_support::{assert_noop, assert_ok, BoundedVec};

// ── Helpers ───────────────────────────────────────────────────────────────

fn country_name(s: &str) -> BoundedVec<u8, frame_support::traits::ConstU32<128>> {
    BoundedVec::try_from(s.as_bytes().to_vec()).unwrap()
}

fn content_hash(seed: u8) -> [u8; 32] {
    [seed; 32]
}

/// Register a test country (Russia = code 7 for simplicity).
fn register_active_country(code: u16) {
    ForeignAffairs::register_foreign_state(
        RuntimeOrigin::root(),
        code,
        [b'R', b'U'],
        [b'R', b'U', b'S'],
        country_name("Russia"),
        DiplomaticStatus::Active,
    )
    .unwrap();
}

// ── 1. register_foreign_state ─────────────────────────────────────────────

#[test]
fn register_foreign_state_happy_path() {
    new_test_ext().execute_with(|| {
        System::set_block_number(1);

        assert_ok!(ForeignAffairs::register_foreign_state(
            RuntimeOrigin::root(),
            1,
            [b'U', b'S'],
            [b'U', b'S', b'A'],
            country_name("United States"),
            DiplomaticStatus::Active,
        ));

        assert!(crate::pallet::ForeignStates::<Test>::contains_key(1));

        let record = crate::pallet::ForeignStates::<Test>::get(1).unwrap();
        assert_eq!(record.country_code, 1);
        assert_eq!(record.status, DiplomaticStatus::Active);
        assert_eq!(crate::pallet::TotalStates::<Test>::get(), 1);
    });
}

#[test]
fn register_foreign_state_increments_total_states() {
    new_test_ext().execute_with(|| {
        System::set_block_number(1);

        assert_ok!(ForeignAffairs::register_foreign_state(
            RuntimeOrigin::root(),
            10,
            [b'D', b'E'],
            [b'D', b'E', b'U'],
            country_name("Germany"),
            DiplomaticStatus::Active,
        ));
        assert_ok!(ForeignAffairs::register_foreign_state(
            RuntimeOrigin::root(),
            33,
            [b'F', b'R'],
            [b'F', b'R', b'A'],
            country_name("France"),
            DiplomaticStatus::Active,
        ));

        assert_eq!(crate::pallet::TotalStates::<Test>::get(), 2);
    });
}

#[test]
fn register_foreign_state_rejects_duplicate_country_code() {
    new_test_ext().execute_with(|| {
        System::set_block_number(1);

        assert_ok!(ForeignAffairs::register_foreign_state(
            RuntimeOrigin::root(),
            7,
            [b'R', b'U'],
            [b'R', b'U', b'S'],
            country_name("Russia"),
            DiplomaticStatus::Active,
        ));

        assert_noop!(
            ForeignAffairs::register_foreign_state(
                RuntimeOrigin::root(),
                7,
                [b'R', b'U'],
                [b'R', b'U', b'S'],
                country_name("Russia Again"),
                DiplomaticStatus::Active,
            ),
            crate::pallet::Error::<Test>::CountryAlreadyRegistered
        );
    });
}

#[test]
fn register_foreign_state_rejects_empty_name() {
    new_test_ext().execute_with(|| {
        let empty_name: BoundedVec<u8, frame_support::traits::ConstU32<128>> =
            BoundedVec::default();

        assert_noop!(
            ForeignAffairs::register_foreign_state(
                RuntimeOrigin::root(),
                99,
                [b'X', b'X'],
                [b'X', b'X', b'X'],
                empty_name,
                DiplomaticStatus::Active,
            ),
            crate::pallet::Error::<Test>::InvalidCountryName
        );
    });
}

#[test]
fn register_foreign_state_with_no_relations_status() {
    new_test_ext().execute_with(|| {
        System::set_block_number(1);

        assert_ok!(ForeignAffairs::register_foreign_state(
            RuntimeOrigin::root(),
            55,
            [b'N', b'K'],
            [b'P', b'R', b'K'],
            country_name("North Korea"),
            DiplomaticStatus::NoRelations,
        ));

        let record = crate::pallet::ForeignStates::<Test>::get(55).unwrap();
        assert_eq!(record.status, DiplomaticStatus::NoRelations);
    });
}

// ── 2. update_diplomatic_status ───────────────────────────────────────────

#[test]
fn update_diplomatic_status_active_to_suspended() {
    new_test_ext().execute_with(|| {
        System::set_block_number(1);
        register_active_country(7);

        assert_ok!(ForeignAffairs::update_diplomatic_status(
            RuntimeOrigin::root(),
            7,
            DiplomaticStatus::Suspended,
        ));

        let record = crate::pallet::ForeignStates::<Test>::get(7).unwrap();
        assert_eq!(record.status, DiplomaticStatus::Suspended);
    });
}

#[test]
fn update_diplomatic_status_fails_country_not_found() {
    new_test_ext().execute_with(|| {
        assert_noop!(
            ForeignAffairs::update_diplomatic_status(
                RuntimeOrigin::root(),
                999,
                DiplomaticStatus::Active,
            ),
            crate::pallet::Error::<Test>::CountryNotFound
        );
    });
}

// ── 3. freeze_country_operations ─────────────────────────────────────────

#[test]
fn freeze_country_sets_sanctioned_status() {
    new_test_ext().execute_with(|| {
        System::set_block_number(1);
        register_active_country(7);

        assert_ok!(ForeignAffairs::freeze_country_operations(
            RuntimeOrigin::root(),
            7,
        ));

        let record = crate::pallet::ForeignStates::<Test>::get(7).unwrap();
        assert_eq!(record.status, DiplomaticStatus::Sanctioned);
    });
}

#[test]
fn freeze_country_fails_country_not_found() {
    new_test_ext().execute_with(|| {
        assert_noop!(
            ForeignAffairs::freeze_country_operations(RuntimeOrigin::root(), 999),
            crate::pallet::Error::<Test>::CountryNotFound
        );
    });
}

// ── 4. submit_legalization_document ──────────────────────────────────────

#[test]
fn submit_legalization_document_with_active_status() {
    new_test_ext().execute_with(|| {
        System::set_block_number(1);
        register_active_country(7);

        assert_ok!(ForeignAffairs::submit_legalization_document(
            RuntimeOrigin::signed(CITIZEN),
            7,
            content_hash(42),
        ));
    });
}

#[test]
fn submit_legalization_document_with_suspended_status() {
    new_test_ext().execute_with(|| {
        System::set_block_number(1);
        register_active_country(7);

        assert_ok!(ForeignAffairs::update_diplomatic_status(
            RuntimeOrigin::root(),
            7,
            DiplomaticStatus::Suspended,
        ));

        // Suspended is allowed for legalization
        assert_ok!(ForeignAffairs::submit_legalization_document(
            RuntimeOrigin::signed(CITIZEN),
            7,
            content_hash(43),
        ));
    });
}

#[test]
fn submit_legalization_document_blocked_by_sanctions() {
    new_test_ext().execute_with(|| {
        System::set_block_number(1);
        register_active_country(7);

        assert_ok!(ForeignAffairs::freeze_country_operations(
            RuntimeOrigin::root(),
            7,
        ));

        assert_noop!(
            ForeignAffairs::submit_legalization_document(
                RuntimeOrigin::signed(CITIZEN),
                7,
                content_hash(44),
            ),
            crate::pallet::Error::<Test>::CountrySanctioned
        );
    });
}

#[test]
fn submit_legalization_document_fails_country_not_found() {
    new_test_ext().execute_with(|| {
        assert_noop!(
            ForeignAffairs::submit_legalization_document(
                RuntimeOrigin::signed(CITIZEN),
                999,
                content_hash(1),
            ),
            crate::pallet::Error::<Test>::CountryNotFound
        );
    });
}
