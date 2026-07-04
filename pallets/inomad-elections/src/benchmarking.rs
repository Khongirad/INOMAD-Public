//! Benchmarks for pallet-inomad-elections
//!
//! Covers all 11 extrinsics / weight placeholders:
//!   0.  cast_vote
//!   1.  add_citizen_to_arbad
//!   2.  promote_leader
//!   3.  create_election
//!   4.  elect_branch_council  (Signed Tumed voter)
//!   5.  confirm_branch_council (Root)
//!   6.  elect_supreme_leader  (Signed council member)
//!   7.  confirm_supreme_leader (Root)
//!   8.  elect_khural_chairman  (Signed Tumed leader)
//!   9.  confirm_khural_chairman (Root)
//!   10. reset_ballot           (Root)

#![cfg(feature = "runtime-benchmarks")]

use super::*;
use alloc::vec::Vec;
use frame_benchmarking::v2::*;
use frame_support::pallet_prelude::*;
use frame_system::RawOrigin;

// ---------------------------------------------------------------------------
// Storage helpers
// ---------------------------------------------------------------------------

fn seed_arbad_member<T: Config>(who: &T::AccountId, arbad_id: u32) {
    CitizenArbad::<T>::insert(who, arbad_id);
}

fn seed_leader<T: Config>(who: &T::AccountId, level: ElectionLevel) {
    ElectedLeaders::<T>::insert(who, level);
}

fn seed_arbad_election<T: Config>(
    election_id: u32,
    candidate: &T::AccountId,
    voter: &T::AccountId,
) {
    let mut candidates: BoundedVec<T::AccountId, T::MaxCandidates> = BoundedVec::default();
    candidates.try_push(candidate.clone()).ok();
    Elections::<T>::insert(
        election_id,
        Election::<T> {
            level: ElectionLevel::Arbad,
            candidates,
            is_active: true,
        },
    );
    seed_arbad_member::<T>(voter, election_id);
}

/// Seed a Branch Council (9 members) into storage.
fn seed_branch_council<T: Config>(branch: &GovernmentBranch) -> Vec<T::AccountId> {
    let mut council: BoundedVec<T::AccountId, ConstU32<9>> = BoundedVec::default();
    let mut members: Vec<T::AccountId> = Vec::new();
    for i in 0..9u32 {
        let m: T::AccountId = account("council_member", i, 0);
        council.try_push(m.clone()).ok();
        members.push(m.clone());
        seed_leader::<T>(&m, ElectionLevel::Tumed);
        // pre-record a vote so WinnerDidNotReceiveVotes passes
        BranchVoteCounts::<T>::insert(branch, &m, 1u32);
    }
    BranchCouncils::<T>::insert(branch, council);
    members
}

// ---------------------------------------------------------------------------
// Benchmarks
// ---------------------------------------------------------------------------

#[benchmarks]
mod benchmarks {
    use super::*;

    // ── 0. cast_vote ──────────────────────────────────────────────────────────

    #[benchmark]
    fn cast_vote() {
        let voter: T::AccountId = whitelisted_caller();
        let candidate: T::AccountId = account("candidate", 0, 0);
        seed_arbad_election::<T>(1, &candidate, &voter);

        #[extrinsic_call]
        cast_vote(RawOrigin::Signed(voter.clone()), 1u32, candidate.clone());

        assert!(Votes::<T>::get(1u32, &voter).is_some());
    }

    // ── 1. add_citizen_to_arbad ───────────────────────────────────────────────

    #[benchmark]
    fn add_citizen_to_arbad() {
        let citizen: T::AccountId = account("citizen", 0, 0);

        #[extrinsic_call]
        add_citizen_to_arbad(RawOrigin::Root, citizen.clone(), 42u32);

        assert_eq!(CitizenArbad::<T>::get(&citizen), Some(42u32));
    }

    // ── 2. promote_leader ────────────────────────────────────────────────────

    #[benchmark]
    fn promote_leader() {
        let leader: T::AccountId = account("leader", 0, 0);

        #[extrinsic_call]
        promote_leader(RawOrigin::Root, leader.clone(), ElectionLevel::Zun);

        assert_eq!(ElectedLeaders::<T>::get(&leader), Some(ElectionLevel::Zun));
    }

    // ── 3. create_election ───────────────────────────────────────────────────

    #[benchmark]
    fn create_election() {
        let candidate: T::AccountId = account("candidate", 0, 0);
        // H-2: candidate must be Tumed for Tumed-level elections; use Arbad here
        let candidates = alloc::vec![candidate.clone()];

        #[extrinsic_call]
        create_election(RawOrigin::Root, 99u32, ElectionLevel::Arbad, candidates);

        assert!(Elections::<T>::contains_key(99u32));
    }

    // ── 4. elect_branch_council ───────────────────────────────────────────────

