//! Unit tests for pallet-licensing.
//!
//! ## Test Matrix
//!
//! | Test | Constitutional Rule |
//! |------|---------------------|
//! | `authorize_ministry_works` | Root can whitelist a ministry |
//! | `deauthorize_ministry_works` | Root can remove a ministry |
//! | `unauthorized_ministry_rejected` | Non-whitelisted caller rejected |
//! | `submit_application_reserves_deposit` | Anti-spam deposit reserved on submit |
//! | `duration_exceeds_cap_rejected` | 10-year hard cap enforced |
//! | `zero_duration_uses_default` | Sensible default if duration=0 |
//! | `khural_delegate_vote_approve` | KhuralDelegate of matching nation can vote |
//! | `regular_citizen_cannot_vote` | Non-delegate rejected |
//! | `wrong_nation_delegate_rejected` | Nation mismatch rejected |
//! | `double_vote_rejected` | One delegate = one vote |
//! | `application_approved_on_initialize` | on_initialize issues license on quorum |
//! | `application_rejected_slashes_deposit` | Deposit slashed on rejection |
//! | `revoke_license_root_only` | Only Root (Judicial) can revoke |
//! | `non_root_cannot_revoke` | Signed origin rejected |
//! | `is_licensed_query` | Cross-pallet licensing query correct |

use crate::{
    mock::*,
    pallet::{
        AppStatus, AuthorizedMinistries, HasVotedOnLicense, LicenseApplications, Licenses,
        NextLicenseAppId, NextLicenseId,
    },
    LicenseType,
};
use frame_support::{assert_noop, assert_ok, traits::Hooks, BoundedVec};
use sp_core::H256;
use sp_runtime::DispatchError;

// ── helpers ───────────────────────────────────────────────────────────────────

fn tag(s: &[u8]) -> BoundedVec<u8, frame_support::traits::ConstU32<64>> {
    BoundedVec::try_from(s.to_vec()).unwrap()
}

fn audit_hash() -> H256 {
    H256::from([0xABu8; 32])
}

/// Authorise ALICE as a whitelisted ministry via Root.
fn authorize_alice(nation_id: Option<u32>) {
    assert_ok!(crate::Pallet::<Test>::authorize_ministry(
        frame_system::RawOrigin::Root.into(),
        ALICE,
        tag(b"Ministry of Natural Resources"),
        nation_id,
    ));
}

/// Submit a standard MineralExtraction application from ALICE for nation_id=1.
fn alice_submit_application(duration: u32) -> u32 {
    let app_id = NextLicenseAppId::<Test>::get();
    assert_ok!(crate::Pallet::<Test>::submit_license_application(
        frame_system::RawOrigin::Signed(ALICE).into(),
        ORG_ACCT,
        LicenseType::MineralExtraction,
        1u8,  // region_id
        1u32, // nation_id
        audit_hash(),
        duration,
    ));
    app_id
}

// ══════════════════════════════════════════════════════════════════════════════
// 1. Ministry Authorization
// ══════════════════════════════════════════════════════════════════════════════

#[test]
fn authorize_ministry_works() {
    new_test_ext().execute_with(|| {
        authorize_alice(Some(1));
        let rec = AuthorizedMinistries::<Test>::get(ALICE).unwrap();
        assert!(rec.is_active);
        assert_eq!(rec.nation_id, Some(1));
    });
}

#[test]
fn deauthorize_ministry_works() {
    new_test_ext().execute_with(|| {
        authorize_alice(Some(1));
        assert_ok!(crate::Pallet::<Test>::deauthorize_ministry(
            frame_system::RawOrigin::Root.into(),
            ALICE,
        ));
        // After deauthorization, storage entry is removed.
        assert!(AuthorizedMinistries::<Test>::get(ALICE).is_none());
    });
}

#[test]
fn non_root_cannot_authorize_ministry() {
    new_test_ext().execute_with(|| {
        assert_noop!(
            crate::Pallet::<Test>::authorize_ministry(
                frame_system::RawOrigin::Signed(ALICE).into(),
                BOB,
                tag(b"Rogue Ministry"),
                None,
            ),
            DispatchError::BadOrigin
        );
    });
}

// ══════════════════════════════════════════════════════════════════════════════
// 2. Submitting License Applications
// ══════════════════════════════════════════════════════════════════════════════

#[test]
fn unauthorized_ministry_rejected() {
    new_test_ext().execute_with(|| {
        // ALICE not whitelisted yet.
        assert_noop!(
            crate::Pallet::<Test>::submit_license_application(
                frame_system::RawOrigin::Signed(ALICE).into(),
                ORG_ACCT,
                LicenseType::MineralExtraction,
                1u8,
                1u32,
                audit_hash(),
                crate::DEFAULT_LICENSE_BLOCKS,
            ),
            crate::pallet::Error::<Test>::NotAuthorizedMinistry
        );
    });
}

