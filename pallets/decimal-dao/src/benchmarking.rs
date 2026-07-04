#![cfg(feature = "runtime-benchmarks")]
use super::*;
use alloc::vec::Vec;
use frame_benchmarking::v2::*;
use frame_support::pallet_prelude::*;
use frame_system::RawOrigin;

fn fund<T: Config>(who: &T::AccountId) {
    use frame_support::traits::Currency;
    let amount = (1_000_000_000_000u128 * 10_000u128)
        .try_into()
        .unwrap_or_default();
    <T as Config>::Currency::make_free_balance_be(who, amount);
}

fn make_org_id() -> [u8; 32] {
    [0u8; 32]
}

/// Seed an org (Councils + OrgTreasuries) without calling the extrinsic.
fn seed_org<T: Config>(org_id: &[u8; 32], founder: &T::AccountId) {
    let mut council: BoundedVec<T::AccountId, T::MaxCouncilMembers> = BoundedVec::default();
    council.try_push(founder.clone()).ok();
    Councils::<T>::insert(org_id, &council);
    OrgTreasuries::<T>::insert(org_id, Pallet::<T>::treasury_account(org_id));
}

/// Seed a ProposalRecord in the Open state.
fn seed_proposal<T: Config>(
    org_id: &[u8; 32],
    prop_id: &[u8; 32],
    proposer: &T::AccountId,
    beneficiary: &T::AccountId,
    amount: BalanceOf<T>,
) {
    Proposals::<T>::insert(
        prop_id,
        ProposalRecord::<T> {
            org_id: *org_id,
            proposer: proposer.clone(),
            amount,
            beneficiary: beneficiary.clone(),
            votes_for: 0u32,
            votes_against: 0u32,
            status: ProposalStatus::Open,
        },
    );
}

#[benchmarks]
mod benchmarks {
    use super::*;

    // ── 0. instantiate_org ───────────────────────────────────────────────────
    // instantiate_org(origin, org_id: [u8;32], initial_council: Vec<AccountId>) — Root
    #[benchmark]
    fn instantiate_org() {
        let founder: T::AccountId = account("founder", 0, 0);
        fund::<T>(&founder);
        let org_id = make_org_id();
        let council: Vec<T::AccountId> = alloc::vec![founder.clone()];

        #[extrinsic_call]
        instantiate_org(RawOrigin::Root, org_id, council);

        assert!(Councils::<T>::contains_key(&org_id));
    }

    // ── 1. sync_council ──────────────────────────────────────────────────────
    // sync_council(origin, org_id: [u8;32], new_council: Vec<AccountId>) — Root
    #[benchmark]
    fn sync_council() {
        let founder: T::AccountId = account("founder", 0, 0);
        fund::<T>(&founder);
        let org_id = make_org_id();
        seed_org::<T>(&org_id, &founder);
        let new_member: T::AccountId = account("member", 0, 0);
        let new_council: Vec<T::AccountId> = alloc::vec![founder.clone(), new_member];

        #[extrinsic_call]
        sync_council(RawOrigin::Root, org_id, new_council);

        let council = Councils::<T>::get(&org_id).expect("council exists");
        assert_eq!(council.len(), 2);
    }

    // ── 2. create_proposal ───────────────────────────────────────────────────
    // create_proposal(origin, org_id, prop_id, proposer, amount, beneficiary) — Root
    #[benchmark]
    fn create_proposal() {
        let proposer: T::AccountId = account("proposer", 0, 0);
        let beneficiary: T::AccountId = account("beneficiary", 0, 0);
        fund::<T>(&proposer);
        let org_id = make_org_id();
        seed_org::<T>(&org_id, &proposer);
        let prop_id = [1u8; 32];
        let amount: BalanceOf<T> = (1_000_000_000_000u128 * 10u128)
            .try_into()
            .unwrap_or_default();

        #[extrinsic_call]
        create_proposal(
            RawOrigin::Root,
            org_id,
            prop_id,
            proposer.clone(),
            amount,
            beneficiary.clone(),
        );

        assert!(Proposals::<T>::contains_key(&prop_id));
    }

    // ── 3. vote_proposal ─────────────────────────────────────────────────────
    // vote_proposal(origin, prop_id: [u8;32], voter: T::AccountId, support: bool) — Root
    #[benchmark]
    fn vote_proposal() {
        let voter: T::AccountId = whitelisted_caller();
        let beneficiary: T::AccountId = account("bene", 0, 0);
        fund::<T>(&voter);
        let org_id = make_org_id();
        seed_org::<T>(&org_id, &voter);
        let prop_id = [1u8; 32];
        let amount: BalanceOf<T> = (1_000_000_000_000u128).try_into().unwrap_or_default();
        seed_proposal::<T>(&org_id, &prop_id, &voter, &beneficiary, amount);

        #[extrinsic_call]
        vote_proposal(RawOrigin::Root, prop_id, voter.clone(), true);

        assert!(ProposalBallots::<T>::contains_key(&prop_id, &voter));
    }

    // ── 4. execute_proposal ──────────────────────────────────────────────────
    // execute_proposal(origin, prop_id: [u8;32]) — Root
    // Proposal must be in Passed status.
    #[benchmark]
    fn execute_proposal() {
        use frame_support::traits::Currency;
        let founder: T::AccountId = account("founder", 0, 0);
        let beneficiary: T::AccountId = account("bene", 0, 0);
        fund::<T>(&founder);
        let org_id = make_org_id();
        seed_org::<T>(&org_id, &founder);
        let prop_id = [2u8; 32];
        let amount: BalanceOf<T> = (1_000_000_000_000u128).try_into().unwrap_or_default();

        // Fund the treasury account
        let treasury = Pallet::<T>::treasury_account(&org_id);
        <T as Config>::Currency::make_free_balance_be(&treasury, amount);

        // Insert a Passed proposal directly
        Proposals::<T>::insert(
            &prop_id,
            ProposalRecord::<T> {
                org_id,
                proposer: founder.clone(),
                amount,
                beneficiary: beneficiary.clone(),
                votes_for: 5u32,
                votes_against: 0u32,
                status: ProposalStatus::Passed,
            },
        );

        #[extrinsic_call]
        execute_proposal(RawOrigin::Root, prop_id);

        let p = Proposals::<T>::get(&prop_id).expect("proposal exists");
        assert_eq!(p.status, ProposalStatus::Passed); // may transition to Executed
    }

    impl_benchmark_test_suite!(Pallet, sp_io::TestExternalities::default(), T);
}
