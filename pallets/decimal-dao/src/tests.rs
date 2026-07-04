//! Unit tests for pallet-decimal-dao.
//!
//! Covers:
//! - `instantiate_org`: happy path, duplicate org guard, council too large
//! - `sync_council`: replace council, org not found
//! - `create_proposal`: happy path, duplicate proposal, non-council proposer
//! - `vote_proposal`: yes/no votes, quorum threshold, double-vote guard
//! - `execute_proposal`: transfers from treasury, not-passed guard

use crate::mock::*;
use frame_support::{assert_noop, assert_ok};
use sp_runtime::traits::AccountIdConversion;

// ── Helpers ───────────────────────────────────────────────────────────────

/// Seed the org treasury with enough funds for proposal execution tests.
fn fund_org_treasury(org: [u8; 32], amount: Balance) {
    use frame_support::PalletId;
    use sp_runtime::traits::AccountIdConversion;
    let pallet_id = PalletId(*b"inm/ddao");
    let treasury: AccountId = pallet_id.into_sub_account_truncating(org);
    // Force-set balance via pallet_balances testing helper
    let _ = Balances::force_set_balance(RuntimeOrigin::root(), treasury, amount);
}

// ── 1. instantiate_org ───────────────────────────────────────────────────

#[test]
fn instantiate_org_happy_path() {
    new_test_ext().execute_with(|| {
        System::set_block_number(1);

        let oid = org_id(b"GUILD-ALPHA");
        assert_ok!(DecimalDao::instantiate_org(
            RuntimeOrigin::root(),
            oid,
            vec![ALICE, BOB, CHARLIE],
        ));

        assert!(crate::pallet::Councils::<Test>::contains_key(&oid));
        assert!(crate::pallet::OrgTreasuries::<Test>::contains_key(&oid));

        let council = DecimalDao::get_council(&oid).expect("council should exist");
        assert_eq!(council.len(), 3);
    });
}

#[test]
fn instantiate_org_rejects_duplicate_org_id() {
    new_test_ext().execute_with(|| {
        let oid = org_id(b"ORG-DUP");

        assert_ok!(DecimalDao::instantiate_org(
            RuntimeOrigin::root(),
            oid,
            vec![ALICE],
        ));

        assert_noop!(
            DecimalDao::instantiate_org(RuntimeOrigin::root(), oid, vec![BOB]),
            crate::pallet::Error::<Test>::OrgAlreadyExists
        );
    });
}

#[test]
fn instantiate_org_rejects_council_too_large() {
    new_test_ext().execute_with(|| {
        let oid = org_id(b"ORG-HUGE");
        // MaxCouncilMembers = 100 — try 101
        let big_council: Vec<AccountId> = (1u64..=101).collect();

        assert_noop!(
            DecimalDao::instantiate_org(RuntimeOrigin::root(), oid, big_council),
            crate::pallet::Error::<Test>::CouncilTooLarge
        );
    });
}

// ── 2. sync_council ──────────────────────────────────────────────────────

#[test]
fn sync_council_replaces_members() {
    new_test_ext().execute_with(|| {
        let oid = org_id(b"ORG-SYNC");

        assert_ok!(DecimalDao::instantiate_org(
            RuntimeOrigin::root(),
            oid,
            vec![ALICE, BOB],
        ));

        // Replace council with CHARLIE + DAVE
        assert_ok!(DecimalDao::sync_council(
            RuntimeOrigin::root(),
            oid,
            vec![CHARLIE, DAVE],
        ));

        let council = DecimalDao::get_council(&oid).unwrap();
        assert!(council.contains(&CHARLIE));
        assert!(council.contains(&DAVE));
        assert!(!council.contains(&ALICE));
    });
}

#[test]
fn sync_council_fails_org_not_found() {
    new_test_ext().execute_with(|| {
        let oid = org_id(b"ORG-NONE");

        assert_noop!(
            DecimalDao::sync_council(RuntimeOrigin::root(), oid, vec![ALICE]),
            crate::pallet::Error::<Test>::OrgNotFound
        );
    });
}

// ── 3. create_proposal ───────────────────────────────────────────────────

