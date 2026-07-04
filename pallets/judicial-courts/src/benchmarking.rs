#![cfg(feature = "runtime-benchmarks")]
use super::*;
use frame_benchmarking::v2::*;
use frame_support::pallet_prelude::*;
use frame_system::RawOrigin;

fn fund_fungible<T: Config>(who: &T::AccountId) {
    use frame_support::traits::fungible::Mutate;
    let amount: BalanceOf<T> = (1_000_000_000_000u128 * 100_000u128)
        .try_into()
        .unwrap_or_default();
    T::Currency::set_balance(who, amount);
}

/// Seed a Guilty case in CourtCases
fn seed_guilty_case<T: Config>(case_id: u32, plaintiff: &T::AccountId, defendant: &T::AccountId) {
    fund_fungible::<T>(defendant);
    let penalty: BalanceOf<T> = (1_000_000_000_000u128 * 10u128)
        .try_into()
        .unwrap_or_default();
    CourtCases::<T>::insert(
        case_id,
        CourtCase::<T> {
            plaintiff: plaintiff.clone(),
            defendant: defendant.clone(),
            whistleblower: None,
            evidence_hash: [0u8; 32],
            verdict_hash: Some([1u8; 32]),
            penalty_amount: penalty,
            status: CaseStatus::Guilty,
            crime_category: Some(CrimeCategory::Economic),
        },
    );
    NextCaseId::<T>::put(case_id + 1);
}

#[benchmarks]
mod benchmarks {
    use super::*;

    // open_case(defendant, evidence_hash, whistleblower) — 3 args
    #[benchmark]
    fn open_case() {
        let plaintiff: T::AccountId = whitelisted_caller();
        let defendant: T::AccountId = account("defendant", 0, 0);
        let evidence_hash: [u8; 32] = [0u8; 32];
        #[extrinsic_call]
        open_case(
            RawOrigin::Signed(plaintiff.clone()),
            defendant.clone(),
            evidence_hash,
            None,
        );
        assert!(NextCaseId::<T>::get() > 0);
    }

    // issue_verdict(case_id, is_guilty, verdict_hash, penalty_amount, crime_category) — 5 args
    #[benchmark]
    fn issue_verdict() {
        let plaintiff: T::AccountId = account("plaintiff", 0, 0);
        let defendant: T::AccountId = account("defendant", 0, 0);
        let case_id: u32 = 0;
        // Seed an Open case
        CourtCases::<T>::insert(
            case_id,
            CourtCase::<T> {
                plaintiff: plaintiff.clone(),
                defendant: defendant.clone(),
                whistleblower: None,
                evidence_hash: [0u8; 32],
                verdict_hash: None,
                penalty_amount: BalanceOf::<T>::from(0u32),
                status: CaseStatus::Open,
                crime_category: None,
            },
        );
        NextCaseId::<T>::put(1u32);
        let verdict_hash: [u8; 32] = [1u8; 32];
        let penalty: BalanceOf<T> = (1_000_000_000_000u128 * 10u128)
            .try_into()
            .unwrap_or_default();
        #[extrinsic_call]
        issue_verdict(
            RawOrigin::Root, // JudgesCollectiveOrigin maps Root in mock
            case_id,
            true,
            verdict_hash,
            penalty,
            CrimeCategory::Economic,
        );
        let case = CourtCases::<T>::get(case_id).expect("exists");
        assert_eq!(case.status, CaseStatus::Guilty);
    }

    // execute_verdict(case_id) — 1 arg
    #[benchmark]
    fn execute_verdict() {
        let caller: T::AccountId = whitelisted_caller();
        let plaintiff: T::AccountId = account("plaintiff", 0, 0);
        let defendant: T::AccountId = account("defendant", 0, 0);
        let treasury = T::Treasury::get();
        fund_fungible::<T>(&treasury);
        let case_id: u32 = 0;
        seed_guilty_case::<T>(case_id, &plaintiff, &defendant);
        #[extrinsic_call]
        execute_verdict(RawOrigin::Signed(caller.clone()), case_id);
        let case = CourtCases::<T>::get(case_id).expect("exists");
        assert_eq!(case.status, CaseStatus::Executed);
    }

    // declare_usurper(target) — 1 arg
    #[benchmark]
    fn declare_usurper() {
        let target: T::AccountId = account("target", 0, 0);
        fund_fungible::<T>(&target);
        #[extrinsic_call]
        declare_usurper(
            RawOrigin::Root, // UsurpationOrigin maps Root in mock
            target.clone(),
        );
        assert!(MartialLawActive::<T>::get());
    }

    impl_benchmark_test_suite!(Pallet, sp_io::TestExternalities::default(), T);
}