    #[benchmark]
    fn elect_branch_council() {
        let branch = GovernmentBranch::Executive;
        let caller: T::AccountId = whitelisted_caller();
        let candidate: T::AccountId = account("bcandidate", 0, 0);

        // Caller and candidate must be Tumed
        seed_leader::<T>(&caller, ElectionLevel::Tumed);
        seed_leader::<T>(&candidate, ElectionLevel::Tumed);

        #[extrinsic_call]
        elect_branch_council(
            RawOrigin::Signed(caller.clone()),
            branch.clone(),
            candidate.clone(),
        );

        assert!(BranchVotes::<T>::get(&branch, &caller).is_some());
    }

    // ── 5. confirm_branch_council ─────────────────────────────────────────────

    #[benchmark]
    fn confirm_branch_council() {
        let branch = GovernmentBranch::Executive;
        // Seed exactly 9 Tumed leaders with ≥1 vote each
        let mut winners: Vec<T::AccountId> = Vec::new();
        for i in 0..9u32 {
            let w: T::AccountId = account("winner", i, 0);
            seed_leader::<T>(&w, ElectionLevel::Tumed);
            BranchVoteCounts::<T>::insert(&branch, &w, 1u32);
            winners.push(w);
        }

        #[extrinsic_call]
        confirm_branch_council(RawOrigin::Root, branch.clone(), winners);

        assert!(BranchCouncils::<T>::contains_key(&branch));
    }

    // ── 6. elect_supreme_leader ───────────────────────────────────────────────

    #[benchmark]
    fn elect_supreme_leader() {
        let branch = GovernmentBranch::Executive;
        let members = seed_branch_council::<T>(&branch);
        let caller = members[0].clone();
        let candidate = members[1].clone();

        #[extrinsic_call]
        elect_supreme_leader(
            RawOrigin::Signed(caller.clone()),
            branch.clone(),
            candidate.clone(),
        );

        assert!(SupremeLeaderVotes::<T>::get(&branch, &caller).is_some());
    }

    // ── 7. confirm_supreme_leader ─────────────────────────────────────────────

    #[benchmark]
    fn confirm_supreme_leader() {
        let branch = GovernmentBranch::Executive;
        seed_branch_council::<T>(&branch);
        let winner: T::AccountId = account("supreme_winner", 0, 0);
        SupremeLeaderVoteCounts::<T>::insert(&branch, &winner, 5u32);

        #[extrinsic_call]
        confirm_supreme_leader(RawOrigin::Root, branch.clone(), winner.clone());

        assert_eq!(SupremeLeaders::<T>::get(&branch), Some(winner));
    }

    // ── 8. elect_khural_chairman ──────────────────────────────────────────────

    #[benchmark]
    fn elect_khural_chairman() {
        let caller: T::AccountId = whitelisted_caller();
        let candidate: T::AccountId = account("kh_candidate", 0, 0);
        seed_leader::<T>(&caller, ElectionLevel::Tumed);
        CitizenArbad::<T>::insert(&candidate, 1u32);

        #[extrinsic_call]
        elect_khural_chairman(RawOrigin::Signed(caller.clone()), candidate.clone());

        assert!(KhuralVotes::<T>::get(&caller).is_some());
    }

    // ── 9. confirm_khural_chairman ────────────────────────────────────────────

    #[benchmark]
    fn confirm_khural_chairman() {
        let winner: T::AccountId = account("kh_winner", 0, 0);
        KhuralVoteCounts::<T>::insert(&winner, 3u32);
        CitizenArbad::<T>::insert(&winner, 7u32);

        #[extrinsic_call]
        confirm_khural_chairman(RawOrigin::Root, winner.clone());

        assert_eq!(KhuralChairman::<T>::get(), Some((winner, 7u32)));
    }

    // ── 10. reset_ballot ──────────────────────────────────────────────────────

    #[benchmark]
    fn reset_ballot() {
        let branch = GovernmentBranch::Executive;
        let member: T::AccountId = account("member", 0, 0);

        BranchVoteCounts::<T>::insert(&branch, &member, 5u32);
        BranchVotes::<T>::insert(&branch, &member, member.clone());
        BranchCandidateCount::<T>::insert(&branch, 1u32);

        let mut council: BoundedVec<T::AccountId, ConstU32<9>> = BoundedVec::default();
        council.try_push(member.clone()).ok();
        BranchCouncils::<T>::insert(&branch, council);
        SupremeLeaders::<T>::insert(&branch, member.clone());

        #[extrinsic_call]
        reset_ballot(RawOrigin::Root, branch.clone());

        assert!(!BranchCouncils::<T>::contains_key(&branch));
        assert!(!SupremeLeaders::<T>::contains_key(&branch));
    }

    impl_benchmark_test_suite!(Pallet, crate::mock::new_test_ext(), crate::mock::Test);
}
