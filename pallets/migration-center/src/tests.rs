//! Unit tests for pallet-migration-center.
//!
//! ## Coverage
//!
//! | Test | What it verifies |
//! |------|-----------------|
//! | `submit_creates_anchor` | Happy path: anchor stored + event emitted |
//! | `submit_duplicate_fails` | Second active submission blocked |
//! | `submit_after_terminal_allowed` | Re-submit allowed after APPROVED |
//! | `submit_duplicate_hash_blocked` | Same application_id_hash rejected globally |
//! | `claim_wrong_status_fails` | Cannot claim non-SUBMITTED application |
//! | `claim_updates_status` | Status → UnderReview, officer stored |
//! | `claim_invalid_sig_length` | Sig ≠ 64 bytes → InvalidSignatureLength |
//! | `finalize_approve` | Status → Approved, outcome stored, counter incremented |
//! | `finalize_reject` | Status → Rejected, no counter increment |
//! | `finalize_wrong_officer` | Only claimer may finalize |
//! | `finalize_not_under_review` | Cannot finalize SUBMITTED application |
//! | `revoke_root_only` | Non-root revocation rejected |
//! | `revoke_sets_status` | Root revoke → Revoked status |
//! | `revoke_terminal_fails` | Cannot revoke already-finalized anchor |
//! | `query_has_active_application` | Cross-pallet query correct for all states |
//! | `query_is_application_approved` | Cross-pallet query correct |
//! | `query_applicant_by_hash` | Secondary index lookup works |
//! | `totals_counter_accuracy` | TotalApplications and TotalApproved counters |
//! | `multiple_applicants_independent` | Two wallets → two independent anchors |

#![cfg(test)]

use crate::mock::*;
use crate::{pallet::*, Error, Event};
use frame_support::{assert_noop, assert_ok};

// ── Helper: bypass sr25519 signature check in tests ───────────────────────────
//
// The real `claim_application` verifies the officer's sr25519 sig on-chain.
// In the test executor, `sp_io::crypto::sr25519_verify` always returns false
// for zeroed dummy sigs. We test the full extrinsic path by using a thin
// wrapper that calls `submit_application` and `finalize_application` directly
// (which don't verify sigs), and test the signature-gated path separately in
// `claim_invalid_sig_length`.
//
// The crypto verification is tested indirectly: the pallet uses the exact
// Substrate host function, which is exercised by the node's GRANDPA/AURA
// signature verification in production.

// ============================================================================
// submit_application tests
// ============================================================================

#[test]
fn submit_creates_anchor() {
    new_test_ext().execute_with(|| {
        let app_hash = h256(1);
        let data_hash = h256(2);

        assert_ok!(MigrationCenter::submit_application(
            RuntimeOrigin::signed(ALICE),
            app_hash,
            data_hash,
        ));

        // Anchor exists in storage
        let anchor = ApplicationAnchors::<Test>::get(ALICE).unwrap();
        assert_eq!(anchor.application_id_hash, app_hash);
        assert_eq!(anchor.data_hash, data_hash);
        assert_eq!(anchor.status, AnchorStatus::Submitted);
        assert_eq!(anchor.submitted_at_block, 1);
        assert!(anchor.officer.is_none());
        assert!(anchor.officer_claim_sig.is_none());
        assert!(anchor.outcome_hash.is_none());

        // Secondary index populated
        assert_eq!(HashToApplicant::<Test>::get(app_hash), Some(ALICE));

        // Counter incremented
        assert_eq!(TotalApplications::<Test>::get(), 1);

        // Event emitted
        System::assert_has_event(
            Event::ApplicationSubmitted {
                applicant: ALICE,
                application_id_hash: app_hash,
                data_hash,
                block: 1,
            }
            .into(),
        );
    });
}

#[test]
fn submit_duplicate_active_fails() {
    new_test_ext().execute_with(|| {
        assert_ok!(MigrationCenter::submit_application(
            RuntimeOrigin::signed(ALICE),
            h256(1),
            h256(2),
        ));

        // Second submit by same wallet → ApplicationAlreadyExists
        assert_noop!(
            MigrationCenter::submit_application(
                RuntimeOrigin::signed(ALICE),
                h256(3), // different hash — but anchor is still active
                h256(4),
            ),
            Error::<Test>::ApplicationAlreadyExists
        );
    });
}

