//! Unit tests for pallet-citizen-voice.
//!
//! Covers:
//!  1. Ticket lifecycle: submit → InReview → resolve/reject
//!  2. Anti-spam deposit mechanics
//!  3. Escalation: only author can escalate, only Root can resolve
//!  4. Sting operation: request → approve → reveal_and_spring_trap
//!  5. Commit-reveal verification (invalid reveal rejected)
//!  6. Whistleblower 20% reward from treasury
//!  7. Double-spring and double-approve prevention

#![cfg(test)]

use crate::mock::*;
use crate::pallet::{Error, FeedbackTarget, FeedbackType, StingStatus, TicketStatus};
use frame_support::{
    assert_noop, assert_ok,
    traits::{Currency, ReservableCurrency},
};
use sp_core::{blake2_256, H256};

/// Helper: free balance of an account.
fn free_bal(who: AccountId) -> Balance {
    <pallet_balances::Pallet<Test> as Currency<AccountId>>::free_balance(&who)
}

/// Helper: reserved balance.
fn reserved_bal(who: AccountId) -> Balance {
    <pallet_balances::Pallet<Test> as ReservableCurrency<AccountId>>::reserved_balance(&who)
}

/// Build a valid commit hash for a sting operation.
/// commit_hash = blake2_256(amount_u128_le || secret_salt)
fn make_commit(amount: Balance, salt: &[u8; 32]) -> [u8; 32] {
    let mut preimage = [0u8; 48];
    preimage[..16].copy_from_slice(&amount.to_le_bytes());
    preimage[16..].copy_from_slice(salt);
    blake2_256(&preimage)
}

// ─── Ticket Submission ─────────────────────────────────────────────────────

#[test]
fn submit_ticket_stores_record_and_reserves_deposit() {
    new_test_ext().execute_with(|| {
        let before = free_bal(ALICE);
        assert_ok!(Voice::submit_ticket(
            RuntimeOrigin::signed(ALICE),
            FeedbackTarget::Entity(TARGET),
            FeedbackType::Complaint,
            H256::repeat_byte(0xAB),
        ));
        let ticket = Voice::tickets(0).expect("ticket 0 must exist");
        assert_eq!(ticket.status, TicketStatus::Open);
        assert_eq!(ticket.author, ALICE);
        // Anti-spam deposit (1 ALTAN) reserved
        assert_eq!(free_bal(ALICE), before - 1 * UNIT);
        assert_eq!(reserved_bal(ALICE), 1 * UNIT);
    });
}

#[test]
fn insufficient_balance_for_deposit_fails() {
    new_test_ext().execute_with(|| {
        assert_noop!(
            Voice::submit_ticket(
                RuntimeOrigin::signed(999), // no balance
                FeedbackTarget::Entity(TARGET),
                FeedbackType::Complaint,
                H256::zero(),
            ),
            Error::<Test>::InsufficientBalance,
        );
    });
}

// ─── Mark In Review ───────────────────────────────────────────────────────

#[test]
fn entity_target_can_mark_in_review() {
    new_test_ext().execute_with(|| {
        assert_ok!(Voice::submit_ticket(
            RuntimeOrigin::signed(ALICE),
            FeedbackTarget::Entity(TARGET),
            FeedbackType::Suggestion,
            H256::zero(),
        ));
        // TARGET is the entity — can mark in review
        assert_ok!(Voice::mark_in_review(RuntimeOrigin::signed(TARGET), 0));
        let ticket = Voice::tickets(0).unwrap();
        assert_eq!(ticket.status, TicketStatus::InReview);
    });
}

// ─── Resolve Ticket ───────────────────────────────────────────────────────

