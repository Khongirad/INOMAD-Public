//! Unit tests for pallet-chancery.
//!
//! Covers:
//!  1. propose_agreement: validation guards
//!  2. sign_agreement: party gate, duplicate prevention, status transitions
//!  3. validate_agreement: professional rank gate, all-validators-signed → Active
//!  4. raise_dispute: party-only, Active-only
//!  5. complete_agreement: party-only, Active-only
//!  6. Ghost-state: annul_signatures cancels PendingSignatures on death/exile

#![cfg(test)]

use crate::mock::*;
use crate::pallet::{AgreementStatus, Error};
use frame_support::{assert_noop, assert_ok};
use sp_core::H256;

fn hash(seed: u8) -> H256 {
    H256::repeat_byte(seed)
}

// ─── propose_agreement ────────────────────────────────────────────────────

#[test]
fn propose_agreement_stores_record() {
    new_test_ext().execute_with(|| {
        assert_ok!(Chancery::propose_agreement(
            RuntimeOrigin::signed(ALICE),
            hash(0x01),
            vec![ALICE, BOB],
            None,
        ));
        let ag = Chancery::agreements(hash(0x01)).expect("must exist");
        assert_eq!(ag.status, AgreementStatus::PendingSignatures);
        assert_eq!(ag.creator, ALICE);
        assert_eq!(ag.parties.len(), 2);
        assert!(ag.validators.is_none());
    });
}

#[test]
fn propose_duplicate_hash_fails() {
    new_test_ext().execute_with(|| {
        assert_ok!(Chancery::propose_agreement(
            RuntimeOrigin::signed(ALICE),
            hash(0x02),
            vec![ALICE, BOB],
            None,
        ));
        assert_noop!(
            Chancery::propose_agreement(
                RuntimeOrigin::signed(ALICE),
                hash(0x02),
                vec![ALICE, BOB],
                None,
            ),
            Error::<Test>::AgreementAlreadyExists,
        );
    });
}

#[test]
fn propose_single_party_fails() {
    new_test_ext().execute_with(|| {
        assert_noop!(
            Chancery::propose_agreement(
                RuntimeOrigin::signed(ALICE),
                hash(0x03),
                vec![ALICE],
                None,
            ),
            Error::<Test>::InsufficientParties,
        );
    });
}

#[test]
fn propose_creator_not_in_parties_fails() {
    new_test_ext().execute_with(|| {
        assert_noop!(
            Chancery::propose_agreement(
                RuntimeOrigin::signed(ALICE),
                hash(0x04),
                vec![BOB, CHARLIE],
                None,
            ),
            Error::<Test>::CreatorNotInParties,
        );
    });
}

// ─── sign_agreement — simple (no validators) ─────────────────────────────

#[test]
fn simple_agreement_becomes_active_after_all_signatures() {
    new_test_ext().execute_with(|| {
        assert_ok!(Chancery::propose_agreement(
            RuntimeOrigin::signed(ALICE),
            hash(0x10),
            vec![ALICE, BOB],
            None,
        ));
        assert_ok!(Chancery::sign_agreement(
            RuntimeOrigin::signed(ALICE),
            hash(0x10)
        ));
        // still PendingSignatures — 1/2 signed
        assert_eq!(
            Chancery::agreements(hash(0x10)).unwrap().status,
            AgreementStatus::PendingSignatures
        );

        assert_ok!(Chancery::sign_agreement(
            RuntimeOrigin::signed(BOB),
            hash(0x10)
        ));
        // all signed, no validators → Active
        assert_eq!(
            Chancery::agreements(hash(0x10)).unwrap().status,
            AgreementStatus::Active
        );
    });
}

#[test]
fn non_party_cannot_sign() {
    new_test_ext().execute_with(|| {
        assert_ok!(Chancery::propose_agreement(
            RuntimeOrigin::signed(ALICE),
            hash(0x11),
            vec![ALICE, BOB],
            None,
        ));
        assert_noop!(
            Chancery::sign_agreement(RuntimeOrigin::signed(CHARLIE), hash(0x11)),
            Error::<Test>::NotAParty,
        );
    });
}

#[test]
fn double_sign_fails() {
    new_test_ext().execute_with(|| {
        assert_ok!(Chancery::propose_agreement(
            RuntimeOrigin::signed(ALICE),
            hash(0x12),
            vec![ALICE, BOB],
            None,
        ));
        assert_ok!(Chancery::sign_agreement(
            RuntimeOrigin::signed(ALICE),
            hash(0x12)
        ));
        assert_noop!(
            Chancery::sign_agreement(RuntimeOrigin::signed(ALICE), hash(0x12)),
            Error::<Test>::AlreadySigned,
        );
    });
}

