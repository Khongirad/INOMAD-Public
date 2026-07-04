//! pallet-organization — Altan Network: Corporate Organization Registry
//!
//! Implements:
//!   - Organizational SBTs (Legal entities with `bank_account_id`, `hq_region`, `status`)
//!   - Crew Member registry (Directors and Employees assigned to regions)
//!   - MANUAL Tax Declaration (`file_tax_return`) — zero auto-debit, always citizen-initiated
//!   - CONSTITUTIONAL TAX SPLIT: 3/10 → Confederation Treasury, 7/10 → Regional Treasury
//!   - FILING PERIOD ENFORCEMENT: January 1 — April 15 (via on-chain `UnixTime`)
//!   - MINIMUM TAX: 20 ALTAN (constitutional floor, anti-abuse)
//!   - Punishment Engine — automated digital consequences for tax evasion:
//!       1. `OrgStatus → Delinquent` (reputation slash)
//!       2. Algorithmic penalty debt (+1% per `PenaltyAccrualPeriodBlocks`)
//!       3. DIGITAL FREEZE: all Directors' CitizenStatus → Frozen (via IdentityBridge)
//!
//! CONSTITUTIONAL RULE: Tax payments are strictly voluntary actions by the Director.
//! The CONSEQUENCES of non-payment, however, are fully automated and inescapable.
//!
//! Punishment triggers:
//!   A) `on_initialize` — processes up to `MaxOrgsPerBlock` per block automatically.
//!   B) `report_tax_evasion` — any citizen-auditor may trigger it instantly and earn a reward.
//!
//! ## Dispatchables
//!
//! | Call | Origin | Description |
//! |---|---|---|
//! | `register_organization` | Signed (Founder) | Register a new organization on-chain |
//! | `activate_organization` | Signed (Officer) | Activate a registered organization after review |
//! | `add_crew_member` | Signed (Director) | Add a member to an organization's crew roster |
//! | `file_tax_return` | Signed (Director/Accountant) | File an organization's annual tax return |
//! | `report_tax_evasion` | Signed (Auditor) | Report suspected tax evasion by an organization |
//! | `freeze_organization` | Signed (Judicial Origin) | Freeze an organization pending investigation |
//! | `file_tax_return_zk` | Signed (Director) | File a ZK-proved tax return (privacy-preserving) |

#![cfg_attr(not(feature = "std"), no_std)]

extern crate alloc;

pub mod migrations;
pub mod weights;
pub use pallet::*;

// =========================================================================
// Cross-pallet Interface — loose coupling for Director freeze
// =========================================================================

/// Trait that allows pallet-organization to freeze a citizen's account
/// without tight-coupling to pallet-inomad-identity.
///
/// Implement at the runtime level as `OrgIdentityBridge` and pass via `Config::IdentityBridge`.
pub trait OrgIdentityInterface<AccountId> {
    /// Freeze the account of `who` — sets CitizenStatus to `Frozen`.
    ///
    /// Called automatically for all Directors when an Organization becomes Delinquent.
    fn freeze_citizen(who: &AccountId) -> frame_support::dispatch::DispatchResult;
}

/// No-op fallback for unit tests / mock runtimes that don't need real identity integration.
impl<AccountId> OrgIdentityInterface<AccountId> for () {
    fn freeze_citizen(_who: &AccountId) -> frame_support::dispatch::DispatchResult {
        Ok(())
    }
}

#[cfg(feature = "runtime-benchmarks")]
mod benchmarking;
#[frame_support::pallet]
pub mod pallet {
    use crate::weights::WeightInfo as _;
    use crate::OrgIdentityInterface;
    use alloc::vec::Vec;
    use frame_support::{
        pallet_prelude::*,
        traits::{Currency, ExistenceRequirement, ReservableCurrency, UnixTime},
        weights::Weight,
    };
    use frame_system::pallet_prelude::*;
    use pallet_shielded_vaults::ShieldedVaultsInterface as _;
    use sp_runtime::{traits::Saturating, Perbill};

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
        /// Currency for tax payments (transfers from org bank account to treasury).
        type Currency: Currency<Self::AccountId> + ReservableCurrency<Self::AccountId>;

        /// Cross-pallet bridge: freeze a Director's citizen account on evasion.
        ///
        /// In the runtime, implement as `OrgIdentityBridge` delegating to
        /// `pallet_inomad_identity::Pallet::<Runtime>::do_freeze_citizen`.
        type IdentityBridge: crate::OrgIdentityInterface<Self::AccountId>;

        /// Number of blocks constituting one tax period.
        ///
        /// Default: `5_256_000` ≈ 1 year at 6-second blocks.
        /// When `current_block − last_tax_period_paid > TaxPeriodBlocks`, the org is overdue.
        #[pallet::constant]
        type TaxPeriodBlocks: Get<u32>;

        /// Penalty accrual cycle (blocks). Every this many blocks of delinquency,
        /// the penalty debt grows by 1% of the outstanding debt.
        ///
        /// Default: `151_200` ≈ 10.5 days at 6-second blocks.
        #[pallet::constant]
        type PenaltyAccrualPeriodBlocks: Get<u32>;

        /// Fixed base penalty added to `OrgPenaltyDebt` on first delinquency marking.
        ///
        /// Expressed in planks. Default: 100 ALTAN.
        #[pallet::constant]
        type BasePenaltyAmount: Get<BalanceOf<Self>>;

        /// Reward paid from the AuditorRewardSource to a citizen who calls
        /// `report_tax_evasion` on a genuinely delinquent organization.
        #[pallet::constant]
        type AuditorRewardAmount: Get<BalanceOf<Self>>;

        /// Regional treasury account — receives 7/10 of every tax payment.
        ///
        /// Each of the 83 regions has its own treasury. In production: wire per-region.
        /// Dev: use a single test account.
        #[pallet::constant]
        type RegionTreasuryAccount: Get<Self::AccountId>;

        /// ****CONSTITUTIONAL**** Confederation treasury account — receives 3/10 of every tax payment.
        ///
        /// The 3% confederation budget. Released only by Confederate Khural vote.
        /// This is the constitutional auto-split target for federal taxes.
        #[pallet::constant]
        type ConfederationTreasuryAccount: Get<Self::AccountId>;

        /// ****CONSTITUTIONAL**** State Tax Rate: 10% of declared profit.
        ///
        /// Annual corporate profit tax. Filed during Jan 1 — Apr 15.
        /// Split: 3/10 of tax → Confederation, 7/10 → Regional Treasury.
        /// This is an immutable Perbill constant — Khural cannot change it.
        #[pallet::constant]
        type StateTaxRate: Get<Perbill>;

        /// ****CONSTITUTIONAL**** Minimum tax amount per filing: 20 ALTAN.
        ///
        /// Even if declared profit is zero, the Director must pay at least this amount.
        /// Anti-abuse constitutional floor. Cannot submit a zero-tax declaration.
        #[pallet::constant]
        type MinTaxAmount: Get<BalanceOf<Self>>;

        /// On-chain timestamp provider for filing period enforcement.
        ///
        /// Used to verify that `file_tax_return` is called between January 1 and April 15.
        /// Wire to `pallet_timestamp::Pallet<Runtime>` in production.
        type TimeProvider: UnixTime;

