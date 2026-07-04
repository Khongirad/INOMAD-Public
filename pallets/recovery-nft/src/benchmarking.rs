#![cfg(feature = "runtime-benchmarks")]
use super::*;
use frame_benchmarking::v2::*;
use frame_support::pallet_prelude::*;
use frame_system::{pallet_prelude::BlockNumberFor, RawOrigin};

fn make_group_id() -> alloc::vec::Vec<u8> {
    b"bench-group-01".to_vec()
}

fn make_request_id() -> alloc::vec::Vec<u8> {
    b"bench-request-01".to_vec()
}

fn make_vault_addr() -> alloc::vec::Vec<u8> {
    b"vault-addr-bench".to_vec()
}

#[benchmarks]
mod benchmarks {
    use super::*;

    // issue_recovery_nfts(group_id, target_account, holders, destination_kind, vault_address, threshold)
    #[benchmark]
    fn issue_recovery_nfts() {
        let target: T::AccountId = account("target", 0, 0);
        let h1: T::AccountId = account("h1", 0, 0);
        let h2: T::AccountId = account("h2", 0, 0);
        let h3: T::AccountId = account("h3", 0, 0);
        let holders = alloc::vec![h1.clone(), h2.clone(), h3.clone()];

        #[extrinsic_call]
        issue_recovery_nfts(
            RawOrigin::Root, // IssuerOrigin maps Root in mock
            make_group_id(),
            target.clone(),
            holders,
            RecoveryDestination::PersonalBankVault,
            make_vault_addr(),
            2u8, // threshold
        );

        assert_eq!(NextRecoveryTokenId::<T>::get(), 3u64);
    }

    // initiate_recovery(token_id, request_id)
    #[benchmark]
    fn initiate_recovery() {
        let holder: T::AccountId = whitelisted_caller();
        let target: T::AccountId = account("target", 0, 0);
        let token_id: u64 = 0;
        let group_id_bv: BoundedVec<u8, ConstU32<64>> =
            make_group_id().try_into().unwrap_or_default();
        let vault_bv: BoundedVec<u8, ConstU32<128>> =
            make_vault_addr().try_into().unwrap_or_default();

        RecoveryNfts::<T>::insert(
            token_id,
            RecoveryNft::<T::AccountId, BlockNumberFor<T>> {
                token_id,
                group_id: group_id_bv.clone(),
                target_account: target.clone(),
                holder: holder.clone(),
                destination_kind: RecoveryDestination::PersonalBankVault,
                vault_address: vault_bv,
                threshold: 2u8,
                total_issued: 3u8,
                group_index: 1u8,
                issued_at: 0u32.into(),
                used_in_recovery: None,
            },
        );
        NextRecoveryTokenId::<T>::put(1u64);

        #[extrinsic_call]
        initiate_recovery(
            RawOrigin::Signed(holder.clone()),
            token_id,
            make_request_id(),
        );

        let req_id_bv: BoundedVec<u8, ConstU32<64>> =
            make_request_id().try_into().unwrap_or_default();
        assert!(RecoveryRequests::<T>::contains_key(&req_id_bv));
    }

    // veto_recovery(request_id)
    #[benchmark]
    fn veto_recovery() {
        let target: T::AccountId = whitelisted_caller();
        let req_id_bv: BoundedVec<u8, ConstU32<64>> =
            make_request_id().try_into().unwrap_or_default();
        let group_id_bv: BoundedVec<u8, ConstU32<64>> =
            make_group_id().try_into().unwrap_or_default();
        let vault_bv: BoundedVec<u8, ConstU32<128>> =
            make_vault_addr().try_into().unwrap_or_default();

        RecoveryRequests::<T>::insert(
            &req_id_bv,
            RecoveryRequest::<T::AccountId, BlockNumberFor<T>> {
                request_id: req_id_bv.clone(),
                group_id: group_id_bv,
                target_account: target.clone(),
                destination_kind: RecoveryDestination::PersonalBankVault,
                vault_address: vault_bv,
                initiated_by: account("initiator", 0, 0),
                initiated_at: 0u32.into(),
                veto_deadline_block: 999_999u32.into(),
                status: RecoveryRequestStatus::PendingVetoWindow,
                confirmation_count: 1u8,
                threshold: 2u8,
            },
        );

        #[extrinsic_call]
        veto_recovery(RawOrigin::Signed(target.clone()), make_request_id());

        let req = RecoveryRequests::<T>::get(&req_id_bv).expect("exists");
        assert_eq!(req.status, RecoveryRequestStatus::OwnerVetoed);
    }

    // confirm_recovery(token_id, request_id)
    #[benchmark]
    fn confirm_recovery() {
        let holder2: T::AccountId = whitelisted_caller();
        let holder1: T::AccountId = account("holder1", 0, 0);
        let target: T::AccountId = account("target", 0, 0);

        let token_id: u64 = 1;
        let group_id_bv: BoundedVec<u8, ConstU32<64>> =
            make_group_id().try_into().unwrap_or_default();
        let vault_bv: BoundedVec<u8, ConstU32<128>> =
            make_vault_addr().try_into().unwrap_or_default();
        let req_id_bv: BoundedVec<u8, ConstU32<64>> =
            make_request_id().try_into().unwrap_or_default();

        RecoveryNfts::<T>::insert(
            token_id,
            RecoveryNft::<T::AccountId, BlockNumberFor<T>> {
                token_id,
                group_id: group_id_bv.clone(),
                target_account: target.clone(),
                holder: holder2.clone(),
                destination_kind: RecoveryDestination::PersonalBankVault,
                vault_address: vault_bv.clone(),
                threshold: 2u8,
                total_issued: 3u8,
                group_index: 2u8,
                issued_at: 0u32.into(),
                used_in_recovery: None,
            },
        );

        RecoveryRequests::<T>::insert(
            &req_id_bv,
            RecoveryRequest::<T::AccountId, BlockNumberFor<T>> {
                request_id: req_id_bv.clone(),
                group_id: group_id_bv,
                target_account: target.clone(),
                destination_kind: RecoveryDestination::PersonalBankVault,
                vault_address: vault_bv,
                initiated_by: holder1.clone(),
                initiated_at: 0u32.into(),
                veto_deadline_block: 0u32.into(), // veto window expired
                status: RecoveryRequestStatus::PendingVetoWindow,
                confirmation_count: 1u8,
                threshold: 2u8,
            },
        );

        #[extrinsic_call]
        confirm_recovery(
            RawOrigin::Signed(holder2.clone()),
            token_id,
            make_request_id(),
        );

        let req = RecoveryRequests::<T>::get(&req_id_bv).expect("exists");
        assert_eq!(req.status, RecoveryRequestStatus::Executed);
    }

    impl_benchmark_test_suite!(Pallet, sp_io::TestExternalities::default(), T);
}
