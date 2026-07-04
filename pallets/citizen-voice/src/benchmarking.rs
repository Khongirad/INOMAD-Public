#![cfg(feature = "runtime-benchmarks")]
use super::*;
use frame_benchmarking::v2::*;
use frame_support::{pallet_prelude::*, traits::ReservableCurrency};
use frame_system::RawOrigin;
use sp_core::H256;

#[benchmarks]
mod benchmarks {
    use super::*;

    // ── 0. submit_ticket ──────────────────────────────────────────────────────
    // submit_ticket(origin, target: FeedbackTarget<AccountId>, feedback_type: FeedbackType, content_hash: H256)
    #[benchmark]
    fn submit_ticket() {
        use frame_support::traits::Currency;
        let caller: T::AccountId = whitelisted_caller();
        let target_account: T::AccountId = account("official", 0, 0);
        let content_hash = H256::from([1u8; 32]);

        // Fund caller with enough for the anti-spam deposit
        let deposit_amount: BalanceOf<T> = (1_000_000_000_000u128 * 1_000u128)
            .try_into()
            .unwrap_or_default();
        <T as Config>::Currency::make_free_balance_be(&caller, deposit_amount);

        #[extrinsic_call]
        submit_ticket(
            RawOrigin::Signed(caller.clone()),
            FeedbackTarget::Entity(target_account),
            FeedbackType::Complaint,
            content_hash,
        );

        assert!(NextTicketId::<T>::get() > 0);
    }

    // ── 1. mark_in_review ────────────────────────────────────────────────────
    // mark_in_review(origin, ticket_id: u32) — Root
    #[benchmark]
    fn mark_in_review() {
        use frame_support::traits::Currency;
        let author: T::AccountId = account("author", 0, 0);
        let target: T::AccountId = account("target", 0, 0);
        let ticket_id: u32 = 0;
        let content_hash = H256::from([2u8; 32]);
        let deposit: BalanceOf<T> = (1_000_000_000_000u128).try_into().unwrap_or_default();

        <T as Config>::Currency::make_free_balance_be(&author, deposit);
        <T as Config>::Currency::reserve(&author, deposit).ok();

        Tickets::<T>::insert(
            ticket_id,
            Ticket::<T> {
                author: author.clone(),
                target: FeedbackTarget::Entity(target.clone()),
                feedback_type: FeedbackType::Complaint,
                content_hash,
                status: TicketStatus::Open,
                deposit,
            },
        );
        TargetTickets::<T>::insert(&FeedbackTarget::Entity(target), ticket_id, ());
        NextTicketId::<T>::put(1u32);

        #[extrinsic_call]
        mark_in_review(RawOrigin::Root, ticket_id);

        let ticket = Tickets::<T>::get(ticket_id).expect("ticket exists");
        assert_eq!(ticket.status, TicketStatus::InReview);
    }

    impl_benchmark_test_suite!(Pallet, sp_io::TestExternalities::default(), T);
}
