//! # pallet-bank-operator
//!
//! **Altan Network — Sprint L1-21: Banking Branch + Credit Rating System**
//!
//! Implements the **Banking Branch of Power** — one of the four constitutional
//! branches, alongside Executive, Judicial, and Legislative.
//!
//! ## Constitutional Mandate
//!
//! The Banking Branch manages **ONLY debt and credit**.  It operates under
//! absolute inviolability of citizen accounts:
//!
//! | Action                              | Allowed |
//! |-------------------------------------|---------|
//! | Issue credit backed by collateral   | ✓      |
//! | Assess Real-World Asset (RWA) value | ✓      |
//! | Lock citizen collateral (w/ consent)| ✓      |
//! | Receive 10% collateral fee          | ✓      |
//! | Update credit score                 | ✓      |
//! | Transfer/withdraw citizen funds     | ✗ NEVER|
//! | Freeze citizen account directly     | ✗ NEVER|
//! | New emission without collateral     | ✗ NEVER|
//!
//! ## Credit Rating System (Inspired by FICO / US Credit Model)
//!
//! Citizens accumulate a `CreditScore` (300–850, default: 650) based on their
//! repayment history. The score determines the annual interest rate tier:
//!
//! | Score Range  | Rating     | Annual Rate | Monthly Rate (basis pts) |
//! |-------------|------------|-------------|--------------------------|
//! | 750 – 850   | Excellent  | 0%          | 0 bps                    |
//! | 700 – 749   | Good       | 3%          | 25 bps                   |
//! | 650 – 699   | Fair       | 7%          | 58 bps                   |
//! | 600 – 649   | Below Fair | 10%         | 83 bps                   |
//! | 300 – 599   | Poor       | 15%         | 125 bps                  |
//!
//! Interest is **accrued per block** and can be paid separately (`pay_interest`)
//! or rolled into the final `repay_credit` call.
//!
//! Score updates:
//! - On-time repayment of full credit: **+20 points** (max 850)
//! - Partial repayment on schedule:    **+5 points**
//! - Court-declared default:           **-100 points** (min 300)
//! - Late payment (>30 days overdue):  **-30 points** (min 300)
//!
//! ## Collateral & Credit Flow
//!
//! ```text
//! Citizen deposits 100 ALTAN collateral
//!   └─► Bank fee = 10 ALTAN (10% of 100)
//!   └─► collateral_net = 90 ALTAN
//!   └─► credit_amount = 90 × 9 = 810 ALTAN (newly minted)
//!   └─► interest_rate = f(credit_score)
//!   └─► On repayment: principal BURNED (TotalIssuance ↓) + interest to Bank
//!   └─► On full repayment: collateral_net returned, credit_score += 20
//! ```
//!
//! ## Hard Emission Guard
//!
//! New ALTAN can ONLY be emitted inside `issue_credit`.  `ReserveMultiplier`
//! is validated at runtime — any value ≠ 9 fails with `InvalidReserveMultiplier`.
//!
//! ## Burn on Repayment (Deflation)
//!
//! Principal repayment → `Currency::withdraw` → `NegativeImbalance` DROPPED.
//! Interest repayment  → `Currency::transfer` to bank_account (bank income).
//!
//! ## Dispatchables
//!
//! | Call | Origin | Description |
//! |---|---|---|
//! | `set_bank_account` | Root | Register the operator's sovereign bank account |
//! | `assess_rwa` | Signed (Assessor) | Record an official RWA (Real-World Asset) valuation |
//! | `lock_collateral` | Signed (Operator) | Lock RWA as collateral against a credit facility |
//! | `request_credit` | Signed (Operator) | Apply for a Central Bank credit tranche |
//! | `issue_credit` | Signed (CB Board) | Approve and disburse a credit tranche to the operator |
//! | `accrue_interest` | Root (Epoch hook) | Apply interest accrual for an operator's outstanding credit |
//! | `repay_credit` | Signed (Operator) | Repay outstanding credit principal and interest |
//! | `declare_default` | Signed (CB Board) | Mark an operator as defaulted; initiate collateral seizure |
//! | `request_account_freeze` | Signed (CB Board) | Request a regulatory freeze on an operator account |
//! | `calculate_accrued_interest` | Any (Query) | Off-chain helper to compute accrued interest |
//! | `total_outstanding_debt_for` | Any (Query) | Off-chain helper to sum operator's total outstanding debt |

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
    use frame_support::sp_runtime::traits::{SaturatedConversion, Saturating, Zero};
    use frame_support::{
        pallet_prelude::*,
        traits::{Currency, ExistenceRequirement, ReservableCurrency},
        weights::Weight,
    };
    use frame_system::pallet_prelude::*;

    // =========================================================================
    // Type Aliases
    // =========================================================================

    pub type BalanceOf<T> =
        <<T as Config>::Currency as Currency<<T as frame_system::Config>::AccountId>>::Balance;

    // =========================================================================
    // Pallet
    // =========================================================================

    #[pallet::pallet]
    pub struct Pallet<T>(_);

    // =========================================================================
    // Config
    // =========================================================================

    #[pallet::config]
    pub trait Config: frame_system::Config<RuntimeEvent: From<Event<Self>>> {
        /// Weight information for extrinsics in this pallet.
        type WeightInfo: crate::weights::WeightInfo;
        /// The currency to use for collateral management.
        type Currency: ReservableCurrency<Self::AccountId>;

        /// The fractional reserve multiplier for CDP credit issuance.
        ///
        /// Constitutional mandate: this MUST equal 9.
        type ReserveMultiplier: Get<u32>;

        /// Blocks per interest accrual period.
        ///
        /// Default: 14,400 blocks = 1 day (6s block time).
        /// Interest is calculated as: `outstanding * rate_bps / 10_000` per period.
        type InterestPeriodBlocks: Get<u32>;
    }

    // =========================================================================
    // Credit Rating System
    // =========================================================================

    /// Credit score tier — determines annual interest rate.
    ///
    /// Modeled after the US FICO credit scoring system, adapted for the
    /// Altan constitutional economy. Citizens with no credit history start
    /// at `Fair` (score 650, 7% APR).
    ///
    /// Scores range from 300 (worst) to 850 (best).
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
    pub enum CreditTier {
        /// Score 750–850: 0% interest. Reward for perfect repayment history.
        Excellent,
        /// Score 700–749: 3% APR (~25 bps/month). Good credit standing.
        Good,
        /// Score 650–699: 7% APR (~58 bps/month). Default starting tier.
        Fair,
        /// Score 600–649: 10% APR (~83 bps/month). Below average history.
        BelowFair,
        /// Score 300–599: 15% APR (~125 bps/month). Poor credit / defaults.
        Poor,
    }

    impl CreditTier {
        /// Resolve tier from raw score (300–850).
        pub fn from_score(score: u16) -> Self {
            match score {
                750..=850 => CreditTier::Excellent,
                700..=749 => CreditTier::Good,
                650..=699 => CreditTier::Fair,
                600..=649 => CreditTier::BelowFair,
                _ => CreditTier::Poor,
            }
        }

        /// Annual interest rate in basis points (1 bp = 0.01%).
        ///
        /// | Tier      | APR   | bps/year |
        /// |-----------|-------|----------|
        /// | Excellent | 0%    | 0        |
        /// | Good      | 3%    | 300      |
        /// | Fair      | 7%    | 700      |
        /// | BelowFair | 10%   | 1000     |
        /// | Poor      | 15%   | 1500     |
        pub fn annual_rate_bps(&self) -> u32 {
            match self {
                CreditTier::Excellent => 0,
                CreditTier::Good => 300,
                CreditTier::Fair => 700,
                CreditTier::BelowFair => 1_000,
                CreditTier::Poor => 1_500,
            }
        }

        /// Daily rate in basis points (annual / 365, rounded up).
        pub fn daily_rate_bps(&self) -> u32 {
            let annual = self.annual_rate_bps();
            if annual == 0 {
                return 0;
            }
            // Ceiling division to avoid rounding to zero on small amounts.
            annual.saturating_add(364) / 365
        }
    }

    // =========================================================================
    // Enums
    // =========================================================================

    /// Type of real-world asset used as collateral.
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
    pub enum AssetType {
        /// Physical ALTAN (coin/token) as collateral.
        AltanCoin,
        /// Land parcel assessed in ALTAN.
        Land,
        /// Natural resource deposit assessed in ALTAN.
        NaturalResource,
        /// Infrastructure or real estate assessed in ALTAN.
        Infrastructure,
    }

    /// Status of a collateral contract.
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
    pub enum CollateralStatus {
        /// Collateral is locked; credit not yet issued.
        Locked,
        /// Collateral is actively backing an issued credit.
        Active,
        /// Credit fully repaid; collateral returned to citizen.
        Released,
    }

    /// Status of a credit contract.
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
    pub enum CreditStatus {
        /// Credit is outstanding; repayments may be ongoing.
        Active,
        /// Credit fully repaid; contract closed and collateral released.
        Repaid,
        /// Credit in default; court order requested.
        Default,
    }

    /// Reason for an account freeze request.
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
    pub enum AccountFreezeReason {
        /// Citizen defaulted on a credit obligation.
        CreditDefault,
        /// Suspected fraudulent collateral assessment.
        FraudulentCollateral,
        /// Other violation of credit contract terms.
        ContractViolation,
    }

    // =========================================================================
    // Structs
    // =========================================================================

    /// On-chain record of a citizen's collateral contract.
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
    pub struct CollateralContract<T: Config> {
        pub citizen: T::AccountId,
        pub asset_type: AssetType,
        pub collateral_amount: BalanceOf<T>,
        pub bank_fee: BalanceOf<T>,
        pub collateral_net: BalanceOf<T>,
        pub created_at: u32,
        pub status: CollateralStatus,
        pub credit_requested: bool,
    }

    /// On-chain record of an issued credit.
    ///
    /// Includes interest tracking fields for the credit rating system.
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
    pub struct CreditContract<T: Config> {
        pub citizen: T::AccountId,
        pub collateral_id: u32,
        /// Total principal issued (in planck). = collateral_net × 9.
        pub credit_amount: BalanceOf<T>,
        /// Remaining principal owed (decreases on repayment, burned on payment).
        pub outstanding: BalanceOf<T>,
        /// Accrued but unpaid interest (in planck). Paid to bank account, NOT burned.
        pub accrued_interest: BalanceOf<T>,
        /// Block number when the credit was issued.
        pub issued_at: u32,
        /// Block number of the last interest accrual calculation.
        pub last_interest_block: u32,
        /// Credit tier at time of issuance (determines interest rate).
        pub tier: CreditTier,
        pub status: CreditStatus,
    }

    /// On-chain credit score record for a citizen.
    ///
    /// Scores range from 300 (worst) to 850 (best).
    /// Default at first credit: 650 (Fair tier).
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
    pub struct CreditScoreRecord {
        /// Current score (300–850).
        pub score: u16,
        /// Number of credits successfully fully repaid.
        pub credits_repaid: u32,
        /// Number of credits that went to default.
        pub credits_defaulted: u32,
    }

    impl CreditScoreRecord {
        pub fn new_default() -> Self {
            Self {
                score: 650, // Fair tier by default
                credits_repaid: 0,
                credits_defaulted: 0,
            }
        }

        /// Apply full-repayment bonus (+20 pts, max 850).
        pub fn apply_full_repayment(&mut self) {
            self.score = self.score.saturating_add(20).min(850);
            self.credits_repaid = self.credits_repaid.saturating_add(1);
        }

        /// Apply partial repayment bonus (+5 pts, max 850).
        pub fn apply_partial_repayment(&mut self) {
            self.score = self.score.saturating_add(5).min(850);
        }

        /// Apply default penalty (-100 pts, min 300).
        pub fn apply_default(&mut self) {
            self.score = self.score.saturating_sub(100).max(300);
            self.credits_defaulted = self.credits_defaulted.saturating_add(1);
        }

        /// Apply late payment penalty (-30 pts, min 300).
        pub fn apply_late_payment(&mut self) {
            self.score = self.score.saturating_sub(30).max(300);
        }

        pub fn tier(&self) -> CreditTier {
            CreditTier::from_score(self.score)
        }
    }

    // =========================================================================
    // Storage
    // =========================================================================

    /// Collateral contracts indexed by sequential ID.
    #[pallet::storage]
    #[pallet::getter(fn collateral_contracts)]
    pub type CollateralContracts<T: Config> =
        StorageMap<_, Blake2_128Concat, u32, CollateralContract<T>, OptionQuery>;

    /// Auto-incrementing counter for collateral contract IDs.
    #[pallet::storage]
    #[pallet::getter(fn next_collateral_id)]
    pub type NextCollateralId<T: Config> = StorageValue<_, u32, ValueQuery>;

    /// Credit contracts indexed by sequential ID.
    #[pallet::storage]
    #[pallet::getter(fn credit_contracts)]
    pub type CreditContracts<T: Config> =
        StorageMap<_, Blake2_128Concat, u32, CreditContract<T>, OptionQuery>;

    /// Auto-incrementing counter for credit contract IDs.
    #[pallet::storage]
    #[pallet::getter(fn next_credit_id)]
    pub type NextCreditId<T: Config> = StorageValue<_, u32, ValueQuery>;

    /// The bank's special account where collateral is held and interest is received.
    #[pallet::storage]
    #[pallet::getter(fn bank_special_account)]
    pub type BankSpecialAccount<T: Config> = StorageValue<_, T::AccountId, OptionQuery>;

    /// Total credit outstanding (principal only, in planck).
    #[pallet::storage]
    #[pallet::getter(fn total_credit_outstanding)]
    pub type TotalCreditOutstanding<T: Config> = StorageValue<_, BalanceOf<T>, ValueQuery>;

    /// Credit scores per citizen account.
    ///
    /// Initialized on first credit issuance. Updated on repayment/default.
    #[pallet::storage]
    #[pallet::getter(fn credit_score)]
    pub type CreditScores<T: Config> =
        StorageMap<_, Blake2_128Concat, T::AccountId, CreditScoreRecord, OptionQuery>;

    // =========================================================================
    // Events
    // =========================================================================

    #[pallet::event]
    #[pallet::generate_deposit(pub(super) fn deposit_event)]
    pub enum Event<T: Config> {
        /// A real-world asset was assessed.
        RwaAssessed {
            citizen: T::AccountId,
            asset_type: AssetType,
            value_in_altan: BalanceOf<T>,
        },
        /// A citizen locked collateral.
        CollateralLocked {
            contract_id: u32,
            citizen: T::AccountId,
            collateral_amount: BalanceOf<T>,
            bank_fee: BalanceOf<T>,
            collateral_net: BalanceOf<T>,
        },
        /// A citizen requested credit issuance.
        CreditRequested {
            contract_id: u32,
            citizen: T::AccountId,
        },
        /// A credit was issued (new ALTAN emitted, backed by collateral).
        CreditIssued {
            credit_id: u32,
            citizen: T::AccountId,
            collateral_id: u32,
            credit_amount: BalanceOf<T>,
            /// Interest tier at time of issuance.
            tier: CreditTier,
            /// Annual interest rate in basis points.
            annual_rate_bps: u32,
            /// Citizen's credit score at time of issuance.
            credit_score: u16,
        },
        /// A citizen made a principal repayment (amount BURNED — TotalIssuance ↓).
        CreditRepaid {
            credit_id: u32,
            citizen: T::AccountId,
            principal_burned: BalanceOf<T>,
            interest_paid: BalanceOf<T>,
            outstanding_remaining: BalanceOf<T>,
        },
        /// A credit was fully repaid and collateral returned.
        CreditClosed {
            credit_id: u32,
            collateral_id: u32,
            citizen: T::AccountId,
            collateral_returned: BalanceOf<T>,
        },
        /// Interest accrued on a credit (updated storage, not yet paid).
        InterestAccrued {
            credit_id: u32,
            citizen: T::AccountId,
            accrued_amount: BalanceOf<T>,
            total_accrued: BalanceOf<T>,
        },
        /// A citizen's credit score was updated.
        CreditScoreUpdated {
            citizen: T::AccountId,
            old_score: u16,
            new_score: u16,
            new_tier: CreditTier,
        },
        /// Bank operator requested a court order to freeze a citizen's account.
        AccountFreezeRequested {
            citizen: T::AccountId,
            reason: AccountFreezeReason,
        },
        /// Bank special account configured.
        BankAccountConfigured { account: T::AccountId },
    }

    // =========================================================================
    // Errors
    // =========================================================================

    #[pallet::error]
    pub enum Error<T> {
        BankAccountNotConfigured,
        CollateralNotFound,
        CreditNotFound,
        InvalidCollateralStatus,
        CreditNotActive,
        InsufficientBalance,
        NotCreditOwner,
        CollateralMismatch,
        ArithmeticOverflow,
        ZeroRepayment,
        ZeroCollateral,
        CreditNotRequested,
        NotCollateralOwner,
        CreditAlreadyRequested,
        /// The `ReserveMultiplier` config constant is not equal to 9.
        InvalidReserveMultiplier,
        /// Arbitrary minting is constitutionally prohibited outside CDP logic.
        MintingProhibited,
        /// Credit already has a fully repaid/closed status.
        CreditAlreadyClosed,
    }

    // =========================================================================
    // Hooks
    // =========================================================================

    #[pallet::hooks]
    impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T> {
        /// Per-block interest accrual.
        ///
        /// Iterates active credit contracts and accrues interest if an
        /// `InterestPeriodBlocks` has elapsed since the last accrual.
        ///
        /// Weight: O(n) where n = number of active credits. In production,
        /// use an off-chain worker or lazy accrual triggered on repayment.
        fn on_initialize(_now: BlockNumberFor<T>) -> Weight {
            // Lazy accrual: interest is computed at repayment time.
            // on_initialize is intentionally lightweight here.
            Weight::from_parts(0, 0)
        }
    }

    // =========================================================================
    // Extrinsics
    // =========================================================================

    #[pallet::call]
    impl<T: Config> Pallet<T> {
        // ─── set_bank_account ─────────────────────────────────────────────────

        /// Configure the bank's special account (Root only).
        #[pallet::call_index(0)]
        #[pallet::weight(T::WeightInfo::set_bank_account())]
        pub fn set_bank_account(origin: OriginFor<T>, account: T::AccountId) -> DispatchResult {
            ensure_root(origin)?;
            BankSpecialAccount::<T>::put(account.clone());
            Self::deposit_event(Event::BankAccountConfigured { account });
            Ok(())
        }

        // ─── assess_rwa ───────────────────────────────────────────────────────

        /// Record a Real-World Asset assessment on-chain (Bank Tumed / Root).
        #[pallet::call_index(1)]
        #[pallet::weight(T::WeightInfo::assess_rwa())]
        pub fn assess_rwa(
            origin: OriginFor<T>,
            citizen: T::AccountId,
            asset_type: AssetType,
            value_in_altan: BalanceOf<T>,
        ) -> DispatchResult {
            ensure_root(origin)?;
            Self::deposit_event(Event::RwaAssessed {
                citizen,
                asset_type,
                value_in_altan,
            });
            Ok(())
        }

        // ─── lock_collateral ──────────────────────────────────────────────────

        /// Citizen locks collateral on the bank's special account.
        ///
        /// 10% fee retained immediately. Net collateral = 90% of amount.
        #[pallet::call_index(2)]
        #[pallet::weight(T::WeightInfo::lock_collateral())]
        pub fn lock_collateral(
            origin: OriginFor<T>,
            amount: BalanceOf<T>,
            asset_type: AssetType,
        ) -> DispatchResult {
            let citizen = ensure_signed(origin)?;
            ensure!(amount > Zero::zero(), Error::<T>::ZeroCollateral);

            let bank_account =
                BankSpecialAccount::<T>::get().ok_or(Error::<T>::BankAccountNotConfigured)?;

            T::Currency::transfer(
                &citizen,
                &bank_account,
                amount,
                ExistenceRequirement::KeepAlive,
            )
            .map_err(|_| Error::<T>::InsufficientBalance)?;

            let bank_fee = amount / BalanceOf::<T>::from(10u32);
            let collateral_net = amount.saturating_sub(bank_fee);

            let contract_id = NextCollateralId::<T>::get();
            let current_block: u32 =
                frame_system::Pallet::<T>::block_number().saturated_into::<u32>();

            CollateralContracts::<T>::insert(
                contract_id,
                CollateralContract::<T> {
                    citizen: citizen.clone(),
                    asset_type,
                    collateral_amount: amount,
                    bank_fee,
                    collateral_net,
                    created_at: current_block,
                    status: CollateralStatus::Locked,
                    credit_requested: false,
                },
            );
            NextCollateralId::<T>::put(contract_id.saturating_add(1));

            Self::deposit_event(Event::CollateralLocked {
                contract_id,
                citizen,
                collateral_amount: amount,
                bank_fee,
                collateral_net,
            });
            Ok(())
        }

        // ─── request_credit ───────────────────────────────────────────────────

        /// Citizen explicitly requests credit issuance (dual-consent step 2/3).
        #[pallet::call_index(3)]
        #[pallet::weight(T::WeightInfo::request_credit())]
        pub fn request_credit(origin: OriginFor<T>, collateral_id: u32) -> DispatchResult {
            let citizen = ensure_signed(origin)?;

            let mut col = CollateralContracts::<T>::get(collateral_id)
                .ok_or(Error::<T>::CollateralNotFound)?;

            ensure!(col.citizen == citizen, Error::<T>::NotCollateralOwner);
            ensure!(
                col.status == CollateralStatus::Locked,
                Error::<T>::InvalidCollateralStatus
            );
            ensure!(!col.credit_requested, Error::<T>::CreditAlreadyRequested);

            col.credit_requested = true;
            CollateralContracts::<T>::insert(collateral_id, col.clone());

            Self::deposit_event(Event::CreditRequested {
                contract_id: collateral_id,
                citizen,
            });
            Ok(())
        }

        // ─── issue_credit ─────────────────────────────────────────────────────

        /// Issue a credit backed by collateral (Root/Bank Tumed).
        ///
        /// ## Credit Rating Integration
        ///
        /// The citizen's `CreditScore` (or default 650) determines the
        /// interest `tier` stored in the `CreditContract`. This tier is fixed
        /// at issuance — early repayment improves score for future credits.
        ///
        /// ## Constitutional Credit Formula
        ///
        ///   credit_amount = collateral_net × 9
        ///
        /// ## Multiplier Guard
        ///
        /// Fails with `InvalidReserveMultiplier` if `ReserveMultiplier ≠ 9`.
        #[pallet::call_index(4)]
        #[pallet::weight(T::WeightInfo::issue_credit())]
        pub fn issue_credit(origin: OriginFor<T>, collateral_id: u32) -> DispatchResult {
            ensure_root(origin)?;

            // ── AUDIT GUARD 1: Multiplier constitutional check ─────────────────
            ensure!(
                T::ReserveMultiplier::get() == 9u32,
                Error::<T>::InvalidReserveMultiplier
            );

            let mut col = CollateralContracts::<T>::get(collateral_id)
                .ok_or(Error::<T>::CollateralNotFound)?;

            ensure!(
                col.status == CollateralStatus::Locked,
                Error::<T>::InvalidCollateralStatus
            );
            ensure!(col.credit_requested, Error::<T>::CreditNotRequested);

            let multiplier = BalanceOf::<T>::from(T::ReserveMultiplier::get());
            let credit_amount = col.collateral_net.saturating_mul(multiplier);

            // ── Credit Rating: resolve tier from citizen's score ───────────────
            let score_record = CreditScores::<T>::get(&col.citizen)
                .unwrap_or_else(|| CreditScoreRecord::new_default());
            let tier = score_record.tier();
            let annual_rate_bps = tier.annual_rate_bps();
            let credit_score = score_record.score;

            // Initialize score record if first credit.
            if CreditScores::<T>::get(&col.citizen).is_none() {
                CreditScores::<T>::insert(&col.citizen, CreditScoreRecord::new_default());
            }

            // ── Constitutional emission: deposit NEW ALTAN to citizen ──────────
            // The `PositiveImbalance` is DROPPED → TotalIssuance ↑ by credit_amount.
            // This is the ONLY place in this pallet where new ALTAN is created.
            // Backed 1:9 by the bank's held collateral_net.
            let _minted = T::Currency::deposit_creating(&col.citizen, credit_amount);
            drop(_minted); // Intentional: TotalIssuance ↑ (CDP mint)

            col.status = CollateralStatus::Active;
            CollateralContracts::<T>::insert(collateral_id, col.clone());

            let credit_id = NextCreditId::<T>::get();
            let current_block: u32 = frame_system::Pallet::<T>::block_number()
                .try_into()
                .unwrap_or(0);

            CreditContracts::<T>::insert(
                credit_id,
                CreditContract::<T> {
                    citizen: col.citizen.clone(),
                    collateral_id,
                    credit_amount,
                    outstanding: credit_amount,
                    accrued_interest: Zero::zero(),
                    issued_at: current_block,
                    last_interest_block: current_block,
                    tier: tier.clone(),
                    status: CreditStatus::Active,
                },
            );
            NextCreditId::<T>::put(credit_id.saturating_add(1));

            TotalCreditOutstanding::<T>::mutate(|total| {
                *total = total.saturating_add(credit_amount);
            });

            Self::deposit_event(Event::CreditIssued {
                credit_id,
                citizen: col.citizen,
                collateral_id,
                credit_amount,
                tier,
                annual_rate_bps,
                credit_score,
            });
            Ok(())
        }

        // ─── accrue_interest ──────────────────────────────────────────────────

        /// Trigger interest accrual for a specific credit (lazy accrual model).
        ///
        /// Can be called by anyone — the bank, the citizen, or an automated worker.
        /// Updates `accrued_interest` in storage. No funds move until `repay_credit`.
        ///
        /// ## Interest Formula
        ///
        /// For Excellent tier (0% APR): no interest accrued.
        ///
        /// For all other tiers:
        /// ```ignore
        /// periods_elapsed = (current_block - last_interest_block) / InterestPeriodBlocks
        /// daily_rate_bps  = annual_rate_bps / 365  (ceiling)
        /// interest         = outstanding × daily_rate_bps × periods_elapsed / 10_000
        /// ```ignore
        #[pallet::call_index(5)]
        #[pallet::weight(T::WeightInfo::accrue_interest())]
        pub fn accrue_interest(origin: OriginFor<T>, credit_id: u32) -> DispatchResult {
            let _caller = ensure_signed(origin)?;

            let mut credit =
                CreditContracts::<T>::get(credit_id).ok_or(Error::<T>::CreditNotFound)?;
            ensure!(
                credit.status == CreditStatus::Active,
                Error::<T>::CreditNotActive
            );

            let accrued = Self::calculate_accrued_interest(&credit)?;

            if accrued > Zero::zero() {
                credit.accrued_interest = credit.accrued_interest.saturating_add(accrued);
                let current_block: u32 = frame_system::Pallet::<T>::block_number()
                    .try_into()
                    .unwrap_or(0);
                credit.last_interest_block = current_block;
                CreditContracts::<T>::insert(credit_id, credit.clone());

                Self::deposit_event(Event::InterestAccrued {
                    credit_id,
                    citizen: credit.citizen,
                    accrued_amount: accrued,
                    total_accrued: credit.accrued_interest,
                });
            }

            Ok(())
        }

        // ─── repay_credit ─────────────────────────────────────────────────────

        /// Citizen repays part or all of their outstanding credit.
        ///
        /// ## Payment Priority
        ///
        /// 1. **Interest first**: Any accrued interest is paid to the bank account
        ///    via `Currency::transfer` (bank income, NOT burned). Interest accrual
        ///    is calculated up to the current block before processing repayment.
        ///
        /// 2. **Principal second**: Remaining `amount` reduces outstanding principal.
        ///    Principal is BURNED via `Currency::withdraw` → `NegativeImbalance` DROPPED.
        ///    This permanently reduces `TotalIssuance` (deflationary mechanism).
        ///
        /// ## Credit Score Update
        ///
        /// - Partial repayment: `credit_score += 5` (max 850)
        /// - Full repayment: `credit_score += 20` + collateral returned
        ///
        /// ## Full Repayment
        ///
        /// On full repayment (`outstanding == 0`):
        ///   - Credit status → `Repaid`
        ///   - Collateral status → `Released`
        ///   - `collateral_net` (90%) returned to citizen from bank account
        ///   - Bank keeps `bank_fee` (10%) as permanent income
        #[pallet::call_index(6)]
        #[pallet::weight(T::WeightInfo::repay_credit())]
        pub fn repay_credit(
            origin: OriginFor<T>,
            credit_id: u32,
            amount: BalanceOf<T>,
        ) -> DispatchResult {
            let citizen = ensure_signed(origin)?;
            ensure!(amount > Zero::zero(), Error::<T>::ZeroRepayment);

            let mut credit =
                CreditContracts::<T>::get(credit_id).ok_or(Error::<T>::CreditNotFound)?;
            ensure!(credit.citizen == citizen, Error::<T>::NotCreditOwner);
            ensure!(
                credit.status == CreditStatus::Active,
                Error::<T>::CreditNotActive
            );

            let bank_account =
                BankSpecialAccount::<T>::get().ok_or(Error::<T>::BankAccountNotConfigured)?;

            // ── Step 0: Lazy interest accrual ────────────────────────────────
            let newly_accrued = Self::calculate_accrued_interest(&credit)?;
            credit.accrued_interest = credit.accrued_interest.saturating_add(newly_accrued);
            let current_block: u32 = frame_system::Pallet::<T>::block_number()
                .try_into()
                .unwrap_or(0);
            credit.last_interest_block = current_block;

            // ── Step 1: Pay interest first (transfer to bank — NOT burned) ────
            let interest_due = credit.accrued_interest;
            let mut remaining_payment = amount;
            let mut interest_paid: BalanceOf<T> = Zero::zero();

            if interest_due > Zero::zero() && remaining_payment > Zero::zero() {
                let pay_interest = interest_due.min(remaining_payment);
                T::Currency::transfer(
                    &citizen,
                    &bank_account,
                    pay_interest,
                    ExistenceRequirement::KeepAlive,
                )
                .map_err(|_| Error::<T>::InsufficientBalance)?;

                credit.accrued_interest = credit.accrued_interest.saturating_sub(pay_interest);
                remaining_payment = remaining_payment.saturating_sub(pay_interest);
                interest_paid = pay_interest;
            }

            // ── Step 2: Burn principal (deflation) ────────────────────────────
            //
            // `withdraw` returns a `NegativeImbalance`. DROPPING it reduces
            // `TotalIssuance` permanently — the constitutional burn mechanism.
            // Credit tokens are destroyed from the monetary base, reversing the
            // CDP emission and maintaining the hard cap integrity.
            let principal_repay = remaining_payment.min(credit.outstanding);
            let mut principal_burned: BalanceOf<T> = Zero::zero();

            if principal_repay > Zero::zero() {
                let negative_imbalance = T::Currency::withdraw(
                    &citizen,
                    principal_repay,
                    frame_support::traits::WithdrawReasons::all(),
                    ExistenceRequirement::KeepAlive,
                )
                .map_err(|_| Error::<T>::InsufficientBalance)?;

                // Intentional burn: drop → TotalIssuance ↓
                drop(negative_imbalance);

                credit.outstanding = credit.outstanding.saturating_sub(principal_repay);
                TotalCreditOutstanding::<T>::mutate(|t| {
                    *t = t.saturating_sub(principal_repay);
                });
                principal_burned = principal_repay;
            }

            // ── Step 3: Credit score update ───────────────────────────────────
            let fully_repaid =
                credit.outstanding == Zero::zero() && credit.accrued_interest == Zero::zero();

            let mut score_record = CreditScores::<T>::get(&citizen)
                .unwrap_or_else(|| CreditScoreRecord::new_default());
            let old_score = score_record.score;

            if fully_repaid {
                score_record.apply_full_repayment();
            } else if principal_burned > Zero::zero() {
                score_record.apply_partial_repayment();
            }

            let new_score = score_record.score;
            let new_tier = score_record.tier();

            if old_score != new_score {
                CreditScores::<T>::insert(&citizen, score_record);
                Self::deposit_event(Event::CreditScoreUpdated {
                    citizen: citizen.clone(),
                    old_score,
                    new_score,
                    new_tier,
                });
            }

            // ── Step 4: Close credit on full repayment ────────────────────────
            if fully_repaid {
                credit.status = CreditStatus::Repaid;
                CreditContracts::<T>::insert(credit_id, credit.clone());

                // Return collateral_net (90%) to citizen; bank keeps bank_fee (10%).
                let col = CollateralContracts::<T>::get(credit.collateral_id)
                    .ok_or(Error::<T>::CollateralNotFound)?;

                T::Currency::transfer(
                    &bank_account,
                    &citizen,
                    col.collateral_net,
                    ExistenceRequirement::KeepAlive,
                )
                .map_err(|_| Error::<T>::InsufficientBalance)?;

                CollateralContracts::<T>::mutate(credit.collateral_id, |maybe| {
                    if let Some(c) = maybe {
                        c.status = CollateralStatus::Released;
                    }
                });

                Self::deposit_event(Event::CreditClosed {
                    credit_id,
                    collateral_id: credit.collateral_id,
                    citizen: citizen.clone(),
                    collateral_returned: col.collateral_net,
                });
            } else {
                CreditContracts::<T>::insert(credit_id, credit.clone());
            }

            Self::deposit_event(Event::CreditRepaid {
                credit_id,
                citizen,
                principal_burned,
                interest_paid,
                outstanding_remaining: credit.outstanding,
            });
            Ok(())
        }

        // ─── declare_default ──────────────────────────────────────────────────

        /// Declare a credit in default (Root/Bank Tumed after court order).
        ///
        /// Sets `CreditStatus::Default` and applies `-100` credit score penalty.
        /// Does NOT freeze the account — that requires a separate court order via
        /// `pallet-judicial-courts::execute_verdict`.
        ///
        /// A `request_account_freeze` event is also emitted to notify the court.
        #[pallet::call_index(7)]
        #[pallet::weight(T::WeightInfo::declare_default())]
        pub fn declare_default(origin: OriginFor<T>, credit_id: u32) -> DispatchResult {
            ensure_root(origin)?;

            let mut credit =
                CreditContracts::<T>::get(credit_id).ok_or(Error::<T>::CreditNotFound)?;
            ensure!(
                credit.status == CreditStatus::Active,
                Error::<T>::CreditNotActive
            );

            credit.status = CreditStatus::Default;
            CreditContracts::<T>::insert(credit_id, credit.clone());

            // Apply default penalty to credit score.
            let mut score = CreditScores::<T>::get(&credit.citizen)
                .unwrap_or_else(|| CreditScoreRecord::new_default());
            let old_score = score.score;
            score.apply_default();
            let new_score = score.score;
            let new_tier = score.tier();
            CreditScores::<T>::insert(&credit.citizen, score);

            Self::deposit_event(Event::CreditScoreUpdated {
                citizen: credit.citizen.clone(),
                old_score,
                new_score,
                new_tier,
            });

            // Notify court for potential account freeze.
            Self::deposit_event(Event::AccountFreezeRequested {
                citizen: credit.citizen,
                reason: AccountFreezeReason::CreditDefault,
            });

            Ok(())
        }

        // ─── request_account_freeze ───────────────────────────────────────────

        /// Bank operator requests a court order to freeze a citizen's account.
        ///
        /// ## Constitutional Constraint
        ///
        /// The bank CANNOT freeze accounts directly.
        /// This records the request on-chain. The actual freeze can ONLY happen
        /// via `pallet-judicial-courts::execute_verdict`.
        #[pallet::call_index(8)]
        #[pallet::weight(T::WeightInfo::request_account_freeze())]
        pub fn request_account_freeze(
            origin: OriginFor<T>,
            citizen: T::AccountId,
            reason: AccountFreezeReason,
        ) -> DispatchResult {
            ensure_root(origin)?;
            Self::deposit_event(Event::AccountFreezeRequested { citizen, reason });
            Ok(())
        }
    }

    // =========================================================================
    // Internal helpers
    // =========================================================================

    impl<T: Config> Pallet<T> {
        /// Calculate interest accrued on a credit since `last_interest_block`.
        ///
        /// Uses lazy accrual: called at repayment time.
        ///
        /// Formula:
        /// ```ignore
        /// periods = (current_block - last_interest_block) / InterestPeriodBlocks
        /// interest = outstanding × daily_rate_bps × periods / 10_000
        /// ```ignore
        ///
        /// For `Excellent` tier (0% APR): always returns 0.
        pub fn calculate_accrued_interest(
            credit: &CreditContract<T>,
        ) -> Result<BalanceOf<T>, DispatchError> {
            let daily_rate_bps = credit.tier.daily_rate_bps();
            if daily_rate_bps == 0 {
                return Ok(Zero::zero());
            }

            let current_block: u32 = frame_system::Pallet::<T>::block_number()
                .try_into()
                .unwrap_or(0);
            let blocks_elapsed = current_block.saturating_sub(credit.last_interest_block);
            let period_blocks = T::InterestPeriodBlocks::get();

            if period_blocks == 0 || blocks_elapsed < period_blocks {
                return Ok(Zero::zero());
            }

            let periods = blocks_elapsed / period_blocks;
            let outstanding_u128: u128 = credit.outstanding.saturated_into();

            // interest = outstanding × daily_rate_bps × periods / 10_000
            let interest_u128 = outstanding_u128
                .saturating_mul(daily_rate_bps as u128)
                .saturating_mul(periods as u128)
                / 10_000u128;

            let interest: BalanceOf<T> = interest_u128.saturated_into();
            Ok(interest)
        }

        /// Query total outstanding debt for a citizen (used by pallet-inheritance).
        ///
        /// Sums all active `CreditContract::outstanding` values for the given account.
        /// Returns planck units (u128).
        pub fn total_outstanding_debt_for(who: &T::AccountId) -> u128 {
            let mut total: u128 = 0u128;
            for (_id, credit) in CreditContracts::<T>::iter() {
                if &credit.citizen == who && credit.status == CreditStatus::Active {
                    let outstanding_u128: u128 = credit.outstanding.saturated_into();
                    total = total.saturating_add(outstanding_u128);
                }
            }
            total
        }
    }
}

// =========================================================================
// Unit Tests
// =========================================================================

#[cfg(test)]
mod mock;

#[cfg(test)]
mod tests;