#[test]
fn submit_application_reserves_deposit() {
    new_test_ext().execute_with(|| {
        authorize_alice(Some(1));
        let before = pallet_balances::Pallet::<Test>::reserved_balance(ALICE);
        alice_submit_application(crate::DEFAULT_LICENSE_BLOCKS);
        let after = pallet_balances::Pallet::<Test>::reserved_balance(ALICE);
        // Deposit of 10 should now be reserved.
        assert_eq!(after - before, 10u64);
    });
}

#[test]
fn duration_exceeds_cap_rejected() {
    new_test_ext().execute_with(|| {
        authorize_alice(Some(1));
        let too_long = crate::MAX_LICENSE_BLOCKS + 1;
        assert_noop!(
            crate::Pallet::<Test>::submit_license_application(
                frame_system::RawOrigin::Signed(ALICE).into(),
                ORG_ACCT,
                LicenseType::MineralExtraction,
                1u8,
                1u32,
                audit_hash(),
                too_long,
            ),
            crate::pallet::Error::<Test>::DurationExceedsConstitutionalMax
        );
    });
}

#[test]
fn zero_duration_uses_type_default() {
    new_test_ext().execute_with(|| {
        authorize_alice(Some(1));
        let app_id = alice_submit_application(0);
        let app = LicenseApplications::<Test>::get(app_id).unwrap();
        // MineralExtraction default = MAX_LICENSE_BLOCKS (10 years).
        assert_eq!(app.requested_duration, crate::MAX_LICENSE_BLOCKS);
    });
}

#[test]
fn application_stored_with_pending_status() {
    new_test_ext().execute_with(|| {
        authorize_alice(Some(1));
        let app_id = alice_submit_application(crate::DEFAULT_LICENSE_BLOCKS);
        let app = LicenseApplications::<Test>::get(app_id).unwrap();
        assert_eq!(app.status, AppStatus::PendingKhural);
        assert_eq!(app.votes_for, 0);
        assert_eq!(app.votes_against, 0);
        assert_eq!(app.nation_id, 1u32);
        assert_eq!(app.org_account, ORG_ACCT);
    });
}

// ══════════════════════════════════════════════════════════════════════════════
// 3. Voting
// ══════════════════════════════════════════════════════════════════════════════

#[test]
fn khural_delegate_can_vote_approve() {
    new_test_ext().execute_with(|| {
        authorize_alice(Some(1));
        let app_id = alice_submit_application(crate::DEFAULT_LICENSE_BLOCKS);

        // BOB is a KhuralDelegate for nation 1.
        insert_khural_delegate(BOB, 1);

        assert_ok!(crate::Pallet::<Test>::vote_on_license(
            frame_system::RawOrigin::Signed(BOB).into(),
            app_id,
            true, // approve
        ));

        let app = LicenseApplications::<Test>::get(app_id).unwrap();
        assert_eq!(app.votes_for, 1);
        assert_eq!(app.votes_against, 0);
    });
}

#[test]
fn regular_citizen_cannot_vote() {
    new_test_ext().execute_with(|| {
        authorize_alice(Some(1));
        let app_id = alice_submit_application(crate::DEFAULT_LICENSE_BLOCKS);

        insert_active_citizen(CHARLIE, 1); // Regular role, not KhuralDelegate

        assert_noop!(
            crate::Pallet::<Test>::vote_on_license(
                frame_system::RawOrigin::Signed(CHARLIE).into(),
                app_id,
                true,
            ),
            crate::pallet::Error::<Test>::NotKhuralDelegate
        );
    });
}

#[test]
fn wrong_nation_delegate_rejected() {
    new_test_ext().execute_with(|| {
        authorize_alice(Some(1));
        // Application is for nation 1.
        let app_id = alice_submit_application(crate::DEFAULT_LICENSE_BLOCKS);

        // BOB is a KhuralDelegate for nation 2 — wrong nation.
        insert_khural_delegate(BOB, 2);

        assert_noop!(
            crate::Pallet::<Test>::vote_on_license(
                frame_system::RawOrigin::Signed(BOB).into(),
                app_id,
                true,
            ),
            crate::pallet::Error::<Test>::WrongNation
        );
    });
}