// ─── sign_agreement — with validators ────────────────────────────────────

#[test]
fn agreement_with_validators_enters_pending_validation() {
    new_test_ext().execute_with(|| {
        assert_ok!(Chancery::propose_agreement(
            RuntimeOrigin::signed(ALICE),
            hash(0x20),
            vec![ALICE, BOB],
            Some(vec![VALIDATOR]),
        ));
        assert_ok!(Chancery::sign_agreement(
            RuntimeOrigin::signed(ALICE),
            hash(0x20)
        ));
        assert_ok!(Chancery::sign_agreement(
            RuntimeOrigin::signed(BOB),
            hash(0x20)
        ));

        // All parties signed → PendingValidation (because validators set)
        assert_eq!(
            Chancery::agreements(hash(0x20)).unwrap().status,
            AgreementStatus::PendingValidation
        );
    });
}

// ─── validate_agreement ───────────────────────────────────────────────────

#[test]
fn validator_activates_agreement_after_all_validate() {
    new_test_ext().execute_with(|| {
        assert_ok!(Chancery::propose_agreement(
            RuntimeOrigin::signed(ALICE),
            hash(0x21),
            vec![ALICE, BOB],
            Some(vec![VALIDATOR]),
        ));
        assert_ok!(Chancery::sign_agreement(
            RuntimeOrigin::signed(ALICE),
            hash(0x21)
        ));
        assert_ok!(Chancery::sign_agreement(
            RuntimeOrigin::signed(BOB),
            hash(0x21)
        ));

        assert_ok!(Chancery::validate_agreement(
            RuntimeOrigin::signed(VALIDATOR),
            hash(0x21)
        ));
        // All: Active
        assert_eq!(
            Chancery::agreements(hash(0x21)).unwrap().status,
            AgreementStatus::Active
        );
    });
}

#[test]
fn non_professional_validator_rejected() {
    new_test_ext().execute_with(|| {
        assert_ok!(Chancery::propose_agreement(
            RuntimeOrigin::signed(ALICE),
            hash(0x22),
            vec![ALICE, BOB],
            Some(vec![CHARLIE]), // CHARLIE is not a guild pro
        ));
        assert_ok!(Chancery::sign_agreement(
            RuntimeOrigin::signed(ALICE),
            hash(0x22)
        ));
        assert_ok!(Chancery::sign_agreement(
            RuntimeOrigin::signed(BOB),
            hash(0x22)
        ));

        assert_noop!(
            Chancery::validate_agreement(RuntimeOrigin::signed(CHARLIE), hash(0x22)),
            Error::<Test>::ValidatorNotProfessional,
        );
    });
}

#[test]
fn double_validate_fails() {
    new_test_ext().execute_with(|| {
        assert_ok!(Chancery::propose_agreement(
            RuntimeOrigin::signed(ALICE),
            hash(0x23),
            vec![ALICE, BOB],
            Some(vec![VALIDATOR]),
        ));
        assert_ok!(Chancery::sign_agreement(
            RuntimeOrigin::signed(ALICE),
            hash(0x23)
        ));
        assert_ok!(Chancery::sign_agreement(
            RuntimeOrigin::signed(BOB),
            hash(0x23)
        ));
        assert_ok!(Chancery::validate_agreement(
            RuntimeOrigin::signed(VALIDATOR),
            hash(0x23)
        ));

        // Agreement is now Active — validate again should fail with NotPendingValidation
        assert_noop!(
            Chancery::validate_agreement(RuntimeOrigin::signed(VALIDATOR), hash(0x23)),
            Error::<Test>::NotPendingValidation,
        );
    });
}

// ─── raise_dispute ────────────────────────────────────────────────────────

#[test]
fn party_can_raise_dispute_on_active_agreement() {
    new_test_ext().execute_with(|| {
        assert_ok!(Chancery::propose_agreement(
            RuntimeOrigin::signed(ALICE),
            hash(0x30),
            vec![ALICE, BOB],
            None,
        ));
        assert_ok!(Chancery::sign_agreement(
            RuntimeOrigin::signed(ALICE),
            hash(0x30)
        ));
        assert_ok!(Chancery::sign_agreement(
            RuntimeOrigin::signed(BOB),
            hash(0x30)
        ));

        assert_ok!(Chancery::raise_dispute(
            RuntimeOrigin::signed(BOB),
            hash(0x30)
        ));
        assert_eq!(
            Chancery::agreements(hash(0x30)).unwrap().status,
            AgreementStatus::Disputed
        );
    });
}