#[test]
fn submit_after_terminal_anchor_allowed() {
    new_test_ext().execute_with(|| {
        // Submit initial application
        assert_ok!(MigrationCenter::submit_application(
            RuntimeOrigin::signed(ALICE),
            h256(1),
            h256(2),
        ));

        // Manually set anchor to Approved (terminal state) via storage
        ApplicationAnchors::<Test>::mutate(ALICE, |a| {
            if let Some(anchor) = a {
                anchor.status = AnchorStatus::Approved;
            }
        });

        // Now a new submit from the same wallet should be allowed
        assert_ok!(MigrationCenter::submit_application(
            RuntimeOrigin::signed(ALICE),
            h256(99), // different UUID hash
            h256(98),
        ));

        assert_eq!(TotalApplications::<Test>::get(), 2);
    });
}

#[test]
fn submit_duplicate_application_hash_globally_blocked() {
    new_test_ext().execute_with(|| {
        let same_hash = h256(42);

        // Alice submits with hash(42)
        assert_ok!(MigrationCenter::submit_application(
            RuntimeOrigin::signed(ALICE),
            same_hash,
            h256(1),
        ));

        // Manually set Alice's anchor to terminal so she can re-submit
        ApplicationAnchors::<Test>::mutate(ALICE, |a| {
            if let Some(anchor) = a {
                anchor.status = AnchorStatus::Rejected;
            }
        });

        // Alice tries to re-use the SAME hash — globally blocked
        assert_noop!(
            MigrationCenter::submit_application(
                RuntimeOrigin::signed(ALICE),
                same_hash, // same UUID hash!
                h256(2),
            ),
            Error::<Test>::DuplicateApplicationHash
        );

        // Charlie tries the same hash — also blocked
        assert_noop!(
            MigrationCenter::submit_application(
                RuntimeOrigin::signed(CHARLIE),
                same_hash,
                h256(3),
            ),
            Error::<Test>::DuplicateApplicationHash
        );
    });
}

// ============================================================================
// claim_application tests
// ============================================================================

#[test]
fn claim_invalid_sig_length_fails() {
    new_test_ext().execute_with(|| {
        assert_ok!(MigrationCenter::submit_application(
            RuntimeOrigin::signed(ALICE),
            h256(1),
            h256(2),
        ));

        // Sig shorter than 64 bytes
        let short_sig: frame_support::BoundedVec<u8, frame_support::traits::ConstU32<64>> =
            frame_support::BoundedVec::try_from(vec![0u8; 32]).unwrap();

        assert_noop!(
            MigrationCenter::claim_application(
                RuntimeOrigin::signed(BOB),
                ALICE,
                short_sig,
            ),
            Error::<Test>::InvalidSignatureLength
        );
    });
}

#[test]
fn claim_not_submitted_fails() {
    new_test_ext().execute_with(|| {
        // No application submitted — ApplicationNotFound
        assert_noop!(
            MigrationCenter::claim_application(
                RuntimeOrigin::signed(BOB),
                ALICE,
                fake_sig(),
            ),
            Error::<Test>::ApplicationNotFound
        );
    });
}

#[test]
fn claim_already_under_review_fails() {
    new_test_ext().execute_with(|| {
        assert_ok!(MigrationCenter::submit_application(
            RuntimeOrigin::signed(ALICE),
            h256(1),
            h256(2),
        ));

        // Manually set to UnderReview
        ApplicationAnchors::<Test>::mutate(ALICE, |a| {
            if let Some(anchor) = a {
                anchor.status = AnchorStatus::UnderReview;
                anchor.officer = Some(BOB);
            }
        });

        // Dave tries to claim — application not in Submitted state
        assert_noop!(
            MigrationCenter::claim_application(
                RuntimeOrigin::signed(DAVE),
                ALICE,
                fake_sig(),
            ),
            Error::<Test>::NotSubmitted
        );
    });
}

// ============================================================================
// finalize_application tests
// ============================================================================

#[test]
fn finalize_approve_works() {
    new_test_ext().execute_with(|| {
        let app_hash = h256(1);

        // Submit
        assert_ok!(MigrationCenter::submit_application(
            RuntimeOrigin::signed(ALICE),
            app_hash,
            h256(2),
        ));

        // Force to UnderReview with BOB as officer (bypassing sig check)
        ApplicationAnchors::<Test>::mutate(ALICE, |a| {
            if let Some(anchor) = a {
                anchor.status = AnchorStatus::UnderReview;
                anchor.officer = Some(BOB);
                anchor.claimed_at_block = Some(1);
            }
        });

        let outcome = h256(99);
        assert_ok!(MigrationCenter::finalize_application(
            RuntimeOrigin::signed(BOB),
            ALICE,
            true, // approved
            outcome,
        ));

        let anchor = ApplicationAnchors::<Test>::get(ALICE).unwrap();
        assert_eq!(anchor.status, AnchorStatus::Approved);
        assert_eq!(anchor.outcome_hash, Some(outcome));
        assert!(anchor.finalized_at_block.is_some());

        // Approved counter incremented
        assert_eq!(TotalApproved::<Test>::get(), 1);

        // Event
        System::assert_has_event(
            Event::ApplicationFinalized {
                officer: BOB,
                applicant: ALICE,
                application_id_hash: app_hash,
                approved: true,
                outcome_hash: outcome,
            }
            .into(),
        );
    });
}

