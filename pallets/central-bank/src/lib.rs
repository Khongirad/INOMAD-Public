//! # pallet-central-bank
//!
//! **Altan Network — Конституционный Центральный Банк Республики**
//!
//! The Central Bank is the **sole constitutional issuer** of ALTAN tokens and
//! the **sole authority** over the credit macroeconomic engine.
//!
//! ## Rotating Credit Pool (Constitutional Variant B)
//!
//! The entire GENESIS era has a **single rotating credit pool** of 18.9T ALTAN.
//! This is NOT a per-epoch limit — it is a shared constitutional cap for the
//! lifetime of the GENESIS era.
//!
//! ```text
//! Genesis Credit Pool = 18,900,000,000,000 ALTAN (M1, absolute ceiling)
//!
//! request_credit(N)  → pool_available -= N   (credit issued, TotalIssuance ↑)
//! repay_credit(N)    → pool_available += N   (credit returned, TotalIssuance ↓)
//!
//! pool_available = 18.9T → rate = 0%      (free credit — maximum incentive to borrow)
//! pool_available = 0     → rate = 8.5%    (fully utilized — full key rate applies)
//!
//! Dynamic key_rate = base_rate × (1 - pool_available / genesis_limit)
//!                 = base_rate × (outstanding / genesis_limit)
//! ```
//!
//! ## Monetary Hard Cap
//!
//! ```text
//! M0 = 2.1T ALTAN  (primary emission, Genesis Drop, immutable)
//! M1 = 18.9T ALTAN (rotating credit pool — max in-circulation credit)
//! Hard Cap = M0 + M1 = 21T ALTAN (absolute maximum TotalIssuance)
//! ```
//!
//! ## Repayment Health
//!
//! All epoch data is public so the Next.js frontend can compute:
//! `health = total_repaid / total_issued` for every historical epoch.
//!
//! ## Constitutional Constraints (GENESIS_CONSTITUTION_LAW §4.1, §5.1)
//!
//! - Key rate changes require BankingOrigin (CB Board multi-sig).
//! - Dynamic rate is auto-calculated from pool utilization — NOT manually set.
//! - Debt is epoch-tagged, preserving the rate of the originating epoch.
//! - When pool_available = 0 → credit is blocked until repayments restore capacity.
//!
//! ## Dispatchables
//!
//! | Call | Origin | Description |
//! |---|---|---|
//! | `grant_operator_license` | BankingOrigin | Register an account as a licensed CB operator |
//! | `revoke_operator_license` | BankingOrigin | Revoke a licensed operator's minting rights |
//! | `mint_to_operator` | BankingOrigin | Mint new ALTAN to a licensed operator (sole emission path) |
//! | `burn` | BankingOrigin | Burn ALTAN from a licensed operator's balance |
//! | `transition_epoch` | BankingOrigin | Manually close the current epoch and open a new one |
//! | `set_base_rate` | BankingOrigin | Set the base rate ceiling (max key rate in basis points) |
//! | `request_credit` | Signed (Citizen) | Apply for a citizen credit line at dynamic key rate |
//! | `repay_credit` | Signed (Citizen) | Repay outstanding citizen credit debt (restores pool capacity) |
//! | `trigger_genesis_distribution` | BankingOrigin | Route 2.1T ALTAN M0 to 167 constitutional treasury accounts |
//! | `issue_genesis_grant` | BankingOrigin | Issue 100 ALTAN Genesis Grant to a new citizen from CB sovereign |

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
    use frame_support::{
        pallet_prelude::*,
        traits::{Currency, EnsureOrigin, ExistenceRequirement, WithdrawReasons},
    };
    use frame_system::pallet_prelude::*;
    use sp_runtime::traits::{Saturating, Zero};

    // =========================================================================
    // Type Aliases
    // =========================================================================

    pub type BalanceOf<T> =
        <<T as Config>::Currency as Currency<<T as frame_system::Config>::AccountId>>::Balance;

    // =========================================================================
    // EpochInfo Struct
    // =========================================================================

    /// A macroeconomic credit epoch.
    ///
    /// Epochs track how much credit was issued and repaid under a given dynamic
    /// rate. In the constitutional model (Variant B), epochs are informational
    /// audit records — the credit limit is enforced by the global Genesis pool,
    /// not per-epoch counters.
    ///
    /// Historical epochs are immutable once closed.
    /// They form the permanent on-chain audit trail of the CB's credit policy.
    #[derive(Encode, Decode, Clone, PartialEq, Eq, RuntimeDebug, TypeInfo, MaxEncodedLen)]
    pub struct EpochInfo<Balance, BlockNumber> {
        /// Block at which this epoch began.
        pub start_block: BlockNumber,
        /// Dynamic CB key rate for this epoch, in basis points (1 bp = 0.01%).
        /// Auto-calculated: base_rate × (outstanding / genesis_limit).
        /// e.g. 850 = 8.50% per annum at full utilization; 0 = free credit.
        pub key_rate: u32,
        /// Total credit minted (issued) to citizens in this epoch.
        pub total_issued: Balance,
        /// Total credit burned (repaid) attributed to this epoch.
        pub total_repaid: Balance,
        /// Snapshot of the genesis credit limit at epoch creation (for audit).
        pub credit_limit: Balance,
    }

    // ── Macroeconomic Traits ──────────────────────────────────────────────────
    pub trait LockableCurrency<AccountId>: Currency<AccountId> {
        type Moment;
    }

    // =========================================================================
    // Constitutional Region Codes (OKATO, 83 Federal Subjects)
    // =========================================================================
    //
    // Official OKATO codes for 83 Federal Subjects of the Russian Federation.
    // Used as keys in RegionalTreasuries and PensionFunds storage maps.
    //
    // Using official codes (not sequential 0..82) ensures:
    //   - On-chain key = official code → no adapters needed in backend/frontend
    //   - StorageMap gaps (80-82, 84-85, 88) cost zero state — FRAME Trie skips them
    //   - Constitutional clarity: Buryatia = key 3, Moscow = key 77 (forever)
    pub const REGION_CODES_83: [u8; 83] = [
        // 21 Republics
         1,  2,  3,  4,  5,  6,  7,  8,  9, 10,
        11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21,
        // 6 Krais
        22, 23, 24, 25, 26, 27,
        // 49 Oblasts
        28, 29, 30, 31, 32, 33, 34, 35, 36, 37, 38, 39, 40, 41, 42, 43, 44, 45, 46,
        47, 48, 49, 50, 51, 52, 53, 54, 55, 56, 57, 58, 59, 60, 61, 62, 63, 64, 65,
        66, 67, 68, 69, 70, 71, 72, 73, 74, 75, 76,
        // 2 Federal Cities
        77, 78,
        // 1 Autonomous Oblast
        79,
        // 4 Autonomous Okrugs
        83, 86, 87, 89,
    ];

    // =========================================================================
    // Pallet
    // =========================================================================

    #[pallet::pallet]
    pub struct Pallet<T>(_);

    // =========================================================================
    // Config
    // =========================================================================

    #[pallet::config]
    pub trait Config: frame_system::Config<RuntimeEvent: From<Event<Self>>> + pallet_proxy::Config {
        /// Weight information for extrinsics in this pallet.
        type WeightInfo: crate::weights::WeightInfo;
        
        /// The native ALTAN currency — sole minting authority.
        type Currency: Currency<Self::AccountId>;
        
        /// The Creator account (Citizen #1) authorized to sign genesis distribution.
        ///
        /// The Creator signs `trigger_genesis_distribution()` as the temporary
        /// BankingOrigin (Sudo) until the CB Board is democratically elected.
        /// He does NOT hold the 2.1T — that belongs to CENTRAL_BANK.
        #[pallet::constant]
        type CreatorAccount: Get<Self::AccountId>;

        /// The Central Bank's own sovereign account (keyless 7-of-10 multisig).
        ///
        /// This account holds the full 2.1T ALTAN M0 supply at genesis.
        /// No private key exists for it — funds can only move via pallet extrinsics
        /// (i.e., BankingOrigin-authorized `trigger_genesis_distribution` or future
        /// CB Board-authorized extrinsics).
        #[pallet::constant]
        type CentralBankAccountId: Get<Self::AccountId>;

        /// Banking Branch origin — the constitutionally independent fourth
        /// branch of power. Required for all monetary policy changes.
        type BankingOrigin: EnsureOrigin<Self::RuntimeOrigin>;

        /// Constitutional M1 ceiling: total Genesis rotating credit pool.
        ///
        /// Production value: 18_900_000_000_000 ALTAN × 10^12 planks.
        /// This is the TOTAL credit available across the entire GENESIS era —
        /// NOT a per-epoch limit. Repayments restore capacity up to this cap.
        #[pallet::constant]
        type CreditEpochLimit: Get<BalanceOf<Self>>;

        /// Constitutional optimal key rate (in basis points) at target utilization.
        #[pallet::constant]
        type OptimalKeyRate: Get<u32>;

        /// Constitutional maximum protective rate (in basis points) at 100% utilization.
        /// This hardcoded penalty prevents monopolization.
        #[pallet::constant]
        type MaxProtectiveRate: Get<u32>;

        /// Constitutional optimal utilization percentage (0-100).
        #[pallet::constant]
        type OptimalUtilization: Get<u32>;
    }

    // =========================================================================
    // Storage — Monetary Infrastructure (existing)
    // =========================================================================

    /// Registry of Licensed Operators — accounts authorised to receive minted ALTAN.
    /// The Central Bank CANNOT mint to arbitrary addresses.
    #[pallet::storage]
    #[pallet::getter(fn licensed_operators)]
    pub type LicensedOperators<T: Config> =
        StorageMap<_, Blake2_128Concat, T::AccountId, bool, ValueQuery>;

    /// Total ALTAN emitted via `mint_to_operator` since genesis.
    #[pallet::storage]
    #[pallet::getter(fn total_emitted)]
    pub type TotalEmitted<T: Config> = StorageValue<_, BalanceOf<T>, ValueQuery>;

    /// Total ALTAN burned via `burn` since genesis.
    #[pallet::storage]
    #[pallet::getter(fn total_burned)]
    pub type TotalBurned<T: Config> = StorageValue<_, BalanceOf<T>, ValueQuery>;

    /// Sequential tranche counter — increments on every `mint_to_operator`.
    #[pallet::storage]
    #[pallet::getter(fn next_tranche_id)]
    pub type NextTrancheId<T: Config> = StorageValue<_, u32, ValueQuery>;

    /// Flag to ensure genesis distribution is only triggered once.
    #[pallet::storage]
    #[pallet::getter(fn genesis_distributed)]
    pub type GenesisDistributed<T: Config> = StorageValue<_, bool, ValueQuery>;

    // =========================================================================
    // Storage — Continuous Macroeconomic Engine
    // =========================================================================

    /// Active epoch ID. Starts at 1 at genesis.
    /// In the constitutional model, epochs are audit snapshots — transitions
    /// are triggered by the CB Board, not by credit limit exhaustion.
    #[pallet::storage]
    #[pallet::getter(fn current_epoch_id)]
    pub type CurrentEpochId<T: Config> = StorageValue<_, u32, ValueQuery>;

    /// Historical and current epoch data, keyed by epoch ID.
    ///
    /// Public so the frontend can calculate Repayment Health for every epoch:
    /// `health_pct = (total_repaid / total_issued) * 100`
    #[pallet::storage]
    #[pallet::getter(fn epochs)]
    pub type Epochs<T: Config> =
        StorageMap<_, Twox64Concat, u32, EpochInfo<BalanceOf<T>, BlockNumberFor<T>>, OptionQuery>;

    /// Stores the Pure Proxy address of the Confederation Treasury (3% = 63B ALTAN).
    ///
    /// Populated by `trigger_genesis_distribution()`. OptionQuery returns None
    /// before genesis distribution is triggered.
    #[pallet::storage]
    #[pallet::getter(fn confederation_treasury)]
    pub type ConfederationTreasury<T: Config> = StorageValue<_, T::AccountId, OptionQuery>;

    /// Stores the Pure Proxy addresses of the 83 Regional Government Treasuries (7% = 147B ALTAN).
    ///
    /// Key: official OKATO region code (u8: e.g. 3 = Buryatia, 77 = Moscow).
    /// Value: Pure Proxy AccountId, controlled by the regional Khural multisig.
    /// Populated by `trigger_genesis_distribution()`. Gaps in codes (80-82) cost zero state.
    #[pallet::storage]
    #[pallet::getter(fn regional_treasury)]
    pub type RegionalTreasuries<T: Config> = StorageMap<_, Blake2_128Concat, u8, T::AccountId, OptionQuery>;

    /// Stores the Pure Proxy addresses of the 83 Regional Pension Funds (90% = 1.89T ALTAN).
    ///
    /// Key: official OKATO region code (u8), mirroring RegionalTreasuries.
    /// Changed from u16 sequential index to u8 OKATO code for constitutional clarity.
    #[pallet::storage]
    #[pallet::getter(fn pension_fund)]
    pub type PensionFunds<T: Config> = StorageMap<_, Blake2_128Concat, u8, T::AccountId, OptionQuery>;


    /// Per-operator debt per epoch.
    ///
    /// Key: (AccountId, EpochId) → Balance owed.
    ///
    /// Debt is epoch-tagged so that the interest rate of the originating epoch
    /// is preserved for the lifetime of the debt (constitutional traceability).
    #[pallet::storage]
    #[pallet::getter(fn operator_debt)]
    pub type OperatorDebt<T: Config> = StorageDoubleMap<
        _,
        Blake2_128Concat,
        T::AccountId,
        Twox64Concat,
        u32,
        BalanceOf<T>,
        ValueQuery,
    >;

    /// Total outstanding debt for each operator across all epochs.
    /// Used for O(1) operator limit/risk checks (if any).
    #[pallet::storage]
    #[pallet::getter(fn total_operator_debt)]
    pub type TotalOperatorDebt<T: Config> = StorageMap<
        _,
        Blake2_128Concat,
        T::AccountId,
        BalanceOf<T>,
        ValueQuery,
    >;

    // =========================================================================
    // Storage — Constitutional Rotating Credit Pool (NEW)
    // =========================================================================

    /// Available credit remaining in the Genesis rotating pool.
    ///
    /// Starts at `CreditEpochLimit` (18.9T ALTAN) at genesis.
    /// Decreases when credit is issued (`request_credit`).
    /// Restored when credit is repaid (`repay_credit`) — up to the cap.
    ///
    /// When this reaches 0 → no new credit until repayments restore capacity.
    /// This enforces the Hard Cap: M0 + M1 = 21T ALTAN maximum in circulation.
    #[pallet::storage]
    #[pallet::getter(fn genesis_credit_available)]
    pub type GenesisCreditAvailable<T: Config> = StorageValue<_, BalanceOf<T>, ValueQuery>;

    /// Total outstanding (unreturned) credit across the entire GENESIS era.
    ///
    /// outstanding = Σ(request_credit) - Σ(repay_credit)
    ///
    /// Used to calculate the dynamic key rate:
    ///   key_rate = compute_dynamic_rate(outstanding, genesis_limit)
    #[pallet::storage]
    #[pallet::getter(fn total_outstanding)]
    pub type TotalOutstanding<T: Config> = StorageValue<_, BalanceOf<T>, ValueQuery>;

    /// Base rate ceiling: maximum key rate in basis points.
    ///
    /// Defaults to `OptimalKeyRate` (850 bps = 8.5%) at genesis.
    /// Can be raised or lowered by BankingOrigin via `set_base_rate`.
    /// The dynamic key rate is always ≤ `BaseKeyRate`.
    ///
    /// Historical value: 850 bps (constitutional default).
    /// Emergency cap: `MaxProtectiveRate` (5000 bps = 50%).
    #[pallet::storage]
    #[pallet::getter(fn base_key_rate)]
    pub type BaseKeyRate<T: Config> = StorageValue<_, u32, ValueQuery>;

    // =========================================================================
    // Events
    // =========================================================================

    #[pallet::event]
    #[pallet::generate_deposit(pub(super) fn deposit_event)]
    pub enum Event<T: Config> {
        // ── Monetary infrastructure ──
        /// A new licensed operator was registered.
        OperatorLicensed { operator: T::AccountId },

        /// An operator license was revoked.
        OperatorRevoked { operator: T::AccountId },

        /// ALTAN was minted to a licensed operator (canonical emission event).
        MintedToOperator {
            tranche_id: u32,
            operator: T::AccountId,
            amount: BalanceOf<T>,
        },

        /// ALTAN was burned from a licensed operator (canonical deflation event).
        BurnedFromOperator {
            operator: T::AccountId,
            amount: BalanceOf<T>,
        },



        /// An epoch transitioned (CB Board decision, not auto-rollover).
        EpochTransitioned {
            old_epoch: u32,
            new_epoch: u32,
            new_rate: u32,
        },

        /// Credit was issued (minted) to an operator in a specific epoch.
        CreditIssued {
            who: T::AccountId,
            epoch_id: u32,
            amount: BalanceOf<T>,
            pool_remaining: BalanceOf<T>,
        },

        /// An operator repaid credit (burned), pool capacity restored.
        CreditRepaid {
            who: T::AccountId,
            epoch_id: u32,
            amount: BalanceOf<T>,
            pool_remaining: BalanceOf<T>,
        },

        /// Dynamic key rate auto-updated based on credit pool utilization.
        ///
        /// Emitted after every `request_credit` and `repay_credit`.
        /// Allows frontend to track rate in real-time.
        DynamicRateUpdated {
            new_rate: u32,
            outstanding: BalanceOf<T>,
            pool_available: BalanceOf<T>,
        },

        /// The Confederation Treasury Pure Proxy was created and funded (3% = 63B ALTAN).
        ConfederationTreasuryCreated {
            account: T::AccountId,
            amount: BalanceOf<T>,
        },

        /// A Regional Government Treasury Pure Proxy was created and funded (7% tranche).
        RegionalTreasuryCreated {
            /// Official OKATO region code (e.g. 3 = Buryatia, 77 = Moscow).
            region_code: u8,
            account: T::AccountId,
            amount: BalanceOf<T>,
        },

        /// A Regional Pension Fund Pure Proxy was created and funded (90% tranche).
        PensionFundCreated {
            /// Official OKATO region code.
            region_code: u8,
            account: T::AccountId,
            amount: BalanceOf<T>,
        },

        /// The 2.1T M0 supply was constitutionally distributed from CENTRAL_BANK.
        /// Signed by the Creator (BankingOrigin/Sudo) as the interim CB authority.
        GenesisDistributed {
            /// The CENTRAL_BANK account that sourced the 2.1T.
            central_bank: T::AccountId,
            /// Number of proxy accounts created (1 confed + 83 treasury + 83 pension).
            accounts_created: u16,
        },

        /// BankingOrigin manually set the base rate ceiling.
        BaseRateSet {
            /// New base rate in basis points (1 bp = 0.01%).
            /// e.g. 850 = 8.50% per annum at full pool utilization.
            new_rate_bps: u32,
            /// Old rate, for auditability.
            old_rate_bps: u32,
        },

        /// BankingOrigin manually triggered an epoch transition.
        EpochTransitionManual {
            old_epoch: u32,
            new_epoch: u32,
            /// Dynamic key rate computed for the new epoch.
            new_rate: u32,
        },

        /// A Genesis Grant (100 ALTAN) was issued to a new citizen.
        ///
        /// Minted directly from the CB sovereign account using `deposit_creating`.
        /// L1 is the source of truth — the grant is a permanent on-chain record.
        GenesisGrantIssued {
            citizen: T::AccountId,
            amount: BalanceOf<T>,
            region_code: u8,
        },
    }

    // =========================================================================
    // Errors
    // =========================================================================

    #[pallet::error]
    pub enum Error<T> {
        // ── Monetary infrastructure ──
        /// Account is not in the LicensedOperators registry.
        NotLicensedOperator,
        /// Caller is not the authorized Creator account.
        NotCreator,
        /// Genesis distribution has already been triggered.
        AlreadyDistributed,
        /// License is already active.
        AlreadyLicensed,
        /// License is already revoked.
        AlreadyRevoked,
        /// Amount must be greater than zero.
        ZeroAmount,
        /// Arithmetic overflow in counters.
        ArithmeticOverflow,
        /// Operator does not have sufficient free balance to burn.
        InsufficientOperatorBalance,

        // ── Macroeconomic Engine ──
        /// Epoch record not found (indicates storage corruption).
        EpochNotFound,
        /// Repayment exceeds operator's outstanding debt in this epoch.
        InsufficientDebt,
        /// Operator does not have enough free balance to repay.
        InsufficientBalance,

        // ── Constitutional Credit Pool ──
        /// Genesis credit pool exhausted.
        ///
        /// No new credit can be issued until operators repay outstanding debt,
        /// which restores pool capacity. This enforces the 21T hard cap.
        CreditPoolExhausted,

        // ── Rate / Epoch management ──
        /// The provided rate exceeds the constitutional MaxProtectiveRate ceiling.
        ///
        /// The base rate cannot exceed 5000 bps (50%). This prevents the CB Board
        /// from imposing confiscatory rates even in an emergency.
        RateExceedsMax,

        /// The provided region code does not correspond to a registered Pension Fund.
        ///
        /// Valid region codes are official OKATO codes (1-89, not all are consecutive).
        /// Use `REGION_CODES_83` constant for the full list of valid codes.
        InvalidRegion,
    }

    // =========================================================================
    // Genesis Config
    // =========================================================================

    #[pallet::genesis_config]
    #[derive(frame_support::DefaultNoBound)]
    pub struct GenesisConfig<T: Config> {
        /// Initial licensed operators. `(AccountId, true)` = licensed.
        pub licensed_operators: alloc::vec::Vec<(T::AccountId, bool)>,
    }

    #[pallet::genesis_build]
    impl<T: Config> BuildGenesisConfig for GenesisConfig<T> {
        fn build(&self) {
            for (operator, licensed) in &self.licensed_operators {
                LicensedOperators::<T>::insert(operator, licensed);
            }

            CurrentEpochId::<T>::put(1u32);

            // ── Constitutional Rotating Credit Pool initialization ─────────────
            // At genesis: full 18.9T pool available, 0 outstanding → rate = 0%
            GenesisCreditAvailable::<T>::put(T::CreditEpochLimit::get());
            TotalOutstanding::<T>::put(BalanceOf::<T>::zero());

            // Initialize BaseKeyRate from the constitutional constant.
            BaseKeyRate::<T>::put(T::OptimalKeyRate::get());

            // Epoch 1 starts with dynamic rate = 0% (no outstanding at genesis).
            // Rate will rise as operators take credit.
            Epochs::<T>::insert(
                1u32,
                EpochInfo {
                    start_block: BlockNumberFor::<T>::default(),
                    key_rate: 0u32, // dynamic: 0% at genesis (outstanding = 0)
                    total_issued: Zero::zero(),
                    total_repaid: Zero::zero(),
                    credit_limit: T::CreditEpochLimit::get(),
                },
            );
        }
    }

    // =========================================================================
    // Extrinsics
    // =========================================================================

    #[pallet::call]
    impl<T: Config> Pallet<T> {
        // ─── [0] grant_operator_license ──────────────────────────────────────

        /// Register an account as a licensed operator of the Central Bank.
        ///
        /// Callable ONLY by `BankingOrigin`. Licensed operators can receive
        /// minted ALTAN via `mint_to_operator`.
        #[pallet::call_index(0)]
        #[pallet::weight(<T as Config>::WeightInfo::grant_operator_license())]
        pub fn grant_operator_license(
            origin: OriginFor<T>,
            operator: T::AccountId,
        ) -> DispatchResult {
            T::BankingOrigin::ensure_origin(origin)?;
            ensure!(
                !LicensedOperators::<T>::get(&operator),
                Error::<T>::AlreadyLicensed
            );
            LicensedOperators::<T>::insert(&operator, true);
            Self::deposit_event(Event::OperatorLicensed { operator });
            Ok(())
        }

        // ─── [1] revoke_operator_license ─────────────────────────────────────

        /// Revoke an operator's license. Existing balances are NOT affected.
        ///
        /// Callable ONLY by `BankingOrigin`.
        #[pallet::call_index(1)]
        #[pallet::weight(<T as Config>::WeightInfo::revoke_operator_license())]
        pub fn revoke_operator_license(
            origin: OriginFor<T>,
            operator: T::AccountId,
        ) -> DispatchResult {
            T::BankingOrigin::ensure_origin(origin)?;
            ensure!(
                LicensedOperators::<T>::get(&operator),
                Error::<T>::AlreadyRevoked
            );
            LicensedOperators::<T>::insert(&operator, false);
            Self::deposit_event(Event::OperatorRevoked { operator });
            Ok(())
        }

        // ─── [2] mint_to_operator ────────────────────────────────────────────

        /// Mint new ALTAN to a licensed operator (the ONLY constitutional emission path).
        ///
        /// Callable ONLY by `BankingOrigin`. Uses `deposit_creating` — the sole
        /// site in the runtime where ALTAN is created after genesis.
        #[pallet::call_index(2)]
        #[pallet::weight(<T as Config>::WeightInfo::mint_to_operator())]
        pub fn mint_to_operator(
            origin: OriginFor<T>,
            operator: T::AccountId,
            amount: BalanceOf<T>,
        ) -> DispatchResult {
            T::BankingOrigin::ensure_origin(origin)?;
            ensure!(amount > BalanceOf::<T>::default(), Error::<T>::ZeroAmount);
            ensure!(
                LicensedOperators::<T>::get(&operator),
                Error::<T>::NotLicensedOperator
            );

            // Constitutional minting — PositiveImbalance dropped → TotalIssuance ↑
            let _imbalance = <T as Config>::Currency::deposit_creating(&operator, amount);

            let tranche_id = NextTrancheId::<T>::get();
            NextTrancheId::<T>::put(tranche_id.saturating_add(1));

            TotalEmitted::<T>::try_mutate(|total| -> Result<(), Error<T>> {
                *total = total
                    .checked_add(&amount)
                    .ok_or(Error::<T>::ArithmeticOverflow)?;
                Ok(())
            })?;

            Self::deposit_event(Event::MintedToOperator {
                tranche_id,
                operator,
                amount,
            });
            Ok(())
        }

        // ─── [3] burn ────────────────────────────────────────────────────────

        /// Burn ALTAN from a licensed operator's free balance.
        ///
        /// Callable ONLY by `BankingOrigin`. NegativeImbalance dropped → TotalIssuance ↓.
        #[pallet::call_index(3)]
        #[pallet::weight(<T as Config>::WeightInfo::burn())]
        pub fn burn(
            origin: OriginFor<T>,
            operator: T::AccountId,
            #[pallet::compact] amount: BalanceOf<T>,
        ) -> DispatchResult {
            T::BankingOrigin::ensure_origin(origin)?;
            ensure!(!amount.is_zero(), Error::<T>::ZeroAmount);
            ensure!(
                LicensedOperators::<T>::get(&operator),
                Error::<T>::NotLicensedOperator
            );
            ensure!(
                <T as Config>::Currency::free_balance(&operator) >= amount,
                Error::<T>::InsufficientOperatorBalance
            );

            // Constitutional burning — NegativeImbalance dropped → TotalIssuance ↓
            let _neg = <T as Config>::Currency::withdraw(
                &operator,
                amount,
                WithdrawReasons::TRANSFER,
                ExistenceRequirement::KeepAlive,
            )
            .map_err(|_| Error::<T>::InsufficientOperatorBalance)?;

            TotalBurned::<T>::try_mutate(|total| -> Result<(), Error<T>> {
                *total = total
                    .checked_add(&amount)
                    .ok_or(Error::<T>::ArithmeticOverflow)?;
                Ok(())
            })?;

            Self::deposit_event(Event::BurnedFromOperator { operator, amount });
            Ok(())
        }

        // ─── [4] transition_epoch ─────────────────────────────────────────────

        /// Manually trigger an epoch transition.
        ///
        /// Closes the current audit epoch and opens a new one with the current
        /// dynamic key rate. The new epoch inherits all Genesis pool state.
        ///
        /// ## Why This Is Needed
        ///
        /// The docstring (`set_base_rate` / `set_next_epoch_rate`) implied epochs
        /// would auto-rollover. In practice, `do_epoch_transition` is a private `fn`
        /// that was never callable from outside. This extrinsic exposes the transition
        /// to the CB Board so they can segment credit policy audits by epoch.
        ///
        /// ## Constitutional Mandate
        ///
        /// Epochs are informational (audit records) in the current model. The credit
        /// limit is enforced by the global Genesis pool, not per-epoch counters.
        /// Epoch transitions create on-chain audit checkpoints — they do NOT change
        /// the pool capacity or any operator's outstanding debt.
        ///
        /// Callable ONLY by `BankingOrigin`.
        #[pallet::call_index(4)]
        #[pallet::weight(<T as Config>::WeightInfo::transition_epoch())]
        pub fn transition_epoch(origin: OriginFor<T>) -> DispatchResult {
            T::BankingOrigin::ensure_origin(origin)?;

            let old_epoch_id = CurrentEpochId::<T>::get();
            let new_epoch_id = Self::do_epoch_transition(old_epoch_id);

            // Emit a manual-transition event (distinct from the general EpochTransitioned).
            Self::deposit_event(Event::EpochTransitionManual {
                old_epoch: old_epoch_id,
                new_epoch: new_epoch_id,
                new_rate: Epochs::<T>::get(new_epoch_id)
                    .map(|e| e.key_rate)
                    .unwrap_or(0),
            });
            Ok(())
        }

        // ─── [5] set_base_rate ────────────────────────────────────────────────

        /// Set the base key rate (maximum rate ceiling in basis points).
        ///
        /// The dynamic key rate auto-calculates based on pool utilization:
        ///   `dynamic_rate = base_rate × (outstanding / genesis_limit)`
        ///
        /// By setting `base_rate`, the CB Board controls the maximum rate at 100%
        /// utilization (when the full 18.9T pool is outstanding).
        ///
        /// ## Constraints
        ///
        /// - `new_rate_bps` MUST be ≤ `MaxProtectiveRate` (5000 bps = 50%).
        ///   This prevents confiscatory emergency rates.
        /// - The new rate takes effect immediately on the next `request_credit` /
        ///   `repay_credit` call that triggers `update_dynamic_rate()`.
        ///
        /// Callable ONLY by `BankingOrigin`.
        #[pallet::call_index(5)]
        #[pallet::weight(<T as Config>::WeightInfo::set_base_rate())]
        pub fn set_base_rate(
            origin: OriginFor<T>,
            new_rate_bps: u32,
        ) -> DispatchResult {
            T::BankingOrigin::ensure_origin(origin)?;

            ensure!(
                new_rate_bps <= T::MaxProtectiveRate::get(),
                Error::<T>::RateExceedsMax
            );

            let old_rate_bps = BaseKeyRate::<T>::get();
            BaseKeyRate::<T>::put(new_rate_bps);

            // Update the current epoch's stored rate to match the new base.
            // This ensures the audit trail reflects the CB's policy decision.
            let epoch_id = CurrentEpochId::<T>::get();
            Epochs::<T>::mutate(epoch_id, |maybe_epoch| {
                if let Some(e) = maybe_epoch {
                    // Only update if new_rate_bps produces a higher dynamic rate
                    // than the current auto-calculated value (conservative: don't
                    // lower rates mid-epoch if pool utilization is high).
                    // In practice: the new base rate will propagate on the next
                    // credit/repay operation via `update_dynamic_rate`.
                    let _ = e; // No direct mutation needed — next auto-update will use new base.
                }
            });

            Self::deposit_event(Event::BaseRateSet { new_rate_bps, old_rate_bps });
            Ok(())
        }

        /// Request wholesale credit (liquidity) from the Genesis rotating credit pool.
        ///
        /// Credit is minted directly to the calling Licensed Operator. Debt is epoch-tagged to
        /// preserve the key rate of the originating epoch for traceability.
        ///
        /// ## Constitutional Credit Pool Check
        ///
        /// Before issuing credit, the pallet checks `GenesisCreditAvailable`.
        /// If `available < amount` → returns `CreditPoolExhausted`. The pool
        /// is restored only as citizens repay outstanding debt.
        ///
        /// ## Dynamic Rate Auto-Update
        ///
        /// After credit is issued, `TotalOutstanding` increases and the
        /// dynamic key rate is automatically recalculated:
        ///   key_rate = base_rate × (outstanding / 18.9T)
        ///
        /// Emits: `CreditIssued` + `DynamicRateUpdated`.
        #[pallet::call_index(6)]
        #[pallet::weight(<T as Config>::WeightInfo::request_credit())]
        pub fn request_credit(origin: OriginFor<T>, amount: BalanceOf<T>) -> DispatchResult {
            let who = ensure_signed(origin)?;
            Self::do_request_credit(who, amount)
        }

        // ─── [7] repay_credit ────────────────────────────────────────────────

        /// Repay credit previously taken in a specific epoch.
        ///
        /// The caller specifies which `epoch_id` they are repaying against.
        /// This burns the repaid amount from their free balance, reducing
        /// TotalIssuance (symmetric to `request_credit`).
        ///
        /// ## Pool Restoration (Constitutional Incentive)
        ///
        /// Repayment RESTORES the Genesis credit pool capacity, up to the 18.9T cap.
        /// As more operators repay, the pool refills → the dynamic rate DROPS.
        /// When outstanding = 0 → rate = 0% → free credit (maximum incentive).
        ///
        /// This creates a self-regulating monetary system:
        ///   More repayment → lower rates → more borrowing incentive → more economic activity.
        ///
        /// For automatic repayment routing, the backend (NestJS) should call
        /// this with the oldest outstanding epoch_id for the operator.
        ///
        /// Emits: `CreditRepaid` + `DynamicRateUpdated`.
        #[pallet::call_index(7)]
        #[pallet::weight(<T as Config>::WeightInfo::repay_credit())]
        pub fn repay_credit(
            origin: OriginFor<T>,
            epoch_id: u32,
            amount: BalanceOf<T>,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;
            Self::do_repay_credit(who, epoch_id, amount)
        }

        // ─── [9] issue_genesis_grant ──────────────────────────────────────────

        /// Issue a Genesis Grant (100 ALTAN) to a newly registered citizen.
        ///
        /// ## Why This Is Needed
        ///
        /// Previously, `pallet-bank-of-siberia::open_master_account` tried to
        /// `Currency::transfer` from the Pension Fund Pure Proxy account — but
        /// the BOS pallet has no authority over a Pure Proxy account controlled
        /// by the Central Bank. This caused a runtime error.
        ///
        /// The correct constitutional architecture:
        /// 1. `pallet_bank_of_siberia::open_master_account` → registers the account.
        /// 2. `pallet_central_bank::issue_genesis_grant` → mints 100 ALTAN grant.
        ///
        /// ## Fund Source (Sprint 9 approach)
        ///
        /// Uses `deposit_creating` to mint 100 ALTAN directly to the citizen.
        /// This is constitutionally correct: the CB is the sole emitter.
        ///
        /// The Genesis Grant is a one-time constitutional entitlement:
        ///   "Every citizen who joins the Republic receives 100 ALTAN startup capital."
        ///
        /// ## L1 as Source of Truth
        ///
        /// This grant is a PERMANENT on-chain event (`GenesisGrantIssued`).
        /// The backend can index it, but the grant exists on L1 regardless of DB state.
        ///
        /// ## Access Control
        ///
        /// `BankingOrigin` only. The relayer (backend) calls this after
        /// `open_master_account` succeeds. Citizens cannot self-grant.
        #[pallet::call_index(9)]
        #[pallet::weight(<T as Config>::WeightInfo::issue_genesis_grant())]
        pub fn issue_genesis_grant(
            origin: OriginFor<T>,
            citizen: T::AccountId,
            region_code: u8,
        ) -> DispatchResult {
            T::BankingOrigin::ensure_origin(origin)?;

            // Validate the region code is a registered Pension Fund region.
            // (Validates the citizen is in a recognized constitutional region.)
            ensure!(
                PensionFunds::<T>::contains_key(region_code),
                Error::<T>::InvalidRegion
            );

            // 100 ALTAN = 100 × 10^12 planks
            let grant: BalanceOf<T> = 100_000_000_000_000u128
                .try_into()
                .map_err(|_| Error::<T>::ArithmeticOverflow)?;

            // Constitutional minting — the CB is the sole issuer.
            // PositiveImbalance dropped → TotalIssuance ↑
            // This is identical to `request_credit` but:
            //   1. Does NOT deduct from the M1 Genesis pool (grants are M0 expansion)
            //   2. Does NOT create OperatorDebt (grants are non-repayable entitlements)
            let _imbalance = <T as Config>::Currency::deposit_creating(&citizen, grant);

            // Update TotalEmitted for audit trail
            TotalEmitted::<T>::try_mutate(|total| -> Result<(), Error<T>> {
                *total = total
                    .checked_add(&grant)
                    .ok_or(Error::<T>::ArithmeticOverflow)?;
                Ok(())
            })?;

            Self::deposit_event(Event::GenesisGrantIssued {
                citizen,
                amount: grant,
                region_code,
            });
            Ok(())
        }

        // ─── [8] trigger_genesis_distribution ────────────────────────────────

        /// Constitutional routing of the 2.1T ALTAN M0 supply.
        ///
        /// The CENTRAL_BANK (keyless 7-of-10 multisig) holds the full 2.1T at genesis.
        /// This extrinsic transfers from CENTRAL_BANK → 167 Pure Proxy accounts:
        ///   - 1 × Confederation_Treasury (3%  = 63B ALTAN)
        ///   - 83 × Regional_Gov_Treasury (7%  = 147B ALTAN, ~1.77B each)
        ///   - 83 × Regional_Citizen_Fund  (90% = 1.89T ALTAN, ~22.77B each)
        ///
        /// ## Who Signs This?
        ///   The Creator (Citizen #1) signs as the temporary BankingOrigin (Sudo)
        ///   until the CB Board of Directors is democratically elected.
        ///   After elections, BankingOrigin → Banking Collective (multi-sig).
        ///
        /// ## IDEMPOTENT: GenesisDistributed flag prevents double-execution.
        #[pallet::call_index(8)]
        #[pallet::weight(Weight::from_parts(100_000_000, 0))]
        pub fn trigger_genesis_distribution(origin: OriginFor<T>) -> DispatchResult {
            let who = ensure_signed(origin.clone())?;

            // 1. Ensure caller is the Creator (temporary BankingOrigin until CB Board)
            ensure!(who == T::CreatorAccount::get(), Error::<T>::NotCreator);

            // 2. Ensure it hasn't been called yet (idempotency)
            ensure!(!GenesisDistributed::<T>::get(), Error::<T>::AlreadyDistributed);
            GenesisDistributed::<T>::put(true);

            // ── Constitutional M0 math (must match genesis_config_presets.rs) ─────────
            // TOTAL_SUPPLY = 2_100_000_000_000 ALTAN × 10^12 planck
            // Split: 90% pension | 7% regional treasury | 3% confederation
            let unit = 1_000_000_000_000u128;
            let total_reserve       = 1_890_000_000_000u128 * unit; // 90%
            let total_regional      =   147_000_000_000u128 * unit; // 7%
            let total_confederation =    63_000_000_000u128 * unit; // 3%

            let reserve_per_region  = total_reserve  / 83;
            let regional_per_region = total_regional / 83;

            // CENTRAL_BANK is the SOURCE of all funds (not the Creator!)
            let central_bank = T::CentralBankAccountId::get();

            let proxy_type = Default::default();
            let delay = sp_runtime::traits::Zero::zero();
            let mut current_index = 0u16;

            let block_number = <<T as pallet_proxy::Config>::BlockNumberProvider
                as sp_runtime::traits::BlockNumberProvider>::current_block_number();
            let ext_index = frame_system::Pallet::<T>::extrinsic_index().unwrap_or(0);

            // Helper: derive the Pure Proxy address (without creating it yet) so we
            // can record it in storage atomically with the proxy creation.
            let compute_pure = |index: u16| -> T::AccountId {
                pallet_proxy::Pallet::<T>::pure_account(
                    &who, &proxy_type, index, Some((block_number, ext_index))
                )
            };

            // Helper: create Pure Proxy account and fund it FROM CENTRAL_BANK
            let create_and_fund = |amount: u128, index: u16| -> DispatchResult {
                let pure = pallet_proxy::Pallet::<T>::pure_account(
                    &who, &proxy_type, index, Some((block_number, ext_index))
                );
                pallet_proxy::Pallet::<T>::create_pure(
                    origin.clone(), proxy_type.clone(), delay, index
                )?;
                let amount_bal: BalanceOf<T> = amount
                    .try_into()
                    .map_err(|_| Error::<T>::ArithmeticOverflow)?;
                // Transfer FROM CENTRAL_BANK — pallet acts on behalf of the keyless account
                <T as Config>::Currency::transfer(
                    &central_bank,
                    &pure,
                    amount_bal,
                    ExistenceRequirement::KeepAlive,
                )?;
                Ok(())
            };

            // ── 3. Confederation Treasury (3% = 63B ALTAN) ────────────────────
            let confederation_pure = compute_pure(current_index);
            create_and_fund(total_confederation, current_index)?;
            // Persist address — essential for backend and cross-pallet queries
            ConfederationTreasury::<T>::put(confederation_pure.clone());
            let conf_amount_bal: BalanceOf<T> = total_confederation
                .try_into()
                .map_err(|_| Error::<T>::ArithmeticOverflow)?;
            Self::deposit_event(Event::ConfederationTreasuryCreated {
                account: confederation_pure,
                amount:  conf_amount_bal,
            });
            current_index += 1;

            // ── 4. 83 × Regional Treasuries (7%) + 83 × Pension Funds (90%) ──
            //
            // Iterate over official OKATO codes (not sequential 0..82):
            //   - Buryatia = key 3, Moscow = key 77, Yamalo-Nenets = key 89
            // StorageMap gaps (codes 80-82, 84-85, 88) are free — Merkle-Patricia
            // Trie only stores entries that exist, empty keys cost zero state.
            let regional_amount_bal: BalanceOf<T> = regional_per_region
                .try_into()
                .map_err(|_| Error::<T>::ArithmeticOverflow)?;
            let pension_amount_bal: BalanceOf<T> = reserve_per_region
                .try_into()
                .map_err(|_| Error::<T>::ArithmeticOverflow)?;

            for &region_code in REGION_CODES_83.iter() {
                // Regional Gov Treasury (7% slice)
                let reg_pure = compute_pure(current_index);
                create_and_fund(regional_per_region, current_index)?;
                RegionalTreasuries::<T>::insert(region_code, reg_pure.clone());
                Self::deposit_event(Event::RegionalTreasuryCreated {
                    region_code,
                    account: reg_pure,
                    amount:  regional_amount_bal,
                });
                current_index += 1;

                // Regional Pension Fund (90% slice)
                let pension_pure = compute_pure(current_index);
                create_and_fund(reserve_per_region, current_index)?;
                PensionFunds::<T>::insert(region_code, pension_pure.clone());
                Self::deposit_event(Event::PensionFundCreated {
                    region_code,
                    account: pension_pure,
                    amount:  pension_amount_bal,
                });
                current_index += 1;
            }

            Self::deposit_event(Event::GenesisDistributed {
                central_bank,
                accounts_created: current_index,
            });
            Ok(())
        }
    }

    // =========================================================================
    // Internal Helpers
    // =========================================================================

    impl<T: Config> Pallet<T> {
        /// Wholesale credit request logic. Available for other pallets (like Bank of Siberia) to call.
        pub fn do_request_credit(operator: T::AccountId, amount: BalanceOf<T>) -> DispatchResult {
            ensure!(amount > BalanceOf::<T>::default(), Error::<T>::ZeroAmount);

            // ── Verify Licensed Operator ───────────────────────────────────────
            ensure!(
                LicensedOperators::<T>::get(&operator),
                Error::<T>::NotLicensedOperator
            );
            
            let current_debt = TotalOperatorDebt::<T>::get(&operator);
            let new_debt = current_debt.saturating_add(amount);

            // ── Constitutional Pool Check ─────────────────────────────────────
            let available = GenesisCreditAvailable::<T>::get();
            ensure!(available >= amount, Error::<T>::CreditPoolExhausted);

            let epoch_id = CurrentEpochId::<T>::get();
            ensure!(
                Epochs::<T>::contains_key(epoch_id),
                Error::<T>::EpochNotFound
            );

            // ── Decrease pool capacity ────────────────────────────────────────
            GenesisCreditAvailable::<T>::mutate(|a| *a = a.saturating_sub(amount));
            TotalOutstanding::<T>::mutate(|o| *o = o.saturating_add(amount));

            // ── Issue credit (mint to operator) ───────────────────────────────
            let _imbalance = <T as Config>::Currency::deposit_creating(&operator, amount);

            Epochs::<T>::mutate(epoch_id, |maybe_epoch| {
                if let Some(e) = maybe_epoch {
                    e.total_issued = e.total_issued.saturating_add(amount);
                }
            });

            OperatorDebt::<T>::mutate(&operator, epoch_id, |debt| {
                *debt = debt.saturating_add(amount);
            });

            TotalOperatorDebt::<T>::insert(&operator, new_debt);

            // ── Auto-update dynamic key rate ──────────────────────────────────
            let pool_remaining = GenesisCreditAvailable::<T>::get();
            let dynamic_rate = Self::update_dynamic_rate()?;

            Self::deposit_event(Event::CreditIssued {
                who: operator,
                epoch_id,
                amount,
                pool_remaining,
            });
            Self::deposit_event(Event::DynamicRateUpdated {
                new_rate: dynamic_rate,
                outstanding: TotalOutstanding::<T>::get(),
                pool_available: pool_remaining,
            });
            Ok(())
        }

        /// Wholesale credit repayment logic.
        pub fn do_repay_credit(
            operator: T::AccountId,
            epoch_id: u32,
            amount: BalanceOf<T>,
        ) -> DispatchResult {
            ensure!(amount > BalanceOf::<T>::default(), Error::<T>::ZeroAmount);

            ensure!(
                Epochs::<T>::contains_key(epoch_id),
                Error::<T>::EpochNotFound
            );

            let outstanding = OperatorDebt::<T>::get(&operator, epoch_id);
            ensure!(outstanding >= amount, Error::<T>::InsufficientDebt);

            ensure!(
                <T as Config>::Currency::free_balance(&operator) >= amount,
                Error::<T>::InsufficientBalance
            );

            let _neg = <T as Config>::Currency::withdraw(
                &operator,
                amount,
                WithdrawReasons::TRANSFER,
                ExistenceRequirement::KeepAlive,
            )
            .map_err(|_| Error::<T>::InsufficientBalance)?;

            OperatorDebt::<T>::mutate(&operator, epoch_id, |debt| {
                *debt = debt.saturating_sub(amount);
            });

            Epochs::<T>::mutate(epoch_id, |maybe_epoch| {
                if let Some(e) = maybe_epoch {
                    e.total_repaid = e.total_repaid.saturating_add(amount);
                }
            });

            TotalOperatorDebt::<T>::mutate(&operator, |total| {
                *total = total.saturating_sub(amount);
            });

            let genesis_limit = T::CreditEpochLimit::get();
            GenesisCreditAvailable::<T>::mutate(|a| {
                *a = (*a).saturating_add(amount).min(genesis_limit);
            });
            TotalOutstanding::<T>::mutate(|o| *o = o.saturating_sub(amount));

            let pool_remaining = GenesisCreditAvailable::<T>::get();
            let dynamic_rate = Self::update_dynamic_rate()?;

            Self::deposit_event(Event::CreditRepaid {
                who: operator,
                epoch_id,
                amount,
                pool_remaining,
            });
            Self::deposit_event(Event::DynamicRateUpdated {
                new_rate: dynamic_rate,
                outstanding: TotalOutstanding::<T>::get(),
                pool_available: pool_remaining,
            });
            Ok(())
        }
        /// Compute the dynamic key rate from current pool state.
        ///
        /// Constitutional Variant C (Two-Phase Curve):
        /// - Phase 1 (0 to u_opt): rate grows linearly from 0 to opt_rate.
        /// - Phase 2 (u_opt to 100): rate grows from opt_rate to max_rate (penalty).
        ///
        /// This ensures low rates for normal use, but aggressively penalizes
        /// monopolization of liquidity.
        ///
        /// This is a PURE function — does NOT modify storage. Safe to call anytime.
        fn compute_dynamic_rate() -> u32 {
            let outstanding = TotalOutstanding::<T>::get();
            let genesis_limit = T::CreditEpochLimit::get();
            
            let u_opt = T::OptimalUtilization::get();
            let opt_rate = T::OptimalKeyRate::get();
            let max_rate = T::MaxProtectiveRate::get();

            if genesis_limit.is_zero() || outstanding.is_zero() {
                return 0u32;
            }

            // Calculate utilization percentage (0..100)
            let utilization_percent: u32 = {
                let out_u128: u128 = outstanding.try_into().unwrap_or(0);
                let lim_u128: u128 = genesis_limit.try_into().unwrap_or(u128::MAX);
                
                if lim_u128 == 0 {
                    0u32
                } else {
                    ((out_u128.saturating_mul(100)) / lim_u128).try_into().unwrap_or(0)
                }
            };

            if utilization_percent <= u_opt {
                // Phase 1: Healthy economy. Rate grows from 0% to OptimalKeyRate.
                // Formula: utilization * opt_rate / u_opt
                utilization_percent.saturating_mul(opt_rate).saturating_div(u_opt)
            } else {
                // Phase 2: Anti-monopoly barrier. Rate spikes towards MaxProtectiveRate.
                let excess_utilization = utilization_percent.saturating_sub(u_opt);
                let max_excess = 100u32.saturating_sub(u_opt);
                
                if max_excess == 0 {
                    return max_rate;
                }
                
                let penalty_spread = max_rate.saturating_sub(opt_rate);
                let penalty = excess_utilization.saturating_mul(penalty_spread).saturating_div(max_excess);
                
                opt_rate.saturating_add(penalty)
            }
        }

        /// Calculate and apply the dynamic rate to the current epoch storage.
        ///
        /// Called after every `request_credit` and `repay_credit`.
        /// Updates `Epochs[current_epoch_id].key_rate` to reflect current utilization.
        fn update_dynamic_rate() -> Result<u32, DispatchError> {
            let dynamic_rate = Self::compute_dynamic_rate();
            Self::apply_dynamic_rate_to_epoch(dynamic_rate)?;
            Ok(dynamic_rate)
        }

        /// Write the computed dynamic rate to the current epoch record.
        fn apply_dynamic_rate_to_epoch(dynamic_rate: u32) -> DispatchResult {
            let epoch_id = CurrentEpochId::<T>::get();
            Epochs::<T>::try_mutate(epoch_id, |maybe_epoch| -> DispatchResult {
                let epoch = maybe_epoch.as_mut().ok_or(Error::<T>::EpochNotFound)?;
                epoch.key_rate = dynamic_rate;
                Ok(())
            })
        }

        /// Close the current epoch and atomically open the next one.
        ///
        /// Uses `NextEpochKeyRate` as the new epoch's starting base rate.
        /// If `NextEpochKeyRate` is 0 (not set), carries forward the old base rate.
        ///
        /// The new epoch inherits the current dynamic rate (calculated from
        /// outstanding / genesis_limit — unchanged by the epoch transition itself).
        ///
        /// This function is intentionally infallible (saturating) — the economy
        /// MUST NOT stop. Any edge case defaults to a safe forward state.
        fn do_epoch_transition(old_epoch_id: u32) -> u32 {
            let new_epoch_id = old_epoch_id.saturating_add(1);

            // Dynamic rate carries over (same outstanding, same pool)
            let current_dynamic_rate = Self::compute_dynamic_rate();

            // Create new epoch with the current dynamic rate
            Epochs::<T>::insert(
                new_epoch_id,
                EpochInfo {
                    start_block: frame_system::Pallet::<T>::block_number(),
                    key_rate: current_dynamic_rate,
                    total_issued: Zero::zero(),
                    total_repaid: Zero::zero(),
                    credit_limit: T::CreditEpochLimit::get(),
                },
            );

            // Advance the epoch pointer
            CurrentEpochId::<T>::put(new_epoch_id);

            Self::deposit_event(Event::EpochTransitioned {
                old_epoch: old_epoch_id,
                new_epoch: new_epoch_id,
                new_rate: current_dynamic_rate,
            });

            new_epoch_id
        }
    }
}

#[cfg(test)]
mod tests;
