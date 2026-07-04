//! Unit tests for pallet-inomad-elections — V4 Constitutional Governance.
//!
//!   A. Decimal Hierarchy
//!   B. Branch Council Election
//!   C. Supreme Leader Election
//!   D. Khural Chairman Election
//!   E. Reset Ballot
//!   G. Audit Fixes (H-1, H-2, H-4)
//!   H. Constitutional Size Requirements (MinArbadSize / MinZunLeaders / MinMyangadLeaders)
//!   I. Indigenous Restriction (Хурал — Legislative branch)

use crate::mock::*;
use crate::pallet::{
    ArbadMemberCount, ElectedLeaders, ElectionLevel, Error, Event, GovernmentBranch,
    MyangadLeaderCount, ZunLeaderCount,
};
use frame_support::{assert_noop, assert_ok};
use sp_runtime::DispatchError;

// =============================================================================
// A. DECIMAL HIERARCHY (regression tests — unchanged logic)
// =============================================================================

#[test]
fn a1_arbad_vote_fails_when_not_in_arbad() {
    new_test_ext().execute_with(|| {
        assert_noop!(
            Elections::cast_vote(RuntimeOrigin::signed(CHARLIE), ARBAD_ELECTION_ID, BOB),
            Error::<Test>::NotInArbad
        );
    });
}

#[test]
fn a2_arbad_vote_succeeds_for_valid_citizen() {
    new_test_ext().execute_with(|| {
        assert_ok!(Elections::cast_vote(
            RuntimeOrigin::signed(ALICE),
            ARBAD_ELECTION_ID,
            BOB
        ));
        assert_eq!(
            crate::pallet::VoteCounts::<Test>::get(ARBAD_ELECTION_ID, BOB),
            1
        );
    });
}

#[test]
fn a3_double_vote_fails() {
    new_test_ext().execute_with(|| {
        assert_ok!(Elections::cast_vote(
            RuntimeOrigin::signed(ALICE),
            ARBAD_ELECTION_ID,
            BOB
        ));
        assert_noop!(
            Elections::cast_vote(RuntimeOrigin::signed(ALICE), ARBAD_ELECTION_ID, BOB),
            Error::<Test>::AlreadyVoted
        );
    });
}

#[test]
fn a4_zun_blocked_for_arbad_citizen() {
    new_test_ext().execute_with(|| {
        assert_noop!(
            Elections::cast_vote(RuntimeOrigin::signed(ALICE), ZUN_ELECTION_ID, DAVE),
            Error::<Test>::NotAuthorizedForLevel
        );
    });
}

#[test]
fn a5_4tier_flow_all_boundaries() {
    new_test_ext().execute_with(|| {
        // Zun leader → Zun: passes, Myangad: blocked
        assert_ok!(Elections::cast_vote(
            RuntimeOrigin::signed(DAVE),
            ZUN_ELECTION_ID,
            EVE
        ));
        assert_noop!(
            Elections::cast_vote(RuntimeOrigin::signed(DAVE), MYANGAD_ELECTION_ID, EVE),
            Error::<Test>::NotAuthorizedForLevel
        );
        // Myangad leader → Myangad: passes, Tumed: passes (EVE has Myangad, not Tumed)
        assert_ok!(Elections::cast_vote(
            RuntimeOrigin::signed(EVE),
            MYANGAD_ELECTION_ID,
            FRANK
        ));
        assert_noop!(
            Elections::cast_vote(RuntimeOrigin::signed(EVE), TUMED_ELECTION_ID, FRANK),
            Error::<Test>::NotAuthorizedForLevel
        );
        // Tumed leader → Tumed: passes
        assert_ok!(Elections::cast_vote(
            RuntimeOrigin::signed(FRANK),
            TUMED_ELECTION_ID,
            FRANK
        ));
    });
}

// =============================================================================
// B. BRANCH COUNCIL ELECTION
// =============================================================================

// ── B1: Authorization checks ──────────────────────────────────────────────────

/// Non-Tumed citizen cannot vote in a Branch Council election.
#[test]
fn b1_branch_council_vote_fails_for_non_tumed() {
    new_test_ext().execute_with(|| {
        // ALICE is only an Arbad citizen
        assert_noop!(
            Elections::elect_branch_council(
                RuntimeOrigin::signed(ALICE),
                GovernmentBranch::Executive,
                FRANK
            ),
            Error::<Test>::NotATumedLeader
        );
    });
}

/// Myangad-level leader is also blocked — must be exactly Tumed.
#[test]
fn b2_branch_council_vote_fails_for_myangad_leader() {
    new_test_ext().execute_with(|| {
        assert_noop!(
            Elections::elect_branch_council(
                RuntimeOrigin::signed(EVE), // Myangad rights only
                GovernmentBranch::Judicial,
                FRANK
            ),
            Error::<Test>::NotATumedLeader
        );
    });
}

/// Legislative branch is disallowed — must use `elect_khural_chairman`.
#[test]
fn b3_branch_council_rejects_legislative_branch() {
    new_test_ext().execute_with(|| {
        assert_noop!(
            Elections::elect_branch_council(
                RuntimeOrigin::signed(FRANK),
                GovernmentBranch::Legislative,
                GRACE
            ),
            Error::<Test>::LegislativeUsesKhuralPath
        );
    });
}

// ── B2: Successful vote + tally ───────────────────────────────────────────────

/// FRANK (Tumed) can vote in the Executive branch council election.
#[test]
fn b4_branch_council_vote_succeeds_for_tumed_leader() {
    new_test_ext().execute_with(|| {
        assert_ok!(Elections::elect_branch_council(
            RuntimeOrigin::signed(FRANK),
            GovernmentBranch::Executive,
            GRACE
        ));

        assert_eq!(
            crate::pallet::BranchVotes::<Test>::get(GovernmentBranch::Executive, FRANK),
            Some(GRACE)
        );
        assert_eq!(
            crate::pallet::BranchVoteCounts::<Test>::get(GovernmentBranch::Executive, GRACE),
            1
        );

        System::assert_last_event(RuntimeEvent::Elections(Event::BranchVoteCast {
            branch: GovernmentBranch::Executive,
            voter: FRANK,
            candidate: GRACE,
        }));
    });
}

/// Double-voting in the same branch is blocked.
#[test]
fn b5_branch_council_double_vote_fails() {
    new_test_ext().execute_with(|| {
        assert_ok!(Elections::elect_branch_council(
            RuntimeOrigin::signed(FRANK),
            GovernmentBranch::Executive,
            GRACE
        ));
        assert_noop!(
            Elections::elect_branch_council(
                RuntimeOrigin::signed(FRANK),
                GovernmentBranch::Executive,
                GRACE
            ),
            Error::<Test>::AlreadyVoted
        );
    });
}