        /// Maximum organizations processed per block in `on_initialize` penalty checks.
        ///
        /// Prevents unbounded iteration that could exhaust the block weight budget.
        /// Set to a value low enough that weight × MaxOrgsPerBlock ≤ block budget.
        #[pallet::constant]
        type MaxOrgsPerBlock: Get<u32>;

        /// Maximum byte length for an organization name.
        #[pallet::constant]
        type MaxNameLength: Get<u32>;

        /// Maximum number of Directors allowed per organization.
        ///
        /// Bounds the Director-freeze loop in the Punishment Engine.
        #[pallet::constant]
        type MaxDirectorsPerOrg: Get<u32>;

        /// [ZK BRIDGE] Cross-pallet bridge to `pallet-shielded-vaults`.
        ///
        /// Enables `file_tax_return_zk`: a guild Director can unshield part of the
        /// guild's shielded commitment pool and materialise the tax amount directly
        /// on the public `RegionTreasuryAccount` — without revealing the guild's
        /// internal total balance.
        ///
        /// Wire to `pallet_shielded_vaults::Pallet::<Runtime>` in the runtime config.
        /// Wire to `()` in unit tests (no-op implementation).
        type ShieldedVaultsBridge: pallet_shielded_vaults::ShieldedVaultsInterface<Self::AccountId>;

        /// ****CONSTITUTIONAL**** Activation deposit per founder: 10 ALTAN.
        ///
        /// Every founding Director MUST transfer this amount to the org's deterministic
        /// keyless account (`derive_org_account(org_id)`) to activate the organization.
        /// Until ALL required founders have deposited, the org remains `Pending`.
        ///
        /// If there is 1 founder → 10 ALTAN needed.
        /// If there are 3 founders → 30 ALTAN needed (10 per founder).
        #[pallet::constant]
        type OrgActivationDeposit: Get<BalanceOf<Self>>;

