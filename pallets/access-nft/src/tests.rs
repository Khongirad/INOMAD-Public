//! Unit tests for pallet-access-nft.
//!
//! Covers:
//! - `issue_access_key`: happy path, validation guards (access_level, portal_mask, entity_id too long)
//! - `revoke_access_key`: happy path, already-revoked, not-issuer guards
//! - `check_access`: active key grants access; revoked key does not
//! - Token ID auto-increment

use crate::mock::*;
use crate::pallet::{EntityKind, PORTAL_ADMIN, PORTAL_ALL, PORTAL_CLIENT, PORTAL_STAFF};
use frame_support::{assert_noop, assert_ok};

// ── Helpers ───────────────────────────────────────────────────────────────

fn entity_id(s: &str) -> Vec<u8> {
    s.as_bytes().to_vec()
}

// ── 1. issue_access_key ───────────────────────────────────────────────────

#[test]
fn issue_access_key_happy_path() {
    new_test_ext().execute_with(|| {
        System::set_block_number(1);

        assert_ok!(AccessNft::issue_access_key(
            RuntimeOrigin::signed(ISSUER),
            EntityKind::Organization,
            entity_id("ORG-001"),
            ALICE,
            0, // role = CITIZEN
            5, // access_level
            PORTAL_CLIENT,
        ));

        // Token ID 0 should be minted
        let key = crate::pallet::AccessKeys::<Test>::get(0).expect("key should exist");
        assert_eq!(key.holder, ALICE);
        assert_eq!(key.access_level, 5);
        assert!(!key.revoked);
        assert_eq!(key.issued_by, ISSUER);

        // Counter advanced to 1
        assert_eq!(crate::pallet::NextTokenId::<Test>::get(), 1);
    });
}

#[test]
fn issue_access_key_increments_token_id() {
    new_test_ext().execute_with(|| {
        System::set_block_number(1);

        assert_ok!(AccessNft::issue_access_key(
            RuntimeOrigin::signed(ISSUER),
            EntityKind::Guild,
            entity_id("GUILD-A"),
            ALICE,
            1,
            3,
            PORTAL_STAFF,
        ));
        assert_ok!(AccessNft::issue_access_key(
            RuntimeOrigin::signed(ISSUER),
            EntityKind::Guild,
            entity_id("GUILD-A"),
            BOB,
            1,
            3,
            PORTAL_STAFF,
        ));

        assert_eq!(crate::pallet::NextTokenId::<Test>::get(), 2);
    });
}

#[test]
fn issue_access_key_rejects_invalid_access_level_zero() {
    new_test_ext().execute_with(|| {
        assert_noop!(
            AccessNft::issue_access_key(
                RuntimeOrigin::signed(ISSUER),
                EntityKind::Organization,
                entity_id("ORG-002"),
                ALICE,
                0,
                0, // invalid: must be 1–10
                PORTAL_CLIENT,
            ),
            crate::pallet::Error::<Test>::InvalidAccessLevel
        );
    });
}

#[test]
fn issue_access_key_rejects_access_level_above_ten() {
    new_test_ext().execute_with(|| {
        assert_noop!(
            AccessNft::issue_access_key(
                RuntimeOrigin::signed(ISSUER),
                EntityKind::Organization,
                entity_id("ORG-002"),
                ALICE,
                0,
                11, // invalid: must be 1–10
                PORTAL_CLIENT,
            ),
            crate::pallet::Error::<Test>::InvalidAccessLevel
        );
    });
}

#[test]
fn issue_access_key_rejects_invalid_portal_mask_zero() {
    new_test_ext().execute_with(|| {
        assert_noop!(
            AccessNft::issue_access_key(
                RuntimeOrigin::signed(ISSUER),
                EntityKind::Organization,
                entity_id("ORG-003"),
                ALICE,
                0,
                5,
                0, // invalid: portal_mask must be 1–7
            ),
            crate::pallet::Error::<Test>::InvalidPortalMask
        );
    });
}

#[test]
fn issue_access_key_rejects_portal_mask_above_seven() {
    new_test_ext().execute_with(|| {
        assert_noop!(
            AccessNft::issue_access_key(
                RuntimeOrigin::signed(ISSUER),
                EntityKind::Organization,
                entity_id("ORG-003"),
                ALICE,
                0,
                5,
                8, // invalid: max is 7 (0x07)
            ),
            crate::pallet::Error::<Test>::InvalidPortalMask
        );
    });
}