/// Votes across different branches are independent.
#[test]
fn b6_branch_council_votes_are_per_branch() {
    new_test_ext().execute_with(|| {
        // FRANK votes Executive
        assert_ok!(Elections::elect_branch_council(
            RuntimeOrigin::signed(FRANK),
            GovernmentBranch::Executive,
            GRACE
        ));
        // Same voter can vote Judicial (different branch)
        assert_ok!(Elections::elect_branch_council(
            RuntimeOrigin::signed(FRANK),
            GovernmentBranch::Judicial,
            HEIDI
        ));

        assert_eq!(
            crate::pallet::BranchVoteCounts::<Test>::get(GovernmentBranch::Executive, GRACE),
            1
        );
        assert_eq!(
            crate::pallet::BranchVoteCounts::<Test>::get(GovernmentBranch::Judicial, HEIDI),
            1
        );
    });
}

// ── B3: confirm_branch_council ────────────────────────────────────────────────

/// Root can confirm the top-9 as the Branch Council.
/// C-1 fix: each winner must have ≥ 1 on-chain vote before confirmation.
#[test]
fn b7_confirm_branch_council_succeeds_with_exactly_9() {
    new_test_ext().execute_with(|| {
        // Each of the 9 COUNCIL members votes for themselves.
        // This gives each exactly 1 on-chain vote in BranchVoteCounts.
        for &voter in &COUNCIL_9 {
            assert_ok!(Elections::elect_branch_council(
                RuntimeOrigin::signed(voter),
                GovernmentBranch::Executive,
                voter // self-nomination
            ));
        }

        // All 9 have ≥ 1 vote → confirm must succeed.
        assert_ok!(Elections::confirm_branch_council(
            RuntimeOrigin::root(),
            GovernmentBranch::Executive,
            COUNCIL_9.to_vec()
        ));

        let stored = crate::pallet::BranchCouncils::<Test>::get(GovernmentBranch::Executive)
            .expect("council must be stored");
        assert_eq!(stored.as_slice(), &COUNCIL_9);

        System::assert_last_event(RuntimeEvent::Elections(Event::BranchCouncilElected {
            branch: GovernmentBranch::Executive,
            council: stored,
        }));
    });
}

/// Confirming with fewer than 9 members fails.
#[test]
fn b8_confirm_branch_council_fails_with_wrong_count() {
    new_test_ext().execute_with(|| {
        assert_noop!(
            Elections::confirm_branch_council(
                RuntimeOrigin::root(),
                GovernmentBranch::Banking,
                vec![FRANK, GRACE] // only 2
            ),
            Error::<Test>::InsufficientCandidates
        );
    });
}

/// Non-root cannot confirm a council.
#[test]
fn b9_confirm_branch_council_requires_root() {
    new_test_ext().execute_with(|| {
        assert_noop!(
            Elections::confirm_branch_council(
                RuntimeOrigin::signed(FRANK),
                GovernmentBranch::Executive,
                COUNCIL_9.to_vec()
            ),
            DispatchError::BadOrigin
        );
    });
}

// ── F1: C-1 regression — confirm rejects unvoted winners ─────────────────────

/// C-1 audit fix: `confirm_branch_council` must reject any winner with 0 on-chain votes.
///
/// Before this fix, Root could arbitrarily seat *any* `AccountId` (including
/// accounts that received zero votes) by supplying their address in the
/// `winners` list. This test proves the guard is active.
#[test]
fn f1_confirm_branch_council_rejects_unvoted_winner() {
    new_test_ext().execute_with(|| {
        // Only FRANK and GRACE vote (for themselves).
        assert_ok!(Elections::elect_branch_council(
            RuntimeOrigin::signed(FRANK),
            GovernmentBranch::Judicial,
            FRANK
        ));
        assert_ok!(Elections::elect_branch_council(
            RuntimeOrigin::signed(GRACE),
            GovernmentBranch::Judicial,
            GRACE
        ));

        // Build a winner list that includes IVAN — who has 0 votes.
        // The pallet must reject this.
        let mut rigged_winners = COUNCIL_9.to_vec();
        *rigged_winners.last_mut().unwrap() = IVAN; // swap last entry to an unvoted account
                                                    // Remove duplicates if IVAN was already in COUNCIL_9: replace with OSCAR instead.
                                                    // OSCAR is a Tumed leader not in COUNCIL_9 and has 0 votes.
        let rigged_winners: Vec<AccountId> =
            [FRANK, GRACE, OSCAR, IVAN, JUDY, KARL, LENA, MIKE, NINA].to_vec();
        // OSCAR and IVAN have 0 votes — either one triggers the error.

        assert_noop!(
            Elections::confirm_branch_council(
                RuntimeOrigin::root(),
                GovernmentBranch::Judicial,
                rigged_winners
            ),
            Error::<Test>::WinnerDidNotReceiveVotes
        );
    });
}

// =============================================================================
// C. SUPREME LEADER ELECTION
// =============================================================================

// ── C1: Authorization checks ──────────────────────────────────────────────────

/// Cannot vote for Supreme Leader if no council exists yet.
#[test]
fn c1_supreme_leader_fails_without_council() {
    new_test_ext().execute_with(|| {
        assert_noop!(
            Elections::elect_supreme_leader(
                RuntimeOrigin::signed(FRANK),
                GovernmentBranch::Executive,
                GRACE
            ),
            Error::<Test>::CouncilNotElected
        );
    });
}

/// A Tumed leader who is NOT in the council cannot vote for Supreme Leader.
#[test]
fn c2_supreme_leader_fails_for_non_council_member() {
    new_test_ext().execute_with(|| {
        // Seat FRANK..NINA as council (OSCAR is NOT in the council)
        seed_council(GovernmentBranch::Executive, COUNCIL_9.to_vec());

        assert_noop!(
            Elections::elect_supreme_leader(
                RuntimeOrigin::signed(OSCAR), // Tumed but not a council member
                GovernmentBranch::Executive,
                FRANK
            ),
            Error::<Test>::NotACouncilMember
        );
    });
}

// ── C2: Successful Supreme Leader election ────────────────────────────────────

