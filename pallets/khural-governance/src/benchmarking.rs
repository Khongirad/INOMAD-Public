//! Benchmarking suite for pallet-khural-governance.
//!
//! ## Citizen setup
//!
//! `create_proposal`, `vote`, `vote_on_expert_bill` gate on a valid `CitizenRecord`
//! in `pallet-inomad-identity::Citizens`. We write the record directly to storage
//! with a helper — no extrinsic call overhead, ensures deterministic benchmark setup.
//!
//! ## Origin
//!
//! All extrinsics use `ensure_signed` — no governance origin required.

use super::*;

#[allow(unused)]
use crate::Pallet as KhuralGov;
use frame_benchmarking::v2::*;
use frame_support::traits::{Currency, ReservableCurrency};
use frame_system::RawOrigin;
use sp_core::H256;

use pallet_inomad_identity::{
    pallet::{
        CitizenRecord, CitizenRole, CitizenStatus, CitizenshipStatus, PassportType,
        VerificationStatus,
    },
    Citizens,
};

const UNIT: u128 = 1_000_000_000_000u128;

/// Insert a minimal-but-valid CitizenRecord for benchmarking.
fn seed_citizen<T: Config + pallet_inomad_identity::Config>(
    account: &T::AccountId,
    nation_id: u32,
    role: CitizenRole,
) {
    Citizens::<T>::insert(
        account,
        CitizenRecord {
            citizen_id: 1u64,
            nation_id,
            naturalized_people_id: None,
            role,
            status: CitizenStatus::Active,
            verification: VerificationStatus::Unverified,
            vesting_level: None,
            branch: None,
            term_end: None,
            khural_terms_served: 0,
            is_indigenous: true,
            citizenship_status: CitizenshipStatus::Indigenous,
            region_id: None,
            birth_region_id: None,
            passport_type: PassportType::Internal,
            document_hash: H256::zero(),
            birth_page_hash: H256::zero(),
            email_hash: H256::zero(),
        },
    );
}

#[benchmarks(where
    BalanceOf<T>: From<u128>,
    T: pallet_inomad_identity::Config,
)]
mod benchmarks {
    use super::*;

    // =========================================================================
    // create_proposal — Active citizen submits a treasury proposal
    // =========================================================================
    /// Worst case: citizen is Active, nation matches. Measures:
    ///   - 2 reads (Citizens, NextProposalId)
    ///   - 1 write (Proposals) + 1 write (NextProposalId)
    #[benchmark]
    fn create_proposal() {
        let caller: T::AccountId = whitelisted_caller();
        seed_citizen::<T>(&caller, 1u32, CitizenRole::Regular);

        let beneficiary: T::AccountId = account("beneficiary", 0, 0);
        let amount: BalanceOf<T> = (1_000u128).into();

        #[extrinsic_call]
        create_proposal(RawOrigin::Signed(caller), 1u32, amount, beneficiary, None);

        assert_eq!(NextProposalId::<T>::get(), 1u32);
    }

    // =========================================================================
    // vote — citizen approves an active proposal
    // =========================================================================
    /// Worst case: proposal Active, citizen Active, not yet voted. Measures:
    ///   - 3 reads (Citizens, Proposals, HasVoted)
    ///   - 2 writes (HasVoted, Proposals)
    #[benchmark]
    fn vote() {
        let proposer: T::AccountId = account("proposer", 0, 0);
        let caller: T::AccountId = whitelisted_caller();
        seed_citizen::<T>(&proposer, 1u32, CitizenRole::Regular);
        seed_citizen::<T>(&caller, 1u32, CitizenRole::Regular);

        // Seed a proposal in storage directly for isolation
        let proposal_id = 0u32;
        let amount: BalanceOf<T> = (1_000u128).into();
        Proposals::<T>::insert(
            proposal_id,
            Proposal {
                proposer: proposer.clone(),
                nation_id: 1u32,
                amount,
                beneficiary: proposer,
                votes_for: 0,
                votes_against: 0,
                status: ProposalStatus::Active,
                end_block: 9_999u32,
                constitutional_basis: None,
            },
        );
        NextProposalId::<T>::put(1u32);

        #[extrinsic_call]
        vote(RawOrigin::Signed(caller.clone()), proposal_id, true);

        assert!(HasVoted::<T>::get(proposal_id, &caller));
    }