#[test]
fn double_vote_rejected() {
    new_test_ext().execute_with(|| {
        authorize_alice(Some(1));
        let app_id = alice_submit_application(crate::DEFAULT_LICENSE_BLOCKS);
        insert_khural_delegate(BOB, 1);

        // First vote succeeds.
        assert_ok!(crate::Pallet::<Test>::vote_on_license(
            frame_system::RawOrigin::Signed(BOB).into(),
            app_id,
            true,
        ));

        // Second vote from the same delegate is rejected.
        assert_noop!(
            crate::Pallet::<Test>::vote_on_license(
                frame_system::RawOrigin::Signed(BOB).into(),
                app_id,
                false,
            ),
            crate::pallet::Error::<Test>::AlreadyVoted
        );

        // Ensure vote count still reflects only the first vote.
        assert!(HasVotedOnLicense::<Test>::get(app_id, BOB));
        let app = LicenseApplications::<Test>::get(app_id).unwrap();
        assert_eq!(app.votes_for, 1);
        assert_eq!(app.votes_against, 0);
    });
}

#[test]
fn vote_after_end_block_rejected() {
    new_test_ext().execute_with(|| {
        authorize_alice(Some(1));
        let app_id = alice_submit_application(crate::DEFAULT_LICENSE_BLOCKS);
        insert_khural_delegate(BOB, 1);

        let end_block = LicenseApplications::<Test>::get(app_id).unwrap().end_block;
        // Jump past the end block.
        System::set_block_number(end_block.into());

        assert_noop!(
            crate::Pallet::<Test>::vote_on_license(
                frame_system::RawOrigin::Signed(BOB).into(),
                app_id,
                true,
            ),
            crate::pallet::Error::<Test>::ApplicationNotActive
        );
    });
}

// ══════════════════════════════════════════════════════════════════════════════
// 4. on_initialize — Auto-Enactment
// ══════════════════════════════════════════════════════════════════════════════

#[test]
fn application_approved_on_initialize_issues_license() {
    new_test_ext().execute_with(|| {
        authorize_alice(Some(1));
        let app_id = alice_submit_application(crate::DEFAULT_LICENSE_BLOCKS);
        let app = LicenseApplications::<Test>::get(app_id).unwrap();
        let end_block = app.end_block;

        insert_khural_delegate(BOB, 1);
        // BOB votes YES — quorum of 1 met (MinLicenseQuorum = 1 in tests).
        assert_ok!(crate::Pallet::<Test>::vote_on_license(
            frame_system::RawOrigin::Signed(BOB).into(),
            app_id,
            true,
        ));

        // Jump to end_block and trigger on_initialize.
        System::set_block_number(end_block.into());
        crate::Pallet::<Test>::on_initialize(end_block.into());

        // Application should now be Approved.
        let app = LicenseApplications::<Test>::get(app_id).unwrap();
        assert_eq!(app.status, AppStatus::Approved);

        // License should exist.
        let license_id = NextLicenseId::<Test>::get().saturating_sub(1);
        let license = Licenses::<Test>::get(license_id).unwrap();
        assert!(license.is_active);
        assert_eq!(license.org_account, ORG_ACCT);
        assert_eq!(license.license_type, LicenseType::MineralExtraction);
        assert_eq!(license.nation_id, 1u32);

        // Deposit should be unreserved (slashed_reserved is 0 after unreserve).
        let reserved = pallet_balances::Pallet::<Test>::reserved_balance(ALICE);
        assert_eq!(reserved, 0u64);
    });
}

#[test]
fn application_rejected_slashes_deposit() {
    new_test_ext().execute_with(|| {
        authorize_alice(Some(1));
        let app_id = alice_submit_application(crate::DEFAULT_LICENSE_BLOCKS);
        let app = LicenseApplications::<Test>::get(app_id).unwrap();
        let end_block = app.end_block;

        // No votes — quorum not met → rejected.
        System::set_block_number(end_block.into());
        crate::Pallet::<Test>::on_initialize(end_block.into());

        let app = LicenseApplications::<Test>::get(app_id).unwrap();
        assert_eq!(app.status, AppStatus::Rejected);

        // Deposit was slashed — neither reserved nor free.
        let reserved = pallet_balances::Pallet::<Test>::reserved_balance(ALICE);
        assert_eq!(reserved, 0u64);
    });
}

// ══════════════════════════════════════════════════════════════════════════════
// 5. License Revocation (Judicial Only)
// ══════════════════════════════════════════════════════════════════════════════

/// Helper to issue a license directly via on_initialize.
fn issue_license_for_org() -> u32 {
    authorize_alice(Some(1));
    let app_id = alice_submit_application(crate::DEFAULT_LICENSE_BLOCKS);
    let end_block = LicenseApplications::<Test>::get(app_id).unwrap().end_block;

    insert_khural_delegate(BOB, 1);
    assert_ok!(crate::Pallet::<Test>::vote_on_license(
        frame_system::RawOrigin::Signed(BOB).into(),
        app_id,
        true,
    ));
    System::set_block_number(end_block.into());
    crate::Pallet::<Test>::on_initialize(end_block.into());

    NextLicenseId::<Test>::get().saturating_sub(1)
}

