//! Unit tests for pallet-khural-governance.
//!
//! Covers:
//!  1. Proposal lifecycle (create → vote → execute)
//!  2. Quorum and majority rules
//!  3. Cross-nation voting prevention
//!  4. Double-vote prevention
//!  5. on_initialize auto-enactment
//!  6. Expert bill: propose (Academician gate), vote (role gate), execute + deposit slash/refund

#![cfg(test)]

use crate::mock::*;
use crate::pallet::{Error, ProposalStatus};
use frame_support::{
    assert_noop, assert_ok,
    traits::{Currency, Hooks, ReservableCurrency},
};

/// Helper: free balance of an account.
fn free_bal(who: AccountId) -> Balance {
    <pallet_balances::Pallet<Test> as Currency<AccountId>>::free_balance(&who)
}
/// Helper: reserved balance of an account.
fn reserved_bal(who: AccountId) -> Balance {
    <pallet_balances::Pallet<Test> as ReservableCurrency<AccountId>>::reserved_balance(&who)
}

// ─── Proposal Lifecycle ────────────────────────────────────────────────────

#[test]
fn create_proposal_emits_event_and_stores_record() {
    new_test_ext().execute_with(|| {
        assert_ok!(Khural::create_proposal(
            RuntimeOrigin::signed(ALICE),
            1,          // nation_id
            100 * UNIT, // amount
            CHARLIE,    // beneficiary
            None,       // no constitutional basis
        ));
        let p = Khural::proposals(0).expect("proposal 0 must exist");
        assert_eq!(p.nation_id, 1);
        assert_eq!(p.amount, 100 * UNIT);
        assert_eq!(p.beneficiary, CHARLIE);
        assert_eq!(p.status, ProposalStatus::Active);
        assert_eq!(p.votes_for, 0);
        assert_eq!(p.votes_against, 0);
    });
}

#[test]
fn unregistered_citizen_cannot_propose() {
    new_test_ext().execute_with(|| {
        assert_noop!(
            Khural::create_proposal(
                RuntimeOrigin::signed(99), // not in Citizens
                1,
                10 * UNIT,
                CHARLIE,
                None,
            ),
            Error::<Test>::NotRegistered,
        );
    });
}

#[test]
fn wrong_nation_cannot_propose_for_other_nation() {
    new_test_ext().execute_with(|| {
        // EVE is nation 2, trying to create nation 1 proposal
        assert_noop!(
            Khural::create_proposal(
                RuntimeOrigin::signed(EVE),
                1, // wrong nation
                10 * UNIT,
                CHARLIE,
                None,
            ),
            Error::<Test>::WrongNation,
        );
    });
}

#[test]
fn vote_records_approve_and_reject_counts() {
    new_test_ext().execute_with(|| {
        assert_ok!(Khural::create_proposal(
            RuntimeOrigin::signed(ALICE),
            1,
            10 * UNIT,
            CHARLIE,
            None,
        ));
        // ALICE votes approve
        assert_ok!(Khural::vote(RuntimeOrigin::signed(ALICE), 0, true));
        // BOB votes reject
        assert_ok!(Khural::vote(RuntimeOrigin::signed(BOB), 0, false));

        let p = Khural::proposals(0).unwrap();
        assert_eq!(p.votes_for, 1);
        assert_eq!(p.votes_against, 1);
    });
}

#[test]
fn double_vote_rejected() {
    new_test_ext().execute_with(|| {
        assert_ok!(Khural::create_proposal(
            RuntimeOrigin::signed(ALICE),
            1,
            10 * UNIT,
            CHARLIE,
            None,
        ));
        assert_ok!(Khural::vote(RuntimeOrigin::signed(ALICE), 0, true));
        assert_noop!(
            Khural::vote(RuntimeOrigin::signed(ALICE), 0, true),
            Error::<Test>::AlreadyVoted,
        );
    });
}

#[test]
fn wrong_nation_cannot_vote_on_proposal() {
    new_test_ext().execute_with(|| {
        assert_ok!(Khural::create_proposal(
            RuntimeOrigin::signed(ALICE),
            1,
            10 * UNIT,
            CHARLIE,
            None,
        ));
        // EVE is nation 2
        assert_noop!(
            Khural::vote(RuntimeOrigin::signed(EVE), 0, true),
            Error::<Test>::WrongNation,
        );
    });
}

