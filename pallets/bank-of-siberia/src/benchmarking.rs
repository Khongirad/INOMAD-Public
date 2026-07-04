//! Benchmarks for pallet-bank-of-siberia
//!
//! Covers all 11 extrinsics / weight placeholders:
//!   0.  open_master_account   (Signed)
//!   1.  open_sub_account      (Signed)
//!   2.  withdraw_from_savings (Signed)
//!   3.  pay_credit            (Signed)
//!   4.  close_sub_account     (Signed)
//!   5.  request_loan          (Signed)
//!   6.  create_escrow         (Signed)
//!   7.  release_escrow        (Signed — depositor)
//!   8.  refund_escrow         (Signed — counterparty)
//!   9.  open_time_deposit     (Signed)
//!   10. claim_time_deposit    (Signed — after maturity)

#![cfg(feature = "runtime-benchmarks")]

use super::*;
use frame_benchmarking::v2::*;
use frame_support::pallet_prelude::*;
use frame_support::traits::{Currency, LockableCurrency};
use frame_system::RawOrigin;

const UNIT: u128 = 1_000_000_000_000u128;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn fund<T: Config>(who: &T::AccountId) {
    let amount: BalanceOf<T> = (UNIT * 10_000).try_into().unwrap_or_default();
    <T as Config>::Currency::make_free_balance_be(who, amount);
}

/// Open a master account for `who` by inserting directly into storage.
fn seed_master<T: Config>(who: &T::AccountId) {
    fund::<T>(who);
    BankAccounts::<T>::insert(
        who,
        BankAccount::<T> {
            owner: who.clone(),
            opened_at: 0u32,
            status: AccountStatus::Active,
        },
    );
}

/// Derive and open a Savings sub-account for `master`.
fn seed_savings<T: Config>(master: &T::AccountId) -> T::AccountId {
    let sub = Pallet::<T>::derive_sub_account(master, AccountType::Savings, 0);
    fund::<T>(&sub);
    SubAccountMeta::<T>::insert(
        &sub,
        SubAccountRecord::<T> {
            master: master.clone(),
            account_type: AccountType::Savings,
            opened_at: 0u32,
        },
    );
    SubAccounts::<T>::insert(master, &sub, AccountType::Savings);
    sub
}

/// Open a Credit sub-account for `master`.
fn seed_credit<T: Config>(master: &T::AccountId) -> T::AccountId {
    let sub = Pallet::<T>::derive_sub_account(master, AccountType::Credit, 0);
    fund::<T>(&sub);
    SubAccountMeta::<T>::insert(
        &sub,
        SubAccountRecord::<T> {
            master: master.clone(),
            account_type: AccountType::Credit,
            opened_at: 0u32,
        },
    );
    SubAccounts::<T>::insert(master, &sub, AccountType::Credit);
    sub
}

/// Seed a Locked escrow contract.
fn seed_escrow<T: Config>(depositor: &T::AccountId, counterparty: &T::AccountId) -> u32 {
    let eid = NextEscrowId::<T>::get();
    NextEscrowId::<T>::put(eid + 1);
    let amount: BalanceOf<T> = (UNIT * 100).try_into().unwrap_or_default();

    // Transfer funds to treasury to simulate locked escrow
    let treasury = Pallet::<T>::treasury_account();
    fund::<T>(&treasury);

    EscrowContracts::<T>::insert(
        eid,
        EscrowContract::<T> {
            depositor: depositor.clone(),
            counterparty: counterparty.clone(),
            amount,
            status: EscrowStatus::Locked,
            item_hash: [0u8; 32],
            created_at: 0u32,
        },
    );
    eid
}

/// Seed a Pending loan.
fn seed_loan<T: Config>(borrower: &T::AccountId) -> u32 {
    let lid = NextLoanId::<T>::get();
    NextLoanId::<T>::put(lid + 1);
    let amount: BalanceOf<T> = (UNIT * 100).try_into().unwrap_or_default();

    LoanRequests::<T>::insert(
        lid,
        LoanRequest::<T> {
            borrower: borrower.clone(),
            amount,
            submitted_at: 0u32,
            status: LoanStatus::Pending,
            outstanding_balance: amount,
        },
    );
    lid
}