/// Council member can cast a vote for the Supreme Leader.
#[test]
fn c3_supreme_leader_vote_succeeds_for_council_member() {
    new_test_ext().execute_with(|| {
        seed_council(GovernmentBranch::Executive, COUNCIL_9.to_vec());

        assert_ok!(Elections::elect_supreme_leader(
            RuntimeOrigin::signed(FRANK),
            GovernmentBranch::Executive,
            GRACE
        ));

        assert_eq!(
            crate::pallet::SupremeLeaderVotes::<Test>::get(GovernmentBranch::Executive, FRANK),
            Some(GRACE)
        );
        assert_eq!(
            crate::pallet::SupremeLeaderVoteCounts::<Test>::get(GovernmentBranch::Executive, GRACE),
            1
        );
        System::assert_last_event(RuntimeEvent::Elections(Event::SupremeLeaderVoteCast {
            branch: GovernmentBranch::Executive,
            voter: FRANK,
            candidate: GRACE,
        }));
    });
}

/// All 9 council members vote — tallies accumulate correctly.
#[test]
fn c4_supreme_leader_tallies_all_9_council_votes() {
    new_test_ext().execute_with(|| {
        seed_council(GovernmentBranch::Executive, COUNCIL_9.to_vec());

        // 8 members vote for GRACE, 1 (NINA) votes for FRANK
        for &member in &[FRANK, GRACE, HEIDI, IVAN, JUDY, KARL, LENA, MIKE] {
            assert_ok!(Elections::elect_supreme_leader(
                RuntimeOrigin::signed(member),
                GovernmentBranch::Executive,
                GRACE
            ));
        }
        assert_ok!(Elections::elect_supreme_leader(
            RuntimeOrigin::signed(NINA),
            GovernmentBranch::Executive,
            FRANK
        ));

        assert_eq!(
            crate::pallet::SupremeLeaderVoteCounts::<Test>::get(GovernmentBranch::Executive, GRACE),
            8
        );
        assert_eq!(
            crate::pallet::SupremeLeaderVoteCounts::<Test>::get(GovernmentBranch::Executive, FRANK),
            1
        );
    });
}

/// Double-voting in the Supreme Leader election is blocked.
#[test]
fn c5_supreme_leader_double_vote_fails() {
    new_test_ext().execute_with(|| {
        seed_council(GovernmentBranch::Executive, COUNCIL_9.to_vec());

        assert_ok!(Elections::elect_supreme_leader(
            RuntimeOrigin::signed(FRANK),
            GovernmentBranch::Executive,
            GRACE
        ));
        assert_noop!(
            Elections::elect_supreme_leader(
                RuntimeOrigin::signed(FRANK),
                GovernmentBranch::Executive,
                GRACE
            ),
            Error::<Test>::AlreadyVoted
        );
    });
}

/// Root can confirm a Supreme Leader — `SupremeLeaders` storage is written.
#[test]
fn c6_confirm_supreme_leader_writes_storage_and_emits_event() {
    new_test_ext().execute_with(|| {
        seed_council(GovernmentBranch::Executive, COUNCIL_9.to_vec());

        // Cast at least one vote so there is a winner
        assert_ok!(Elections::elect_supreme_leader(
            RuntimeOrigin::signed(FRANK),
            GovernmentBranch::Executive,
            GRACE
        ));

        // Root finalises
        assert_ok!(Elections::confirm_supreme_leader(
            RuntimeOrigin::root(),
            GovernmentBranch::Executive,
            GRACE
        ));

        assert_eq!(
            crate::pallet::SupremeLeaders::<Test>::get(GovernmentBranch::Executive),
            Some(GRACE)
        );
        System::assert_last_event(RuntimeEvent::Elections(Event::SupremeLeaderElected {
            branch: GovernmentBranch::Executive,
            leader: GRACE,
        }));
    });
}

/// Legislative branch is rejected in `confirm_supreme_leader`.
#[test]
fn c7_confirm_supreme_leader_rejects_legislative() {
    new_test_ext().execute_with(|| {
        assert_noop!(
            Elections::confirm_supreme_leader(
                RuntimeOrigin::root(),
                GovernmentBranch::Legislative,
                HEIDI
            ),
            Error::<Test>::LegislativeUsesKhuralPath
        );
    });
}

/// C-3 fix: `elect_supreme_leader` must ALSO reject the Legislative branch.
///
/// Constitutional separation (AGENTS.md §2.6): the Khural has a Chairman,
/// not a Supreme Leader. Before this fix, a council seeded for Legislative
/// could accept votes via `elect_supreme_leader`, silently writing
/// `SupremeLeaders[Legislative]` and violating branch separation.
#[test]
fn c8_elect_supreme_leader_rejects_legislative_branch() {
    new_test_ext().execute_with(|| {
        // Artificially seed a Legislative council (should never happen in prod,
        // but tests the defence-in-depth guard inside elect_supreme_leader).
        seed_council(GovernmentBranch::Legislative, COUNCIL_9.to_vec());

        // Even if a council somehow existed, voting must be blocked.
        assert_noop!(
            Elections::elect_supreme_leader(
                RuntimeOrigin::signed(FRANK),
                GovernmentBranch::Legislative,
                GRACE
            ),
            Error::<Test>::LegislativeUsesKhuralPath
        );

        // Confirm that SupremeLeaders storage was NOT written.
        assert!(
            crate::pallet::SupremeLeaders::<Test>::get(GovernmentBranch::Legislative).is_none(),
            "SupremeLeaders[Legislative] must remain empty — Хурал имеет Chairman, не Supreme Leader"
        );
    });
}

// =============================================================================
// D. KHURAL CHAIRMAN ELECTION
// =============================================================================

// ── D1: Authorization checks ──────────────────────────────────────────────────

/// A citizen below Tumed level cannot vote for the Khural Chairman.
#[test]
fn d1_khural_vote_fails_for_non_tumed() {
    new_test_ext().execute_with(|| {
        assert_noop!(
            Elections::elect_khural_chairman(RuntimeOrigin::signed(ALICE), HEIDI),
            Error::<Test>::NotATumedLeader
        );
    });
}

/// Myangad-level leader is also blocked from Khural Chairman election.
#[test]
fn d2_khural_vote_fails_for_myangad_leader() {
    new_test_ext().execute_with(|| {
        assert_noop!(
            Elections::elect_khural_chairman(RuntimeOrigin::signed(EVE), HEIDI),
            Error::<Test>::NotATumedLeader
        );
    });
}

// ── D2: Successful Khural Chairman election ───────────────────────────────────