#[test]
fn execute_proposal_transfers_funds_on_quorum_met() {
    new_test_ext().execute_with(|| {
        let before = free_bal(CHARLIE);
        assert_ok!(Khural::create_proposal(
            RuntimeOrigin::signed(ALICE),
            1,
            100 * UNIT,
            CHARLIE,
            None,
        ));
        // 3 votes approve → quorum met (MinQuorum = 3)
        assert_ok!(Khural::vote(RuntimeOrigin::signed(ALICE), 0, true));
        assert_ok!(Khural::vote(RuntimeOrigin::signed(BOB), 0, true));
        assert_ok!(Khural::vote(RuntimeOrigin::signed(CHARLIE), 0, true));

        // Advance past VotingPeriod (100 blocks)
        System::set_block_number(102);
        assert_ok!(Khural::execute_proposal(RuntimeOrigin::signed(ALICE), 0));

        let p = Khural::proposals(0).unwrap();
        assert_eq!(p.status, ProposalStatus::Executed);
        assert_eq!(free_bal(CHARLIE), before + 100 * UNIT);
    });
}

#[test]
fn execute_proposal_rejected_when_insufficient_votes() {
    new_test_ext().execute_with(|| {
        let before = free_bal(CHARLIE);
        assert_ok!(Khural::create_proposal(
            RuntimeOrigin::signed(ALICE),
            1,
            100 * UNIT,
            CHARLIE,
            None,
        ));
        // Only 2 approve votes — quorum is 3
        assert_ok!(Khural::vote(RuntimeOrigin::signed(ALICE), 0, true));
        assert_ok!(Khural::vote(RuntimeOrigin::signed(BOB), 0, true));

        System::set_block_number(102);
        assert_ok!(Khural::execute_proposal(RuntimeOrigin::signed(ALICE), 0));

        let p = Khural::proposals(0).unwrap();
        assert_eq!(p.status, ProposalStatus::Rejected);
        // No funds transferred
        assert_eq!(free_bal(CHARLIE), before);
    });
}

#[test]
fn execute_proposal_rejected_when_majority_against() {
    new_test_ext().execute_with(|| {
        // Create a proposal; nobody votes (0 for, 0 against)
        // MinQuorum = 3 → votes_for=0 < 3 → Rejected
        assert_ok!(Khural::create_proposal(
            RuntimeOrigin::signed(ALICE),
            1,
            50 * UNIT,
            BOB,
            None,
        ));
        // Only 1 opposing vote — quorum for FOR is not met anyway
        assert_ok!(Khural::vote(RuntimeOrigin::signed(ALICE), 0, false));
        assert_ok!(Khural::vote(RuntimeOrigin::signed(BOB), 0, false));
        assert_ok!(Khural::vote(RuntimeOrigin::signed(CHARLIE), 0, false));
        // votes_for=0, votes_against=3 → quorum_met(for=0>=3)=false → Rejected
        System::set_block_number(102);
        assert_ok!(Khural::execute_proposal(RuntimeOrigin::signed(ALICE), 0));
        let p = Khural::proposals(0).unwrap();
        assert_eq!(p.status, ProposalStatus::Rejected);
    });
}

#[test]
fn cannot_execute_proposal_before_voting_window_ends() {
    new_test_ext().execute_with(|| {
        assert_ok!(Khural::create_proposal(
            RuntimeOrigin::signed(ALICE),
            1,
            10 * UNIT,
            CHARLIE,
            None,
        ));
        // Still at block 1 — window is 100 blocks
        assert_noop!(
            Khural::execute_proposal(RuntimeOrigin::signed(ALICE), 0),
            Error::<Test>::ProposalNotActive,
        );
    });
}

// ─── on_initialize Auto-Enactment ─────────────────────────────────────────

