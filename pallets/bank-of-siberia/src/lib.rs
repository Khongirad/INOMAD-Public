//! # pallet-bank-of-siberia
//!
//! **Altan Network — v3 Architecture: Bank of Siberia (Credit Operations Tier)**
//!
//! The Bank of Siberia is the **credit operator, financial oracle, escrow service,
//! and account registration gateway** of the Altan Network.
//!
//! ## Constitutional Mandate
//!
//! | Action                                      | Allowed  |
//! |---------------------------------------------|----------|
//! | Open citizen master bank accounts           | ✓        |
//! | Open deterministic keyless sub-accounts     | ✓        |
//! | Issue collateral-backed loans               | ✓        |
//! | Create and manage escrow contracts          | ✓        |
//! | Lock citizen funds (with consent)           | ✓        |
//! | Issue primary ALTAN emission                | ✗ NEVER  |
//! | Transfer/withdraw citizen funds unilaterally | ✗ NEVER  |
//!
//! ## Separation of Powers (v3 Architecture)
//!
//! - **Central Bank** → sole issuer of primary ALTAN supply.
//! - **Bank of Siberia** → credit operator; uses funds **allocated** by the
//!   Central Bank.  It never calls `Currency::deposit_creating` directly.
//!
//! ## Keyless Sub-Account Architecture
//!
//! Citizens do NOT generate a new 12-word seed for each account type.
//! Instead, sub-accounts are **deterministic derived addresses** produced by:
//!
//! ```text
//! sub_account = Blake2_256( master_bytes ++ account_type_byte ++ nonce_le )
//! ```
//!
//! The resulting address has **no private key** — nobody knows the preimage seed.
//! Control is maintained exclusively through the Master Key (Altan Vault native
//! signing identity, which already holds Social Recovery).
//!
//! ## Social Recovery Compatibility
//!
//! All access-control checks use `ensure_signed(origin)` (the master AccountId).
//! When `pallet-recovery::as_recovered` restores the Master Key origin, the
//! recovered identity retains full ownership of ALL sub-accounts transparently.
//!
//! ## Account Hierarchy
//!
//! ```text
//! Master Account  (12-word seed, Social Recovery)
//!   ├─► Savings Account    (keyless, Blake2_256 derived)
//!   ├─► Credit Account     (keyless, Blake2_256 derived)
//!   └─► Company Account    (keyless, Blake2_256 derived)
//! ```
//!
//! ## Sprint Note
//!
//! This is the **v3 scaffold** (Sprint L1-FinancialCore v2).  The extrinsics
//! `withdraw_from_savings`, `pay_credit`, and `close_sub_account` carry skeletal
//! logic: they validate ownership, store intent, and emit events.  Full fund
//! locking (`LockableCurrency::set_lock`) will be wired in the next sprint.
//!
//! ## Dispatchables
//!
//! | Call | Origin | Description |
//! |---|---|---|
//! | `open_master_account` | Signed | Open a primary savings account for a citizen |
//! | `open_sub_account` | Signed | Open a named sub-account under a master account |
//! | `withdraw_from_savings` | Signed | Withdraw ALTAN from a savings or sub-account |
//! | `pay_credit` | Signed | Make a repayment towards an outstanding credit line |
//! | `close_sub_account` | Signed | Close an empty sub-account |
//! | `request_loan` | Signed | Apply for a citizen credit line (subject to CB credit epoch) |
//! | `approve_loan` | BankingOrigin | Approve a pending loan request and disburse funds |
//! | `cancel_loan_request` | Signed (borrower) | Cancel a pending loan request and release collateral |
//! | `fund_treasury` | BankingOrigin | Fund BOS treasury (for time deposit interest reserves) |
//! | `create_escrow` | Signed | Lock funds in escrow pending a contractual condition |
//! | `release_escrow` | Signed (Buyer) | Release escrowed funds to the beneficiary on condition met |
//! | `refund_escrow` | Signed (Seller) | Return escrowed funds to the depositor if condition fails |
//! | `open_time_deposit` | Signed | Lock ALTAN for a fixed term at a guaranteed yield |
//! | `claim_time_deposit` | Signed | Unlock a matured time deposit and collect interest |

#![cfg_attr(not(feature = "std"), no_std)]

extern crate alloc;

pub mod migrations;
pub mod weights;
pub use pallet::*;

#[cfg(feature = "runtime-benchmarks")]
mod benchmarking;

#[frame_support::pallet]
pub mod pallet {
    use crate::weights::WeightInfo as _;
    use alloc::vec::Vec;
    use codec::{Decode, Encode};
    use frame_support::{
        pallet_prelude::*,
        traits::{Currency, EnsureOrigin, ExistenceRequirement, LockableCurrency},
        PalletId,
    };
    use frame_system::pallet_prelude::*;
    use sp_io::hashing::blake2_256;
    use sp_runtime::{traits::{AccountIdConversion, Verify, SaturatedConversion}, MultiSignature, Perbill, Saturating};

    // =========================================================================
    // Type Aliases
    // =========================================================================

    /// Convenience alias for the balance type derived from the `Currency` associated type.
    pub type BalanceOf<T> =
        <<T as Config>::Currency as Currency<<T as frame_system::Config>::AccountId>>::Balance;

    // =========================================================================
    // Pallet
    // =========================================================================

    #[pallet::pallet]
    pub struct Pallet<T>(_);

    // =========================================================================
    // Economic Constants
    // =========================================================================

    /// Fixed annual interest rate for Time Deposits, in basis points (500 = 5% APY).
    ///
    /// Constitutional mandate: sovereign citizens who voluntarily transfer
    /// liquidity to the Bank treasury earn 5% APY.  The Bank does NOT invent
    /// yield — it is funded through operational revenue routed by the Central Bank.
    const ANNUAL_RATE_BPS: u32 = 500;

    /// Approximate block count per calendar year at 6-second block time.
    /// 365 × 24 × 3_600 / 6 = 5_256_000 blocks.
    const BLOCKS_PER_YEAR: u32 = 5_256_000;

    /// `LockIdentifier` used when locking loan collateral via `LockableCurrency::set_lock`.
    ///
    /// This 8-byte ID is unique to the Bank of Siberia and distinguishes our
    /// collateral locks from staking locks, vesting locks, and democracy locks.
    /// It MUST remain stable across runtime upgrades — changing it would make
    /// existing locked funds inaccessible to `remove_lock`.
    const LOAN_LOCK_ID: frame_support::traits::LockIdentifier = *b"bos/loan";

    // =========================================================================
    // Config
    // =========================================================================

    /// Bank of Siberia pallet configuration.
    ///
    /// The `Currency` associated type requires `LockableCurrency` because the
    /// bank must be able to lock citizen funds as collateral for loans and
    /// hold funds in escrow contracts without transferring ownership.
    ///
    /// Critically, this pallet does NOT use `Currency::deposit_creating` —
    /// primary ALTAN emission is the exclusive domain of `pallet-central-bank`.
    #[pallet::config]
    pub trait Config: frame_system::Config<RuntimeEvent: From<Event<Self>>>
        + pallet_migration_center::Config
        + pallet_central_bank::Config
    {
        /// Weight information for extrinsics in this pallet.
        type WeightInfo: crate::weights::WeightInfo;
        /// The native currency of the Altan Network (ALTAN token).
        ///
        /// Must implement `LockableCurrency` to support:
        /// - Fund locking for loan collateral (`request_loan`)
        /// - Fund locking for escrow contracts (`create_escrow`)
        ///
        /// This pallet NEVER calls `deposit_creating` — primary emission is
        /// the exclusive mandate of `pallet-central-bank`.
        type Currency: LockableCurrency<Self::AccountId, Balance = pallet_central_bank::BalanceOf<Self>>;

        /// The sovereign `PalletId` of the Bank of Siberia treasury.
        ///
        /// ALTAN deposited as Time Deposits are **physically transferred** into
        /// the account derived from this ID (`PalletId::into_account_truncating`).
        /// The treasury has no private key — it is controlled exclusively by
        /// the pallet logic.
        ///
        /// Recommended value: `PalletId(*b"bos/depo")`
        #[pallet::constant]
        type PalletId: Get<PalletId>;

        /// Banking Branch origin — the constitutionally independent fourth branch.
        ///
        /// Required for privileged operations:
        /// - `approve_loan`: disbursing loan funds to the borrower
        /// - `fund_treasury`: topping up the BOS treasury for interest reserves
        ///
        /// In development: `EnsureRoot` (Sudo key).
        /// In production: `EnsureRootOrBankingCouncil` — same as `pallet-central-bank`.
        type BankingOrigin: EnsureOrigin<Self::RuntimeOrigin>;

        // ── Constitutional Constants (Physics of the Network) ─────────────────
        //
        // These values are the IMMUTABLE LAWS of the Altan Republic.
        // They are encoded as type-system constants via `Get<Perbill>` and
        // backed by `parameter_types!` in the runtime — which means they
        // CANNOT be changed at runtime without a full WASM upgrade referendum.
        //
        // The State Khural (pallet-khural-governance) has NO access to these
        // values.  The Khural manages budgets and grants; it does NOT control
        // the gravitational constants of the economy.

        /// **Constitutional Cross-Transfer Fee: 0.03%**
        ///
        /// Applied to every cross-border ALTAN transfer and inter-nation
        /// transaction.  Distributed per the constitutional split defined in
        /// `pallet-altan-tax`:
        ///
        ///   - INOMAD AG   → 36%
        ///   - Validators  → 10%
        ///   - Khural Foundation → 54% (27% tech + 27% indigenous, governed by 86 Academy reps)
        ///
        /// Mathematically: `fee = amount × Perbill::from_rational(3, 10_000)`
        ///
        /// **This constant is immutable.  The Khural cannot change it.**
        /// Configure in the runtime as:
        /// `type CrossTransferFee = ConstCrossTransferFee; // Perbill::from_parts(300_000)`
        #[pallet::constant]
        type CrossTransferFee: Get<Perbill>;

        /// **Constitutional State Tax Rate: 10%**
        ///
        /// Applied to citizen income and commercial profit within the Altan
        /// Network.  Routed entirely to the nation's sovereign treasury for
        /// public services, infrastructure, and guild-managed programmes.
        ///
        /// Mathematically: `tax = income × Perbill::from_percent(10)`
        ///
        /// **This constant is immutable.  The Khural cannot change it.**
        /// Configure in the runtime as:
        /// `type StateTaxRate = ConstStateTaxRate; // Perbill::from_percent(10)`
        #[pallet::constant]
        type StateTaxRate: Get<Perbill>;
    }