/// A Tumed leader can cast their Khural Chairman vote successfully.
#[test]
fn d3_khural_vote_succeeds_for_tumed_leader() {
    new_test_ext().execute_with(|| {
        assert_ok!(Elections::elect_khural_chairman(
            RuntimeOrigin::signed(FRANK),
            HEIDI
        ));

        assert_eq!(crate::pallet::KhuralVotes::<Test>::get(FRANK), Some(HEIDI));
        assert_eq!(crate::pallet::KhuralVoteCounts::<Test>::get(HEIDI), 1);

        System::assert_last_event(RuntimeEvent::Elections(Event::KhuralVoteCast {
            voter: FRANK,
            candidate: HEIDI,
        }));
    });
}

/// Multiple Tumed leaders vote — tallies accumulate.
#[test]
fn d4_khural_vote_tallies_accumulate() {
    new_test_ext().execute_with(|| {
        // 5 Tumed leaders vote for HEIDI
        for &voter in &[FRANK, GRACE, IVAN, JUDY, KARL] {
            assert_ok!(Elections::elect_khural_chairman(
                RuntimeOrigin::signed(voter),
                HEIDI
            ));
        }
        // 2 vote for FRANK
        for &voter in &[LENA, MIKE] {
            assert_ok!(Elections::elect_khural_chairman(
                RuntimeOrigin::signed(voter),
                FRANK
            ));
        }

        assert_eq!(crate::pallet::KhuralVoteCounts::<Test>::get(HEIDI), 5);
        assert_eq!(crate::pallet::KhuralVoteCounts::<Test>::get(FRANK), 2);
    });
}

/// Double-voting in Khural Chairman election is blocked.
#[test]
fn d5_khural_double_vote_fails() {
    new_test_ext().execute_with(|| {
        assert_ok!(Elections::elect_khural_chairman(
            RuntimeOrigin::signed(FRANK),
            HEIDI
        ));
        assert_noop!(
            Elections::elect_khural_chairman(RuntimeOrigin::signed(FRANK), HEIDI),
            Error::<Test>::AlreadyVoted
        );
    });
}

// ── D3: confirm_khural_chairman — the constitutional Arbad binding ────────────

/// The critical test: when the Khural Chairman is confirmed, their original
/// Arbad ID is fetched from `CitizenArbad` and stored in `KhuralChairman`.
///
/// Constitutional requirement: HEIDI's home Arbad (ID = HEIDI_ARBAD_ID = 7)
/// must be stored alongside her AccountId.
#[test]
fn d6_confirm_khural_chairman_stores_arbad_id_constitutionally() {
    new_test_ext().execute_with(|| {
        // Vote phase: FRANK votes for HEIDI
        assert_ok!(Elections::elect_khural_chairman(
            RuntimeOrigin::signed(FRANK),
            HEIDI
        ));

        // Root confirms HEIDI as the winner
        assert_ok!(Elections::confirm_khural_chairman(
            RuntimeOrigin::root(),
            HEIDI
        ));

        // Crucial check: (AccountId, arbad_id) stored correctly
        let stored =
            crate::pallet::KhuralChairman::<Test>::get().expect("KhuralChairman must be set");

        assert_eq!(stored.0, HEIDI, "Chairman AccountId must match");
        assert_eq!(
            stored.1, HEIDI_ARBAD_ID,
            "Chairman's Arbad ID must be fetched from CitizenArbad"
        );

        System::assert_last_event(RuntimeEvent::Elections(Event::KhuralChairmanElected {
            chairman: HEIDI,
            arbad_id: HEIDI_ARBAD_ID,
        }));
    });
}

/// Confirming a chairman who has no Arbad entry fails with `WinnerHasNoArbad`.
///
/// OSCAR is a Tumed leader but has no `CitizenArbad` entry — unconstitutional
/// to make him Chairman (the Arbad representing the nation would be unknown).
#[test]
fn d7_confirm_khural_chairman_fails_if_winner_has_no_arbad() {
    new_test_ext().execute_with(|| {
        // OSCAR has no CitizenArbad entry
        assert_noop!(
            Elections::confirm_khural_chairman(RuntimeOrigin::root(), OSCAR),
            Error::<Test>::WinnerHasNoArbad
        );
    });
}

/// Non-root cannot confirm the Khural Chairman.
#[test]
fn d8_confirm_khural_chairman_requires_root() {
    new_test_ext().execute_with(|| {
        assert_noop!(
            Elections::confirm_khural_chairman(RuntimeOrigin::signed(FRANK), HEIDI),
            DispatchError::BadOrigin
        );
    });
}

// ── D4: Full end-to-end flow ──────────────────────────────────────────────────

/// E2E: All 9 Tumed leaders vote → HEIDI wins → confirmed → Arbad #7 stored.
#[test]
fn d9_full_khural_chairman_e2e_flow() {
    new_test_ext().execute_with(|| {
        // All COUNCIL_9 members vote for HEIDI; OSCAR also votes
        for &voter in &[
            FRANK, GRACE, HEIDI, IVAN, JUDY, KARL, LENA, MIKE, NINA, OSCAR,
        ] {
            assert_ok!(Elections::elect_khural_chairman(
                RuntimeOrigin::signed(voter),
                HEIDI
            ));
        }

        assert_eq!(crate::pallet::KhuralVoteCounts::<Test>::get(HEIDI), 10);

        // Root confirms the winner
        assert_ok!(Elections::confirm_khural_chairman(
            RuntimeOrigin::root(),
            HEIDI
        ));

        let (chairman, arbad_id) = crate::pallet::KhuralChairman::<Test>::get().unwrap();

        assert_eq!(chairman, HEIDI);
        assert_eq!(arbad_id, HEIDI_ARBAD_ID);
    });
}

// =============================================================================
// E. RESET BALLOT (C-2 audit fix)
// =============================================================================
//
// Validates that `reset_ballot` correctly purges all voting state,
// enabling a second election cycle within the same chain state.
//
// Test matrix:
//   e1 — Non-root cannot reset
//   e2 — Branch reset clears BranchVotes + BranchVoteCounts
//   e3 — Branch reset clears SupremeLeaderVotes + SupremeLeaderVoteCounts
//   e4 — Branch reset removes BranchCouncils + SupremeLeaders
//   e5 — Legislative reset clears KhuralVotes + KhuralVoteCounts + KhuralChairman
//   e6 — Full re-election cycle: vote → confirm → reset → vote again (no AlreadyVoted)

// ── E1: Access control ────────────────────────────────────────────────────────

/// Non-root cannot trigger a ballot reset.
#[test]
fn e1_reset_ballot_requires_root() {
    new_test_ext().execute_with(|| {
        assert_noop!(
            Elections::reset_ballot(RuntimeOrigin::signed(FRANK), GovernmentBranch::Executive),
            DispatchError::BadOrigin
        );
    });
}