#[test]
fn create_proposal_happy_path() {
    new_test_ext().execute_with(|| {
        let oid = org_id(b"ORG-PROP");
        let pid = prop_id(b"PROP-001");

        assert_ok!(DecimalDao::instantiate_org(
            RuntimeOrigin::root(),
            oid,
            vec![ALICE, BOB, CHARLIE],
        ));

        assert_ok!(DecimalDao::create_proposal(
            RuntimeOrigin::root(),
            oid,
            pid,
            ALICE, // proposer (council member)
            100 * UNIT,
            BOB, // beneficiary
        ));

        let proposal = crate::pallet::Proposals::<Test>::get(&pid).expect("proposal should exist");
        assert_eq!(proposal.amount, 100 * UNIT);
        assert_eq!(proposal.beneficiary, BOB);
    });
}

#[test]
fn create_proposal_rejects_non_council_proposer() {
    new_test_ext().execute_with(|| {
        let oid = org_id(b"ORG-PROP2");
        let pid = prop_id(b"PROP-002");

        assert_ok!(DecimalDao::instantiate_org(
            RuntimeOrigin::root(),
            oid,
            vec![ALICE, BOB],
        ));

        // EVE is NOT in the council
        assert_noop!(
            DecimalDao::create_proposal(RuntimeOrigin::root(), oid, pid, EVE, 100 * UNIT, BOB,),
            crate::pallet::Error::<Test>::NotACouncilMember
        );
    });
}

#[test]
fn create_proposal_rejects_duplicate_prop_id() {
    new_test_ext().execute_with(|| {
        let oid = org_id(b"ORG-DUP2");
        let pid = prop_id(b"PROP-DUP");

        assert_ok!(DecimalDao::instantiate_org(
            RuntimeOrigin::root(),
            oid,
            vec![ALICE],
        ));

        assert_ok!(DecimalDao::create_proposal(
            RuntimeOrigin::root(),
            oid,
            pid,
            ALICE,
            50 * UNIT,
            BOB,
        ));

        assert_noop!(
            DecimalDao::create_proposal(RuntimeOrigin::root(), oid, pid, ALICE, 50 * UNIT, BOB,),
            crate::pallet::Error::<Test>::ProposalAlreadyExists
        );
    });
}

// ── 4. vote_proposal ─────────────────────────────────────────────────────

#[test]
fn vote_proposal_majority_passes_proposal() {
    new_test_ext().execute_with(|| {
        let oid = org_id(b"ORG-VOTE");
        let pid = prop_id(b"PROP-VOTE");

        assert_ok!(DecimalDao::instantiate_org(
            RuntimeOrigin::root(),
            oid,
            vec![ALICE, BOB, CHARLIE],
        ));
        assert_ok!(DecimalDao::create_proposal(
            RuntimeOrigin::root(),
            oid,
            pid,
            ALICE,
            100 * UNIT,
            DAVE,
        ));

        // 2 votes for out of 3 = majority (>50%)
        assert_ok!(DecimalDao::vote_proposal(
            RuntimeOrigin::root(),
            pid,
            ALICE,
            true,
        ));
        assert_ok!(DecimalDao::vote_proposal(
            RuntimeOrigin::root(),
            pid,
            BOB,
            true,
        ));

        let proposal = crate::pallet::Proposals::<Test>::get(&pid).unwrap();
        assert_eq!(proposal.status, crate::pallet::ProposalStatus::Passed);
    });
}

#[test]
fn vote_proposal_majority_rejects_proposal() {
    new_test_ext().execute_with(|| {
        let oid = org_id(b"ORG-REJ");
        let pid = prop_id(b"PROP-REJ");

        assert_ok!(DecimalDao::instantiate_org(
            RuntimeOrigin::root(),
            oid,
            vec![ALICE, BOB, CHARLIE],
        ));
        assert_ok!(DecimalDao::create_proposal(
            RuntimeOrigin::root(),
            oid,
            pid,
            ALICE,
            100 * UNIT,
            DAVE,
        ));

        assert_ok!(DecimalDao::vote_proposal(
            RuntimeOrigin::root(),
            pid,
            ALICE,
            false
        ));
        assert_ok!(DecimalDao::vote_proposal(
            RuntimeOrigin::root(),
            pid,
            BOB,
            false
        ));

        let proposal = crate::pallet::Proposals::<Test>::get(&pid).unwrap();
        assert_eq!(proposal.status, crate::pallet::ProposalStatus::Rejected);
    });
}