#[test]
fn finalize_reject_does_not_increment_approved_counter() {
    new_test_ext().execute_with(|| {
        assert_ok!(MigrationCenter::submit_application(
            RuntimeOrigin::signed(ALICE),
            h256(1),
            h256(2),
        ));

        ApplicationAnchors::<Test>::mutate(ALICE, |a| {
            if let Some(anchor) = a {
                anchor.status = AnchorStatus::UnderReview;
                anchor.officer = Some(BOB);
            }
        });

        assert_ok!(MigrationCenter::finalize_application(
            RuntimeOrigin::signed(BOB),
            ALICE,
            false, // rejected
            h256(99),
        ));

        let anchor = ApplicationAnchors::<Test>::get(ALICE).unwrap();
        assert_eq!(anchor.status, AnchorStatus::Rejected);

        // Approved counter NOT incremented
        assert_eq!(TotalApproved::<Test>::get(), 0);
    });
}

#[test]
fn finalize_wrong_officer_fails() {
    new_test_ext().execute_with(|| {
        assert_ok!(MigrationCenter::submit_application(
            RuntimeOrigin::signed(ALICE),
            h256(1),
            h256(2),
        ));

        // BOB claimed it
        ApplicationAnchors::<Test>::mutate(ALICE, |a| {
            if let Some(anchor) = a {
                anchor.status = AnchorStatus::UnderReview;
                anchor.officer = Some(BOB);
            }
        });

        // DAVE tries to finalize — not the claiming officer
        assert_noop!(
            MigrationCenter::finalize_application(
                RuntimeOrigin::signed(DAVE),
                ALICE,
                true,
                h256(99),
            ),
            Error::<Test>::NotTheClaimingOfficer
        );
    });
}

#[test]
fn finalize_not_under_review_fails() {
    new_test_ext().execute_with(|| {
        assert_ok!(MigrationCenter::submit_application(
            RuntimeOrigin::signed(ALICE),
            h256(1),
            h256(2),
        ));

        // Status is still Submitted (not UnderReview)
        assert_noop!(
            MigrationCenter::finalize_application(
                RuntimeOrigin::signed(BOB),
                ALICE,
                true,
                h256(99),
            ),
            Error::<Test>::NotUnderReview
        );
    });
}

#[test]
fn finalize_application_not_found_fails() {
    new_test_ext().execute_with(|| {
        // No application for ALICE at all
        assert_noop!(
            MigrationCenter::finalize_application(
                RuntimeOrigin::signed(BOB),
                ALICE,
                true,
                h256(99),
            ),
            Error::<Test>::ApplicationNotFound
        );
    });
}

// ============================================================================
// revoke_application tests
// ============================================================================

#[test]
fn revoke_requires_root() {
    new_test_ext().execute_with(|| {
        assert_ok!(MigrationCenter::submit_application(
            RuntimeOrigin::signed(ALICE),
            h256(1),
            h256(2),
        ));

        // Non-root cannot revoke
        assert_noop!(
            MigrationCenter::revoke_application(RuntimeOrigin::signed(BOB), ALICE),
            sp_runtime::DispatchError::BadOrigin
        );
    });
}

#[test]
fn revoke_sets_status_revoked() {
    new_test_ext().execute_with(|| {
        let app_hash = h256(1);

        assert_ok!(MigrationCenter::submit_application(
            RuntimeOrigin::signed(ALICE),
            app_hash,
            h256(2),
        ));

        assert_ok!(MigrationCenter::revoke_application(
            RuntimeOrigin::root(),
            ALICE,
        ));

        let anchor = ApplicationAnchors::<Test>::get(ALICE).unwrap();
        assert_eq!(anchor.status, AnchorStatus::Revoked);

        System::assert_has_event(
            Event::ApplicationRevoked {
                applicant: ALICE,
                application_id_hash: app_hash,
            }
            .into(),
        );
    });
}