#[test]
fn on_initialize_enacts_expired_proposal() {
    new_test_ext().execute_with(|| {
        let before = free_bal(CHARLIE);
        assert_ok!(Khural::create_proposal(
            RuntimeOrigin::signed(ALICE),
            1,
            50 * UNIT,
            CHARLIE,
            None,
        ));
        // 3 approve votes
        assert_ok!(Khural::vote(RuntimeOrigin::signed(ALICE), 0, true));
        assert_ok!(Khural::vote(RuntimeOrigin::signed(BOB), 0, true));
        assert_ok!(Khural::vote(RuntimeOrigin::signed(CHARLIE), 0, true));

        // Trigger on_initialize at block 102
        System::set_block_number(102);
        Khural::on_initialize(102u32.into());

        let p = Khural::proposals(0).unwrap();
        assert_eq!(p.status, ProposalStatus::Executed);
        assert_eq!(free_bal(CHARLIE), before + 50 * UNIT);
    });
}

#[test]
fn vote_after_end_block_rejected() {
    new_test_ext().execute_with(|| {
        assert_ok!(Khural::create_proposal(
            RuntimeOrigin::signed(ALICE),
            1,
            10 * UNIT,
            CHARLIE,
            None,
        ));
        System::set_block_number(102); // past end_block
        assert_noop!(
            Khural::vote(RuntimeOrigin::signed(ALICE), 0, true),
            Error::<Test>::ProposalNotActive,
        );
    });
}

// ─── Expert Bill Lifecycle ─────────────────────────────────────────────────

#[test]
fn non_academician_cannot_propose_expert_bill() {
    new_test_ext().execute_with(|| {
        // ALICE is not an academician
        assert_noop!(
            Khural::propose_expert_bill(
                RuntimeOrigin::signed(ALICE),
                sp_core::H256::repeat_byte(0xAB),
                b"mining".to_vec(),
            ),
            Error::<Test>::NotAcademician,
        );
    });
}

#[test]
fn academician_can_propose_expert_bill_and_deposit_is_reserved() {
    new_test_ext().execute_with(|| {
        add_academician(ALICE);
        let free_before = free_bal(ALICE);

        assert_ok!(Khural::propose_expert_bill(
            RuntimeOrigin::signed(ALICE),
            sp_core::H256::repeat_byte(0xDE),
            b"energy".to_vec(),
        ));

        // Deposit (10 ALTAN) should be reserved, not free
        assert_eq!(free_bal(ALICE), free_before - 10 * UNIT,);
        assert_eq!(reserved_bal(ALICE), 10 * UNIT);

        let bill = Khural::expert_bills(0).expect("bill 0 must exist");
        assert_eq!(bill.status, ProposalStatus::Active);
        assert_eq!(bill.votes_for, 0);

        clear_academicians();
    });
}

#[test]
fn non_arbad_leader_cannot_vote_on_expert_bill() {
    new_test_ext().execute_with(|| {
        add_academician(ALICE);
        assert_ok!(Khural::propose_expert_bill(
            RuntimeOrigin::signed(ALICE),
            sp_core::H256::repeat_byte(0xAA),
            b"tech".to_vec(),
        ));
        // ALICE is Regular — not ArbadLeader+
        assert_noop!(
            Khural::vote_on_expert_bill(RuntimeOrigin::signed(ALICE), 0, true),
            Error::<Test>::InsufficientRole,
        );
        clear_academicians();
    });
}

#[test]
fn arbad_leader_can_vote_on_expert_bill() {
    new_test_ext().execute_with(|| {
        add_academician(ALICE);
        assert_ok!(Khural::propose_expert_bill(
            RuntimeOrigin::signed(ALICE),
            sp_core::H256::repeat_byte(0xBB),
            b"law".to_vec(),
        ));
        // DAVE is ArbadLeader
        assert_ok!(Khural::vote_on_expert_bill(
            RuntimeOrigin::signed(DAVE),
            0,
            true
        ));
        let bill = Khural::expert_bills(0).unwrap();
        assert_eq!(bill.votes_for, 1);
        clear_academicians();
    });
}