// ── E2: Branch reset — Phase 1 state cleared ─────────────────────────────────

/// After resetting Executive, all BranchVotes and BranchVoteCounts are gone.
#[test]
fn e2_branch_reset_clears_branch_vote_state() {
    new_test_ext().execute_with(|| {
        // Two Tumed leaders vote in the branch council phase.
        assert_ok!(Elections::elect_branch_council(
            RuntimeOrigin::signed(FRANK),
            GovernmentBranch::Executive,
            GRACE
        ));
        assert_ok!(Elections::elect_branch_council(
            RuntimeOrigin::signed(GRACE),
            GovernmentBranch::Executive,
            GRACE
        ));

        // Pre-reset: state is populated.
        assert!(
            crate::pallet::BranchVotes::<Test>::get(GovernmentBranch::Executive, FRANK).is_some()
        );
        assert_eq!(
            crate::pallet::BranchVoteCounts::<Test>::get(GovernmentBranch::Executive, GRACE),
            2
        );

        // Reset.
        assert_ok!(Elections::reset_ballot(
            RuntimeOrigin::root(),
            GovernmentBranch::Executive
        ));

        // Post-reset: all cleared.
        assert!(
            crate::pallet::BranchVotes::<Test>::get(GovernmentBranch::Executive, FRANK).is_none(),
            "BranchVotes must be cleared"
        );
        assert!(
            crate::pallet::BranchVotes::<Test>::get(GovernmentBranch::Executive, GRACE).is_none(),
            "BranchVotes must be cleared"
        );
        assert_eq!(
            crate::pallet::BranchVoteCounts::<Test>::get(GovernmentBranch::Executive, GRACE),
            0,
            "BranchVoteCounts must be zero after reset"
        );

        System::assert_last_event(RuntimeEvent::Elections(Event::BallotReset {
            branch: GovernmentBranch::Executive,
        }));
    });
}

// ── E3: Branch reset — Phase 2 state cleared ─────────────────────────────────

/// After resetting Executive, SupremeLeaderVotes and SupremeLeaderVoteCounts are purged.
#[test]
fn e3_branch_reset_clears_supreme_leader_vote_state() {
    new_test_ext().execute_with(|| {
        // Seed a council and cast a supreme leader vote.
        seed_council(GovernmentBranch::Executive, COUNCIL_9.to_vec());
        assert_ok!(Elections::elect_supreme_leader(
            RuntimeOrigin::signed(FRANK),
            GovernmentBranch::Executive,
            GRACE
        ));

        // Pre-reset.
        assert!(
            crate::pallet::SupremeLeaderVotes::<Test>::get(GovernmentBranch::Executive, FRANK)
                .is_some()
        );
        assert_eq!(
            crate::pallet::SupremeLeaderVoteCounts::<Test>::get(GovernmentBranch::Executive, GRACE),
            1
        );

        // Reset.
        assert_ok!(Elections::reset_ballot(
            RuntimeOrigin::root(),
            GovernmentBranch::Executive
        ));

        // Post-reset.
        assert!(
            crate::pallet::SupremeLeaderVotes::<Test>::get(GovernmentBranch::Executive, FRANK)
                .is_none(),
            "SupremeLeaderVotes must be cleared"
        );
        assert_eq!(
            crate::pallet::SupremeLeaderVoteCounts::<Test>::get(GovernmentBranch::Executive, GRACE),
            0,
            "SupremeLeaderVoteCounts must be zero after reset"
        );
    });
}

// ── E4: Branch reset — confirmed council + leader removed ───────────────────

/// After reset, BranchCouncils and SupremeLeaders entries are removed.
#[test]
fn e4_branch_reset_removes_council_and_supreme_leader() {
    new_test_ext().execute_with(|| {
        // Seat council and confirm a leader.
        seed_council(GovernmentBranch::Executive, COUNCIL_9.to_vec());
        assert_ok!(Elections::confirm_supreme_leader(
            RuntimeOrigin::root(),
            GovernmentBranch::Executive,
            GRACE
        ));

        // Pre-reset: both are set.
        assert!(crate::pallet::BranchCouncils::<Test>::get(GovernmentBranch::Executive).is_some());
        assert!(crate::pallet::SupremeLeaders::<Test>::get(GovernmentBranch::Executive).is_some());

        // Reset.
        assert_ok!(Elections::reset_ballot(
            RuntimeOrigin::root(),
            GovernmentBranch::Executive
        ));

        // Post-reset: both removed.
        assert!(
            crate::pallet::BranchCouncils::<Test>::get(GovernmentBranch::Executive).is_none(),
            "BranchCouncils must be removed after reset"
        );
        assert!(
            crate::pallet::SupremeLeaders::<Test>::get(GovernmentBranch::Executive).is_none(),
            "SupremeLeaders must be removed after reset"
        );
    });
}

// ── E5: Legislative reset ────────────────────────────────────────────────────

/// Reset of Legislative branch clears KhuralVotes, KhuralVoteCounts, KhuralChairman.
#[test]
fn e5_legislative_reset_clears_all_khural_state() {
    new_test_ext().execute_with(|| {
        // Full Legislative cycle: vote → confirm.
        assert_ok!(Elections::elect_khural_chairman(
            RuntimeOrigin::signed(FRANK),
            HEIDI
        ));
        assert_ok!(Elections::elect_khural_chairman(
            RuntimeOrigin::signed(GRACE),
            HEIDI
        ));
        assert_ok!(Elections::confirm_khural_chairman(
            RuntimeOrigin::root(),
            HEIDI
        ));

        // Pre-reset: all Khural state is populated.
        assert!(crate::pallet::KhuralVotes::<Test>::get(FRANK).is_some());
        assert!(crate::pallet::KhuralVotes::<Test>::get(GRACE).is_some());
        assert_eq!(crate::pallet::KhuralVoteCounts::<Test>::get(HEIDI), 2);
        assert!(crate::pallet::KhuralChairman::<Test>::get().is_some());

        // Reset.
        assert_ok!(Elections::reset_ballot(
            RuntimeOrigin::root(),
            GovernmentBranch::Legislative
        ));

        // Post-reset: everything cleared.
        assert!(
            crate::pallet::KhuralVotes::<Test>::get(FRANK).is_none(),
            "KhuralVotes**FRANK** must be cleared"
        );
        assert!(
            crate::pallet::KhuralVotes::<Test>::get(GRACE).is_none(),
            "KhuralVotes**GRACE** must be cleared"
        );
        assert_eq!(
            crate::pallet::KhuralVoteCounts::<Test>::get(HEIDI),
            0,
            "KhuralVoteCounts must be zero after reset"
        );
        assert!(
            crate::pallet::KhuralChairman::<Test>::get().is_none(),
            "KhuralChairman must be cleared"
        );

        System::assert_last_event(RuntimeEvent::Elections(Event::BallotReset {
            branch: GovernmentBranch::Legislative,
        }));
    });
}