#[test]
fn non_party_cannot_raise_dispute() {
    new_test_ext().execute_with(|| {
        assert_ok!(Chancery::propose_agreement(
            RuntimeOrigin::signed(ALICE),
            hash(0x31),
            vec![ALICE, BOB],
            None,
        ));
        assert_ok!(Chancery::sign_agreement(
            RuntimeOrigin::signed(ALICE),
            hash(0x31)
        ));
        assert_ok!(Chancery::sign_agreement(
            RuntimeOrigin::signed(BOB),
            hash(0x31)
        ));

        assert_noop!(
            Chancery::raise_dispute(RuntimeOrigin::signed(CHARLIE), hash(0x31)),
            Error::<Test>::NotAParty,
        );
    });
}

#[test]
fn cannot_dispute_pending_agreement() {
    new_test_ext().execute_with(|| {
        assert_ok!(Chancery::propose_agreement(
            RuntimeOrigin::signed(ALICE),
            hash(0x32),
            vec![ALICE, BOB],
            None,
        ));
        // still PendingSignatures
        assert_noop!(
            Chancery::raise_dispute(RuntimeOrigin::signed(ALICE), hash(0x32)),
            Error::<Test>::NotActive,
        );
    });
}

// ─── complete_agreement ───────────────────────────────────────────────────

#[test]
fn party_can_complete_active_agreement() {
    new_test_ext().execute_with(|| {
        assert_ok!(Chancery::propose_agreement(
            RuntimeOrigin::signed(ALICE),
            hash(0x40),
            vec![ALICE, BOB],
            None,
        ));
        assert_ok!(Chancery::sign_agreement(
            RuntimeOrigin::signed(ALICE),
            hash(0x40)
        ));
        assert_ok!(Chancery::sign_agreement(
            RuntimeOrigin::signed(BOB),
            hash(0x40)
        ));

        assert_ok!(Chancery::complete_agreement(
            RuntimeOrigin::signed(ALICE),
            hash(0x40)
        ));
        assert_eq!(
            Chancery::agreements(hash(0x40)).unwrap().status,
            AgreementStatus::Completed
        );
    });
}

// ─── Ghost-State: annul_signatures ───────────────────────────────────────

#[test]
fn annul_signatures_cancels_pending_agreement_on_terminal_status() {
    new_test_ext().execute_with(|| {
        // ALICE & BOB have an unsigned agreement
        assert_ok!(Chancery::propose_agreement(
            RuntimeOrigin::signed(ALICE),
            hash(0x50),
            vec![ALICE, BOB],
            None,
        ));
        // ALICE signs, BOB hasn't → still PendingSignatures
        assert_ok!(Chancery::sign_agreement(
            RuntimeOrigin::signed(ALICE),
            hash(0x50)
        ));

        // ALICE dies → annul_signatures must cancel the pending agreement
        Chancery::annul_signatures(&ALICE);

        assert_eq!(
            Chancery::agreements(hash(0x50)).unwrap().status,
            AgreementStatus::Cancelled
        );
    });
}

#[test]
fn annul_signatures_does_not_cancel_active_agreement() {
    new_test_ext().execute_with(|| {
        assert_ok!(Chancery::propose_agreement(
            RuntimeOrigin::signed(ALICE),
            hash(0x51),
            vec![ALICE, BOB],
            None,
        ));
        assert_ok!(Chancery::sign_agreement(
            RuntimeOrigin::signed(ALICE),
            hash(0x51)
        ));
        assert_ok!(Chancery::sign_agreement(
            RuntimeOrigin::signed(BOB),
            hash(0x51)
        ));
        // Now Active
        assert_eq!(
            Chancery::agreements(hash(0x51)).unwrap().status,
            AgreementStatus::Active
        );

        // ALICE is exiled — Active agreement stays intact (legal record)
        Chancery::annul_signatures(&ALICE);

        assert_eq!(
            Chancery::agreements(hash(0x51)).unwrap().status,
            AgreementStatus::Active // NOT cancelled
        );
    });
}