#[test]
fn vote_proposal_prevents_double_vote() {
    new_test_ext().execute_with(|| {
        let oid = org_id(b"ORG-DBL");
        let pid = prop_id(b"PROP-DBL");

        assert_ok!(DecimalDao::instantiate_org(
            RuntimeOrigin::root(),
            oid,
            vec![ALICE, BOB],
        ));
        assert_ok!(DecimalDao::create_proposal(
            RuntimeOrigin::root(),
            oid,
            pid,
            ALICE,
            10 * UNIT,
            BOB,
        ));

        assert_ok!(DecimalDao::vote_proposal(
            RuntimeOrigin::root(),
            pid,
            ALICE,
            true
        ));

        assert_noop!(
            DecimalDao::vote_proposal(RuntimeOrigin::root(), pid, ALICE, true),
            crate::pallet::Error::<Test>::AlreadyVoted
        );
    });
}

#[test]
fn vote_proposal_fails_non_council_voter() {
    new_test_ext().execute_with(|| {
        let oid = org_id(b"ORG-NCV");
        let pid = prop_id(b"PROP-NCV");

        assert_ok!(DecimalDao::instantiate_org(
            RuntimeOrigin::root(),
            oid,
            vec![ALICE, BOB],
        ));
        assert_ok!(DecimalDao::create_proposal(
            RuntimeOrigin::root(),
            oid,
            pid,
            ALICE,
            10 * UNIT,
            BOB,
        ));

        assert_noop!(
            DecimalDao::vote_proposal(RuntimeOrigin::root(), pid, EVE, true),
            crate::pallet::Error::<Test>::NotACouncilMember
        );
    });
}

// ── 5. execute_proposal ──────────────────────────────────────────────────

#[test]
fn execute_proposal_transfers_funds() {
    new_test_ext().execute_with(|| {
        System::set_block_number(1);

        let oid = org_id(b"ORG-EXEC");
        let pid = prop_id(b"PROP-EXEC");
        let amount = 500 * UNIT;

        assert_ok!(DecimalDao::instantiate_org(
            RuntimeOrigin::root(),
            oid,
            vec![ALICE, BOB, CHARLIE],
        ));

        // Seed the derived treasury
        fund_org_treasury(oid, 10_000 * UNIT);

        assert_ok!(DecimalDao::create_proposal(
            RuntimeOrigin::root(),
            oid,
            pid,
            ALICE,
            amount,
            DAVE,
        ));

        // Pass proposal
        assert_ok!(DecimalDao::vote_proposal(
            RuntimeOrigin::root(),
            pid,
            ALICE,
            true
        ));
        assert_ok!(DecimalDao::vote_proposal(
            RuntimeOrigin::root(),
            pid,
            BOB,
            true
        ));

        let dave_before = Balances::free_balance(DAVE);

        assert_ok!(DecimalDao::execute_proposal(RuntimeOrigin::root(), pid));

        assert!(Balances::free_balance(DAVE) >= dave_before + amount);
        // Proposal record removed after execution
        assert!(crate::pallet::Proposals::<Test>::get(&pid).is_none());
    });
}

#[test]
fn execute_proposal_fails_if_not_passed() {
    new_test_ext().execute_with(|| {
        let oid = org_id(b"ORG-EX2");
        let pid = prop_id(b"PROP-EX2");

        assert_ok!(DecimalDao::instantiate_org(
            RuntimeOrigin::root(),
            oid,
            vec![ALICE, BOB],
        ));
        assert_ok!(DecimalDao::create_proposal(
            RuntimeOrigin::root(),
            oid,
            pid,
            ALICE,
            10 * UNIT,
            BOB,
        ));

        // No votes — still Open
        assert_noop!(
            DecimalDao::execute_proposal(RuntimeOrigin::root(), pid),
            crate::pallet::Error::<Test>::ProposalNotPassed
        );
    });
}