// ── E6: Full re-election cycle (the core C-2 regression) ─────────────────────

/// CYCLE 1: HEIDI wins → confirm.
/// RESET.
/// CYCLE 2: OSCAR wins → confirm.
///
/// Without `reset_ballot`, CYCLE 2 would fail with `AlreadyVoted` for all
/// Tumed leaders who voted in CYCLE 1. This test proves the fix is complete.
#[test]
fn e6_full_two_cycle_khural_reelection_after_reset() {
    new_test_ext().execute_with(|| {
        // ── Cycle 1 ─────────────────────────────────────────────────────────
        for &voter in &[FRANK, GRACE, IVAN, JUDY, KARL] {
            assert_ok!(Elections::elect_khural_chairman(
                RuntimeOrigin::signed(voter),
                HEIDI
            ));
        }
        assert_ok!(Elections::confirm_khural_chairman(
            RuntimeOrigin::root(),
            HEIDI
        ));
        assert_eq!(
            crate::pallet::KhuralChairman::<Test>::get().unwrap().0,
            HEIDI
        );

        // ── Reset between terms ──────────────────────────────────────────────
        assert_ok!(Elections::reset_ballot(
            RuntimeOrigin::root(),
            GovernmentBranch::Legislative
        ));

        // ── Cycle 2 ── same voters must be able to vote again ────────────────
        // Without the fix these would all fail with `AlreadyVoted`.
        for &voter in &[FRANK, GRACE, IVAN, JUDY, KARL] {
            assert_ok!(
                Elections::elect_khural_chairman(RuntimeOrigin::signed(voter), OSCAR),
                // OSCAR has no CitizenArbad — we only test the voting phase here.
            );
        }
        // HEIDI votes for herself in cycle 2.
        assert_ok!(Elections::elect_khural_chairman(
            RuntimeOrigin::signed(HEIDI),
            HEIDI
        ));

        assert_eq!(crate::pallet::KhuralVoteCounts::<Test>::get(OSCAR), 5);
        assert_eq!(crate::pallet::KhuralVoteCounts::<Test>::get(HEIDI), 1);

        // Confirm HEIDI again (she has an Arbad; OSCAR does not).
        assert_ok!(Elections::confirm_khural_chairman(
            RuntimeOrigin::root(),
            HEIDI
        ));
        assert_eq!(
            crate::pallet::KhuralChairman::<Test>::get().unwrap().0,
            HEIDI
        );
    });
}

// =============================================================================
// G. HIGH-PRIORITY AUDIT FIXES (H-1, H-2, H-4)
// =============================================================================
//
//   g1 — H-2: non-Tumed candidate is rejected in elect_branch_council
//   g2 — H-1: MaxBranchCandidates is enforced (DOS guard)
//   g3 — H-1: BranchCandidateCount is reset by reset_ballot
//   g4 — H-4: create_election rejects duplicate election_id

// ── G1: H-2 — Candidate must be a Tumed leader ───────────────────────────────

/// A Tumed voter cannot nominate a non-Tumed candidate (H-2 fix).
///
/// Before this fix, any `AccountId` could be nominated. This allowed accounts
/// with zero voting rights to accumulate tallies in `BranchVoteCounts`, and
/// Root could then (accidentally or maliciously) pass them via `confirm_branch_council`
/// — which would then fail only at the C-1 validation. The H-2 fix blocks the
/// vote at source: the candidate must already be a confirmed Tumed leader.
#[test]
fn g1_elect_branch_council_rejects_non_tumed_candidate() {
    new_test_ext().execute_with(|| {
        // ALICE is an Arbad citizen, not a Tumed leader — invalid candidate.
        assert_noop!(
            Elections::elect_branch_council(
                RuntimeOrigin::signed(FRANK), // Tumed voter — valid
                GovernmentBranch::Executive,
                ALICE // Arbad citizen — INVALID candidate
            ),
            Error::<Test>::CandidateNotATumedLeader
        );

        // CHARLIE has no membership at all — also invalid.
        assert_noop!(
            Elections::elect_branch_council(
                RuntimeOrigin::signed(FRANK),
                GovernmentBranch::Executive,
                CHARLIE
            ),
            Error::<Test>::CandidateNotATumedLeader
        );
    });
}

// ── G2: H-1 — MaxBranchCandidates DOS guard ──────────────────────────────────

/// Once `MaxBranchCandidates` distinct candidates have received votes,
/// any new candidate nomination is rejected with `TooManyCandidates`.
///
/// The mock sets `MaxBranchCandidates = 32`. We seed a special low-limit mock
/// inline by reducing Tumed voters to 10 and using a small limit via the
/// BranchCandidateCount counter directly.
///
/// Strategy: artificially fill `BranchCandidateCount[Banking]` to 32
/// (the mock's `MaxBranchCandidates`), then attempt to add a 33rd.
#[test]
fn g2_max_branch_candidates_is_enforced() {
    new_test_ext().execute_with(|| {
        // Pre-fill the counter to the maximum (32 = T::MaxBranchCandidates).
        crate::pallet::BranchCandidateCount::<Test>::insert(GovernmentBranch::Banking, 32u32);

        // FRANK (Tumed) tries to nominate GRACE (Tumed) for Banking.
        // GRACE has 0 votes in Banking → she would be a *new* candidate →
        // the counter would go to 33, exceeding the limit.
        assert_noop!(
            Elections::elect_branch_council(
                RuntimeOrigin::signed(FRANK),
                GovernmentBranch::Banking,
                GRACE
            ),
            Error::<Test>::TooManyCandidates
        );

        // Voting for a candidate who *already* has votes does NOT increment
        // the counter → it must still succeed.
        // Give GRACE 1 existing vote first (bypassing the extrinsic via storage):
        crate::pallet::BranchVoteCounts::<Test>::insert(GovernmentBranch::Banking, GRACE, 1u32);

        assert_ok!(Elections::elect_branch_council(
            RuntimeOrigin::signed(FRANK),
            GovernmentBranch::Banking,
            GRACE // already has 1 vote → is NOT new → counter stays at 32
        ));
    });
}

// ── G3: H-1 — reset_ballot clears BranchCandidateCount ──────────────────────