    // =========================================================================
    // execute_proposal — manual enactment of an expired proposal
    // =========================================================================
    /// Worst case: votes_for >= MinQuorum, end_block passed, treasury transfer fails
    /// (no treasury funds → proposal rejected). Still measures all reads + writes.
    #[benchmark]
    fn execute_proposal() {
        let caller: T::AccountId = whitelisted_caller();
        let beneficiary: T::AccountId = account("beneficiary", 0, 0);
        let amount: BalanceOf<T> = (1_000u128).into();

        let proposal_id = 0u32;
        Proposals::<T>::insert(
            proposal_id,
            Proposal {
                proposer: caller.clone(),
                nation_id: 1u32,
                amount,
                beneficiary,
                votes_for: 100u32,
                votes_against: 0,
                status: ProposalStatus::Active,
                end_block: 0u32, // already expired
                constitutional_basis: None,
            },
        );

        #[extrinsic_call]
        execute_proposal(RawOrigin::Signed(caller), proposal_id);

        let proposal = Proposals::<T>::get(proposal_id).unwrap();
        // Proposal must no longer be Active regardless of treasury outcome
        assert!(proposal.status != ProposalStatus::Active);
    }

    // =========================================================================
    // vote_on_expert_bill — ArbadLeader+ delegate votes on a bill
    // =========================================================================
    /// Worst case: bill Active, KhuralDelegate voter, not yet voted. Measures:
    ///   - 4 reads (Citizens, ExpertBills, HasVotedOnBill, check)
    ///   - 2 writes (HasVotedOnBill, ExpertBills)
    #[benchmark]
    fn vote_on_expert_bill() {
        let caller: T::AccountId = whitelisted_caller();
        // KhuralDelegate passes `is_at_least_arbad_leader`
        seed_citizen::<T>(&caller, 1u32, CitizenRole::KhuralDelegate);

        let bill_id = 0u32;
        ExpertBills::<T>::insert(
            bill_id,
            ExpertBill {
                proposer: account("proposer", 0, 0),
                bill_hash: H256::from([0xABu8; 32]),
                industry_tag: Default::default(),
                initiative_type: BillInitiativeType::ExpertInitiative,
                status: ProposalStatus::Active,
                votes_for: 0,
                votes_against: 0,
            },
        );

        #[extrinsic_call]
        vote_on_expert_bill(RawOrigin::Signed(caller.clone()), bill_id, true);

        assert!(HasVotedOnBill::<T>::get(bill_id, &caller));
    }

    // =========================================================================
    // execute_expert_bill — finalise a passed expert bill + return deposit
    // =========================================================================
    /// Worst case: bill passes (votes_for > 2), deposit is unreserved.
    /// Measures: ExpertBills read+write + ExpertBillDepositor take + unreserve.
    #[benchmark]
    fn execute_expert_bill() {
        let caller: T::AccountId = whitelisted_caller();
        let depositor: T::AccountId = account("depositor", 0, 0);
        let deposit: BalanceOf<T> = (UNIT).into();
        <T as Config>::Currency::make_free_balance_be(&depositor, (10u128 * UNIT).into());
        <T as Config>::Currency::reserve(&depositor, deposit)
            .expect("benchmark: reserve must succeed");

        let bill_id = 0u32;
        ExpertBills::<T>::insert(
            bill_id,
            ExpertBill {
                proposer: depositor.clone(),
                bill_hash: H256::from([0xCDu8; 32]),
                industry_tag: Default::default(),
                initiative_type: BillInitiativeType::ExpertInitiative,
                status: ProposalStatus::Active,
                votes_for: 10u32, // > 2, passes
                votes_against: 0,
            },
        );
        ExpertBillDepositor::<T>::insert(bill_id, (depositor, deposit));

        #[extrinsic_call]
        execute_expert_bill(RawOrigin::Signed(caller), bill_id);

        let bill = ExpertBills::<T>::get(bill_id).unwrap();
        assert_eq!(bill.status, ProposalStatus::Executed);
    }

    // =========================================================================
    // Test suite wiring
    // =========================================================================
    impl_benchmark_test_suite!(KhuralGov, crate::mock::new_test_ext(), crate::mock::Test);
}