#[test]
fn resolve_ticket_helpful_returns_deposit_to_author() {
    new_test_ext().execute_with(|| {
        assert_ok!(Voice::submit_ticket(
            RuntimeOrigin::signed(ALICE),
            FeedbackTarget::Entity(TARGET),
            FeedbackType::Complaint,
            H256::zero(),
        ));
        let before = free_bal(ALICE);
        assert_ok!(Voice::resolve_ticket(
            RuntimeOrigin::signed(TARGET),
            0,
            H256::zero(),
            true, // helpful = true → deposit returned
        ));
        let ticket = Voice::tickets(0).unwrap();
        assert_eq!(ticket.status, TicketStatus::Resolved);
        assert_eq!(free_bal(ALICE), before + 1 * UNIT); // deposit back
        assert_eq!(reserved_bal(ALICE), 0);
    });
}

#[test]
fn resolve_ticket_rejected_burns_deposit() {
    new_test_ext().execute_with(|| {
        assert_ok!(Voice::submit_ticket(
            RuntimeOrigin::signed(ALICE),
            FeedbackTarget::Entity(TARGET),
            FeedbackType::Complaint,
            H256::zero(),
        ));
        let alice_before = free_bal(ALICE);
        assert_ok!(Voice::resolve_ticket(
            RuntimeOrigin::signed(TARGET),
            0,
            H256::zero(),
            false, // spam → deposit slashed
        ));
        let ticket = Voice::tickets(0).unwrap();
        assert_eq!(ticket.status, TicketStatus::Rejected);
        // Deposit burned — free balance stays at before level (it was reserved, not free)
        assert_eq!(free_bal(ALICE), alice_before);
        assert_eq!(reserved_bal(ALICE), 0);
    });
}

// ─── Escalation ───────────────────────────────────────────────────────────

#[test]
fn submitter_can_escalate_open_ticket() {
    new_test_ext().execute_with(|| {
        assert_ok!(Voice::submit_ticket(
            RuntimeOrigin::signed(ALICE),
            FeedbackTarget::Entity(TARGET),
            FeedbackType::Whistleblower,
            H256::zero(),
        ));
        assert_ok!(Voice::escalate_ticket(RuntimeOrigin::signed(ALICE), 0));
        let ticket = Voice::tickets(0).unwrap();
        assert_eq!(ticket.status, TicketStatus::Escalated);
    });
}

#[test]
fn non_author_cannot_escalate() {
    new_test_ext().execute_with(|| {
        assert_ok!(Voice::submit_ticket(
            RuntimeOrigin::signed(ALICE),
            FeedbackTarget::Entity(TARGET),
            FeedbackType::Complaint,
            H256::zero(),
        ));
        // TARGET is not the author
        assert_noop!(
            Voice::escalate_ticket(RuntimeOrigin::signed(TARGET), 0),
            Error::<Test>::OnlyAuthorCanEscalate,
        );
    });
}

// ─── Sting Operation — Full Happy Path ────────────────────────────────────

#[test]
fn full_sting_flow_exiles_target_and_rewards_citizen() {
    new_test_ext().execute_with(|| {
        let bribe_amount = 500 * UNIT;
        let salt = [0x42u8; 32];
        let commit = make_commit(bribe_amount, &salt);

        let citizen_before = free_bal(ALICE);
        let treasury_before = free_bal(TREASURY);

        // Step 1: Citizen requests sting
        assert_ok!(Voice::request_sting_operation(
            RuntimeOrigin::signed(ALICE),
            TARGET,
            bribe_amount,
            commit,
        ));
        // Amount reserved
        assert_eq!(reserved_bal(ALICE), bribe_amount);

        // Step 2: Prosecutor approves
        assert_ok!(Voice::approve_sting(RuntimeOrigin::root(), 0));
        let op = Voice::sting_operations(0).unwrap();
        assert_eq!(op.status, StingStatus::Approved);

        // Step 3: Root reveals and springs the trap
        assert_ok!(Voice::reveal_and_spring_trap(
            RuntimeOrigin::root(),
            0,
            salt,
        ));

        // Target has been exiled
        assert!(is_exiled(TARGET));

        // Citizen's reserved amount was unreserved
        assert_eq!(reserved_bal(ALICE), 0);

        // Whistleblower reward: 20% of 500 UNIT = 100 UNIT (from treasury)
        let expected_reward = 100 * UNIT;
        assert_eq!(free_bal(ALICE), citizen_before + expected_reward);
        assert_eq!(free_bal(TREASURY), treasury_before - expected_reward);

        // Operation marked Sprung
        let op_after = Voice::sting_operations(0).unwrap();
        assert_eq!(op_after.status, StingStatus::Sprung);

        clear_mocks();
    });
}

