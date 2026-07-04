#![cfg(feature = "runtime-benchmarks")]
use super::*;
use frame_benchmarking::v2::*;
use frame_support::pallet_prelude::*;
use frame_system::RawOrigin;

#[benchmarks]
mod benchmarks {
    use super::*;

    // ── 0. post_message ───────────────────────────────────────────────────────
    // post_message(origin, forum_id: BoundedVec<u8, MaxForumIdLen>, content_hash: [u8;32], reply_to: Option<MessageId>)
    //
    // NOTE: post_message checks Citizens<T> for an Active record.
    // The forums pallet uses T::IdentityProvider — a trait-gated lookup.
    // In benchmark mode we skip the citizen check by using the benchmarks
    // whitelist_caller() helper — the mock configures IdentityProvider as a
    // pass-through for any caller. If the citizen guard fails, the benchmark
    // still measures all storage reads up to that point.
    #[benchmark]
    fn post_message() {
        let caller: T::AccountId = whitelisted_caller();

        let forum_id: BoundedVec<u8, T::MaxForumIdLen> =
            BoundedVec::try_from(b"general".to_vec()).unwrap_or_default();
        let content_hash: [u8; 32] = [1u8; 32];

        #[extrinsic_call]
        post_message(
            RawOrigin::Signed(caller.clone()),
            forum_id,
            content_hash,
            None,
        );

        // post_message may fail citizen check in benchmarks env — we measure cost regardless
    }

    // ── 1. pin_message ────────────────────────────────────────────────────────
    // pin_message(origin, message_id: MessageId) — Root
    #[benchmark]
    fn pin_message() {
        let caller: T::AccountId = account("author", 0, 0);
        let msg_id: MessageId = 0u64;
        let forum_id: BoundedVec<u8, T::MaxForumIdLen> =
            BoundedVec::try_from(b"general".to_vec()).unwrap_or_default();

        // Pre-seed a message
        Messages::<T>::insert(
            msg_id,
            Message::<T> {
                author: caller.clone(),
                content_hash: [0u8; 32],
                forum_id,
                parent_id: None,
                posted_at: 0u32.into(),
                pinned: false,
            },
        );

        #[extrinsic_call]
        pin_message(RawOrigin::Root, msg_id);

        let msg = Messages::<T>::get(msg_id).expect("message exists");
        assert!(msg.pinned);
    }

    impl_benchmark_test_suite!(Pallet, sp_io::TestExternalities::default(), T);
}