#[test]
fn expert_bill_double_vote_rejected() {
    new_test_ext().execute_with(|| {
        add_academician(ALICE);
        assert_ok!(Khural::propose_expert_bill(
            RuntimeOrigin::signed(ALICE),
            sp_core::H256::repeat_byte(0xCC),
            b"finance".to_vec(),
        ));
        assert_ok!(Khural::vote_on_expert_bill(
            RuntimeOrigin::signed(DAVE),
            0,
            true
        ));
        assert_noop!(
            Khural::vote_on_expert_bill(RuntimeOrigin::signed(DAVE), 0, true),
            Error::<Test>::AlreadyVoted,
        );
        clear_academicians();
    });
}

#[test]
fn expert_bill_passes_deposit_returned_to_academician() {
    new_test_ext().execute_with(|| {
        add_academician(ALICE);
        // Seed more ArbadLeaders so we can reach 3 votes (votes_for > 2 → passes)
        seed_citizen(20, 1, CitizenRole::ArbadLeader);
        seed_citizen(21, 1, CitizenRole::ArbadLeader);
        pallet_balances::Pallet::<Test>::force_set_balance(RuntimeOrigin::root(), 20, 100 * UNIT)
            .ok();
        pallet_balances::Pallet::<Test>::force_set_balance(RuntimeOrigin::root(), 21, 100 * UNIT)
            .ok();

        let before = free_bal(ALICE);
        assert_ok!(Khural::propose_expert_bill(
            RuntimeOrigin::signed(ALICE),
            sp_core::H256::repeat_byte(0xDD),
            b"science".to_vec(),
        ));

        // 3 approve votes → quorum (votes_for > 2)
        assert_ok!(Khural::vote_on_expert_bill(
            RuntimeOrigin::signed(DAVE),
            0,
            true
        ));
        assert_ok!(Khural::vote_on_expert_bill(
            RuntimeOrigin::signed(20),
            0,
            true
        ));
        assert_ok!(Khural::vote_on_expert_bill(
            RuntimeOrigin::signed(21),
            0,
            true
        ));

        assert_ok!(Khural::execute_expert_bill(RuntimeOrigin::signed(ALICE), 0));

        let bill = Khural::expert_bills(0).unwrap();
        assert_eq!(bill.status, ProposalStatus::Executed);
        // Deposit must be returned
        assert_eq!(free_bal(ALICE), before);
        assert_eq!(reserved_bal(ALICE), 0);

        clear_academicians();
    });
}

#[test]
fn expert_bill_rejected_deposit_slashed() {
    new_test_ext().execute_with(|| {
        add_academician(ALICE);
        let before = free_bal(ALICE);

        assert_ok!(Khural::propose_expert_bill(
            RuntimeOrigin::signed(ALICE),
            sp_core::H256::repeat_byte(0xEE),
            b"infrastructure".to_vec(),
        ));
        // Only 1 vote — insufficient for quorum (votes_for > 2 required)
        assert_ok!(Khural::vote_on_expert_bill(
            RuntimeOrigin::signed(DAVE),
            0,
            true
        ));

        assert_ok!(Khural::execute_expert_bill(RuntimeOrigin::signed(ALICE), 0));

        let bill = Khural::expert_bills(0).unwrap();
        assert_eq!(bill.status, ProposalStatus::Rejected);
        // Deposit slashed — ALICE lost 10 ALTAN permanently
        assert_eq!(free_bal(ALICE), before - 10 * UNIT);
        assert_eq!(reserved_bal(ALICE), 0);

        clear_academicians();
    });
}

#[test]
fn proposal_not_found_errors_correctly() {
    new_test_ext().execute_with(|| {
        assert_noop!(
            Khural::vote(RuntimeOrigin::signed(ALICE), 999, true),
            Error::<Test>::ProposalNotFound,
        );
        System::set_block_number(102);
        assert_noop!(
            Khural::execute_proposal(RuntimeOrigin::signed(ALICE), 999),
            Error::<Test>::ProposalNotFound,
        );
    });
}

#[test]
fn constitutional_basis_hash_stored() {
    new_test_ext().execute_with(|| {
        let basis = [0x42u8; 32];
        assert_ok!(Khural::create_proposal(
            RuntimeOrigin::signed(ALICE),
            1,
            5 * UNIT,
            BOB,
            Some(basis),
        ));
        let p = Khural::proposals(0).unwrap();
        assert_eq!(p.constitutional_basis, Some(basis));
    });
}