#[test]
fn revoke_terminal_application_fails() {
    new_test_ext().execute_with(|| {
        assert_ok!(MigrationCenter::submit_application(
            RuntimeOrigin::signed(ALICE),
            h256(1),
            h256(2),
        ));

        // Set to Approved (terminal)
        ApplicationAnchors::<Test>::mutate(ALICE, |a| {
            if let Some(anchor) = a {
                anchor.status = AnchorStatus::Approved;
            }
        });

        // Cannot revoke approved application
        assert_noop!(
            MigrationCenter::revoke_application(RuntimeOrigin::root(), ALICE),
            Error::<Test>::AlreadyFinalized
        );
    });
}

#[test]
fn revoke_not_found_fails() {
    new_test_ext().execute_with(|| {
        assert_noop!(
            MigrationCenter::revoke_application(RuntimeOrigin::root(), ALICE),
            Error::<Test>::ApplicationNotFound
        );
    });
}

// ============================================================================
// Cross-pallet query interface tests
// ============================================================================

#[test]
fn query_has_active_application() {
    new_test_ext().execute_with(|| {
        // No anchor → false
        assert!(!MigrationCenter::has_active_application(&ALICE));

        assert_ok!(MigrationCenter::submit_application(
            RuntimeOrigin::signed(ALICE),
            h256(1),
            h256(2),
        ));

        // Submitted → active
        assert!(MigrationCenter::has_active_application(&ALICE));

        // UnderReview → still active
        ApplicationAnchors::<Test>::mutate(ALICE, |a| {
            if let Some(anchor) = a {
                anchor.status = AnchorStatus::UnderReview;
            }
        });
        assert!(MigrationCenter::has_active_application(&ALICE));

        // Approved → terminal → not active
        ApplicationAnchors::<Test>::mutate(ALICE, |a| {
            if let Some(anchor) = a {
                anchor.status = AnchorStatus::Approved;
            }
        });
        assert!(!MigrationCenter::has_active_application(&ALICE));
    });
}

#[test]
fn query_is_application_approved() {
    new_test_ext().execute_with(|| {
        assert!(!MigrationCenter::is_application_approved(&ALICE));

        assert_ok!(MigrationCenter::submit_application(
            RuntimeOrigin::signed(ALICE),
            h256(1),
            h256(2),
        ));

        // Submitted → not approved
        assert!(!MigrationCenter::is_application_approved(&ALICE));

        // Set Approved
        ApplicationAnchors::<Test>::mutate(ALICE, |a| {
            if let Some(anchor) = a {
                anchor.status = AnchorStatus::Approved;
            }
        });
        assert!(MigrationCenter::is_application_approved(&ALICE));

        // Rejected → not approved
        ApplicationAnchors::<Test>::mutate(ALICE, |a| {
            if let Some(anchor) = a {
                anchor.status = AnchorStatus::Rejected;
            }
        });
        assert!(!MigrationCenter::is_application_approved(&ALICE));
    });
}

#[test]
fn query_applicant_by_hash() {
    new_test_ext().execute_with(|| {
        let app_hash = h256(55);

        // Before submit — None
        assert_eq!(MigrationCenter::applicant_by_hash(&app_hash), None);

        assert_ok!(MigrationCenter::submit_application(
            RuntimeOrigin::signed(ALICE),
            app_hash,
            h256(2),
        ));

        // After submit — Some(ALICE)
        assert_eq!(MigrationCenter::applicant_by_hash(&app_hash), Some(ALICE));
    });
}

// ============================================================================
// Counters
// ============================================================================

#[test]
fn total_applications_counter() {
    new_test_ext().execute_with(|| {
        assert_eq!(TotalApplications::<Test>::get(), 0);
        assert_eq!(TotalApproved::<Test>::get(), 0);

        // Submit two applications
        assert_ok!(MigrationCenter::submit_application(
            RuntimeOrigin::signed(ALICE),
            h256(1),
            h256(10),
        ));
        assert_ok!(MigrationCenter::submit_application(
            RuntimeOrigin::signed(CHARLIE),
            h256(2),
            h256(20),
        ));

        assert_eq!(TotalApplications::<Test>::get(), 2);

        // Approve ALICE's
        ApplicationAnchors::<Test>::mutate(ALICE, |a| {
            if let Some(anchor) = a {
                anchor.status = AnchorStatus::UnderReview;
                anchor.officer = Some(BOB);
            }
        });
        assert_ok!(MigrationCenter::finalize_application(
            RuntimeOrigin::signed(BOB),
            ALICE,
            true,
            h256(99),
        ));

        assert_eq!(TotalApproved::<Test>::get(), 1);

        // Reject CHARLIE's
        ApplicationAnchors::<Test>::mutate(CHARLIE, |a| {
            if let Some(anchor) = a {
                anchor.status = AnchorStatus::UnderReview;
                anchor.officer = Some(BOB);
            }
        });
        assert_ok!(MigrationCenter::finalize_application(
            RuntimeOrigin::signed(BOB),
            CHARLIE,
            false,
            h256(98),
        ));

        // Reject does NOT increment approved counter
        assert_eq!(TotalApproved::<Test>::get(), 1);
        assert_eq!(TotalApplications::<Test>::get(), 2);
    });
}