#[test]
fn revoke_license_root_only_works() {
    new_test_ext().execute_with(|| {
        let license_id = issue_license_for_org();
        let reason: BoundedVec<u8, frame_support::traits::ConstU32<256>> =
            BoundedVec::try_from(b"Court verdict: fraud".to_vec()).unwrap();

        assert_ok!(crate::Pallet::<Test>::revoke_license(
            frame_system::RawOrigin::Root.into(),
            license_id,
            reason,
        ));

        let license = Licenses::<Test>::get(license_id).unwrap();
        assert!(!license.is_active);
        assert!(license.revocation_reason.is_some());
    });
}

#[test]
fn non_root_cannot_revoke() {
    new_test_ext().execute_with(|| {
        let license_id = issue_license_for_org();
        let reason: BoundedVec<u8, frame_support::traits::ConstU32<256>> =
            BoundedVec::try_from(b"Unauthorized removal attempt".to_vec()).unwrap();

        assert_noop!(
            crate::Pallet::<Test>::revoke_license(
                frame_system::RawOrigin::Signed(CHARLIE).into(),
                license_id,
                reason,
            ),
            DispatchError::BadOrigin
        );

        // License still active.
        let license = Licenses::<Test>::get(license_id).unwrap();
        assert!(license.is_active);
    });
}

#[test]
fn revoke_nonexistent_license_returns_error() {
    new_test_ext().execute_with(|| {
        let reason: BoundedVec<u8, frame_support::traits::ConstU32<256>> =
            BoundedVec::try_from(b"Ghost license".to_vec()).unwrap();
        assert_noop!(
            crate::Pallet::<Test>::revoke_license(
                frame_system::RawOrigin::Root.into(),
                9999u32,
                reason,
            ),
            crate::pallet::Error::<Test>::LicenseNotFound
        );
    });
}

// ══════════════════════════════════════════════════════════════════════════════
// 6. Cross-Pallet Query: is_licensed
// ══════════════════════════════════════════════════════════════════════════════

#[test]
fn is_licensed_returns_true_when_active() {
    new_test_ext().execute_with(|| {
        issue_license_for_org();
        assert!(crate::Pallet::<Test>::is_licensed(
            &ORG_ACCT,
            &LicenseType::MineralExtraction,
            1u8,
        ));
    });
}

#[test]
fn is_licensed_returns_false_after_revocation() {
    new_test_ext().execute_with(|| {
        let license_id = issue_license_for_org();
        let reason: BoundedVec<u8, frame_support::traits::ConstU32<256>> =
            BoundedVec::try_from(b"Revocation".to_vec()).unwrap();
        assert_ok!(crate::Pallet::<Test>::revoke_license(
            frame_system::RawOrigin::Root.into(),
            license_id,
            reason,
        ));

        assert!(!crate::Pallet::<Test>::is_licensed(
            &ORG_ACCT,
            &LicenseType::MineralExtraction,
            1u8,
        ));
    });
}

#[test]
fn is_licensed_returns_false_for_wrong_type() {
    new_test_ext().execute_with(|| {
        issue_license_for_org(); // issues MineralExtraction
                                 // Should be false for a different license type.
        assert!(!crate::Pallet::<Test>::is_licensed(
            &ORG_ACCT,
            &LicenseType::OilGasExtraction,
            1u8,
        ));
    });
}

// ══════════════════════════════════════════════════════════════════════════════
// 7. Constitutional Hard Cap: LicenseType::is_confederal_only
// ══════════════════════════════════════════════════════════════════════════════

#[test]
fn weapons_license_is_confederal_only() {
    assert!(LicenseType::WeaponsManufacture.is_confederal_only());
    assert!(LicenseType::NuclearOperator.is_confederal_only());
    assert!(!LicenseType::MineralExtraction.is_confederal_only());
    assert!(!LicenseType::AviationOperator.is_confederal_only());
}

#[test]
fn mineral_extraction_default_duration_is_ten_years() {
    assert_eq!(
        LicenseType::MineralExtraction.default_duration(),
        crate::MAX_LICENSE_BLOCKS
    );
}

#[test]
fn aviation_operator_default_duration_is_five_years() {
    assert_eq!(
        LicenseType::AviationOperator.default_duration(),
        crate::BLOCKS_PER_YEAR * 5,
    );
}
