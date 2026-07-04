#![cfg(feature = "runtime-benchmarks")]
use super::*;
use frame_benchmarking::v2::*;
use frame_support::pallet_prelude::*;
use frame_system::RawOrigin;
use sp_core::H256;

fn make_hash(seed: u8) -> H256 {
    let mut bytes = [0u8; 32];
    bytes[0] = seed;
    H256::from(bytes)
}

#[benchmarks]
mod benchmarks {
    use super::*;

    // propose_agreement(document_hash, parties, validators)
    #[benchmark]
    fn propose_agreement() {
        let creator: T::AccountId = whitelisted_caller();
        let party2: T::AccountId = account("party2", 0, 0);
        let doc_hash = make_hash(1u8);

        #[extrinsic_call]
        propose_agreement(
            RawOrigin::Signed(creator.clone()),
            doc_hash,
            alloc::vec![creator.clone(), party2.clone()],
            None,
        );

        assert!(Agreements::<T>::contains_key(doc_hash));
    }

    // sign_agreement(document_hash) — 1 arg
    #[benchmark]
    fn sign_agreement() {
        let creator: T::AccountId = account("creator", 0, 0);
        let signer: T::AccountId = whitelisted_caller();
        let doc_hash = make_hash(2u8);

        let parties: BoundedVec<T::AccountId, T::MaxParties> =
            alloc::vec![creator.clone(), signer.clone()]
                .try_into()
                .unwrap_or_default();

        Agreements::<T>::insert(
            doc_hash,
            Agreement::<T> {
                document_hash: doc_hash,
                creator: creator.clone(),
                parties,
                validators: None,
                signed_by: BoundedVec::default(),
                validated_by: BoundedVec::default(),
                status: AgreementStatus::PendingSignatures,
            },
        );

        #[extrinsic_call]
        sign_agreement(RawOrigin::Signed(signer.clone()), doc_hash);

        let ag = Agreements::<T>::get(doc_hash).expect("exists");
        assert!(ag.signed_by.contains(&signer));
    }

    impl_benchmark_test_suite!(Pallet, sp_io::TestExternalities::default(), T);
}