#[test]
fn issue_access_key_rejects_entity_id_too_long() {
    new_test_ext().execute_with(|| {
        // 65 bytes — exceeds 64-byte BoundedVec limit
        let long_id = vec![b'X'; 65];
        assert_noop!(
            AccessNft::issue_access_key(
                RuntimeOrigin::signed(ISSUER),
                EntityKind::Organization,
                long_id,
                ALICE,
                0,
                5,
                PORTAL_CLIENT,
            ),
            crate::pallet::Error::<Test>::EntityIdTooLong
        );
    });
}

#[test]
fn issue_access_key_max_valid_access_level_and_portal_mask() {
    new_test_ext().execute_with(|| {
        System::set_block_number(1);
        // access_level=10, portal_mask=7 (PORTAL_ALL) are the maximums
        assert_ok!(AccessNft::issue_access_key(
            RuntimeOrigin::signed(ISSUER),
            EntityKind::GovernmentBody,
            entity_id("GOV-001"),
            ALICE,
            99, // high role
            10,
            PORTAL_ALL,
        ));
        let key = crate::pallet::AccessKeys::<Test>::get(0).unwrap();
        assert_eq!(key.access_level, 10);
        assert_eq!(key.portal_mask, PORTAL_ALL);
    });
}

// ── 2. revoke_access_key ─────────────────────────────────────────────────

#[test]
fn revoke_access_key_happy_path() {
    new_test_ext().execute_with(|| {
        System::set_block_number(1);

        assert_ok!(AccessNft::issue_access_key(
            RuntimeOrigin::signed(ISSUER),
            EntityKind::Organization,
            entity_id("ORG-001"),
            ALICE,
            0,
            5,
            PORTAL_CLIENT,
        ));

        assert_ok!(AccessNft::revoke_access_key(
            RuntimeOrigin::signed(ISSUER),
            0,
        ));

        let key = crate::pallet::AccessKeys::<Test>::get(0).unwrap();
        assert!(key.revoked);
    });
}

#[test]
fn revoke_access_key_fails_not_found() {
    new_test_ext().execute_with(|| {
        assert_noop!(
            AccessNft::revoke_access_key(RuntimeOrigin::signed(ISSUER), 999),
            crate::pallet::Error::<Test>::TokenNotFound
        );
    });
}

#[test]
fn revoke_access_key_fails_already_revoked() {
    new_test_ext().execute_with(|| {
        System::set_block_number(1);

        assert_ok!(AccessNft::issue_access_key(
            RuntimeOrigin::signed(ISSUER),
            EntityKind::Organization,
            entity_id("ORG-001"),
            ALICE,
            0,
            5,
            PORTAL_CLIENT,
        ));
        assert_ok!(AccessNft::revoke_access_key(
            RuntimeOrigin::signed(ISSUER),
            0
        ));
        assert_noop!(
            AccessNft::revoke_access_key(RuntimeOrigin::signed(ISSUER), 0),
            crate::pallet::Error::<Test>::AlreadyRevoked
        );
    });
}

#[test]
fn revoke_access_key_fails_not_issuer() {
    new_test_ext().execute_with(|| {
        System::set_block_number(1);

        // ISSUER issues key
        assert_ok!(AccessNft::issue_access_key(
            RuntimeOrigin::signed(ISSUER),
            EntityKind::Organization,
            entity_id("ORG-001"),
            ALICE,
            0,
            5,
            PORTAL_CLIENT,
        ));

        // BOB (not the issuer) tries to revoke
        assert_noop!(
            AccessNft::revoke_access_key(RuntimeOrigin::signed(BOB), 0),
            crate::pallet::Error::<Test>::NotIssuer
        );
    });
}

// ── 3. check_access ───────────────────────────────────────────────────────

#[test]
fn check_access_returns_true_for_active_key() {
    new_test_ext().execute_with(|| {
        System::set_block_number(1);

        assert_ok!(AccessNft::issue_access_key(
            RuntimeOrigin::signed(ISSUER),
            EntityKind::Organization,
            entity_id("ORG-001"),
            ALICE,
            0,
            5,
            PORTAL_STAFF,
        ));

        assert!(AccessNft::check_access(&ALICE, b"ORG-001", PORTAL_STAFF));
        // PORTAL_CLIENT bit not set — should return false
        assert!(!AccessNft::check_access(&ALICE, b"ORG-001", PORTAL_ADMIN));
    });
}

#[test]
fn check_access_returns_false_for_revoked_key() {
    new_test_ext().execute_with(|| {
        System::set_block_number(1);

        assert_ok!(AccessNft::issue_access_key(
            RuntimeOrigin::signed(ISSUER),
            EntityKind::Organization,
            entity_id("ORG-001"),
            ALICE,
            0,
            5,
            PORTAL_CLIENT,
        ));
        assert_ok!(AccessNft::revoke_access_key(
            RuntimeOrigin::signed(ISSUER),
            0
        ));

        assert!(!AccessNft::check_access(&ALICE, b"ORG-001", PORTAL_CLIENT));
    });
}