/// After reset_ballot, the candidate counter returns to 0,
/// so a new election cycle starts with a clean slate.
#[test]
fn g3_reset_ballot_clears_branch_candidate_count() {
    new_test_ext().execute_with(|| {
        // FRANK votes for GRACE (Executive) — GRACE becomes a new candidate.
        assert_ok!(Elections::elect_branch_council(
            RuntimeOrigin::signed(FRANK),
            GovernmentBranch::Executive,
            GRACE
        ));

        // Counter should be 1.
        assert_eq!(
            crate::pallet::BranchCandidateCount::<Test>::get(GovernmentBranch::Executive),
            1
        );

        // Reset.
        assert_ok!(Elections::reset_ballot(
            RuntimeOrigin::root(),
            GovernmentBranch::Executive
        ));

        // Counter must be 0 after reset.
        assert_eq!(
            crate::pallet::BranchCandidateCount::<Test>::get(GovernmentBranch::Executive),
            0,
            "BranchCandidateCount must be zero after reset"
        );
    });
}

// ── G4: H-4 — create_election blocks overwrite ────────────────────────────────

/// Calling `create_election` with an already-existing ID must fail (H-4 fix).
///
/// Before this fix, Root could silently overwrite an in-progress election,
/// discarding all existing vote tallies and replacing the candidate list.
/// This could be exploited to erase votes or introduce new candidates mid-election.
#[test]
fn g4_create_election_rejects_duplicate_id() {
    new_test_ext().execute_with(|| {
        // ARBAD_ELECTION_ID = 10 is already seeded by new_test_ext().
        // Trying to create it again must fail.
        assert_noop!(
            Elections::create_election(
                RuntimeOrigin::root(),
                ARBAD_ELECTION_ID,
                crate::pallet::ElectionLevel::Arbad,
                vec![ALICE, BOB]
            ),
            Error::<Test>::ElectionAlreadyExists
        );

        // A fresh ID (999) must succeed.
        assert_ok!(Elections::create_election(
            RuntimeOrigin::root(),
            999,
            crate::pallet::ElectionLevel::Tumed,
            vec![FRANK, GRACE]
        ));
    });
}

// =============================================================================
// H. CONSTITUTIONAL SIZE REQUIREMENTS
// =============================================================================

#[test]
fn h1_promote_zun_fails_when_arbad_too_small() {
    new_test_ext().execute_with(|| {
        let small_arbad: u32 = 99;
        let leader: AccountId = 50;
        ArbadMemberCount::<Test>::insert(small_arbad, 5u32);
        crate::pallet::CitizenArbad::<Test>::insert(leader, small_arbad);
        assert_noop!(
            Elections::promote_leader(RuntimeOrigin::root(), leader, ElectionLevel::Zun, None),
            Error::<Test>::ArbadTooSmall
        );
    });
}

#[test]
fn h1b_promote_zun_fails_when_not_in_arbad() {
    new_test_ext().execute_with(|| {
        assert_noop!(
            Elections::promote_leader(RuntimeOrigin::root(), 99u64, ElectionLevel::Zun, None),
            Error::<Test>::NotInArbad
        );
    });
}

#[test]
fn h2_promote_zun_succeeds_when_arbad_full() {
    new_test_ext().execute_with(|| {
        // ARBAD_ID has ArbadMemberCount=10 from new_test_ext.
        assert_ok!(Elections::promote_leader(RuntimeOrigin::root(), DAVE, ElectionLevel::Zun, None));
        assert_eq!(ElectedLeaders::<Test>::get(DAVE), Some(ElectionLevel::Zun));
        assert_eq!(ZunLeaderCount::<Test>::get(ARBAD_ID), 1);
    });
}

#[test]
fn h3_promote_myangad_fails_when_zun_leaders_too_few() {
    new_test_ext().execute_with(|| {
        let zun_id: u32 = 5;
        ZunLeaderCount::<Test>::insert(zun_id, 7u32); // 7 < 10
        assert_noop!(
            Elections::promote_leader(RuntimeOrigin::root(), DAVE, ElectionLevel::Myangad, Some(zun_id)),
            Error::<Test>::ZunTooSmall
        );
    });
}

#[test]
fn h4_promote_myangad_succeeds_when_zun_leaders_full() {
    new_test_ext().execute_with(|| {
        let zun_id: u32 = 5;
        seed_zun_ready(zun_id); // ZunLeaderCount[zun_id] = 10
        assert_ok!(Elections::promote_leader(RuntimeOrigin::root(), DAVE, ElectionLevel::Myangad, Some(zun_id)));
        assert_eq!(ElectedLeaders::<Test>::get(DAVE), Some(ElectionLevel::Myangad));
        assert_eq!(MyangadLeaderCount::<Test>::get(zun_id), 1);
    });
}

#[test]
fn h5_promote_tumed_fails_when_myangad_leaders_too_few() {
    new_test_ext().execute_with(|| {
        let myangad_id: u32 = 3;
        MyangadLeaderCount::<Test>::insert(myangad_id, 9u32); // 9 < 10
        assert_noop!(
            Elections::promote_leader(RuntimeOrigin::root(), DAVE, ElectionLevel::Tumed, Some(myangad_id)),
            Error::<Test>::MyangadTooSmall
        );
    });
}

#[test]
fn h6_promote_tumed_succeeds_when_myangad_leaders_full() {
    new_test_ext().execute_with(|| {
        let myangad_id: u32 = 3;
        seed_myangad_ready(myangad_id); // MyangadLeaderCount[myangad_id] = 10
        assert_ok!(Elections::promote_leader(RuntimeOrigin::root(), DAVE, ElectionLevel::Tumed, Some(myangad_id)));
        assert_eq!(ElectedLeaders::<Test>::get(DAVE), Some(ElectionLevel::Tumed));
    });
}

#[test]
fn h7_promote_myangad_fails_without_zone_id() {
    new_test_ext().execute_with(|| {
        assert_noop!(
            Elections::promote_leader(RuntimeOrigin::root(), DAVE, ElectionLevel::Myangad, None),
            Error::<Test>::MissingZoneId
        );
    });
}

#[test]
fn h8_promote_tumed_fails_without_zone_id() {
    new_test_ext().execute_with(|| {
        assert_noop!(
            Elections::promote_leader(RuntimeOrigin::root(), DAVE, ElectionLevel::Tumed, None),
            Error::<Test>::MissingZoneId
        );
    });
}