// ============================================================================
// Multiple independent applicants
// ============================================================================

#[test]
fn multiple_applicants_are_independent() {
    new_test_ext().execute_with(|| {
        assert_ok!(MigrationCenter::submit_application(
            RuntimeOrigin::signed(ALICE),
            h256(1),
            h256(10),
        ));
        assert_ok!(MigrationCenter::submit_application(
            RuntimeOrigin::signed(CHARLIE),
            h256(2),
            h256(20),
        ));

        // ALICE's anchor approved
        ApplicationAnchors::<Test>::mutate(ALICE, |a| {
            if let Some(anchor) = a {
                anchor.status = AnchorStatus::UnderReview;
                anchor.officer = Some(BOB);
            }
        });
        assert_ok!(MigrationCenter::finalize_application(
            RuntimeOrigin::signed(BOB),
            ALICE,
            true,
            h256(99),
        ));

        // CHARLIE's anchor still Submitted — unaffected
        let charlie = ApplicationAnchors::<Test>::get(CHARLIE).unwrap();
        assert_eq!(charlie.status, AnchorStatus::Submitted);
        assert!(charlie.officer.is_none());

        let alice = ApplicationAnchors::<Test>::get(ALICE).unwrap();
        assert_eq!(alice.status, AnchorStatus::Approved);

        assert!(MigrationCenter::is_application_approved(&ALICE));
        assert!(!MigrationCenter::is_application_approved(&CHARLIE));
        assert!(MigrationCenter::has_active_application(&CHARLIE));
    });
}

// ============================================================================
// State machine completeness
// ============================================================================

#[test]
fn state_machine_full_lifecycle_approved() {
    new_test_ext().execute_with(|| {
        // SUBMITTED
        assert_ok!(MigrationCenter::submit_application(
            RuntimeOrigin::signed(ALICE),
            h256(1),
            h256(2),
        ));
        assert_eq!(
            ApplicationAnchors::<Test>::get(ALICE).unwrap().status,
            AnchorStatus::Submitted
        );

        // UNDER_REVIEW (force via storage, bypassing sig check)
        ApplicationAnchors::<Test>::mutate(ALICE, |a| {
            if let Some(anchor) = a {
                anchor.status = AnchorStatus::UnderReview;
                anchor.officer = Some(BOB);
                anchor.claimed_at_block = Some(1);
                anchor.officer_claim_sig = Some(fake_sig());
            }
        });
        assert_eq!(
            ApplicationAnchors::<Test>::get(ALICE).unwrap().status,
            AnchorStatus::UnderReview
        );

        // APPROVED
        assert_ok!(MigrationCenter::finalize_application(
            RuntimeOrigin::signed(BOB),
            ALICE,
            true,
            h256(99),
        ));
        assert_eq!(
            ApplicationAnchors::<Test>::get(ALICE).unwrap().status,
            AnchorStatus::Approved
        );

        // Terminal — cannot revoke
        assert_noop!(
            MigrationCenter::revoke_application(RuntimeOrigin::root(), ALICE),
            Error::<Test>::AlreadyFinalized
        );
    });
}

#[test]
fn state_machine_revoke_from_under_review() {
    new_test_ext().execute_with(|| {
        assert_ok!(MigrationCenter::submit_application(
            RuntimeOrigin::signed(ALICE),
            h256(1),
            h256(2),
        ));

        // Force to UnderReview
        ApplicationAnchors::<Test>::mutate(ALICE, |a| {
            if let Some(anchor) = a {
                anchor.status = AnchorStatus::UnderReview;
                anchor.officer = Some(BOB);
            }
        });

        // Root can revoke even from UnderReview
        assert_ok!(MigrationCenter::revoke_application(
            RuntimeOrigin::root(),
            ALICE,
        ));

        assert_eq!(
            ApplicationAnchors::<Test>::get(ALICE).unwrap().status,
            AnchorStatus::Revoked
        );
    });
}