#[test]
fn check_access_returns_false_for_different_entity() {
    new_test_ext().execute_with(|| {
        System::set_block_number(1);

        assert_ok!(AccessNft::issue_access_key(
            RuntimeOrigin::signed(ISSUER),
            EntityKind::Organization,
            entity_id("ORG-001"),
            ALICE,
            0,
            5,
            PORTAL_CLIENT,
        ));

        // Different entity — should not have access
        assert!(!AccessNft::check_access(&ALICE, b"ORG-999", PORTAL_CLIENT));
    });
}

// ── 4. issue_sbt_key (Dual Soulbound) ─────────────────────────────────────

#[test]
fn issue_sbt_key_happy_path() {
    new_test_ext().execute_with(|| {
        System::set_block_number(1);

        assert_ok!(AccessNft::issue_sbt_key(
            RuntimeOrigin::signed(ISSUER),
            EntityKind::Organization,
            entity_id("ORG-SBT-001"),
            b"REG-001".to_vec(),  // org_reg_number
            ALICE,
            9, // role = HR_MANAGER
            5,
            PORTAL_STAFF,
            1, // hr_role = HR_OFFICER
        ));

        let key = crate::pallet::AccessKeys::<Test>::get(0).expect("sbt key should exist");
        assert_eq!(key.holder, ALICE);
        assert!(key.wallet_bound);
        assert_eq!(key.hr_role, 1);
        assert_eq!(key.org_reg_number.as_slice(), b"REG-001");
        assert!(!key.revoked);
    });
}

#[test]
fn issue_sbt_key_rejects_empty_org_reg() {
    new_test_ext().execute_with(|| {
        assert_noop!(
            AccessNft::issue_sbt_key(
                RuntimeOrigin::signed(ISSUER),
                EntityKind::Organization,
                entity_id("ORG-SBT-002"),
                vec![], // empty org_reg — should fail
                ALICE,
                9,
                5,
                PORTAL_STAFF,
                0,
            ),
            crate::pallet::Error::<Test>::OrgRegTooLong
        );
    });
}

#[test]
fn issue_sbt_key_rejects_org_reg_too_long() {
    new_test_ext().execute_with(|| {
        assert_noop!(
            AccessNft::issue_sbt_key(
                RuntimeOrigin::signed(ISSUER),
                EntityKind::Organization,
                entity_id("ORG-SBT-003"),
                vec![b'X'; 33], // 33 bytes — exceeds 32-byte limit
                ALICE,
                9,
                5,
                PORTAL_STAFF,
                0,
            ),
            crate::pallet::Error::<Test>::OrgRegTooLong
        );
    });
}

#[test]
fn verify_sbt_returns_true_for_valid_dual_binding() {
    new_test_ext().execute_with(|| {
        System::set_block_number(1);

        assert_ok!(AccessNft::issue_sbt_key(
            RuntimeOrigin::signed(ISSUER),
            EntityKind::Organization,
            entity_id("ORG-SBT-001"),
            b"REG-001".to_vec(),
            ALICE,
            9,
            5,
            PORTAL_STAFF,
            1,
        ));

        // Both bindings satisfied → true
        assert!(AccessNft::verify_sbt(
            &ALICE,
            b"ORG-SBT-001",
            b"REG-001",
            PORTAL_STAFF,
        ));

        // Wrong org_reg → false
        assert!(!AccessNft::verify_sbt(
            &ALICE,
            b"ORG-SBT-001",
            b"REG-WRONG",
            PORTAL_STAFF,
        ));

        // Wrong entity → false
        assert!(!AccessNft::verify_sbt(
            &ALICE,
            b"ORG-OTHER",
            b"REG-001",
            PORTAL_STAFF,
        ));
    });
}

#[test]
fn get_hr_role_returns_correct_role() {
    new_test_ext().execute_with(|| {
        System::set_block_number(1);

        assert_ok!(AccessNft::issue_sbt_key(
            RuntimeOrigin::signed(ISSUER),
            EntityKind::Organization,
            entity_id("ORG-001"),
            b"REG-ABC".to_vec(),
            ALICE,
            9,
            5,
            PORTAL_STAFF,
            1, // HR_OFFICER
        ));

        assert_eq!(AccessNft::get_hr_role(&ALICE, b"REG-ABC"), Some(1));
        assert_eq!(AccessNft::get_hr_role(&BOB, b"REG-ABC"), None);
    });
}