#[test]
fn h9_add_citizen_increments_arbad_member_count() {
    new_test_ext().execute_with(|| {
        let fresh_arbad: u32 = 200;
        let before = ArbadMemberCount::<Test>::get(fresh_arbad);
        assert_ok!(Elections::add_citizen_to_arbad(RuntimeOrigin::root(), 50u64, fresh_arbad));
        assert_eq!(ArbadMemberCount::<Test>::get(fresh_arbad), before + 1);
    });
}

#[test]
fn h10_promote_zun_increments_zun_leader_count() {
    new_test_ext().execute_with(|| {
        let before = ZunLeaderCount::<Test>::get(ARBAD_ID);
        assert_ok!(Elections::promote_leader(RuntimeOrigin::root(), DAVE, ElectionLevel::Zun, None));
        assert_eq!(ZunLeaderCount::<Test>::get(ARBAD_ID), before + 1);
    });
}

#[test]
fn h11_promote_myangad_increments_myangad_leader_count() {
    new_test_ext().execute_with(|| {
        let zun_id: u32 = 7;
        seed_zun_ready(zun_id);
        let before = MyangadLeaderCount::<Test>::get(zun_id);
        assert_ok!(Elections::promote_leader(RuntimeOrigin::root(), DAVE, ElectionLevel::Myangad, Some(zun_id)));
        assert_eq!(MyangadLeaderCount::<Test>::get(zun_id), before + 1);
    });
}

#[test]
fn h12_full_constitutional_promotion_chain() {
    new_test_ext().execute_with(|| {
        // Step 1: DAVE (ARBAD_ID has 10 members) → Zun
        assert_ok!(Elections::promote_leader(RuntimeOrigin::root(), DAVE, ElectionLevel::Zun, None));
        // Step 2: Fill Zun zone to 10 leaders
        ZunLeaderCount::<Test>::insert(ARBAD_ID, 10u32);
        // Step 3: EVE → Myangad from zun_id=ARBAD_ID
        assert_ok!(Elections::promote_leader(RuntimeOrigin::root(), EVE, ElectionLevel::Myangad, Some(ARBAD_ID)));
        // Step 4: Fill Myangad zone to 10 leaders
        MyangadLeaderCount::<Test>::insert(ARBAD_ID, 10u32);
        // Step 5: FRANK → Tumed from myangad_id=ARBAD_ID
        assert_ok!(Elections::promote_leader(RuntimeOrigin::root(), FRANK, ElectionLevel::Tumed, Some(ARBAD_ID)));
        assert_eq!(ElectedLeaders::<Test>::get(FRANK), Some(ElectionLevel::Tumed));
    });
}

// =============================================================================
// I. INDIGENOUS RESTRICTION — LEGISLATIVE BRANCH (Хурал)
// =============================================================================

#[test]
fn i1_khural_rejects_non_indigenous_voter() {
    new_test_ext().execute_with(|| {
        ElectedLeaders::<Test>::insert(CHARLIE, ElectionLevel::Tumed);
        assert_noop!(
            Elections::elect_khural_chairman(RuntimeOrigin::signed(CHARLIE), HEIDI),
            Error::<Test>::NotIndigenous
        );
    });
}

#[test]
fn i2_khural_rejects_non_indigenous_candidate() {
    new_test_ext().execute_with(|| {
        ElectedLeaders::<Test>::insert(CHARLIE, ElectionLevel::Tumed);
        assert_noop!(
            Elections::elect_khural_chairman(RuntimeOrigin::signed(FRANK), CHARLIE),
            Error::<Test>::NotIndigenous
        );
    });
}

#[test]
fn i3_khural_succeeds_when_both_indigenous() {
    new_test_ext().execute_with(|| {
        assert_ok!(Elections::elect_khural_chairman(RuntimeOrigin::signed(FRANK), HEIDI));
        assert_eq!(crate::pallet::KhuralVotes::<Test>::get(FRANK), Some(HEIDI));
    });
}

#[test]
fn i4_non_indigenous_tumed_blocked_only_at_khural() {
    new_test_ext().execute_with(|| {
        ElectedLeaders::<Test>::insert(CHARLIE, ElectionLevel::Tumed);
        // Can vote in Executive (no indigenous check there)
        assert_ok!(Elections::elect_branch_council(
            RuntimeOrigin::signed(CHARLIE), GovernmentBranch::Executive, FRANK
        ));
        // But blocked at Khural
        assert_noop!(
            Elections::elect_khural_chairman(RuntimeOrigin::signed(CHARLIE), HEIDI),
            Error::<Test>::NotIndigenous
        );
    });
}

#[test]
fn i5_indigenous_restriction_does_not_apply_to_executive_branch() {
    new_test_ext().execute_with(|| {
        ElectedLeaders::<Test>::insert(CHARLIE, ElectionLevel::Tumed);
        assert_ok!(Elections::elect_branch_council(
            RuntimeOrigin::signed(CHARLIE), GovernmentBranch::Executive, FRANK
        ));
        assert_ok!(Elections::elect_branch_council(
            RuntimeOrigin::signed(FRANK), GovernmentBranch::Executive, GRACE
        ));
    });
}

#[test]
fn i6_full_khural_cycle_indigenous_only() {
    new_test_ext().execute_with(|| {
        for &voter in &[FRANK, GRACE, IVAN, JUDY, KARL] {
            assert_ok!(Elections::elect_khural_chairman(RuntimeOrigin::signed(voter), HEIDI));
        }
        assert_eq!(crate::pallet::KhuralVoteCounts::<Test>::get(HEIDI), 5);
        assert_ok!(Elections::confirm_khural_chairman(RuntimeOrigin::root(), HEIDI));
        let (chairman, arbad_id) = crate::pallet::KhuralChairman::<Test>::get().unwrap();
        assert_eq!(chairman, HEIDI);
        assert_eq!(arbad_id, HEIDI_ARBAD_ID);
    });
}

#[test]
fn i7_non_indigenous_voter_blocked_mid_session() {
    new_test_ext().execute_with(|| {
        ElectedLeaders::<Test>::insert(CHARLIE, ElectionLevel::Tumed);
        assert_ok!(Elections::elect_khural_chairman(RuntimeOrigin::signed(FRANK), HEIDI));
        assert_ok!(Elections::elect_khural_chairman(RuntimeOrigin::signed(GRACE), HEIDI));
        assert_noop!(
            Elections::elect_khural_chairman(RuntimeOrigin::signed(CHARLIE), HEIDI),
            Error::<Test>::NotIndigenous
        );
        // Tally must be exactly 2 — CHARLIE's vote was not counted
        assert_eq!(crate::pallet::KhuralVoteCounts::<Test>::get(HEIDI), 2);
    });
}