    // =========================================================================
    // Enums
    // =========================================================================

    /// Classification of a citizen's sub-account type.
    ///
    /// Each type maps to a unique derivation index, ensuring a single master
    /// key can open at most one sub-account per category.
    #[derive(
        Encode,
        Decode,
        DecodeWithMemTracking,
        Clone,
        Copy,
        PartialEq,
        Eq,
        RuntimeDebug,
        TypeInfo,
        MaxEncodedLen,
    )]
    pub enum AccountType {
        /// Personal savings vault — long-term ALTAN reserves.
        Savings,
        /// Credit line account — tracks loan disbursements and repayments.
        Credit,
        /// Corporate/organisational account — for registered entities.
        Company,
    }

    impl AccountType {
        /// Stable single-byte discriminant used in sub-account derivation.
        ///
        /// MUST be stable across runtime upgrades — changing these values
        /// would derive different addresses for existing sub-accounts.
        pub fn derivation_index(self) -> u8 {
            match self {
                AccountType::Savings => 0,
                AccountType::Credit => 1,
                AccountType::Company => 2,
            }
        }
    }

    /// Status of a Time Deposit contract.
    #[derive(
        Encode,
        Decode,
        DecodeWithMemTracking,
        Clone,
        PartialEq,
        Eq,
        RuntimeDebug,
        TypeInfo,
        MaxEncodedLen,
    )]
    pub enum DepositStatus {
        /// Funds are locked in the Bank treasury; maturity not yet reached.
        Active,
        /// Maturity reached; principal + interest have been claimed.
        Claimed,
    }

    /// Status of a citizen's master bank account.
    #[derive(
        Encode,
        Decode,
        DecodeWithMemTracking,
        Clone,
        PartialEq,
        Eq,
        RuntimeDebug,
        TypeInfo,
        MaxEncodedLen,
    )]
    pub enum AccountStatus {
        /// Account is active and in good standing.
        Active,
        /// Account has been suspended pending judicial review.
        Suspended,
    }

    /// Status of a loan request.
    #[derive(
        Encode,
        Decode,
        DecodeWithMemTracking,
        Clone,
        PartialEq,
        Eq,
        RuntimeDebug,
        TypeInfo,
        MaxEncodedLen,
    )]
    pub enum LoanStatus {
        /// Loan request submitted, awaiting bank approval.
        Pending,
        /// Loan approved and funds disbursed.
        Active,
        /// Loan fully repaid.
        Repaid,
        /// Loan in default; court order may be requested.
        Default,
    }

    /// Status of an escrow contract.
    #[derive(
        Encode,
        Decode,
        DecodeWithMemTracking,
        Clone,
        PartialEq,
        Eq,
        RuntimeDebug,
        TypeInfo,
        MaxEncodedLen,
    )]
    pub enum EscrowStatus {
        /// Funds are locked in the Bank treasury; awaiting buyer confirmation.
        Locked,
        /// Buyer confirmed receipt; funds released to the seller.
        Released,
        /// Seller initiated refund; funds returned to the buyer.
        Refunded,
    }

    // =========================================================================
    // Structs
    // =========================================================================

    /// On-chain master bank account record for a registered citizen.
    #[derive(
        Encode,
        Decode,
        DecodeWithMemTracking,
        Clone,
        PartialEq,
        Eq,
        RuntimeDebug,
        TypeInfo,
        MaxEncodedLen,
    )]
    #[scale_info(skip_type_params(T))]
    pub struct BankAccount<T: Config> {
        /// The citizen's on-chain master account ID.
        pub owner: T::AccountId,
        /// Block number when the account was opened.
        pub opened_at: u32,
        /// Current status of the account.
        pub status: AccountStatus,
    }

    /// Metadata stored for each derived sub-account.
    #[derive(
        Encode,
        Decode,
        DecodeWithMemTracking,
        Clone,
        PartialEq,
        Eq,
        RuntimeDebug,
        TypeInfo,
        MaxEncodedLen,
    )]
    #[scale_info(skip_type_params(T))]
    pub struct SubAccountRecord<T: Config> {
        /// The master account that controls this sub-account.
        pub master: T::AccountId,
        /// The type of this sub-account.
        pub account_type: AccountType,
        /// Block number when the sub-account was opened.
        pub opened_at: u32,
    }

    /// On-chain loan request submitted by a citizen.
    #[derive(
        Encode,
        Decode,
        DecodeWithMemTracking,
        Clone,
        PartialEq,
        Eq,
        RuntimeDebug,
        TypeInfo,
        MaxEncodedLen,
    )]
    #[scale_info(skip_type_params(T))]
    pub struct LoanRequest<T: Config> {
        /// The borrower's master account.
        pub borrower: T::AccountId,
        /// Locked collateral amount (in ALTAN planck).
        pub collateral: BalanceOf<T>,
        /// Requested loan principal amount (in ALTAN planck).
        pub amount: BalanceOf<T>,
        /// Block number when the request was submitted.
        pub submitted_at: u32,
        /// Current status of the loan request.
        pub status: LoanStatus,
        /// Outstanding balance remaining to repay.
        pub outstanding_balance: BalanceOf<T>,
    }

    /// A Time Deposit contract: the citizen voluntarily transfers `amount` ALTAN
    /// to the Bank of Siberia treasury for `duration_blocks` and earns 5% APY.
    ///
    /// ## Chancellery Integration
    ///
    /// `document_hash` is the Blake2-256 hash of the signed PDF / legal-text
    /// document stored in the Altan Chancellery (Document Registry pallet).
    /// This ensures the on-chain deposit is inseparably linked to the
    /// off-chain legal contract — no "magic yield" without a signed agreement.
    ///
    /// ## Interest Formula (pro-rated APY)
    ///
    /// ```text
    /// interest = amount × (interest_rate_bps / 10_000) × (duration / BLOCKS_PER_YEAR)
    ///          = amount × Perbill::from_rational(
    ///                interest_rate_bps × duration,
    ///                10_000 × BLOCKS_PER_YEAR
    ///            )
    /// ```
    #[derive(
        Encode,
        Decode,
        DecodeWithMemTracking,
        Clone,
        PartialEq,
        Eq,
        RuntimeDebug,
        TypeInfo,
        MaxEncodedLen,
    )]
    #[scale_info(skip_type_params(T))]
    pub struct TimeDeposit<T: Config> {
        /// Citizen who opened the deposit (must match signer on claim).
        pub depositor: T::AccountId,
        /// Principal amount transferred to the Bank treasury.
        pub amount: BalanceOf<T>,
        /// Committed annual interest rate in basis points (e.g., 500 = 5% APY).
        pub interest_rate_bps: u32,
        /// Block number when the deposit was opened (used for pro-rated interest).
        pub opened_at: u32,
        /// Block at or after which the deposit can be claimed.
        pub maturity_block: u32,
        /// Blake2-256 hash of the signed legal document in the Chancellery.
        pub document_hash: [u8; 32],
        /// Current status of the deposit.
        pub status: DepositStatus,
    }

    /// On-chain escrow contract between two parties.
    ///
    /// ## Fund Flow
    ///
    /// ```text
    /// Buyer (depositor) ──[create_escrow]──► Bank Treasury
    ///                   ──[release_escrow]──► Seller (counterparty)
    ///                   ──[refund_escrow]──► Buyer
    /// ```
    ///
    /// `item_hash` is the Blake2-256 hash of the traded item description
    /// or the P2P contract document, binding the on-chain contract to the
    /// real-world transaction.
    #[derive(
        Encode,
        Decode,
        DecodeWithMemTracking,
        Clone,
        PartialEq,
        Eq,
        RuntimeDebug,
        TypeInfo,
        MaxEncodedLen,
    )]
    #[scale_info(skip_type_params(T))]
    pub struct EscrowContract<T: Config> {
        /// The party depositing funds into escrow (Buyer).
        pub depositor: T::AccountId,
        /// The counterparty who will receive funds upon confirmation (Seller).
        pub counterparty: T::AccountId,
        /// Amount locked in the Bank treasury (in ALTAN planck).
        pub amount: BalanceOf<T>,
        /// Blake2-256 hash of the traded item / P2P contract document.
        pub item_hash: [u8; 32],
        /// Block number when the escrow was created.
        pub created_at: u32,
        /// Current status of the escrow.
        pub status: EscrowStatus,
    }

    // =========================================================================
    // Storage
    // =========================================================================

    /// Master bank accounts indexed by citizen AccountId.
    ///
    /// A citizen must register their master account before opening any
    /// sub-accounts or accessing credit/escrow services.
    #[pallet::storage]
    #[pallet::getter(fn bank_accounts)]
    pub type BankAccounts<T: Config> =
        StorageMap<_, Blake2_128Concat, T::AccountId, BankAccount<T>, OptionQuery>;

    // ── Sub-Account Storage ───────────────────────────────────────────────────

    /// Sub-accounts indexed by (master AccountId, sub AccountId) → AccountType.
    ///
    /// Allows efficient enumeration of all sub-accounts belonging to a master.
    ///
    /// Derivation: `sub_account = Blake2_256(master_bytes ++ type_byte ++ 0u32_le)`
    #[pallet::storage]
    #[pallet::getter(fn sub_accounts)]
    pub type SubAccounts<T: Config> = StorageDoubleMap<
        _,
        Blake2_128Concat,
        T::AccountId, // Primary key: master account
        Blake2_128Concat,
        T::AccountId, // Secondary key: derived sub-account address
        AccountType,
        OptionQuery,
    >;

    /// Reverse lookup: sub-account address → full SubAccountRecord (includes master).
    ///
    /// Enables O(1) ownership verification: given a sub-account address,
    /// instantly verify who the controlling master identity is.
    ///
    /// Also supports `pallet-recovery`: `as_recovered` restores the master
    /// AccountId as the signing origin, so the ownership check succeeds
    /// transparently without any special recovery-aware logic.
    #[pallet::storage]
    #[pallet::getter(fn sub_account_meta)]
    pub type SubAccountMeta<T: Config> = StorageMap<
        _,
        Blake2_128Concat,
        T::AccountId,        // Sub-account address
        SubAccountRecord<T>, // Full record (master + type + opened_at)
        OptionQuery,
    >;

    // ── Credit / Escrow Storage ───────────────────────────────────────────────

    /// Loan requests indexed by sequential ID.
    #[pallet::storage]
    #[pallet::getter(fn loan_requests)]
    pub type LoanRequests<T: Config> =
        StorageMap<_, Blake2_128Concat, u32, LoanRequest<T>, OptionQuery>;

    /// Auto-incrementing counter for loan request IDs.
    #[pallet::storage]
    #[pallet::getter(fn next_loan_id)]
    pub type NextLoanId<T: Config> = StorageValue<_, u32, ValueQuery>;

    /// O(1) index: borrower → current active loan_id.
    ///
    /// Set by `approve_loan` when a loan transitions `Pending → Active`.
    /// Cleared by `pay_credit` when the loan is fully repaid.
    /// Also cleared by `cancel_loan_request`.
    ///
    /// This enables `pay_credit` to find the active loan in O(1)
    /// instead of iterating the entire `LoanRequests` map.
    #[pallet::storage]
    #[pallet::getter(fn active_loan_by_borrower)]
    pub type ActiveLoanByBorrower<T: Config> =
        StorageMap<_, Blake2_128Concat, T::AccountId, u32, OptionQuery>;

    /// Loan collateral: maps (borrower, loan_id) → locked amount.
    ///
    /// Populated by `request_loan` via `LockableCurrency::set_lock`.
    /// Cleared by `remove_loan_collateral` (internal helper) when the loan
    /// is repaid or defaults.
    #[pallet::storage]
    #[pallet::getter(fn loan_collateral)]
    pub type LoanCollateral<T: Config> = StorageDoubleMap<
        _,
        Blake2_128Concat,
        T::AccountId, // borrower
        Blake2_128Concat,
        u32,          // loan_id
        BalanceOf<T>, // locked collateral amount
        OptionQuery,
    >;

    /// Escrow contracts indexed by sequential ID.
    #[pallet::storage]
    #[pallet::getter(fn escrow_contracts)]
    pub type EscrowContracts<T: Config> =
        StorageMap<_, Blake2_128Concat, u32, EscrowContract<T>, OptionQuery>;

    /// Auto-incrementing counter for escrow contract IDs.
    #[pallet::storage]
    #[pallet::getter(fn next_escrow_id)]
    pub type NextEscrowId<T: Config> = StorageValue<_, u32, ValueQuery>;

    // ── Time Deposit Storage ──────────────────────────────────────────────────

    /// Time Deposit contracts indexed by sequential ID.
    ///
    /// Funds are physically held in the Bank treasury (`PalletId` account).
    /// Citizens must present a signed legal document hash (Chancellery) to open.
    #[pallet::storage]
    #[pallet::getter(fn time_deposits)]
    pub type TimeDeposits<T: Config> =
        StorageMap<_, Blake2_128Concat, u32, TimeDeposit<T>, OptionQuery>;

    /// Auto-incrementing counter for Time Deposit IDs.
    #[pallet::storage]
    #[pallet::getter(fn next_time_deposit_id)]
    pub type NextTimeDepositId<T: Config> = StorageValue<_, u32, ValueQuery>;

    // ── Cash Advance Storage ──────────────────────────────────────────────────

    #[pallet::storage]
    #[pallet::getter(fn bank_server_key)]
    pub type BankServerKey<T: Config> = StorageValue<_, T::AccountId, OptionQuery>;

    #[pallet::storage]
    #[pallet::getter(fn used_nonces)]
    pub type UsedNonces<T: Config> = StorageMap<_, Blake2_128Concat, u64, bool, ValueQuery>;

    /// On-chain collateral for Secured Credit Cards and Consumer Loans.
    #[pallet::storage]
    #[pallet::getter(fn collateral_balances)]
    pub type CollateralBalances<T: Config> = StorageMap<_, Blake2_128Concat, T::AccountId, BalanceOf<T>, ValueQuery>;

    // =========================================================================
    // Events
    // =========================================================================

    #[pallet::event]
    #[pallet::generate_deposit(pub(super) fn deposit_event)]
    pub enum Event<T: Config> {
        // ── Master Account Events ─────────────────────────────────────────────
        /// A citizen successfully opened their master bank account.
        MasterAccountOpened { citizen: T::AccountId },

        // ── Sub-Account Events ────────────────────────────────────────────────
        /// A deterministic keyless sub-account was created for a citizen.
        ///
        /// The `sub_account` address was derived via:
        /// `Blake2_256(master_bytes ++ account_type_byte ++ 0u32_le)`
        /// No private key exists for this address — the master key controls it.
        SubAccountOpened {
            /// The master account controlling this sub-account.
            master: T::AccountId,
            /// The derived keyless sub-account address.
            sub_account: T::AccountId,
            /// The type of sub-account opened.
            account_type: AccountType,
        },

        /// A sub-account was closed by its master.
        SubAccountClosed {
            master: T::AccountId,
            sub_account: T::AccountId,
        },

        /// A savings withdrawal intent was recorded for a sub-account.
        ///
        /// Full fund transfer is wired in the next sprint (requires balance checks +
        /// existential deposit handling for pure-proxy accounts).
        SavingsWithdrawalRequested {
            master: T::AccountId,
            sub_account: T::AccountId,
            amount: BalanceOf<T>,
        },

        /// Savings funds were physically transferred from sub-account to master.
        SavingsWithdrawn {
            master: T::AccountId,
            sub_account: T::AccountId,
            amount: BalanceOf<T>,
        },

        /// A credit repayment intent was recorded by the master of a Credit sub-account.
        CreditPaymentRecorded {
            master: T::AccountId,
            sub_account: T::AccountId,
            amount: BalanceOf<T>,
        },

        /// A credit repayment was applied to the outstanding loan balance.
        ///
        /// Emitted alongside `CreditPaymentRecorded`.
        CreditRepaymentApplied {
            loan_id: u32,
            borrower: T::AccountId,
            payment: BalanceOf<T>,
            remaining: BalanceOf<T>,
        },

        /// Loan collateral was locked via `LockableCurrency::set_lock`.
        LoanCollateralLocked {
            loan_id: u32,
            borrower: T::AccountId,
            collateral: BalanceOf<T>,
        },

        // ── Loan / Escrow Events ──────────────────────────────────────────────
        /// A citizen submitted a loan request.
        LoanRequested {
            loan_id: u32,
            borrower: T::AccountId,
            amount: BalanceOf<T>,
        },

        /// A loan request was approved by BankingOrigin; funds disbursed to borrower.
        LoanApproved {
            loan_id: u32,
            borrower: T::AccountId,
            amount: BalanceOf<T>,
        },

        /// A pending loan request was cancelled by the borrower; collateral released.
        LoanCancelled {
            loan_id: u32,
            borrower: T::AccountId,
        },

        /// The BOS treasury was funded by BankingOrigin (for interest reserves).
        TreasuryFunded {
            funder: T::AccountId,
            amount: BalanceOf<T>,
        },

        /// An escrow contract was created; funds are locked in the Bank treasury.
        EscrowCreated {
            escrow_id: u32,
            depositor: T::AccountId,
            counterparty: T::AccountId,
            amount: BalanceOf<T>,
            item_hash: [u8; 32],
        },

        /// Buyer confirmed receipt; funds released from treasury to seller.
        EscrowReleased {
            escrow_id: u32,
            depositor: T::AccountId,
            counterparty: T::AccountId,
            amount: BalanceOf<T>,
        },

        /// Seller initiated refund; funds returned from treasury to buyer.
        EscrowRefunded {
            escrow_id: u32,
            depositor: T::AccountId,
            counterparty: T::AccountId,
            amount: BalanceOf<T>,
        },

        // ── Time Deposit Events ───────────────────────────────────────────────
        /// A citizen opened a Time Deposit, transferring funds to the Bank treasury.
        ///
        /// The `document_hash` links this deposit to the signed legal contract
        /// stored in the Altan Chancellery (Document Registry).
        TimeDepositOpened {
            /// Unique deposit ID.
            id: u32,
            /// The citizen who created the deposit.
            depositor: T::AccountId,
            /// Principal amount transferred to the Bank treasury.
            amount: BalanceOf<T>,
            /// Block at which the deposit matures and can be claimed.
            maturity_block: u32,
            /// Blake2-256 hash of the Chancellery document.
            document_hash: [u8; 32],
        },

        /// A matured Time Deposit was claimed: principal + interest returned to depositor.
        TimeDepositClaimed {
            /// Unique deposit ID.
            id: u32,
            /// The citizen who received the funds.
            depositor: T::AccountId,
            /// Original principal amount.
            amount: BalanceOf<T>,
            /// Pro-rated interest earned (calculated at claim time).
            interest: BalanceOf<T>,
        },

        // ── Cash Advance Events ───────────────────────────────────────────────
        /// A cash advance was claimed via cryptographic signature.
        CashAdvanceClaimed { account: T::AccountId, amount: BalanceOf<T>, nonce: u64 },

        /// A cash advance was repaid to the Bank Treasury.
        CashAdvanceRepaid { account: T::AccountId, amount: BalanceOf<T> },

        /// Collateral was locked in the Bank Treasury for a Secured Card / Loan.
        CollateralLocked { account: T::AccountId, amount: BalanceOf<T> },

        /// Collateral was unlocked and returned to the citizen by the Bank Server.
        CollateralUnlocked { account: T::AccountId, amount: BalanceOf<T> },

        /// Bank server key was updated.
        BankServerKeyUpdated { key: T::AccountId },

        // ── Wholesale Credit Events ──────────────────────────────────────────
        /// Wholesale credit borrowed from Central Bank
        WholesaleCreditBorrowed {
            amount: BalanceOf<T>,
        },
        /// Wholesale credit repaid to Central Bank
        WholesaleCreditRepaid {
            epoch_id: u32,
            amount: BalanceOf<T>,
        },
    }

    // =========================================================================
    // Errors
    // =========================================================================

    #[pallet::error]
    pub enum Error<T> {
        // ── Master Account Errors ─────────────────────────────────────────────
        /// The caller already has an open master bank account.
        AccountAlreadyExists,
        /// The caller does not have a registered master bank account.
        /// Open a master account via `open_master_account` first.
        MasterAccountRequired,

        // ── Sub-Account Errors ────────────────────────────────────────────────
        /// This master key already has a sub-account of the requested type.
        /// Only one sub-account per type (Savings/Credit/Company) is allowed per master.
        DuplicateAccountType,
        /// The provided sub-account address does not exist in storage.
        SubAccountNotFound,
        /// The caller is not the master controlling this sub-account.
        /// Access denied — only the master key (or an `as_recovered` origin) may act.
        NotSubAccountOwner,

        // ── Credit / Escrow Errors ────────────────────────────────────────────
        /// The requested loan amount must be greater than zero.
        ZeroLoanAmount,
        /// The escrow amount must be greater than zero.
        ZeroEscrowAmount,
        /// The counterparty for the escrow cannot be the same as the depositor.
        SelfEscrow,
        /// The withdrawal or payment amount must be greater than zero.
        ZeroAmount,
        /// No escrow contract exists with the provided ID.
        EscrowNotFound,
        /// The escrow is not in `Locked` state (already released or refunded).
        EscrowNotLocked,
        /// Only the depositor (buyer) may call `release_escrow`.
        NotDepositor,
        /// Only the counterparty (seller) may call `refund_escrow`.
        NotCounterparty,
        /// No loan request exists with the provided ID.
        LoanNotFound,
        /// The loan must be in `Pending` status for this operation.
        /// `approve_loan` and `cancel_loan_request` require `Pending`.
        LoanNotPending,
        /// The borrower already has an active loan.
        /// Repay the existing loan before requesting a new one.
        BorrowerHasActiveLoan,

        // ── Time Deposit Errors ───────────────────────────────────────────────
        /// The deposit amount must be greater than zero.
        ZeroDepositAmount,
        /// The deposit duration must be at least one block.
        ZeroDepositDuration,
        /// The requested Time Deposit does not exist.
        DepositNotFound,
        /// Only the original depositor may claim this deposit.
        DepositorMismatch,
        /// The deposit has not yet reached its maturity block.
        DepositNotMatured,
        /// The time deposit is still active and locked.
        TimeDepositNotMatured,
        /// This deposit has already been claimed.
        DepositAlreadyClaimed,
        /// The BOS treasury does not have sufficient funds to pay principal + interest.
        ///
        /// BankingOrigin must call `fund_treasury` to top up the interest reserve
        /// before deposits can be claimed.
        TreasuryInsufficientFunds,

        /// The sub-account still holds a non-zero ALTAN balance.
        ///
        /// Call `withdraw_from_savings` (or equivalent) to drain the sub-account
        /// before closing it.  This prevents silently stranding funds in an
        /// address that has no private key.
        SubAccountNotEmpty,

        /// The citizen's migration application is not approved.
        CitizenNotApproved,

        /// Arithmetic overflow occurred.
        ArithmeticOverflow,

        // ── Cash Advance Errors ───────────────────────────────────────────────
        NonceAlreadyUsed,
        InvalidSignature,
    }

    // =========================================================================
    // Internal helpers
    // =========================================================================

    impl<T: Config> Pallet<T> {
        /// Derive a deterministic keyless sub-account address from a master key.
        ///
        /// ## Derivation
        ///
        /// ```text
        /// payload = master_bytes ++ [account_type.derivation_index()] ++ 0u32_le
        /// sub_account_id = Blake2_256(payload)
        /// ```
        ///
        /// The nonce (`0u32`) is reserved for future multi-account-per-type support.
        /// In v3, only one sub-account per type is allowed, so nonce is always 0.
        ///
        /// ## Properties
        ///
        /// - **Deterministic**: same inputs always produce the same address.
        /// - **Keyless**: the 32-byte hash has no corresponding private key
        ///   in any standard derivation scheme (ed25519/sr25519/ecdsa).
        /// - **Collision-resistant**: Blake2_256 with distinct type bytes and
        ///   different master keys produces distinct addresses.
        /// - **Recovery-transparent**: `pallet-recovery::as_recovered` restores
        ///   the master AccountId as the signing origin; ownership checks pass
        ///   without any special recovery-awareness in this pallet.
        pub fn derive_sub_account(
            master: &T::AccountId,
            account_type: AccountType,
            nonce: u32,
        ) -> T::AccountId {
            let mut payload: Vec<u8> = master.encode();
            payload.push(account_type.derivation_index());
            payload.extend_from_slice(&nonce.to_le_bytes());
            let hash = blake2_256(&payload);
            // Blake2_256 output is 32 bytes; AccountId is also 32 bytes in all
            // standard Substrate runtimes → decode is always infallible.
            T::AccountId::decode(&mut &hash[..])
                .expect("32-byte blake2_256 hash always decodes to AccountId")
        }

        /// Verify that `master` owns the given `sub_account`.
        ///
        /// Returns `Ok(SubAccountRecord)` if ownership is confirmed,
        /// or an appropriate `DispatchError` otherwise.
        ///
        /// Compatible with `pallet-recovery::as_recovered` because that call
        /// restores the original master AccountId as the signed origin.
        pub fn ensure_sub_account_owner(
            master: &T::AccountId,
            sub_account: &T::AccountId,
        ) -> Result<SubAccountRecord<T>, DispatchError> {
            let record =
                SubAccountMeta::<T>::get(sub_account).ok_or(Error::<T>::SubAccountNotFound)?;
            ensure!(&record.master == master, Error::<T>::NotSubAccountOwner);
            Ok(record)
        }

        /// Return the current block number as `u32` (standard in all Substrate runtimes).
        fn current_block() -> u32 {
            frame_system::Pallet::<T>::block_number()
                .try_into()
                .unwrap_or(0u32)
        }

        /// Return the sovereign treasury account of the Bank of Siberia.
        ///
        /// This account is derived deterministically from `T::PalletId` and has
        /// no private key.  It is controlled exclusively by pallet logic.
        ///
        /// Time Deposit principals are transferred INTO this account on open,
        /// and transferred OUT (principal + interest) on a successful claim.
        pub fn treasury_account() -> T::AccountId {
            T::PalletId::get().into_account_truncating()
        }

        /// Calculate pro-rated interest for a Time Deposit.
        ///
        /// ```text
        /// interest = amount × Perbill::from_rational(
        ///     interest_rate_bps × duration_blocks,
        ///     10_000 × BLOCKS_PER_YEAR,
        /// )
        /// ```
        ///
        /// Uses `Perbill` (parts-per-billion fixed-point) to avoid overflow on
        /// large balances and remain `no_std` compatible.
        pub fn calculate_interest(
            amount: BalanceOf<T>,
            interest_rate_bps: u32,
            opened_at: u32,
            maturity_block: u32,
        ) -> BalanceOf<T> {
            let duration = maturity_block.saturating_sub(opened_at) as u64;
            let numerator = (interest_rate_bps as u64).saturating_mul(duration);
            let denominator = (10_000_u64).saturating_mul(BLOCKS_PER_YEAR as u64);
            if denominator == 0 {
                return BalanceOf::<T>::default();
            }
            let fraction = Perbill::from_rational(numerator, denominator);
            fraction * amount
        }
    }

    // =========================================================================
    // Extrinsics
    // =========================================================================

    #[pallet::call]
    impl<T: Config> Pallet<T> {
        // ─── open_master_account ──────────────────────────────────────────────

        /// Register a citizen's master bank account with the Bank of Siberia.
        ///
        /// This is the **mandatory first step** before a citizen can open any
        /// sub-accounts or access credit/escrow services.
        ///
        /// ## Access Control
        ///
        /// Signed by the citizen's Master Key (Altan Vault). No bank authority
        /// required for registration.
        ///
        /// ## Future Sprint
        ///
        /// KYC oracle integration, citizenship verification via
        /// `pallet-inomad-identity`, and AML checks will be added.
        #[pallet::call_index(0)]
        #[pallet::weight(<T as Config>::WeightInfo::open_master_account())]
        pub fn open_master_account(origin: OriginFor<T>) -> DispatchResult {
            let citizen = ensure_signed(origin)?;

            // 1. Ensure the citizen is approved by the Migration Center
            ensure!(
                pallet_migration_center::Pallet::<T>::is_application_approved(&citizen),
                Error::<T>::CitizenNotApproved
            );

            // 2. Ensure they don't already have an account
            ensure!(
                !BankAccounts::<T>::contains_key(&citizen),
                Error::<T>::AccountAlreadyExists
            );

            // 3. Register the Master Account
            //
            // NOTE (Sprint 9): Genesis Grant (100 ALTAN) is NO LONGER issued here.
            // The previous `Currency::transfer(&pension_fund, ...)` was architecturally
            // incorrect — the BOS pallet cannot unilaterally move funds from a Pure Proxy
            // account controlled by the Central Bank.
            //
            // The Genesis Grant is now issued via `pallet_central_bank::issue_genesis_grant`
            // which must be called by BankingOrigin after `open_master_account` succeeds.
            // L1 is the source of truth — the grant is a separate on-chain transaction.
            BankAccounts::<T>::insert(
                &citizen,
                BankAccount::<T> {
                    owner: citizen.clone(),
                    opened_at: Self::current_block(),
                    status: AccountStatus::Active,
                },
            );

            Self::deposit_event(Event::MasterAccountOpened { citizen });
            Ok(())
        }

        // ─── open_sub_account ─────────────────────────────────────────────────

        /// Open a deterministic keyless sub-account under the caller's Master Key.
        ///
        /// The sub-account address is derived as:
        /// `Blake2_256(master_bytes ++ account_type_byte ++ 0u32_le)`
        ///
        /// No new seed phrase is generated. The sub-account is controlled
        /// exclusively by the Master Key. If the Master Key is recovered via
        /// `pallet-recovery::as_recovered`, the recovered identity retains full
        /// ownership of all sub-accounts automatically.
        ///
        /// ## Constraints
        ///
        /// - Caller must have a registered master bank account.
        /// - Only one sub-account per `AccountType` (Savings/Credit/Company).
        ///
        /// ## Access Control
        ///
        /// `ensure_signed` → Master Key (or `as_recovered` restored identity).
        #[pallet::call_index(1)]
        #[pallet::weight(<T as Config>::WeightInfo::open_sub_account())]
        pub fn open_sub_account(origin: OriginFor<T>, account_type: AccountType) -> DispatchResult {
            let master = ensure_signed(origin)?;

            // Master must have an active bank account first.
            ensure!(
                BankAccounts::<T>::contains_key(&master),
                Error::<T>::MasterAccountRequired
            );

            // Derive the deterministic sub-account address.
            // Nonce = 0 (v3 allows one sub-account per type per master).
            let sub_account = Self::derive_sub_account(&master, account_type, 0);

            // Prevent duplicate sub-accounts of the same type.
            ensure!(
                !SubAccounts::<T>::contains_key(&master, &sub_account),
                Error::<T>::DuplicateAccountType
            );

            let opened_at = Self::current_block();

            // Register in both storage maps:
            // 1. SubAccounts: master → sub → type  (forward listing)
            SubAccounts::<T>::insert(&master, &sub_account, account_type);
            // 2. SubAccountMeta: sub → record  (reverse O(1) ownership check)
            SubAccountMeta::<T>::insert(
                &sub_account,
                SubAccountRecord::<T> {
                    master: master.clone(),
                    account_type,
                    opened_at,
                },
            );

            Self::deposit_event(Event::SubAccountOpened {
                master,
                sub_account,
                account_type,
            });
            Ok(())
        }

        // ─── withdraw_from_savings ────────────────────────────────────────────

        /// Request a withdrawal from a Savings sub-account back to the Master Key.
        ///
        /// ## Access Control
        ///
        /// `ensure_signed(origin)` must match the master controlling `sub_account`.
        ///
        /// Compatible with `pallet-recovery::as_recovered` — the recovered
        /// master identity passes the ownership check transparently.
        ///
        /// ## Fund Flow (Sprint 8 — wired)
        ///
        /// Transfers `amount` from the derived keyless Savings sub-account back
        /// to the master key. Uses `ExistenceRequirement::KeepAlive` to prevent
        /// the sub-account from being reaped (it may be re-used in the future).
        ///
        /// If the sub-account balance is insufficient, the transfer returns
        /// `pallet_balances::Error::InsufficientBalance` automatically.
        #[pallet::call_index(2)]
        #[pallet::weight(<T as Config>::WeightInfo::withdraw_from_savings())]
        pub fn withdraw_from_savings(
            origin: OriginFor<T>,
            sub_account: T::AccountId,
            amount: BalanceOf<T>,
        ) -> DispatchResult {
            let master = ensure_signed(origin)?;

            // Verify ownership: master must control this sub-account.
            // Works with pallet-recovery `as_recovered` transparently.
            let record = Self::ensure_sub_account_owner(&master, &sub_account)?;
            ensure!(
                record.account_type == AccountType::Savings,
                Error::<T>::NotSubAccountOwner
            );

            ensure!(amount > BalanceOf::<T>::default(), Error::<T>::ZeroAmount);

            // Sprint 9 FIX: detect full-drain BEFORE transfer.
            //
            // If the citizen is withdrawing all their funds (amount >= free_balance),
            // the keyless sub-account will be reaped by the runtime after the transfer.
            // We pre-emptively remove pallet storage records to avoid orphaned entries
            // (storage records pointing to a reaped account with no on-chain balance).
            //
            // L1 is the source of truth — the storage records must match the on-chain state.
            let sub_balance = <T as Config>::Currency::free_balance(&sub_account);
            if amount >= sub_balance {
                SubAccounts::<T>::remove(&master, &sub_account);
                SubAccountMeta::<T>::remove(&sub_account);
            }

            // AllowDeath: keyless sub-accounts have no private key — reaping is safe
            // since the address is deterministically re-derivable from the master key.
            <T as Config>::Currency::transfer(
                &sub_account,
                &master,
                amount,
                ExistenceRequirement::AllowDeath,
            )?;

            // Emit both events for auditability.
            Self::deposit_event(Event::SavingsWithdrawalRequested {
                master: master.clone(),
                sub_account: sub_account.clone(),
                amount,
            });
            Self::deposit_event(Event::SavingsWithdrawn {
                master,
                sub_account,
                amount,
            });
            Ok(())
        }

        // ─── pay_credit ───────────────────────────────────────────────────────

        /// Record a credit repayment from the Master Key toward a Credit sub-account.
        ///
        /// ## Access Control
        ///
        /// `ensure_signed(origin)` must be the master controlling the Credit sub-account.
        ///
        /// ## Fund Flow (Sprint 8 — wired)
        ///
        /// 1. Transfers `amount` from master → Bank treasury (Credit sub-account
        ///    acts as the routing label; funds go to the sovereign treasury).
        /// 2. Finds the borrower's most recent Active loan request and reduces
        ///    `outstanding_balance` by the payment amount.
        /// 3. If `outstanding_balance` reaches zero, transitions the loan to
        ///    `LoanStatus::Repaid` and removes the collateral lock.
        #[pallet::call_index(3)]
        #[pallet::weight(<T as Config>::WeightInfo::pay_credit())]
        pub fn pay_credit(
            origin: OriginFor<T>,
            sub_account: T::AccountId,
            amount: BalanceOf<T>,
        ) -> DispatchResult {
            let master = ensure_signed(origin)?;

            let record = Self::ensure_sub_account_owner(&master, &sub_account)?;
            ensure!(
                record.account_type == AccountType::Credit,
                Error::<T>::NotSubAccountOwner
            );

            ensure!(amount > BalanceOf::<T>::default(), Error::<T>::ZeroAmount);

            // Sprint 9 FIX: O(1) lookup via ActiveLoanByBorrower index.
            //
            // Previously used O(N) LoanRequests::iter().find(...) which scans all loans.
            // Now uses a dedicated StorageMap for O(1) access.
            //
            // L1 principle: reject payment if no active loan exists on-chain.
            // Don't silently accept funds into treasury without a matching debt record.
            let loan_id = ActiveLoanByBorrower::<T>::get(&master)
                .ok_or(Error::<T>::LoanNotFound)?;

            let mut loan = LoanRequests::<T>::get(loan_id)
                .ok_or(Error::<T>::LoanNotFound)?;

            // Verify the indexed loan is still Active (defensive check).
            ensure!(
                matches!(loan.status, LoanStatus::Active),
                Error::<T>::LoanNotFound
            );

            // Transfer payment: master → Bank treasury.
            let treasury = Self::treasury_account();
            <T as Config>::Currency::transfer(
                &master, &treasury, amount, ExistenceRequirement::KeepAlive
            )?;

            // Reduce outstanding balance (floor at zero — no over-payment tracking in v3).
            let new_outstanding = loan.outstanding_balance.saturating_sub(amount);
            loan.outstanding_balance = new_outstanding;

            let fully_repaid = new_outstanding == BalanceOf::<T>::default();
            if fully_repaid {
                loan.status = LoanStatus::Repaid;
                // Release collateral lock.
                <T as Config>::Currency::remove_lock(LOAN_LOCK_ID, &master);
                LoanCollateral::<T>::remove(&master, loan_id);
                // Clear the O(1) index — borrower no longer has an active loan.
                ActiveLoanByBorrower::<T>::remove(&master);
            }

            LoanRequests::<T>::insert(loan_id, &loan);

            Self::deposit_event(Event::CreditRepaymentApplied {
                loan_id,
                borrower: master.clone(),
                payment: amount,
                remaining: new_outstanding,
            });
            Self::deposit_event(Event::CreditPaymentRecorded {
                master,
                sub_account,
                amount,
            });
            Ok(())
        }

        // ─── close_sub_account ────────────────────────────────────────────────

        /// Close a sub-account controlled by the caller's Master Key.
        ///
        /// Removes the sub-account from both `SubAccounts` and `SubAccountMeta`.
        /// Closes a sub-account after verifying its ALTAN balance is zero.
        ///
        /// ## Sprint 8 — Balance Guard
        ///
        /// The on-chain balance of the sub-account must be zero (or below the
        /// existential deposit, i.e., effectively zero from the pallet's
        /// perspective) before closure is permitted.  This prevents silently
        /// stranding funds in an unreachable address.
        ///
        /// Citizens must call `withdraw_from_savings` first to drain a Savings
        /// sub-account before they can close it.
        ///
        /// ## Access Control
        ///
        /// Only the master controlling the sub-account may close it.
        #[pallet::call_index(4)]
        #[pallet::weight(<T as Config>::WeightInfo::close_sub_account())]
        pub fn close_sub_account(
            origin: OriginFor<T>,
            sub_account: T::AccountId,
        ) -> DispatchResult {
            let master = ensure_signed(origin)?;

            // Verify ownership.
            Self::ensure_sub_account_owner(&master, &sub_account)?;

            // Sprint 8: enforce zero-balance before closure.
            // We compare against zero (not the existential deposit) because
            // keyless sub-accounts are not subject to automatic reaping —
            // their existence is maintained exclusively by this pallet's storage.
            ensure!(
                <T as Config>::Currency::free_balance(&sub_account) == BalanceOf::<T>::default(),
                Error::<T>::SubAccountNotEmpty
            );

            // Remove from both storage maps.
            SubAccounts::<T>::remove(&master, &sub_account);
            SubAccountMeta::<T>::remove(&sub_account);

            Self::deposit_event(Event::SubAccountClosed {
                master,
                sub_account,
            });
            Ok(())
        }

        // ─── request_loan ─────────────────────────────────────────────────────

        /// Citizen submits a loan request to the Bank of Siberia.
        ///
        /// ## Access Control
        ///
        /// Only signed by a citizen with a registered master bank account.
        ///
        /// ## Future Sprint
        ///
        /// - Collateral locking via `LockableCurrency::set_lock`.
        /// - Bank officer approval flow (dual-consent with Central Bank).
        /// - Integration with `pallet-bank-operator` for CDP mechanics.
        /// - 0% APR enforcement for loyal citizens (constitutional mandate).
        ///
        /// ## Collateral Locking (Sprint 8 — wired)
        ///
        /// Locks `amount` ALTAN in the borrower's master account via
        /// `LockableCurrency::set_lock`.  Locked funds cannot be transferred
        /// but are still counted toward the borrower's free balance for
        /// existential deposit purposes.
        ///
        /// The lock is identified by `LOAN_LOCK_ID` (*b"bos/loan") and released
        /// automatically when `pay_credit` detects full repayment.
        ///
        /// **Important**: In v3, a citizen can only have ONE active collateral lock.
        /// Multiple concurrent loans sharing the same `LOAN_LOCK_ID` are serialised
        /// (the lock covers the MAXIMUM of any outstanding collateral amount).
        /// Multi-loan support with distinct lock identifiers will be added in Phase 8.
        #[pallet::call_index(5)]
        #[pallet::weight(<T as Config>::WeightInfo::request_loan())]
        pub fn request_loan(origin: OriginFor<T>, amount: BalanceOf<T>, collateral: BalanceOf<T>) -> DispatchResult {
            let borrower = ensure_signed(origin)?;

            ensure!(
                BankAccounts::<T>::contains_key(&borrower),
                Error::<T>::MasterAccountRequired
            );

            ensure!(
                amount > BalanceOf::<T>::default(),
                Error::<T>::ZeroLoanAmount
            );

            let loan_id = NextLoanId::<T>::get();
            NextLoanId::<T>::put(loan_id.saturating_add(1));

            // Lock collateral via LockableCurrency::set_lock.
            <T as Config>::Currency::set_lock(
                LOAN_LOCK_ID,
                &borrower,
                collateral,
                frame_support::traits::WithdrawReasons::all(),
            );

            // Record collateral in dedicated storage for O(1) lookup on repayment.
            LoanCollateral::<T>::insert(&borrower, loan_id, collateral);

            LoanRequests::<T>::insert(
                loan_id,
                LoanRequest::<T> {
                    borrower: borrower.clone(),
                    collateral,
                    amount,
                    submitted_at: Self::current_block(),
                    status: LoanStatus::Pending,
                    outstanding_balance: amount, // starts at full loan amount
                },
            );

            Self::deposit_event(Event::LoanCollateralLocked {
                loan_id,
                borrower: borrower.clone(),
                collateral,
            });
            Self::deposit_event(Event::LoanRequested {
                loan_id,
                borrower,
                amount,
            });
            Ok(())
        }

        // ─── create_escrow ────────────────────────────────────────────────────

        /// Buyer creates a P2P escrow contract with a seller.
        ///
        /// Physically transfers `amount` ALTAN from the buyer's account into
        /// the Bank of Siberia treasury (a keyless PalletId account). Funds
        /// remain there until either:
        /// - `release_escrow` is called by the buyer (goods received → seller paid), or
        /// - `refund_escrow` is called by the seller (goods unavailable → buyer refunded).
        ///
        /// `item_hash` is the Blake2-256 hash of the item description or the
        /// signed P2P contract document — it binds this on-chain contract to
        /// the real-world transaction irreversibly.
        ///
        /// ## Fund Flow
        ///
        /// ```text
        /// Buyer wallet ──[Currency::transfer]──► Bank treasury (PalletId)
        /// ```
        ///
        /// ## Access Control
        ///
        /// Signed by the buyer's Master Key. Buyer must have a registered
        /// master bank account.
        #[pallet::call_index(6)]
        #[pallet::weight(<T as Config>::WeightInfo::create_escrow())]
        pub fn create_escrow(
            origin: OriginFor<T>,
            counterparty: T::AccountId,
            amount: BalanceOf<T>,
            item_hash: [u8; 32],
        ) -> DispatchResult {
            let depositor = ensure_signed(origin)?;

            ensure!(
                BankAccounts::<T>::contains_key(&depositor),
                Error::<T>::MasterAccountRequired
            );

            ensure!(
                amount > BalanceOf::<T>::default(),
                Error::<T>::ZeroEscrowAmount
            );

            ensure!(depositor != counterparty, Error::<T>::SelfEscrow);

            let treasury = Self::treasury_account();

            // Physically transfer funds from buyer → Bank treasury.
            <T as Config>::Currency::transfer(
                &depositor,
                &treasury,
                amount,
                ExistenceRequirement::AllowDeath,
            )?;

            let escrow_id = NextEscrowId::<T>::get();
            NextEscrowId::<T>::put(escrow_id.saturating_add(1));

            EscrowContracts::<T>::insert(
                escrow_id,
                EscrowContract::<T> {
                    depositor: depositor.clone(),
                    counterparty: counterparty.clone(),
                    amount,
                    item_hash,
                    created_at: Self::current_block(),
                    status: EscrowStatus::Locked,
                },
            );

            Self::deposit_event(Event::EscrowCreated {
                escrow_id,
                depositor,
                counterparty,
                amount,
                item_hash,
            });
            Ok(())
        }

        // ─── release_escrow ───────────────────────────────────────────────────

        /// Buyer confirms successful receipt of goods/services.
        ///
        /// Transfers `amount` from the Bank treasury to the seller (counterparty).
        /// Only the buyer (depositor) may call this — they are the party
        /// confirming that the deal was fulfilled correctly.
        ///
        /// ## Fund Flow
        ///
        /// ```text
        /// Bank treasury ──[Currency::transfer]──► Seller (counterparty)
        /// ```
        ///
        /// ## Access Control
        ///
        /// Only `depositor` (buyer). The seller cannot self-release.
        #[pallet::call_index(9)]
        #[pallet::weight(<T as Config>::WeightInfo::release_escrow())]
        pub fn release_escrow(origin: OriginFor<T>, escrow_id: u32) -> DispatchResult {
            let caller = ensure_signed(origin)?;

            let mut contract =
                EscrowContracts::<T>::get(escrow_id).ok_or(Error::<T>::EscrowNotFound)?;

            // Only the buyer (depositor) may confirm and release.
            ensure!(contract.depositor == caller, Error::<T>::NotDepositor);

            // Escrow must still be in Locked state.
            ensure!(
                matches!(contract.status, EscrowStatus::Locked),
                Error::<T>::EscrowNotLocked
            );

            let treasury = Self::treasury_account();

            // Transfer from treasury → seller.
            <T as Config>::Currency::transfer(
                &treasury,
                &contract.counterparty,
                contract.amount,
                ExistenceRequirement::AllowDeath,
            )?;

            contract.status = EscrowStatus::Released;
            EscrowContracts::<T>::insert(escrow_id, &contract);

            Self::deposit_event(Event::EscrowReleased {
                escrow_id,
                depositor: contract.depositor,
                counterparty: contract.counterparty,
                amount: contract.amount,
            });
            Ok(())
        }

        // ─── refund_escrow ────────────────────────────────────────────────────

        /// Seller initiates a refund (goods unavailable or deal cancelled).
        ///
        /// Transfers `amount` from the Bank treasury back to the buyer (depositor).
        /// Only the seller (counterparty) may call this — they are the party
        /// acknowledging that the deal cannot be fulfilled.
        ///
        /// ## Fund Flow
        ///
        /// ```text
        /// Bank treasury ──[Currency::transfer]──► Buyer (depositor)
        /// ```
        ///
        /// ## Access Control
        ///
        /// Only `counterparty` (seller). The buyer cannot self-refund.
        #[pallet::call_index(10)]
        #[pallet::weight(<T as Config>::WeightInfo::refund_escrow())]
        pub fn refund_escrow(origin: OriginFor<T>, escrow_id: u32) -> DispatchResult {
            let caller = ensure_signed(origin)?;

            let mut contract =
                EscrowContracts::<T>::get(escrow_id).ok_or(Error::<T>::EscrowNotFound)?;

            // Only the seller (counterparty) may initiate a refund.
            ensure!(contract.counterparty == caller, Error::<T>::NotCounterparty);

            // Escrow must still be in Locked state.
            ensure!(
                matches!(contract.status, EscrowStatus::Locked),
                Error::<T>::EscrowNotLocked
            );

            let treasury = Self::treasury_account();

            // Transfer from treasury → buyer.
            <T as Config>::Currency::transfer(
                &treasury,
                &contract.depositor,
                contract.amount,
                ExistenceRequirement::AllowDeath,
            )?;

            contract.status = EscrowStatus::Refunded;
            EscrowContracts::<T>::insert(escrow_id, &contract);

            Self::deposit_event(Event::EscrowRefunded {
                escrow_id,
                depositor: contract.depositor,
                counterparty: contract.counterparty,
                amount: contract.amount,
            });
            Ok(())
        }

        // ─── open_time_deposit ────────────────────────────────────────────────

        /// Open a Time Deposit: transfer `amount` ALTAN to the Bank of Siberia
        /// treasury and earn 5% APY on maturity.
        ///
        /// ## Chancellery Requirement
        ///
        /// `document_hash` must be the Blake2-256 hash of a signed legal PDF
        /// stored in the Altan Chancellery (Document Registry).  The Bank does
        /// NOT generate "magic yield" — the legal document binds the agreement
        /// on-chain and constitutes the depositor's operational mandate.
        ///
        /// ## Fund Flow
        ///
        /// ```text
        /// Citizen wallet ──[Currency::transfer]──► Bank of Siberia treasury
        ///                                          (PalletId account, no private key)
        /// ```
        ///
        /// On `claim_time_deposit`, the treasury sends `amount + interest` back.
        ///
        /// ## Access Control
        ///
        /// Signed by the citizen's Master Key.  Citizen must have a registered
        /// master bank account.
        #[pallet::call_index(7)]
        #[pallet::weight(<T as Config>::WeightInfo::open_time_deposit())]
        pub fn open_time_deposit(
            origin: OriginFor<T>,
            amount: BalanceOf<T>,
            duration_blocks: u32,
            document_hash: [u8; 32],
        ) -> DispatchResult {
            let depositor = ensure_signed(origin)?;

            ensure!(
                BankAccounts::<T>::contains_key(&depositor),
                Error::<T>::MasterAccountRequired
            );
            ensure!(
                amount > BalanceOf::<T>::default(),
                Error::<T>::ZeroDepositAmount
            );
            ensure!(duration_blocks > 0, Error::<T>::ZeroDepositDuration);

            let opened_at = Self::current_block();
            let maturity_block = opened_at.saturating_add(duration_blocks);
            let treasury = Self::treasury_account();

            // Physically transfer funds from citizen → Bank treasury.
            <T as Config>::Currency::transfer(
                &depositor,
                &treasury,
                amount,
                ExistenceRequirement::AllowDeath,
            )?;

            let deposit_id = NextTimeDepositId::<T>::get();
            NextTimeDepositId::<T>::put(deposit_id.saturating_add(1));

            TimeDeposits::<T>::insert(
                deposit_id,
                TimeDeposit::<T> {
                    depositor: depositor.clone(),
                    amount,
                    interest_rate_bps: ANNUAL_RATE_BPS,
                    opened_at,
                    maturity_block,
                    document_hash,
                    status: DepositStatus::Active,
                },
            );

            Self::deposit_event(Event::TimeDepositOpened {
                id: deposit_id,
                depositor,
                amount,
                maturity_block,
                document_hash,
            });
            Ok(())
        }

        // ─── claim_time_deposit ───────────────────────────────────────────────

        /// Claim a matured Time Deposit: receive principal + pro-rated 5% APY
        /// interest from the Bank of Siberia treasury.
        ///
        /// ## Conditions
        ///
        /// - Caller must be the original depositor.
        /// - `current_block >= maturity_block`.
        /// - Deposit status must be `Active` (not already claimed).
        ///
        /// ## Fund Flow
        ///
        /// ```text
        /// Bank of Siberia treasury ──[Currency::transfer]──► Citizen wallet
        ///                             principal + interest
        /// ```
        ///
        /// ## Interest Formula
        ///
        /// `interest = amount × Perbill::from_rational(rate_bps × duration, 10_000 × BLOCKS_PER_YEAR)`
        ///
        /// ## Access Control
        ///
        /// Only the original depositor may claim.  Compatible with
        /// `pallet-recovery::as_recovered` — the recovered master AccountId
        /// passes the depositor check transparently.
        #[pallet::call_index(8)]
        #[pallet::weight(<T as Config>::WeightInfo::claim_time_deposit())]
        pub fn claim_time_deposit(origin: OriginFor<T>, deposit_id: u32) -> DispatchResult {
            let caller = ensure_signed(origin)?;

            let mut deposit =
                TimeDeposits::<T>::get(deposit_id).ok_or(Error::<T>::DepositNotFound)?;

            ensure!(&deposit.depositor == &caller, Error::<T>::DepositorMismatch);
            ensure!(
                matches!(deposit.status, DepositStatus::Active),
                Error::<T>::DepositAlreadyClaimed
            );

            let current = Self::current_block();
            ensure!(
                current >= deposit.maturity_block,
                Error::<T>::DepositNotMatured
            );

            // Calculate pro-rated interest.
            let interest = Self::calculate_interest(
                deposit.amount,
                deposit.interest_rate_bps,
                deposit.opened_at,
                deposit.maturity_block,
            );
            let total = deposit.amount.saturating_add(interest);

            let treasury = Self::treasury_account();

            // Sprint 9 FIX: verify treasury has sufficient funds before transfer.
            //
            // The BOS treasury must be pre-funded with interest reserves via `fund_treasury`
            // (called by BankingOrigin after receiving operational revenue from the CB).
            // L1 source of truth: if the treasury can't pay, the operation fails on-chain.
            let treasury_balance = <T as Config>::Currency::free_balance(&treasury);
            ensure!(
                treasury_balance >= total,
                Error::<T>::TreasuryInsufficientFunds
            );

            // Transfer principal + interest from treasury → depositor.
            <T as Config>::Currency::transfer(
                &treasury, &caller, total, ExistenceRequirement::AllowDeath
            )?;

            // Mark as claimed.
            deposit.status = DepositStatus::Claimed;
            TimeDeposits::<T>::insert(deposit_id, &deposit);

            Self::deposit_event(Event::TimeDepositClaimed {
                id: deposit_id,
                depositor: caller,
                amount: deposit.amount,
                interest,
            });
            Ok(())
        }
        // ─── approve_loan ─────────────────────────────────────────────────────

        /// Approve a pending loan request and disburse funds to the borrower.
        ///
        /// ## Constitutional Mandate
        ///
        /// Only `BankingOrigin` (CB Board or Sudo in dev) may approve loans.
        /// This is the missing link between `request_loan` (citizen submits)
        /// and `pay_credit` (citizen repays). Without approval, funds are never
        /// disbursed and the credit system does not function.
        ///
        /// ## Fund Flow
        ///
        /// ```text
        /// BOS Treasury ──[Currency::transfer]──► Borrower wallet
        ///              (loan amount disbursed)
        /// ```
        ///
        /// The BOS treasury must have sufficient funds (pre-funded by CB via
        /// `fund_treasury` or `mint_to_operator` to BOS treasury address).
        ///
        /// ## State Transitions
        ///
        /// `LoanStatus::Pending → LoanStatus::Active`
        /// `ActiveLoanByBorrower[borrower] = loan_id` (O(1) index set)
        ///
        /// ## Access Control
        ///
        /// `BankingOrigin` only. Citizens cannot self-approve.
        #[pallet::call_index(11)]
        #[pallet::weight(<T as Config>::WeightInfo::approve_loan())]
        pub fn approve_loan(
            origin: OriginFor<T>,
            loan_id: u32,
        ) -> DispatchResult {
            <T as crate::pallet::Config>::BankingOrigin::ensure_origin(origin)?;

            let mut loan = LoanRequests::<T>::get(loan_id)
                .ok_or(Error::<T>::LoanNotFound)?;

            // Only Pending loans can be approved.
            ensure!(
                matches!(loan.status, LoanStatus::Pending),
                Error::<T>::LoanNotPending
            );

            // Ensure borrower does not already have an active loan
            // (defensive: prevents double-funding if called twice on different loan_ids).
            ensure!(
                !ActiveLoanByBorrower::<T>::contains_key(&loan.borrower),
                Error::<T>::BorrowerHasActiveLoan
            );

            let treasury = Self::treasury_account();

            // Check treasury has sufficient funds to disburse.
            let treasury_balance = <T as Config>::Currency::free_balance(&treasury);
            ensure!(
                treasury_balance >= loan.amount,
                Error::<T>::TreasuryInsufficientFunds
            );

            // Disburse: BOS treasury → borrower.
            <T as Config>::Currency::transfer(
                &treasury,
                &loan.borrower,
                loan.amount,
                ExistenceRequirement::KeepAlive,
            )?;

            // Transition Pending → Active.
            loan.status = LoanStatus::Active;
            LoanRequests::<T>::insert(loan_id, &loan);

            // Register in O(1) index so pay_credit can find this loan instantly.
            ActiveLoanByBorrower::<T>::insert(&loan.borrower, loan_id);

            Self::deposit_event(Event::LoanApproved {
                loan_id,
                borrower: loan.borrower,
                amount: loan.amount,
            });
            Ok(())
        }

        // ─── cancel_loan_request ──────────────────────────────────────────────

        /// Cancel a pending loan request and release the collateral lock.
        ///
        /// Only the borrower may cancel their own pending request.
        /// Once a loan is `Active` (approved, funds disbursed), it cannot be
        /// cancelled — the borrower must repay via `pay_credit`.
        ///
        /// ## State Transition
        ///
        /// `LoanStatus::Pending → LoanStatus::Repaid` (reuses Repaid as terminal state)
        /// Collateral lock is released via `LockableCurrency::remove_lock`.
        ///
        /// ## Access Control
        ///
        /// Only the borrower (original `request_loan` signer) may call this.
        #[pallet::call_index(13)]
        #[pallet::weight(<T as Config>::WeightInfo::cancel_loan_request())]
        pub fn cancel_loan_request(
            origin: OriginFor<T>,
            loan_id: u32,
        ) -> DispatchResult {
            let borrower = ensure_signed(origin)?;

            let mut loan = LoanRequests::<T>::get(loan_id)
                .ok_or(Error::<T>::LoanNotFound)?;

            // Only the borrower who submitted the request can cancel it.
            ensure!(loan.borrower == borrower, Error::<T>::NotSubAccountOwner);

            // Only Pending loans can be cancelled. Active loans must be repaid.
            ensure!(
                matches!(loan.status, LoanStatus::Pending),
                Error::<T>::LoanNotPending
            );

            // Release the collateral lock placed by `request_loan`.
            <T as Config>::Currency::remove_lock(LOAN_LOCK_ID, &borrower);
            LoanCollateral::<T>::remove(&borrower, loan_id);

            // Mark as Repaid (terminal state — reuses existing enum variant).
            // A dedicated `Cancelled` variant can be added in a future sprint.
            loan.status = LoanStatus::Repaid;
            LoanRequests::<T>::insert(loan_id, &loan);

            Self::deposit_event(Event::LoanCancelled { loan_id, borrower });
            Ok(())
        }

        // ─── fund_treasury ────────────────────────────────────────────────────

        /// Fund the BOS treasury to cover time deposit interest payments.
        ///
        /// ## Constitutional Mandate
        ///
        /// The BOS treasury must hold enough ALTAN to pay `principal + interest`
        /// when time deposits mature. Interest reserves come from operational
        /// revenue routed by the Central Bank via `mint_to_operator` to a
        /// licensed operator, which then calls `fund_treasury`.
        ///
        /// ## Fund Flow
        ///
        /// ```text
        /// Caller wallet ──[Currency::transfer]──► BOS treasury (PalletId account)
        /// ```
        ///
        /// ## Access Control
        ///
        /// `BankingOrigin` only. Prevents unauthorized inflation of the treasury.
        #[pallet::call_index(12)]
        #[pallet::weight(<T as Config>::WeightInfo::fund_treasury())]
        pub fn fund_treasury(
            origin: OriginFor<T>,
            amount: BalanceOf<T>,
        ) -> DispatchResult {
            // Signed check first to get the caller for the transfer.
            // BankingOrigin validation follows (covers both Root and BankingCouncil).
            let who = ensure_signed(origin.clone())?;
            <T as crate::pallet::Config>::BankingOrigin::ensure_origin(origin)?;

            ensure!(amount > BalanceOf::<T>::default(), Error::<T>::ZeroAmount);

            let treasury = Self::treasury_account();
            <T as Config>::Currency::transfer(
                &who,
                &treasury,
                amount,
                ExistenceRequirement::KeepAlive,
            )?;

            Self::deposit_event(Event::TreasuryFunded { funder: who, amount });
            Ok(())
        }

        /// Sets the public key of the Bank Server authorized to sign Cash Advance requests.
        #[pallet::call_index(14)]
        #[pallet::weight(<T as Config>::WeightInfo::request_loan())]
        pub fn set_bank_server_key(
            origin: OriginFor<T>,
            key: T::AccountId,
        ) -> DispatchResult {
            <T as crate::pallet::Config>::BankingOrigin::ensure_origin(origin)?;
            BankServerKey::<T>::put(&key);
            Self::deposit_event(Event::BankServerKeyUpdated { key });
            Ok(())
        }

        /// Lock collateral for a Secured Credit Card or Consumer Loan.
        /// Transfers `amount` from caller to Bank Treasury and records it in `CollateralBalances`.
        #[pallet::call_index(16)]
        #[pallet::weight(<T as Config>::WeightInfo::withdraw_from_savings())]
        pub fn lock_collateral(origin: OriginFor<T>, amount: BalanceOf<T>) -> DispatchResult {
            let caller = ensure_signed(origin)?;
            ensure!(amount > BalanceOf::<T>::default(), Error::<T>::ZeroAmount);

            let treasury = Self::treasury_account();
            <T as Config>::Currency::transfer(
                &caller,
                &treasury,
                amount,
                ExistenceRequirement::KeepAlive,
            )?;

            let new_balance = CollateralBalances::<T>::get(&caller).saturating_add(amount);
            CollateralBalances::<T>::insert(&caller, new_balance);

            Self::deposit_event(Event::CollateralLocked {
                account: caller,
                amount,
            });

            Ok(())
        }

        /// Unlock collateral. Requires a cryptographic signature from the Bank Server.
        #[pallet::call_index(17)]
        #[pallet::weight(<T as Config>::WeightInfo::withdraw_from_savings())]
        pub fn unlock_collateral(
            origin: OriginFor<T>,
            amount: BalanceOf<T>,
            nonce: u64,
            bank_signature: MultiSignature,
        ) -> DispatchResult {
            let caller = ensure_signed(origin)?;

            ensure!(amount > BalanceOf::<T>::default(), Error::<T>::ZeroAmount);
            ensure!(!UsedNonces::<T>::get(nonce), Error::<T>::NonceAlreadyUsed);

            let server_key = BankServerKey::<T>::get().ok_or(Error::<T>::InvalidSignature)?;

            let server_key_32 = sp_runtime::AccountId32::decode(&mut &server_key.encode()[..]).map_err(|_| Error::<T>::InvalidSignature)?;

            let payload = (caller.clone(), amount, nonce).encode();

            ensure!(
                bank_signature.verify(&payload[..], &server_key_32),
                Error::<T>::InvalidSignature
            );

            UsedNonces::<T>::insert(nonce, true);

            let current_balance = CollateralBalances::<T>::get(&caller);
            ensure!(current_balance >= amount, Error::<T>::ZeroAmount); // Must have enough collateral

            CollateralBalances::<T>::insert(&caller, current_balance.saturating_sub(amount));

            let treasury = Self::treasury_account();
            <T as Config>::Currency::transfer(
                &treasury,
                &caller,
                amount,
                ExistenceRequirement::KeepAlive,
            )?;

            Self::deposit_event(Event::CollateralUnlocked {
                account: caller,
                amount,
            });

            Ok(())
        }

        /// Claims a Cash Advance withdrawing from the Bank of Siberia corporate pool to a personal L1 wallet.
        /// Requires a cryptographic signature from the `BankServerKey`.
        #[pallet::call_index(15)]
        #[pallet::weight(<T as Config>::WeightInfo::withdraw_from_savings())]
        pub fn claim_cash_advance(
            origin: OriginFor<T>,
            amount: BalanceOf<T>,
            nonce: u64,
            bank_signature: MultiSignature,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;
            ensure!(!UsedNonces::<T>::get(nonce), Error::<T>::NonceAlreadyUsed);
            let server_key = BankServerKey::<T>::get().ok_or(Error::<T>::InvalidSignature)?;
            let server_key_32 = sp_runtime::AccountId32::decode(&mut &server_key.encode()[..]).map_err(|_| Error::<T>::InvalidSignature)?;
            let payload = (who.clone(), amount, nonce).encode();
            ensure!(bank_signature.verify(&payload[..], &server_key_32), Error::<T>::InvalidSignature);
            UsedNonces::<T>::insert(nonce, true);
            <T as Config>::Currency::transfer(&Self::treasury_account(), &who, amount, ExistenceRequirement::KeepAlive)?;
            Self::deposit_event(Event::CashAdvanceClaimed { account: who, amount, nonce });
            Ok(())
        }

        /// Repays an outstanding Cash Advance directly to the Bank of Siberia treasury.
        #[pallet::call_index(18)]
        #[pallet::weight(<T as Config>::WeightInfo::withdraw_from_savings())]
        pub fn repay_cash_advance(
            origin: OriginFor<T>,
            amount: BalanceOf<T>,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;
            ensure!(amount > BalanceOf::<T>::default(), Error::<T>::ZeroAmount);

            <T as Config>::Currency::transfer(
                &who,
                &Self::treasury_account(),
                amount,
                ExistenceRequirement::KeepAlive,
            )?;

            Self::deposit_event(Event::CashAdvanceRepaid { account: who, amount });
            Ok(())
        }

        // ─── [19] borrow_wholesale ───────────────────────────────────────────

        /// Borrow wholesale credit from the Central Bank into the BOS Treasury.
        ///
        /// ## Constitutional Mandate
        ///
        /// Operational Banks (like Bank of Siberia) request liquidity from the Central Bank.
        /// This liquidity is then used to issue citizen loans and cash advances.
        ///
        /// ## Fund Flow
        ///
        /// `Central Bank (Mint)` ───► `BOS Treasury`
        ///
        /// ## Access Control
        ///
        /// `BankingOrigin` only (Bank of Siberia officers).
        #[pallet::call_index(19)]
        #[pallet::weight(<T as Config>::WeightInfo::fund_treasury())]
        pub fn borrow_wholesale(
            origin: OriginFor<T>,
            amount: BalanceOf<T>,
        ) -> DispatchResult {
            <T as Config>::BankingOrigin::ensure_origin(origin)?;
            
            let treasury = Self::treasury_account();
            
            pallet_central_bank::Pallet::<T>::do_request_credit(treasury, amount)?;

            Self::deposit_event(Event::WholesaleCreditBorrowed { amount });
            Ok(())
        }

        // ─── [20] repay_wholesale ────────────────────────────────────────────

        /// Repay wholesale credit to the Central Bank from the BOS Treasury.
        ///
        /// ## Constitutional Mandate
        ///
        /// Operational Banks must repay the Central Bank, which burns the returned ALTAN
        /// and restores the Central Bank's Genesis Credit Pool.
        ///
        /// ## Access Control
        ///
        /// `BankingOrigin` only (Bank of Siberia officers).
        #[pallet::call_index(20)]
        #[pallet::weight(<T as Config>::WeightInfo::fund_treasury())]
        pub fn repay_wholesale(
            origin: OriginFor<T>,
            epoch_id: u32,
            amount: BalanceOf<T>,
        ) -> DispatchResult {
            <T as Config>::BankingOrigin::ensure_origin(origin)?;
            
            let treasury = Self::treasury_account();
            
            pallet_central_bank::Pallet::<T>::do_repay_credit(treasury, epoch_id, amount)?;

            Self::deposit_event(Event::WholesaleCreditRepaid { epoch_id, amount });
            Ok(())
        }
    }
}

#[cfg(test)]
mod mock;

#[cfg(test)]
mod tests;