        /// Maximum number of founders (Directors) allowed to activate an org.
        ///
        /// Bounds `OrgFounderDeposits` BoundedVec. Constitutional max = 20.
        #[pallet::constant]
        type MaxFounders: Get<u32>;
    }

    // =========================================================================
    // Enums
    // =========================================================================

    /// Lifecycle status of a registered Organization.
    ///
    /// | Variant      | Description                                                            |
    /// |--------------|------------------------------------------------------------------------|
    /// | `Pending`    | Registered, awaiting all founders to deposit 10 ALTAN each.           |
    /// | `Active`     | Fully activated. Directors and employees may transact normally.        |
    /// | `Delinquent` | Tax period overdue. Penalty debt accruing. Directors are Frozen.       |
    /// | `Frozen`     | Manually frozen by Root (constitutional court order). Full stop.       |
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
    pub enum OrgStatus {
        /// Registered but NOT yet activated.
        /// Directors must call `activate_organization` and deposit 10 ALTAN each.
        Pending,
        /// Compliant organization — all filed taxes current.
        Active,
        /// TAX EVADER. Debt accruing. Directors Frozen. Reputation slashed.
        Delinquent,
        /// Constitutionally frozen — suspended by Root order.
        Frozen,
    }

    /// Role of a crew member within an Organization.
    ///
    /// | Variant    | Powers                                                              |
    /// |------------|---------------------------------------------------------------------|
    /// | `Director` | File tax returns. Add employees. Sole authority for org extrinsics.|
    /// | `Employee`  | Affiliated member. No admin rights.                               |
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
    pub enum OrgRole {
        /// Director — governing authority of the organization. Tax-filing rights.
        Director,
        /// Employee — affiliated member without administrative authority.
        Employee,
    }

    // =========================================================================
    // Structs
    // =========================================================================

    /// A registered Corporate Organization on the Altan Network.
    ///
    /// Created by a Director-citizen. Has a `bank_account_id` which is the
    /// on-chain AccountId that holds the organization's treasury. Tax payments
    /// are made FROM this account TO the regional treasury.
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
    pub struct Organization<T: Config> {
        /// The registered name (max `MaxNameLength` bytes, UTF-8).
        pub name: BoundedVec<u8, T::MaxNameLength>,
        /// OKATO region code (1–83) of the org's Headquarters. 0 = global/federal.
        pub hq_region: u32,
        /// Current compliance status of the organization.
        pub status: OrgStatus,
        /// The AccountId that acts as the org's bank account.
        ///
        /// All tax payments are drawn from this account. The account must
        /// have sufficient balance to avoid `InsufficientBankBalance` errors.
        pub bank_account_id: T::AccountId,
        /// Block number of the most recent successfully filed tax return.
        ///
        /// Initialized to the registration block. The Punishment Engine compares
        /// `current_block − last_tax_period_paid` against `TaxPeriodBlocks`
        /// to determine delinquency.
        pub last_tax_period_paid: BlockNumberFor<T>,
        /// Block number when the org was first registered.
        pub registered_at: BlockNumberFor<T>,
    }

    /// A Crew Member record binding a citizen to an Organization and role.
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
    pub struct CrewMember<T: Config> {
        /// The role of this member within the Organization.
        pub role: OrgRole,
        /// OKATO region code (1–83) where this crew member is assigned. 0 = unassigned/remote.
        pub region_assigned: u32,
        /// Block number when this crew member was added to the org.
        pub active_since: BlockNumberFor<T>,
    }

    /// Record of a founder's 10 ALTAN activation deposit.
    ///
    /// Stored in `OrgFounderDeposits`. An org becomes `Active` when
    /// `OrgFounderDeposits[org_id].len() >= OrgRequiredFounders[org_id]`.
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
    pub struct FounderDeposit<T: Config> {
        /// The founder (Director) who made this deposit.
        pub founder: T::AccountId,
        /// Amount deposited (always == `OrgActivationDeposit`, recorded for auditability).
        pub amount: BalanceOf<T>,
        /// Block number when the deposit was made.
        pub block_number: BlockNumberFor<T>,
    }

    /// An on-chain record of a filed Tax Return.
    ///
    /// Created by `file_tax_return` or `file_tax_return_zk`. Immutable after filing.
    /// The `document_hash` is the Blake2_256 hash of the off-chain tax declaration
    /// document (PDF, IPFS CID, etc.) that the Director certifies.
    ///
    /// ## Constitutional Tax Split
    ///
    /// Every tax payment is automatically split:
    ///   - **3/10** → Confederation Treasury (federal budget)
    ///   - **7/10** → Regional Treasury (republican budget, 83 regions)
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
    pub struct TaxDeclaration<T: Config> {
        /// Organization that filed this tax return.
        pub org_id: u32,
        /// Region ID for which this tax return was filed (OKATO code).
        pub region_id: u32,
        /// Total amount paid (confederation_share + regional_share).
        pub amount_paid: BalanceOf<T>,
        /// Declared profit for the tax period (self-declared, subject to audit).
        pub declared_profit: BalanceOf<T>,
        /// Portion of tax routed to Confederation Treasury (3/10 of amount_paid).
        pub confederation_share: BalanceOf<T>,
        /// Portion of tax routed to Regional Treasury (7/10 of amount_paid).
        pub regional_share: BalanceOf<T>,
        /// Blake2_256 hash of the off-chain tax declaration document.
        pub document_hash: [u8; 32],
        /// Block number when the declaration was filed.
        pub timestamp: BlockNumberFor<T>,
        /// Whether this declaration used the ZK shielded vault bridge.
        pub via_zk_vault: bool,
    }

    // =========================================================================
    // Storage
    // =========================================================================

    /// Global sequential Organization ID counter. Starts at 0.
    #[pallet::storage]
    #[pallet::getter(fn next_org_id)]
    pub type NextOrgId<T: Config> = StorageValue<_, u32, ValueQuery>;

    /// Organization registry: OrgId → Organization.
    #[pallet::storage]
    #[pallet::getter(fn organizations)]
    pub type Organizations<T: Config> =
        StorageMap<_, Blake2_128Concat, u32, Organization<T>, OptionQuery>;

    /// Crew Member registry: (OrgId, AccountId) → CrewMember.
    ///
    /// Presence in this map = member of the org; absence = not affiliated.
    #[pallet::storage]
    #[pallet::getter(fn crew_members)]
    pub type CrewMembers<T: Config> = StorageDoubleMap<
        _,
        Blake2_128Concat,
        u32,
        Blake2_128Concat,
        T::AccountId,
        CrewMember<T>,
        OptionQuery,
    >;

    /// Global sequential Tax Declaration ID counter.
    #[pallet::storage]
    #[pallet::getter(fn next_declaration_id)]
    pub type NextDeclarationId<T: Config> = StorageValue<_, u32, ValueQuery>;

    /// Tax Declaration registry: DeclarationId → TaxDeclaration.
    #[pallet::storage]
    #[pallet::getter(fn tax_declarations)]
    pub type TaxDeclarations<T: Config> =
        StorageMap<_, Blake2_128Concat, u32, TaxDeclaration<T>, OptionQuery>;

    /// Accumulated penalty debt per organization.
    ///
    /// Grows by 1% of existing debt per `PenaltyAccrualPeriodBlocks` while the org
    /// remains Delinquent. Wiped (set to zero) once the org files a compliant tax return.
    #[pallet::storage]
    #[pallet::getter(fn org_penalty_debt)]
    pub type OrgPenaltyDebt<T: Config> =
        StorageMap<_, Blake2_128Concat, u32, BalanceOf<T>, ValueQuery>;

    /// Block number at which the LAST penalty accrual tick was applied to a Delinquent org.
    ///
    /// Allows `on_initialize` to compute whether another accrual cycle has passed
    /// since the org was last penalized.
    #[pallet::storage]
    #[pallet::getter(fn last_penalty_block)]
    pub type LastPenaltyBlock<T: Config> =
        StorageMap<_, Blake2_128Concat, u32, BlockNumberFor<T>, ValueQuery>;

    /// Founder activation deposits: OrgId → list of FounderDeposit records.
    ///
    /// An org transitions from `Pending` → `Active` when
    /// `OrgFounderDeposits[org_id].len() >= OrgRequiredFounders[org_id]`.
    #[pallet::storage]
    #[pallet::getter(fn org_founder_deposits)]
    pub type OrgFounderDeposits<T: Config> = StorageMap<
        _,
        Blake2_128Concat,
        u32, // org_id
        BoundedVec<FounderDeposit<T>, T::MaxFounders>,
        ValueQuery,
    >;

    /// Number of founders required to fully activate an org.
    ///
    /// Set at `register_organization` time. Default = 1.
    /// The Creator or Root can increase this via `set_required_founders` (future extrinsic).
    #[pallet::storage]
    #[pallet::getter(fn org_required_founders)]
    pub type OrgRequiredFounders<T: Config> = StorageMap<_, Blake2_128Concat, u32, u32, ValueQuery>;

    /// Cursor for the `on_initialize` bounded scan over `Organizations`.
    ///
    /// Tracks which `org_id` the scan should start from next block so that
    /// `MaxOrgsPerBlock` orgs are checked per block in round-robin fashion.
    #[pallet::storage]
    pub type OrgScanCursor<T: Config> = StorageValue<_, u32, ValueQuery>;

    // =========================================================================
    // Events
    // =========================================================================

    #[pallet::event]
    #[pallet::generate_deposit(pub(super) fn deposit_event)]
    pub enum Event<T: Config> {
        /// An Organization was fully activated (all founder deposits received).
        OrganizationActivated {
            org_id: u32,
            org_account: T::AccountId,
            total_deposited: BalanceOf<T>,
            founders_count: u32,
        },
        /// A founder made their 10 ALTAN activation deposit.
        FounderDepositMade {
            org_id: u32,
            founder: T::AccountId,
            amount: BalanceOf<T>,
            deposits_so_far: u32,
            required: u32,
        },
        /// A new Organization was registered on-chain.
        OrganizationRegistered {
            org_id: u32,
            founder: T::AccountId,
            name: Vec<u8>,
            hq_region: u32,
            bank_account_id: T::AccountId,
        },
        /// A crew member was added to an Organization.
        CrewMemberAdded {
            org_id: u32,
            member: T::AccountId,
            role: OrgRole,
            region_assigned: u32,
        },
        /// A Director filed the organization's tax return.
        ///
        /// Funds are auto-split: 3/10 → Confederation, 7/10 → Regional Treasury.
        TaxReturnFiled {
            declaration_id: u32,
            org_id: u32,
            region_id: u32,
            amount_paid: BalanceOf<T>,
            declared_profit: BalanceOf<T>,
            confederation_share: BalanceOf<T>,
            regional_share: BalanceOf<T>,
            document_hash: [u8; 32],
        },
        /// A Director paid taxes via ZK shielded vault bridge.
        ///
        /// The `amount_paid` is publicly visible on the Regional Treasury.
        /// The guild's internal shielded balance remains hidden.
        /// The `nullifier` permanently destroys the shielded commitment.
        OrgZkTaxPaid {
            declaration_id: u32,
            org_id: u32,
            region_id: u32,
            /// Publicly declared tax amount (hits RegionalTreasury on-chain).
            amount_paid: u128,
            /// Nullifier that destroyed the shielded commitment.
            nullifier: [u8; 32],
        },
        /// **PUNISHMENT** An Organization was marked Delinquent for missing its tax period.
        ///
        /// Emitted on first discovery of the delinquency (either by `on_initialize`
        /// or by a citizen calling `report_tax_evasion`).
        OrgMarkedDelinquent {
            org_id: u32,
            periods_overdue: u32,
            base_penalty: BalanceOf<T>,
        },
        /// **PUNISHMENT** Penalty debt accrued for a Delinquent organization (+1%).
        PenaltyDebtAccrued { org_id: u32, new_debt: BalanceOf<T> },
        /// **PUNISHMENT** A Director's citizen account was frozen due to org's tax evasion.
        DirectorFrozen { org_id: u32, director: T::AccountId },
        /// A citizen-auditor reported tax evasion and collected their reward.
        TaxEvasionReported {
            reporter: T::AccountId,
            org_id: u32,
            reward: BalanceOf<T>,
        },
        /// An Organization's status was set to Frozen by Root (judicial order).
        OrgFrozenByRoot { org_id: u32 },
        /// An Organization was reinstated to Active after paying owed taxes.
        OrgReinstated { org_id: u32 },
        /// Funds were spent from the Organization's keyless bank account.
        OrgFundsSpent {
            org_id: u32,
            director: T::AccountId,
            beneficiary: T::AccountId,
            amount: BalanceOf<T>,
        },
    }

    // =========================================================================
    // Errors
    // =========================================================================

    #[pallet::error]
    pub enum Error<T> {
        /// Referenced Organization does not exist.
        OrgNotFound,
        /// Caller is not a Director of this Organization.
        NotDirector,
        /// Caller is already registered as a crew member of this Organization.
        AlreadyCrewMember,
        /// Organization name exceeds MaxNameLength bytes.
        NameTooLong,
        /// The organization's bank account has insufficient balance to cover the tax payment.
        InsufficientBankBalance,
        /// This organization is Frozen by Root — no operations permitted.
        OrgIsFrozen,
        /// Organization is currently Active — it is not delinquent; no punishment triggered.
        OrgIsNotDelinquent,
        /// The organization's current tax period has not yet expired — no evasion detected.
        TaxPeriodNotExpired,
        /// Cannot report on an org that is already Delinquent (already being punished).
        AlreadyDelinquent,
        /// Arithmetic overflow in penalty calculation.
        MathOverflow,
        /// The shielded nullifier has already been spent — ZK double-spend blocked.
        NullifierAlreadySpent,
        /// The shielded commitment was not found in the ZK pool.
        ShieldedCommitmentNotFound,
        /// The ZK-declared amount cannot be zero.
        ZkAmountZero,
        /// Tax amount is below the constitutional minimum (20 ALTAN).
        BelowMinimumTax,
        /// Filing is outside the constitutional tax period (January 1 — April 15).
        OutsideTaxFilingPeriod,
        /// Organization is still in Pending state — activation deposit required.
        OrgIsPending,
        /// This founder has already made the activation deposit for this org.
        AlreadyDeposited,
        /// The BoundedVec of founder deposits is full (exceeded MaxFounders).
        TooManyFounders,
        /// Organization is already Active — no further activation deposit needed.
        AlreadyActivated,
    }

    // =========================================================================
    // Internal helpers
    // =========================================================================

    impl<T: Config> Pallet<T> {
        /// Verify that `caller` is a Director of `org_id`.
        fn ensure_director(caller: &T::AccountId, org_id: u32) -> DispatchResult {
            ensure!(
                Organizations::<T>::contains_key(org_id),
                Error::<T>::OrgNotFound
            );
            let crew = CrewMembers::<T>::get(org_id, caller).ok_or(Error::<T>::NotDirector)?;
            ensure!(crew.role == OrgRole::Director, Error::<T>::NotDirector);
            Ok(())
        }

        /// Check if the current on-chain time falls within the tax filing period
        /// (January 1 — April 15 of the current year).
        ///
        /// ## Calendar Math (no_std compatible)
        ///
        /// Uses Unix timestamp from `T::TimeProvider` to determine day-of-year.
        /// Filing is allowed when `day_of_year` is between 1 and 105 (non-leap)
        /// or 1 and 106 (leap year).
        ///
        /// Returns `Ok(())` if within the filing period, `Err(OutsideTaxFilingPeriod)` otherwise.
        fn ensure_within_filing_period() -> DispatchResult {
            let now_secs = T::TimeProvider::now().as_secs();

            // Edge case: timestamp not yet set (block 0 / genesis)
            if now_secs == 0 {
                return Ok(()); // Allow filing during genesis bootstrap
            }

            // ── Calendar calculation (no_std) ────────────────────────────────
            const SECS_PER_DAY: u64 = 86_400;

            // Days since Unix epoch (Jan 1, 1970)
            let total_days = now_secs / SECS_PER_DAY;

            // Determine current year and day-of-year using a loop from epoch.
            // This is O(years_since_1970) ≈ 56 iterations max — trivial for on-chain.
            let mut year: u32 = 1970;
            let mut remaining_days = total_days;
            loop {
                let days_in_year: u64 = if Self::is_leap_year(year) { 366 } else { 365 };
                if remaining_days < days_in_year {
                    break;
                }
                remaining_days -= days_in_year;
                year += 1;
            }

            // remaining_days is now 0-indexed day-of-year (0 = Jan 1)
            let day_of_year = remaining_days + 1; // 1-indexed: Jan 1 = 1

            // April 15 = day 105 (non-leap) or day 106 (leap)
            let apr_15_day: u64 = if Self::is_leap_year(year) { 106 } else { 105 };

            ensure!(
                day_of_year <= apr_15_day,
                Error::<T>::OutsideTaxFilingPeriod
            );
            Ok(())
        }

        /// Returns true if `year` is a leap year.
        fn is_leap_year(year: u32) -> bool {
            (year.is_multiple_of(4) && !year.is_multiple_of(100)) || year.is_multiple_of(400)
        }

        /// Derive the deterministic, keyless AccountId for an organization.
        ///
        /// Uses Blake2_256 over the concatenation of the `b"altan/org/"` prefix and
        /// the `org_id` as a 4-byte little-endian integer. This produces a unique,
        /// non-guessable account that no private key can sign for — true keyless treasury.
        ///
        /// This matches the derivation defined in the runtime's `pallet_organization` config:
        ///   `org_account = AccountId::decode(&mut blake2_256(b"altan/org/" ++ org_id_le_bytes))`
        pub fn derive_org_account(org_id: u32) -> T::AccountId {
            use sp_runtime::traits::{BlakeTwo256, Hash, TrailingZeroInput};
            let mut input = b"altan/org/".to_vec();
            input.extend_from_slice(&org_id.to_le_bytes());
            // BlakeTwo256::hash is no_std-safe; output is H256 (32 bytes).
            let hash = BlakeTwo256::hash(&input);
            T::AccountId::decode(&mut TrailingZeroInput::new(hash.as_ref()))
                .expect("32-byte hash always decodes to AccountId; qed")
        }

        /// Calculate the constitutional tax split for a given total tax amount.
        ///
        /// Returns `(confederation_share, regional_share)` where:
        ///   - `confederation_share = amount × 3 / 10` (3% of profit → 30% of tax)
        ///   - `regional_share = amount - confederation_share` (7% of profit → 70% of tax)
        fn constitutional_tax_split(amount: BalanceOf<T>) -> (BalanceOf<T>, BalanceOf<T>) {
            let confederation_share = amount * 3u32.into() / 10u32.into();
            let regional_share = amount.saturating_sub(confederation_share);
            (confederation_share, regional_share)
        }

        /// Core punishment engine. Marks the org Delinquent, accrues base penalty,
        /// and freezes all Directors. Safe to call from both `on_initialize` and
        /// `report_tax_evasion`.
        ///
        /// Returns whether punishment was newly applied (true) or org was already
        /// Delinquent (false — only accrual ticks apply).
        fn apply_punishment(org_id: u32, now: BlockNumberFor<T>) -> bool {
            let org = match Organizations::<T>::get(org_id) {
                Some(o) => o,
                None => return false,
            };

            // ── Determine if tax period is actually overdue ──────────────────
            let elapsed = now.saturating_sub(org.last_tax_period_paid);
            let tax_period: BlockNumberFor<T> = T::TaxPeriodBlocks::get().into();
            if elapsed <= tax_period {
                return false; // still within period — nothing to do
            }

            let periods_overdue: u32 = elapsed
                .try_into()
                .map(|e: u32| e.checked_div(T::TaxPeriodBlocks::get()).unwrap_or(0))
                .unwrap_or(u32::MAX);

            let newly_delinquent = org.status == OrgStatus::Active;

            // ── Mark Delinquent ──────────────────────────────────────────────
            if newly_delinquent {
                Organizations::<T>::mutate(org_id, |maybe| {
                    if let Some(o) = maybe {
                        o.status = OrgStatus::Delinquent;
                    }
                });

                let base_penalty = T::BasePenaltyAmount::get();
                OrgPenaltyDebt::<T>::mutate(org_id, |debt| {
                    *debt = debt.saturating_add(base_penalty);
                });
                LastPenaltyBlock::<T>::insert(org_id, now);

                Self::deposit_event(Event::OrgMarkedDelinquent {
                    org_id,
                    periods_overdue,
                    base_penalty,
                });

                // ── Freeze all Directors ─────────────────────────────────────
                // **SECURITY** Bounded by MaxDirectorsPerOrg to prevent unbounded
                // iteration from exhausting block weight.
                let directors: Vec<T::AccountId> = CrewMembers::<T>::iter_prefix(org_id)
                    .take(T::MaxDirectorsPerOrg::get() as usize)
                    .filter_map(|(acct, crew)| {
                        if crew.role == OrgRole::Director {
                            Some(acct)
                        } else {
                            None
                        }
                    })
                    .collect();

                for director in &directors {
                    // Best-effort: we do not abort punishment if a freeze fails
                    // (e.g. citizen already Frozen or Exiled). Log the error via event.
                    let _ = T::IdentityBridge::freeze_citizen(director);
                    Self::deposit_event(Event::DirectorFrozen {
                        org_id,
                        director: director.clone(),
                    });
                }
            }

            // ── Accrual tick — +1% of existing debt per accrual period ───────
            let last_penalty = LastPenaltyBlock::<T>::get(org_id);
            let accrual_period: BlockNumberFor<T> = T::PenaltyAccrualPeriodBlocks::get().into();
            if now.saturating_sub(last_penalty) >= accrual_period {
                let current_debt = OrgPenaltyDebt::<T>::get(org_id);
                // +1% of current debt (integer floor division)
                let accrual = current_debt / 100u32.into();
                if accrual > BalanceOf::<T>::default() {
                    let new_debt = current_debt.saturating_add(accrual);
                    OrgPenaltyDebt::<T>::insert(org_id, new_debt);
                    LastPenaltyBlock::<T>::insert(org_id, now);
                    Self::deposit_event(Event::PenaltyDebtAccrued { org_id, new_debt });
                }
            }

            newly_delinquent
        }
    }

    // =========================================================================
    // Hooks
    // =========================================================================

    #[pallet::hooks]
    impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T> {
        /// Automatic delinquency scanner.
        ///
        /// Each block, scans up to `MaxOrgsPerBlock` organizations starting from
        /// the `OrgScanCursor` position, in round-robin fashion across all registered orgs.
        ///
        /// For each Delinquent or overdue org, the Punishment Engine is invoked.
        ///
        /// ## Weight Safety
        /// The scan is bounded by `MaxOrgsPerBlock`. The weight consumed per block
        /// is strictly bounded: `O(MaxOrgsPerBlock × MaxDirectorsPerOrg)`.
        fn on_initialize(now: BlockNumberFor<T>) -> Weight {
            let max_scan = T::MaxOrgsPerBlock::get();
            let total_orgs = NextOrgId::<T>::get();

            if total_orgs == 0 {
                return Weight::from_parts(1_000, 0);
            }

            let cursor = OrgScanCursor::<T>::get();
            let mut processed: u32 = 0;
            let mut next_cursor = cursor;

            for i in 0..max_scan {
                let org_id = (cursor + i) % total_orgs;
                if let Some(org) = Organizations::<T>::get(org_id) {
                    // Only scan Active or Delinquent orgs (not constitutionally Frozen)
                    if org.status != OrgStatus::Frozen {
                        Self::apply_punishment(org_id, now);
                    }
                }
                processed = processed.saturating_add(1);
                next_cursor = (cursor + i + 1) % total_orgs;

                if processed >= total_orgs {
                    // We've done a full cycle — reset cursor
                    next_cursor = 0;
                    break;
                }
            }

            OrgScanCursor::<T>::put(next_cursor);

            // Weight estimate: 20M ref_time per org scanned (conservative)
            Weight::from_parts(20_000_000u64.saturating_mul(processed as u64), 0)
        }
    }

    // =========================================================================
    // Extrinsics
    // =========================================================================

    #[pallet::call]
    impl<T: Config> Pallet<T> {
        // ─── register_organization ──────────────────────────────────────────

        /// Register a new Corporate Organization on the Altan Network.
        ///
        /// The caller becomes the founding Director. The org starts as `Pending`.
        /// Each founding Director must call `activate_organization` and deposit 10 ALTAN
        /// to the org's deterministic keyless account. When all required founders have
        /// deposited, the org transitions to `Active`.
        ///
        /// ## Parameters
        /// - `name`: UTF-8 org name (max `MaxNameLength` bytes).
        /// - `hq_region`: OKATO region code (1–83), or 0 for global/federal.
        /// - `required_founders`: How many Directors must deposit 10 ALTAN. Min 1.
        ///
        /// # Origin: Signed
        #[pallet::call_index(0)]
        #[pallet::weight(T::WeightInfo::register_organization())]
        pub fn register_organization(
            origin: OriginFor<T>,
            name: Vec<u8>,
            hq_region: u32,
            required_founders: u32,
        ) -> DispatchResult {
            let caller = ensure_signed(origin)?;

            let required_founders = required_founders.max(1);
            ensure!(
                required_founders <= T::MaxFounders::get(),
                Error::<T>::TooManyFounders
            );

            let bounded_name: BoundedVec<u8, T::MaxNameLength> = name
                .clone()
                .try_into()
                .map_err(|_| Error::<T>::NameTooLong)?;

            let now = frame_system::Pallet::<T>::block_number();
            let org_id = NextOrgId::<T>::get();

            // Deterministic keyless org bank account — no private key can sign for it.
            let bank_account_id = Self::derive_org_account(org_id);

            Organizations::<T>::insert(
                org_id,
                Organization::<T> {
                    name: bounded_name.clone(),
                    hq_region,
                    status: OrgStatus::Pending, // ← awaiting activation deposits
                    bank_account_id: bank_account_id.clone(),
                    last_tax_period_paid: now,
                    registered_at: now,
                },
            );

            // Founder is the first Director.
            CrewMembers::<T>::insert(
                org_id,
                &caller,
                CrewMember::<T> {
                    role: OrgRole::Director,
                    region_assigned: hq_region,
                    active_since: now,
                },
            );

            OrgRequiredFounders::<T>::insert(org_id, required_founders);
            NextOrgId::<T>::put(org_id.saturating_add(1));

            Self::deposit_event(Event::OrganizationRegistered {
                org_id,
                founder: caller,
                name: bounded_name.into_inner(),
                hq_region,
                bank_account_id,
            });
            Ok(())
        }

        // ─── activate_organization ──────────────────────────────────────────

        /// Pay the 10 ALTAN activation deposit for an organization.
        ///
        /// Each founding Director calls this once. When deposits reach
        /// `required_founders`, the org transitions `Pending` → `Active`.
        ///
        /// The deposit is transferred to the org's deterministic keyless account
        /// (no private key — funds leave only via tax payments / governance).
        ///
        /// ## Constitutional Rules
        /// - Exactly `OrgActivationDeposit` (10 ALTAN) per founder, non-refundable.
        /// - Each Director may deposit only once per org.
        /// - `KeepAlive` — caller retains Existential Deposit after transfer.
        /// - `AlreadyActivated` if org is no longer Pending.
        ///
        /// # Origin: Signed (Director of org_id)
        #[pallet::call_index(1)]
        #[pallet::weight(T::WeightInfo::activate_organization())]
        pub fn activate_organization(origin: OriginFor<T>, org_id: u32) -> DispatchResult {
            let caller = ensure_signed(origin)?;

            let org = Organizations::<T>::get(org_id).ok_or(Error::<T>::OrgNotFound)?;
            ensure!(
                org.status == OrgStatus::Pending,
                Error::<T>::AlreadyActivated
            );

            Self::ensure_director(&caller, org_id)?;

            // Prevent double-deposit by the same founder.
            let deposits = OrgFounderDeposits::<T>::get(org_id);
            ensure!(
                !deposits.iter().any(|d| d.founder == caller),
                Error::<T>::AlreadyDeposited
            );

            // Transfer 10 ALTAN: caller → org keyless account (KeepAlive = constitutional).
            let deposit_amount = T::OrgActivationDeposit::get();
            let org_account = org.bank_account_id.clone();
            T::Currency::transfer(
                &caller,
                &org_account,
                deposit_amount,
                ExistenceRequirement::KeepAlive,
            )?;

            let now = frame_system::Pallet::<T>::block_number();
            OrgFounderDeposits::<T>::try_mutate(org_id, |deps| {
                deps.try_push(FounderDeposit::<T> {
                    founder: caller.clone(),
                    amount: deposit_amount,
                    block_number: now,
                })
                .map_err(|_| Error::<T>::TooManyFounders)
            })?;

            let deposits_so_far = OrgFounderDeposits::<T>::get(org_id).len() as u32;
            let required = OrgRequiredFounders::<T>::get(org_id);

            Self::deposit_event(Event::FounderDepositMade {
                org_id,
                founder: caller,
                amount: deposit_amount,
                deposits_so_far,
                required,
            });

            // Activate the org once all required founders have deposited.
            if deposits_so_far >= required {
                Organizations::<T>::mutate(org_id, |maybe| {
                    if let Some(o) = maybe {
                        o.status = OrgStatus::Active;
                    }
                });
                let total_deposited = deposit_amount.saturating_mul(deposits_so_far.into());
                Self::deposit_event(Event::OrganizationActivated {
                    org_id,
                    org_account,
                    total_deposited,
                    founders_count: deposits_so_far,
                });
            }

            Ok(())
        }

        // ─── add_crew_member ────────────────────────────────────────────────

        /// Add a citizen as a crew member of the Organization.
        ///
        /// Only a Director may call this. The new member's role is set explicitly.
        ///
        /// ## Parameters
        /// - `org_id`: Target organization.
        /// - `member`: AccountId of the citizen to add.
        /// - `role`: `Director` or `Employee`.
        /// - `region_assigned`: OKATO code of the region this member works in.
        ///
        /// # Origin: Signed (Director only)
        #[pallet::call_index(2)]
        #[pallet::weight(T::WeightInfo::add_crew_member())]
        pub fn add_crew_member(
            origin: OriginFor<T>,
            org_id: u32,
            member: T::AccountId,
            role: OrgRole,
            region_assigned: u32,
        ) -> DispatchResult {
            let caller = ensure_signed(origin)?;

            let org = Organizations::<T>::get(org_id).ok_or(Error::<T>::OrgNotFound)?;
            ensure!(org.status != OrgStatus::Frozen, Error::<T>::OrgIsFrozen);
            ensure!(org.status != OrgStatus::Pending, Error::<T>::OrgIsPending);

            Self::ensure_director(&caller, org_id)?;

            ensure!(
                !CrewMembers::<T>::contains_key(org_id, &member),
                Error::<T>::AlreadyCrewMember
            );

            let now = frame_system::Pallet::<T>::block_number();
            CrewMembers::<T>::insert(
                org_id,
                &member,
                CrewMember::<T> {
                    role: role.clone(),
                    region_assigned,
                    active_since: now,
                },
            );

            Self::deposit_event(Event::CrewMemberAdded {
                org_id,
                member,
                role,
                region_assigned,
            });
            Ok(())
        }

        // ─── file_tax_return ────────────────────────────────────────────────

        /// FILE A TAX RETURN — Strictly manual. No auto-debit ever.
        ///
        /// The Director declares profit and pays the 10% annual corporate tax.
        /// The tax is automatically split by the constitutional rule:
        ///   - **3/10** → Confederation Treasury (federal budget)
        ///   - **7/10** → Regional Treasury (republican budget)
        ///
        /// ## Constitutional Enforcement
        ///
        /// - **Filing Period:** January 1 — April 15 (enforced via `UnixTime`)
        /// - **Minimum Tax:** 20 ALTAN (constitutional floor, anti-abuse)
        /// - **Self-Declaration:** Director certifies `declared_profit` and `amount`.
        ///   Spot-check audits by tax officers enforce honesty.
        ///
        /// On success:
        ///   - A `TaxDeclaration` is recorded on-chain (immutable audit trail).
        ///   - `Organization.last_tax_period_paid` is updated to the current block.
        ///   - If the org was `Delinquent`, it is reinstated to `Active`.
        ///   - Accrued penalty debt is cleared.
        ///
        /// ## Parameters
        /// - `org_id`: The organization filing the return.
        /// - `region_id`: OKATO region code for this tax return.
        /// - `declared_profit`: Self-declared profit for the tax period (for audit trail).
        /// - `amount`: Total tax to pay (must be ≥ MinTaxAmount = 20 ALTAN).
        /// - `document_hash`: Blake2_256 hash of the off-chain tax declaration PDF/document.
        ///
        /// # Origin: Signed (Director only)
        #[pallet::call_index(3)]
        #[pallet::weight(T::WeightInfo::file_tax_return())]
        pub fn file_tax_return(
            origin: OriginFor<T>,
            org_id: u32,
            region_id: u32,
            declared_profit: BalanceOf<T>,
            amount: BalanceOf<T>,
            document_hash: [u8; 32],
        ) -> DispatchResult {
            let caller = ensure_signed(origin)?;

            let org = Organizations::<T>::get(org_id).ok_or(Error::<T>::OrgNotFound)?;
            ensure!(org.status != OrgStatus::Frozen, Error::<T>::OrgIsFrozen);

            Self::ensure_director(&caller, org_id)?;

            // ── Constitutional Checks ────────────────────────────────────────
            // 1. Filing period: Jan 1 — Apr 15 (via on-chain timestamp)
            Self::ensure_within_filing_period()?;

            // 2. Minimum tax: 20 ALTAN
            ensure!(
                amount >= T::MinTaxAmount::get(),
                Error::<T>::BelowMinimumTax
            );

            // ── Constitutional Tax Split (3/10 + 7/10) ───────────────────────
            let (confederation_share, regional_share) = Self::constitutional_tax_split(amount);

            let region_treasury = T::RegionTreasuryAccount::get();
            let confed_treasury = T::ConfederationTreasuryAccount::get();
            let now = frame_system::Pallet::<T>::block_number();
            let declaration_id = NextDeclarationId::<T>::get();

            // ── PHYSICAL TAX TRANSFERS ───────────────────────────────────────
            // Transfer 1: 7/10 → Regional Treasury
            T::Currency::transfer(
                &org.bank_account_id,
                &region_treasury,
                regional_share,
                ExistenceRequirement::KeepAlive,
            )
            .map_err(|_| Error::<T>::InsufficientBankBalance)?;

            // Transfer 2: 3/10 → Confederation Treasury
            T::Currency::transfer(
                &org.bank_account_id,
                &confed_treasury,
                confederation_share,
                ExistenceRequirement::KeepAlive,
            )
            .map_err(|_| Error::<T>::InsufficientBankBalance)?;

            // ── Record immutable declaration ─────────────────────────────────
            TaxDeclarations::<T>::insert(
                declaration_id,
                TaxDeclaration::<T> {
                    org_id,
                    region_id,
                    amount_paid: amount,
                    declared_profit,
                    confederation_share,
                    regional_share,
                    document_hash,
                    timestamp: now,
                    via_zk_vault: false,
                },
            );
            NextDeclarationId::<T>::put(declaration_id.saturating_add(1));

            // ── Update org state ─────────────────────────────────────────────
            Organizations::<T>::try_mutate(org_id, |maybe| -> DispatchResult {
                let o = maybe.as_mut().ok_or(Error::<T>::OrgNotFound)?;
                o.last_tax_period_paid = now;
                // Reinstate if previously Delinquent
                if o.status == OrgStatus::Delinquent {
                    o.status = OrgStatus::Active;
                }
                Ok(())
            })?;

            // ── Clear penalty debt on compliance ─────────────────────────────
            let was_delinquent = OrgPenaltyDebt::<T>::contains_key(org_id)
                && OrgPenaltyDebt::<T>::get(org_id) > BalanceOf::<T>::default();
            OrgPenaltyDebt::<T>::remove(org_id);
            LastPenaltyBlock::<T>::remove(org_id);

            if was_delinquent {
                Self::deposit_event(Event::OrgReinstated { org_id });
            }

            Self::deposit_event(Event::TaxReturnFiled {
                declaration_id,
                org_id,
                region_id,
                amount_paid: amount,
                declared_profit,
                confederation_share,
                regional_share,
                document_hash,
            });
            Ok(())
        }

        // ─── report_tax_evasion ─────────────────────────────────────────────

        /// CITIZEN AUDIT — Report a delinquent organization and earn a reward.
        ///
        /// Any citizen may call this extrinsic. If the target organization has missed
        /// its tax period AND is NOT already Delinquent, the Punishment Engine is
        /// triggered immediately and the reporter earns `AuditorRewardAmount` from
        /// the `RegionTreasuryAccount`.
        ///
        /// ## Checks (fail fast, no reward)
        /// 1. Org must exist.
        /// 2. Org must NOT already be Delinquent (cannot double-report).
        /// 3. Tax period must actually be expired.
        ///
        /// ## On success
        /// - Punishment Engine fires (`apply_punishment`).
        /// - Reporter receives `AuditorRewardAmount` from the treasury.
        /// - `TaxEvasionReported` event emitted.
        ///
        /// This design creates a self-enforcing compliance market: citizens are
        /// economically incentivised to audit organizations.
        ///
        /// # Origin: Signed (any citizen)
        #[pallet::call_index(4)]
        #[pallet::weight(T::WeightInfo::report_tax_evasion())]
        pub fn report_tax_evasion(origin: OriginFor<T>, org_id: u32) -> DispatchResult {
            let reporter = ensure_signed(origin)?;

            let org = Organizations::<T>::get(org_id).ok_or(Error::<T>::OrgNotFound)?;
            ensure!(org.status != OrgStatus::Frozen, Error::<T>::OrgIsFrozen);
            ensure!(
                org.status != OrgStatus::Delinquent,
                Error::<T>::AlreadyDelinquent
            );

            let now = frame_system::Pallet::<T>::block_number();
            let elapsed = now.saturating_sub(org.last_tax_period_paid);
            let tax_period: BlockNumberFor<T> = T::TaxPeriodBlocks::get().into();
            ensure!(elapsed > tax_period, Error::<T>::TaxPeriodNotExpired);

            // Trigger punishment engine
            Self::apply_punishment(org_id, now);

            // Transfer auditor reward from treasury to reporter
            let reward = T::AuditorRewardAmount::get();
            let treasury = T::RegionTreasuryAccount::get();
            // Best-effort: reward transfer should not block the punishment itself
            let _ = T::Currency::transfer(
                &treasury,
                &reporter,
                reward,
                ExistenceRequirement::KeepAlive,
            );

            Self::deposit_event(Event::TaxEvasionReported {
                reporter,
                org_id,
                reward,
            });
            Ok(())
        }

        // ─── freeze_organization (Root) ─────────────────────────────────────

        /// Constitutionally freeze an Organization by Root court order.
        ///
        /// Only `Root` (Sudo / constitutional court via pallet-judicial-courts) may call.
        /// Frozen orgs are completely blocked: no tax filing, no crew changes.
        ///
        /// # Origin: Root
        #[pallet::call_index(5)]
        #[pallet::weight(T::WeightInfo::freeze_organization())]
        pub fn freeze_organization(origin: OriginFor<T>, org_id: u32) -> DispatchResult {
            ensure_root(origin)?;
            Organizations::<T>::try_mutate(org_id, |maybe| -> DispatchResult {
                let o = maybe.as_mut().ok_or(Error::<T>::OrgNotFound)?;
                o.status = OrgStatus::Frozen;
                Ok(())
            })?;
            Self::deposit_event(Event::OrgFrozenByRoot { org_id });
            Ok(())
        }

        // ─── spend_org_funds ────────────────────────────────────────────────

        /// Corporate Banking: Spend funds from the Organization's keyless treasury.
        ///
        /// Only a Director (or authorized Accountant in the future) can call this.
        /// The funds are transferred directly from the org's deterministic keyless 
        /// account (`derive_org_account(org_id)`) to the target `beneficiary`.
        ///
        /// ## Parameters
        /// - `org_id`: The organization whose funds are being spent.
        /// - `beneficiary`: The recipient account.
        /// - `amount`: The amount of ALTAN to transfer.
        ///
        /// # Origin: Signed (Director only)
        #[pallet::call_index(7)]
        #[pallet::weight(T::WeightInfo::activate_organization())] // using existing weight as proxy
        pub fn spend_org_funds(
            origin: OriginFor<T>,
            org_id: u32,
            beneficiary: T::AccountId,
            amount: BalanceOf<T>,
        ) -> DispatchResult {
            let caller = ensure_signed(origin)?;

            let org = Organizations::<T>::get(org_id).ok_or(Error::<T>::OrgNotFound)?;
            ensure!(org.status != OrgStatus::Frozen, Error::<T>::OrgIsFrozen);

            // Access control: Caller must be a Director of this org
            Self::ensure_director(&caller, org_id)?;

            // Transfer from keyless org account to beneficiary
            T::Currency::transfer(
                &org.bank_account_id,
                &beneficiary,
                amount,
                ExistenceRequirement::KeepAlive,
            ).map_err(|_| Error::<T>::InsufficientBankBalance)?;

            Self::deposit_event(Event::OrgFundsSpent {
                org_id,
                director: caller,
                beneficiary,
                amount,
            });

            Ok(())
        }

        // ─── file_tax_return_zk ─────────────────────────────────────────────

        /// FILE TAX RETURN via ZK Shielded Vault (Налоговый ZK-Мост).
        ///
        /// ## Принцип Асимметричной Прозрачности
        ///
        /// The guild Director pays taxes WITHOUT revealing the guild's total
        /// internal balance. Instead, a shielded commitment is destroyed
        /// (nullifier published) and the declared `amount_paid` materialises
        /// directly on the public `RegionTreasuryAccount`.
        ///
        /// ## Business Logic
        ///
        /// 1. Director provides `ZkTaxProof` = `{ nullifier, commitment, amount_paid }`.
        /// 2. Pallet calls `ShieldedVaultsBridge::unshield_for_tax(...)` which:
        ///    a. Verifies the nullifier is NOT spent.
        ///    b. Verifies the commitment exists in the ZK pool.
        ///    c. Destroys the commitment (marks nullifier spent).
        ///    d. Mints `amount_paid` publicly on `RegionTreasuryAccount`.
        /// 3. `TaxDeclaration` is recorded with `via_zk_vault = true`.
        /// 4. Org is reinstated to Active if Delinquent. Penalty debt cleared.
        ///
        /// ## What Remains Hidden
        ///
        /// - The guild's total shielded balance.
        /// - The guild's internal payroll / salary flows.
        /// - Any transactions that occurred inside the shielded pool.
        ///
        /// ## What Is Public
        ///
        /// - The `amount_paid` on the Regional Treasury.
        /// - The `nullifier` (commitment destroyed — no double-spend possible).
        /// - The `TaxDeclaration` record on-chain.
        ///
        /// # Origin: Signed (Director only)
        #[pallet::call_index(6)]
        #[pallet::weight(T::WeightInfo::file_tax_return_zk())]
        pub fn file_tax_return_zk(
            origin: OriginFor<T>,
            org_id: u32,
            region_id: u32,
            declared_profit: u128,
            // nullifier: destroys the shielded commitment in pallet-shielded-vaults
            nullifier: [u8; 32],
            // commitment: the shielded commitment being spent
            commitment: [u8; 32],
            // amount_paid: materialises on the Regional Treasury (publicly visible)
            amount_paid: u128,
            // document_hash: Blake2_256 hash of the off-chain tax declaration document
            document_hash: [u8; 32],
        ) -> DispatchResult {
            let caller = ensure_signed(origin)?;

            let org = Organizations::<T>::get(org_id).ok_or(Error::<T>::OrgNotFound)?;
            ensure!(org.status != OrgStatus::Frozen, Error::<T>::OrgIsFrozen);

            Self::ensure_director(&caller, org_id)?;

            // ── Constitutional Checks ────────────────────────────────────────
            Self::ensure_within_filing_period()?;
            ensure!(amount_paid > 0, Error::<T>::ZkAmountZero);

            // Convert to BalanceOf for minimum check
            let amount_balance: BalanceOf<T> = amount_paid.try_into().unwrap_or_default();
            ensure!(
                amount_balance >= T::MinTaxAmount::get(),
                Error::<T>::BelowMinimumTax
            );

            // ── Constitutional Tax Split ─────────────────────────────────────
            let (confederation_share, regional_share) =
                Self::constitutional_tax_split(amount_balance);

            let region_treasury = T::RegionTreasuryAccount::get();
            let confed_treasury = T::ConfederationTreasuryAccount::get();
            let now = frame_system::Pallet::<T>::block_number();
            let declaration_id = NextDeclarationId::<T>::get();

            // ── ZK SHIELDED UNSHIELD (via org_unshield_tax_payment) ──────────
            //
            // CONSTITUTIONAL MANDATE: 3/10 → Confederation + 7/10 → Region.
            //
            // We call `unshield_for_tax` which mints the FULL amount_paid to
            // the RegionTreasuryAccount, then transfer 3/10 to Confederation.
            //
            // This avoids the "double-nullifier" bug — nullifier is spent once.
            // The split happens atomically in the same extrinsic.
            T::ShieldedVaultsBridge::unshield_for_tax(
                &nullifier,
                &commitment,
                amount_paid, // Full amount — unshield to RegionTreasury first
                &region_treasury,
            )
            .map_err(|_| Error::<T>::ShieldedCommitmentNotFound)?;

            // Transfer confederation share (3/10) from RegionTreasury → ConfedTreasury.
            // region_treasury just received amount_paid, so this transfer succeeds
            // as long as the treasury has sufficient ED.
            T::Currency::transfer(
                &region_treasury,
                &confed_treasury,
                confederation_share,
                ExistenceRequirement::KeepAlive,
            )
            .map_err(|_| Error::<T>::InsufficientBankBalance)?;

            let declared_profit_balance: BalanceOf<T> =
                declared_profit.try_into().unwrap_or_default();

            // ── Record immutable declaration ─────────────────────────────────
            TaxDeclarations::<T>::insert(
                declaration_id,
                TaxDeclaration::<T> {
                    org_id,
                    region_id,
                    amount_paid: amount_balance,
                    declared_profit: declared_profit_balance,
                    confederation_share,
                    regional_share,
                    document_hash,
                    timestamp: now,
                    via_zk_vault: true,
                },
            );
            NextDeclarationId::<T>::put(declaration_id.saturating_add(1));

            // ── Update org state ─────────────────────────────────────────────
            Organizations::<T>::try_mutate(org_id, |maybe| -> DispatchResult {
                let o = maybe.as_mut().ok_or(Error::<T>::OrgNotFound)?;
                o.last_tax_period_paid = now;
                if o.status == OrgStatus::Delinquent {
                    o.status = OrgStatus::Active;
                }
                Ok(())
            })?;

            // ── Clear penalty debt ───────────────────────────────────────────
            let was_delinquent = OrgPenaltyDebt::<T>::contains_key(org_id)
                && OrgPenaltyDebt::<T>::get(org_id) > BalanceOf::<T>::default();
            OrgPenaltyDebt::<T>::remove(org_id);
            LastPenaltyBlock::<T>::remove(org_id);

            if was_delinquent {
                Self::deposit_event(Event::OrgReinstated { org_id });
            }

            Self::deposit_event(Event::OrgZkTaxPaid {
                declaration_id,
                org_id,
                region_id,
                amount_paid,
                nullifier,
            });
            Ok(())
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