// ─── Commit-Reveal Integrity ──────────────────────────────────────────────

#[test]
fn invalid_reveal_salt_rejected() {
    new_test_ext().execute_with(|| {
        let bribe_amount = 100 * UNIT;
        let correct_salt = [0x01u8; 32];
        let wrong_salt = [0x99u8; 32];
        let commit = make_commit(bribe_amount, &correct_salt);

        assert_ok!(Voice::request_sting_operation(
            RuntimeOrigin::signed(ALICE),
            TARGET,
            bribe_amount,
            commit,
        ));
        assert_ok!(Voice::approve_sting(RuntimeOrigin::root(), 0));

        assert_noop!(
            Voice::reveal_and_spring_trap(RuntimeOrigin::root(), 0, wrong_salt),
            Error::<Test>::InvalidReveal,
        );
        clear_mocks();
    });
}

// ─── Double-Spring Prevention ─────────────────────────────────────────────

#[test]
fn cannot_spring_already_sprung_trap() {
    new_test_ext().execute_with(|| {
        let amount = 200 * UNIT;
        let salt = [0x11u8; 32];
        let commit = make_commit(amount, &salt);

        assert_ok!(Voice::request_sting_operation(
            RuntimeOrigin::signed(ALICE),
            TARGET,
            amount,
            commit,
        ));
        assert_ok!(Voice::approve_sting(RuntimeOrigin::root(), 0));
        assert_ok!(Voice::reveal_and_spring_trap(
            RuntimeOrigin::root(),
            0,
            salt
        ));

        assert_noop!(
            Voice::reveal_and_spring_trap(RuntimeOrigin::root(), 0, salt),
            Error::<Test>::StingAlreadySprung,
        );
        clear_mocks();
    });
}

// ─── Approve Guards ───────────────────────────────────────────────────────

#[test]
fn cannot_spring_unapproved_sting() {
    new_test_ext().execute_with(|| {
        let amount = 100 * UNIT;
        let salt = [0x22u8; 32];
        let commit = make_commit(amount, &salt);

        assert_ok!(Voice::request_sting_operation(
            RuntimeOrigin::signed(ALICE),
            TARGET,
            amount,
            commit,
        ));
        assert_noop!(
            Voice::reveal_and_spring_trap(RuntimeOrigin::root(), 0, salt),
            Error::<Test>::StingNotApproved,
        );
        clear_mocks();
    });
}

#[test]
fn cannot_approve_already_approved_sting() {
    new_test_ext().execute_with(|| {
        let amount = 100 * UNIT;
        let salt = [0x33u8; 32];
        let commit = make_commit(amount, &salt);

        assert_ok!(Voice::request_sting_operation(
            RuntimeOrigin::signed(ALICE),
            TARGET,
            amount,
            commit,
        ));
        assert_ok!(Voice::approve_sting(RuntimeOrigin::root(), 0));
        assert_noop!(
            Voice::approve_sting(RuntimeOrigin::root(), 0),
            Error::<Test>::StingAlreadyApproved,
        );
        clear_mocks();
    });
}

// ─── Sting Not Found ──────────────────────────────────────────────────────

#[test]
fn sting_not_found_returns_error() {
    new_test_ext().execute_with(|| {
        assert_noop!(
            Voice::approve_sting(RuntimeOrigin::root(), 999),
            Error::<Test>::StingNotFound,
        );
        assert_noop!(
            Voice::reveal_and_spring_trap(RuntimeOrigin::root(), 999, [0u8; 32]),
            Error::<Test>::StingNotFound,
        );
    });
}
