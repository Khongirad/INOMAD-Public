#![cfg(feature = "runtime-benchmarks")]
use super::*;
use frame_benchmarking::v2::*;
use frame_support::pallet_prelude::*;
use frame_system::RawOrigin;
use sp_core::H256;

#[benchmarks]
mod benchmarks {
    use super::*;

    // ── 0. publish_document ───────────────────────────────────────────────────
    // publish_document(origin, content_hash: H256, ipfs_cid: Vec<u8>, category: DocumentCategory)
    #[benchmark]
    fn publish_document() {
        let caller: T::AccountId = whitelisted_caller();
        let content_hash = H256::from([1u8; 32]);
        let ipfs_cid = b"QmTvf7ANyuXQ6Q4Y8v7PDzXpqU5q4oT1kGHVb3z5g9K7L2".to_vec();

        #[extrinsic_call]
        publish_document(
            RawOrigin::Signed(caller.clone()),
            content_hash,
            ipfs_cid,
            DocumentCategory::Media,
        );

        assert!(Documents::<T>::contains_key(content_hash));
    }

    // ── 1. donate_to_author ───────────────────────────────────────────────────
    // donate_to_author(origin, document_hash: H256, amount: BalanceOf<T>)
    #[benchmark]
    fn donate_to_author() {
        use frame_support::traits::Currency;
        let donor: T::AccountId = whitelisted_caller();
        let author: T::AccountId = account("author", 0, 0);
        let content_hash = H256::from([2u8; 32]);
        let amount: BalanceOf<T> = (1_000_000_000_000u128 * 10u128)
            .try_into()
            .unwrap_or_default();

        // Fund donor
        <T as Config>::Currency::make_free_balance_be(&donor, amount);

        // Pre-seed a document by author
        Documents::<T>::insert(
            content_hash,
            DocumentRecord::<T> {
                owner: author.clone(),
                content_hash,
                ipfs_cid: BoundedVec::try_from(b"QmTest".to_vec()).unwrap_or_default(),
                category: DocumentCategory::Media,
                block_number: 0u32.into(),
            },
        );

        #[extrinsic_call]
        donate_to_author(RawOrigin::Signed(donor.clone()), content_hash, amount);
    }

    impl_benchmark_test_suite!(Pallet, sp_io::TestExternalities::default(), T);
}