/// Seed a matured time deposit by setting maturity_block = 0 (always past).
fn seed_matured_deposit<T: Config>(depositor: &T::AccountId) -> u32 {
    let did = NextTimeDepositId::<T>::get();
    NextTimeDepositId::<T>::put(did + 1);
    let amount: BalanceOf<T> = (UNIT * 500).try_into().unwrap_or_default();

    // Fund the treasury so interest can be paid out
    let treasury = Pallet::<T>::treasury_account();
    fund::<T>(&treasury);

    TimeDeposits::<T>::insert(
        did,
        TimeDeposit::<T> {
            depositor: depositor.clone(),
            amount,
            opened_at: 0u32,
            maturity_block: 0u32, // already matured at block 0
            status: DepositStatus::Active,
            document_hash: [0u8; 32],
            interest_rate_bps: 500u32,
        },
    );
    did
}

// ---------------------------------------------------------------------------
// Benchmark suite
// ---------------------------------------------------------------------------

#[benchmarks]
mod benchmarks {
    use super::*;

    // ── open_master_account ───────────────────────────────────────────────────

    #[benchmark]
    fn open_master_account() {
        let caller: T::AccountId = whitelisted_caller();
        fund::<T>(&caller);

        #[extrinsic_call]
        open_master_account(RawOrigin::Signed(caller.clone()));

        assert!(BankAccounts::<T>::contains_key(&caller));
    }

    // ── open_sub_account ─────────────────────────────────────────────────────

    #[benchmark]
    fn open_sub_account() {
        let caller: T::AccountId = whitelisted_caller();
        seed_master::<T>(&caller);

        #[extrinsic_call]
        open_sub_account(RawOrigin::Signed(caller.clone()), AccountType::Savings);

        let sub = Pallet::<T>::derive_sub_account(&caller, AccountType::Savings, 0);
        assert!(SubAccountMeta::<T>::contains_key(&sub));
    }

    // ── withdraw_from_savings ─────────────────────────────────────────────────

    #[benchmark]
    fn withdraw_from_savings() {
        let caller: T::AccountId = whitelisted_caller();
        seed_master::<T>(&caller);
        let sub = seed_savings::<T>(&caller);

        let amount: BalanceOf<T> = (UNIT * 10).try_into().unwrap_or_default();

        #[extrinsic_call]
        withdraw_from_savings(RawOrigin::Signed(caller.clone()), sub.clone(), amount);
    }

    // ── pay_credit ────────────────────────────────────────────────────────────

    #[benchmark]
    fn pay_credit() {
        let caller: T::AccountId = whitelisted_caller();
        seed_master::<T>(&caller);
        let sub = seed_credit::<T>(&caller);

        let amount: BalanceOf<T> = (UNIT * 10).try_into().unwrap_or_default();

        #[extrinsic_call]
        pay_credit(RawOrigin::Signed(caller.clone()), sub.clone(), amount);
    }

    // ── close_sub_account ─────────────────────────────────────────────────────

    #[benchmark]
    fn close_sub_account() {
        let caller: T::AccountId = whitelisted_caller();
        seed_master::<T>(&caller);

        // Open a Company sub-account with zero balance (easier to close)
        let sub = Pallet::<T>::derive_sub_account(&caller, AccountType::Company, 0);
        SubAccountMeta::<T>::insert(
            &sub,
            SubAccountRecord::<T> {
                master: caller.clone(),
                account_type: AccountType::Company,
                opened_at: 0u32,
            },
        );
        SubAccounts::<T>::insert(&caller, &sub, AccountType::Company);

        #[extrinsic_call]
        close_sub_account(RawOrigin::Signed(caller.clone()), sub.clone());

        assert!(!SubAccountMeta::<T>::contains_key(&sub));
    }

    // ── request_loan ──────────────────────────────────────────────────────────

    #[benchmark]
    fn request_loan() {
        let caller: T::AccountId = whitelisted_caller();
        seed_master::<T>(&caller);

        let amount: BalanceOf<T> = (UNIT * 500).try_into().unwrap_or_default();

        #[extrinsic_call]
        request_loan(RawOrigin::Signed(caller.clone()), amount);

        assert!(NextLoanId::<T>::get() > 0);
    }

    // ── create_escrow ─────────────────────────────────────────────────────────

    #[benchmark]
    fn create_escrow() {
        let caller: T::AccountId = whitelisted_caller();
        let counterparty: T::AccountId = account("counterparty", 0, 0);
        seed_master::<T>(&caller);
        fund::<T>(&counterparty);

        let amount: BalanceOf<T> = (UNIT * 100).try_into().unwrap_or_default();
        let item_hash = [0x42u8; 32];

        #[extrinsic_call]
        create_escrow(
            RawOrigin::Signed(caller.clone()),
            counterparty.clone(),
            amount,
            item_hash,
        );

        assert!(NextEscrowId::<T>::get() > 0);
    }

    // ── release_escrow ────────────────────────────────────────────────────────

    #[benchmark]
    fn release_escrow() {
        let depositor: T::AccountId = whitelisted_caller();
        let counterparty: T::AccountId = account("counterparty", 0, 0);
        fund::<T>(&depositor);
        fund::<T>(&counterparty);
        let eid = seed_escrow::<T>(&depositor, &counterparty);

        #[extrinsic_call]
        release_escrow(RawOrigin::Signed(depositor.clone()), eid);

        let escrow = EscrowContracts::<T>::get(eid).expect("exists");
        assert_eq!(escrow.status, EscrowStatus::Released);
    }

    // ── refund_escrow ─────────────────────────────────────────────────────────

    #[benchmark]
    fn refund_escrow() {
        let depositor: T::AccountId = account("depositor", 0, 0);
        let counterparty: T::AccountId = whitelisted_caller();
        fund::<T>(&depositor);
        fund::<T>(&counterparty);
        let eid = seed_escrow::<T>(&depositor, &counterparty);

        #[extrinsic_call]
        refund_escrow(RawOrigin::Signed(counterparty.clone()), eid);

        let escrow = EscrowContracts::<T>::get(eid).expect("exists");
        assert_eq!(escrow.status, EscrowStatus::Refunded);
    }

    // ── open_time_deposit ─────────────────────────────────────────────────────

    #[benchmark]
    fn open_time_deposit() {
        let caller: T::AccountId = whitelisted_caller();
        seed_master::<T>(&caller);

        let amount: BalanceOf<T> = (UNIT * 1_000).try_into().unwrap_or_default();
        let duration_blocks: u32 = 100u32;
        let doc_hash = [0xAAu8; 32];

        // Fund the treasury so transfer succeeds
        let treasury = Pallet::<T>::treasury_account();
        fund::<T>(&treasury);

        #[extrinsic_call]
        open_time_deposit(
            RawOrigin::Signed(caller.clone()),
            amount,
            duration_blocks,
            doc_hash,
        );

        assert!(NextTimeDepositId::<T>::get() > 0);
    }

    // ── claim_time_deposit ────────────────────────────────────────────────────

    #[benchmark]
    fn claim_time_deposit() {
        let caller: T::AccountId = whitelisted_caller();
        seed_master::<T>(&caller);
        let did = seed_matured_deposit::<T>(&caller);

        #[extrinsic_call]
        claim_time_deposit(RawOrigin::Signed(caller.clone()), did);

        let deposit = TimeDeposits::<T>::get(did).expect("exists");
        assert_eq!(deposit.status, DepositStatus::Claimed);
    }

    impl_benchmark_test_suite!(
        Pallet,
        // bank-of-siberia has no test module — use a minimal runtime
        // If tests are added later, replace with: crate::mock::new_test_ext()
        sp_io::TestExternalities::default(),
        sp_runtime::testing::H256 // placeholder — will be replaced when mock is added
    );
}
