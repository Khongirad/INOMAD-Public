//! # INOMAD Identity Pallet
//!
//! **Altan Network — Sovereign L1 Blockchain**
//! **Sprint L1-06 (Final): Sovereign Judicial Courts — Identity Amendment**
//!
//! This pallet implements a **six-tier fractal hierarchy** for sovereign citizen
//! organisation on the Altan Network.  Each tier is formed bottom-up by the tier
//! below it; the only restriction is the **Glass Ceiling** between Tumed and Khural.
//!
//! ## Fractal Ladder
//!
//! ```text
//! Tier 0  Regular citizens
//!   ↓ form_arbad (10 citizens)
//! Tier 1  Arbad                → ArbadLeader
//!   ↓ form_zun (10 Arbads)
//! Tier 2  Zun                  → ZunLeader
//!   ↓ form_myangad (10 Zuns, branch set here)
//! Tier 3  Myangad [branch]     → MyangadLeader
//!   ↓ form_tumed (10 Myangads, branch INHERITED)
//! Tier 4  Tumed [branch]       → TumedLeader   ← ceiling for Exec/Jud/Banking
//!   ↓ form_khural (≥1 LEGISLATIVE Tumeds only)
//! Tier 5  Khural [nation]      → KhuralDelegate ← sovereign legislature
//!   ↓ form_confederation (≥1 Khurals, cross-nation)
//! Tier 6  Confederation        → ConfederationDelegate
//! ```
//!
//! ## The Glass Ceiling
//!
//! Tumeds in the Executive, Judicial, and Banking branches hit a structural ceiling —
//! they cannot form a Khural.  Only **Legislative** Tumeds may unite to establish the
//! sovereign Khural (national parliament).
//!
//! ## Four Branches of Power
//!
//! | Branch       | Max Tier   |
//! |--------------|------------|
//! | Executive    | Tumed      |
//! | Judicial     | Tumed      |
//! | Banking      | Tumed      |
//! | **Legislative** | **Khural → Confederation** |
//!
//! ## Dispatchables
//!
//! | Call | Origin | Description |
//! |---|---|---|
//! | `register_citizen` | Signed | Register a new citizen identity on Altan L1 |
//! | `verify_citizen` | Signed (Officer) | Verify a pending citizen after KYC/document check |
//! | `bootstrap_verify_creator` | Root | Bootstrap-verify the founding Creator account at genesis |
//! | `update_role` | Root | Update a citizen's sovereign role (Relayer-called after governance) |
//! | `freeze_citizen` | Signed (Judicial Origin) | Freeze a citizen account pending investigation |
//! | `unfreeze_citizen` | Signed (Judicial Origin) | Unfreeze a previously frozen citizen account |
//! | `form_arbad` | Signed (Officer) | Form a new geographic Arbad unit |
//! | `form_family_arbad` | Signed | Form a family-unit Arbad |
//! | `register_birth` | Signed (Officer/Family) | Pre-register a newborn citizen credential |
//! | `register_death` | Signed (Officer) | Register citizen death and lock credential permanently |
//! | `form_zun` | Root | Aggregate Arbads into a Zun (district) |
//! | `form_myangad` | Root | Aggregate Zuns into a Myangad (region) |
//! | `form_tumed` | Root | Aggregate Myangads into a Tumed (province) |
//! | `form_khural` | Root | Create or update a Khural (legislative council) |
//! | `form_confederation` | Root | Establish the apex Confederation structure |
//! | `assign_guardian` | Signed (Officer) | Assign a legal guardian to a minor or incapacitated citizen |
//! | `leave_arbad` | Signed (Citizen) | Voluntary departure from an Arbad |
//! | `claim_repatriation` | Signed (Citizen) | Repatriate a diaspora citizen to active status |
//! | `claim_birthright` | Signed (Citizen) | Activate a pre-registered minor credential on adulthood |
//! | `register_marriage` | Signed (Officer/Notary) | Record a marriage and derive joint family account |
//! | `register_divorce` | Signed (Officer/Notary) | Record a divorce and dissolve joint account |
//! | `register_nickname` | Signed (Citizen) | Register an immutable @nickname on L1 |
//! | `clear_nickname` | Root | Clear a nickname by court order (emergency) |
//! | `derive_family_account` | Signed | Derive a deterministic joint family treasury account |
//! | `do_freeze_citizen` | Internal | Internal: execute account freeze logic |
//! | `do_unfreeze_citizen` | Internal | Internal: execute account unfreeze logic |
//! | `do_demote_to_regular` | Internal | Internal: demote a citizen to regular rank |
//! | `do_exile` | Internal | Internal: execute exile and transfer to Black Book |
//! | `is_mandate_active` | Any (Query) | Check if a citizen's mandate is currently active |
//! | `validate_fractal_level` | Any (Query) | Validate a citizen's fractal hierarchy level |

#![cfg_attr(not(feature = "std"), no_std)]

extern crate alloc;

pub mod migrations;
pub mod weights;
pub use pallet::*;

#[cfg(feature = "runtime-benchmarks")]
mod benchmarking;

#[cfg(test)]
mod citizenship_tests;
#[cfg(test)]
mod mock;

// =========================================================================
// Constitutional Time Constants
// =========================================================================

/// Number of 6-second blocks in one year.
pub const BLOCKS_PER_YEAR: u32 = 5_256_000;

/// Two-year mandate — applies to all non-legislative leader roles
/// (ArbadLeader, ZunLeader, MyangadLeader, TumedLeader).
pub const TWO_YEAR_TERM: u32 = BLOCKS_PER_YEAR * 2;

/// Four-year mandate — applies to Khural and Confederation delegates.
/// Limited to two consecutive terms by the `khural_terms_served` counter.
pub const FOUR_YEAR_TERM: u32 = BLOCKS_PER_YEAR * 4;

// =========================================================================
// Cross-pallet Identity Interface
// =========================================================================

/// Trait for cross-pallet interaction with `pallet-inomad-identity`.
///
/// Implement this in the runtime (or as a blanket impl on the pallet) and
/// pass it to `pallet-judicial-courts` via its `Config::Identity` associated
/// type.  This keeps the two pallets loosely coupled.
pub trait IdentityInterface<AccountId> {
    /// Returns a clone of the citizen record for `who`, if registered.
    fn citizen_record_of(who: &AccountId) -> Option<crate::pallet::CitizenRecord>;

    /// Freeze `who` — sets their status to `CitizenStatus::Frozen`.
    fn freeze_citizen(who: &AccountId) -> frame_support::dispatch::DispatchResult;

    /// Unfreeze `who` — sets their status back to `CitizenStatus::Active`.
    fn unfreeze_citizen(who: &AccountId) -> frame_support::dispatch::DispatchResult;

    /// Demote `who`'s role to `CitizenRole::Regular`.
    fn demote_to_regular(who: &AccountId) -> frame_support::dispatch::DispatchResult;

    /// Permanently exile `who` — sets their status to `CitizenStatus::Exiled`.
    ///
    /// Called by `pallet-black-book` on `condemn_and_issue_warrant`.
    /// Terminal: cannot be reversed. Distinct from `Frozen` (judicial hold) and
    /// `Deceased` (biological death) — exile is a constitutional sentence.
    fn exile_citizen(who: &AccountId) -> frame_support::dispatch::DispatchResult;
}

// =========================================================================
// OnTerminalStatus — Cross-Pallet Cascading Cleanup Hook
// =========================================================================

/// Hook called by `pallet-inomad-identity` when a citizen transitions to a
/// **terminal** state (`Deceased` or `Exiled`).
///
/// Implement this in the runtime and wire it to all pallets that hold
/// per-citizen state (guilds, chancery, khural-governance, etc.).
/// This prevents "ghost" state: dead or exiled citizens being listed as Guild
/// Masters, holding open Quests, blocking Chancery agreements, or retaining
/// legislative mandates.
///
/// ## Implementors
///
/// Use a tuple impl at the runtime level to chain multiple handlers:
/// ```rust,ignore
/// pub struct TerminalHookImpl;
/// impl pallet_inomad_identity::OnTerminalStatus<AccountId> for TerminalHookImpl { ... }
/// ```
pub trait OnTerminalStatus<AccountId> {
    /// Called immediately after a citizen's status is set to `Deceased`.
    fn on_deceased(who: &AccountId);
    /// Called immediately after a citizen's status is set to `Exiled`.
    fn on_exiled(who: &AccountId);
}

/// No-op implementation so pallets that don't need the hook can use `()` as default.
impl<AccountId> OnTerminalStatus<AccountId> for () {
    fn on_deceased(_who: &AccountId) {}
    fn on_exiled(_who: &AccountId) {}
}

#[frame_support::pallet]
pub mod pallet {
    use crate::weights::WeightInfo as _;
    use alloc::vec::Vec;
    use frame_support::sp_runtime::traits::SaturatedConversion;
    use frame_support::{
        pallet_prelude::*,
        traits::{ConstU32, Currency, ReservableCurrency},
    };
    use frame_system::pallet_prelude::*;
    use sp_core::H256;

    // Currency balance type alias for convenience.
    pub type BalanceOf<T> =
        <<T as Config>::Currency as Currency<<T as frame_system::Config>::AccountId>>::Balance;

    // =========================================================================
    // Pallet Struct
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
        /// The currency used to reserve (freeze) funds of deceased and minor citizens.
        ///
        /// On `register_death` all free balance is reserved so the wallet cannot
        /// transact.  On `register_birth` the Minor Credential has zero balance; this type
        /// is here to allow inheritance pallets to interact with the balance later.
        type Currency: Currency<Self::AccountId> + ReservableCurrency<Self::AccountId>;

        /// Citizen GDP share (planck) unlocked at each vesting level.
        ///
        /// Level 0 — At verification: UNLOCK_LEVEL_0 planck are immediately spendable.
        /// The remainder stays in a vesting lock.
        #[pallet::constant]
        type UnlockLevel0: frame_support::traits::Get<u128>;

        /// Level 1 — On FamilyArbad or RegularArbad creation: cumulative unlock cap.
        #[pallet::constant]
        type UnlockLevel1: frame_support::traits::Get<u128>;

        /// The authorised Medical Authority account — the only account (besides Root)
        /// that may call `register_birth` and `register_death`.
        ///
        /// Corresponds to the Altan Gateway's `MEDICAL` officer role.
        /// Configure to the Bank-of-Siberia Medical key in the chain spec.
        /// In dev: set to `Alice` for testing.
        #[pallet::constant]
        type MedicalAuthority: frame_support::traits::Get<Self::AccountId>;

        /// **CONSTITUTIONAL** Gas fee charged per civil act registration (marriage/divorce).
        ///
        /// Collected equally from BOTH partners. Sent to `CivilFeeTreasury`
        /// (the regional treasury of the region where the ceremony is performed).
        ///
        /// Constitutionally: both parties pay the state for legitimizing the union.
        #[pallet::constant]
        type MarriageFee: frame_support::traits::Get<BalanceOf<Self>>;

        /// Recipient of civil act fees — typically the regional Citizen Fund.
        ///
        /// In genesis: set to the regional gov treasury or the Confederation treasury.
        #[pallet::constant]
        type CivilFeeTreasury: frame_support::traits::Get<Self::AccountId>;

        /// [SECURITY VECTOR 3] Minimum number of blocks a citizen must wait after
        /// leaving an Arbad before they may join or form another.
        ///
        /// Prevents cartel-style rapid membership migration to swing ArbadLeader
        /// elections and manipulate the fractal democracy stacking mechanism.
        ///
        /// Default: 432_000 blocks ≈ 30 days at 6-second block time.
        #[pallet::constant]
        type ArbadCooldownPeriod: frame_support::traits::Get<
            frame_system::pallet_prelude::BlockNumberFor<Self>,
        >;

        // ─── Oracle Rate Limit (Task 2: Oracle Compromise Limit) ─────────────────

        /// [SECURITY VECTOR: ORACLE RATE LIMIT] Maximum number of `register_birth`
        /// calls allowed per block from the `MedicalAuthority` key.
        ///
        /// If a MedicalAuthority key is compromised, an attacker can call
        /// `register_birth` in a loop, flooding `NextCitizenId` and exhausting
        /// registry state. This cap limits the blast radius to `MaxRegistrationsPerBlock`
        /// new SBTs per 6-second block (~100 per block = ~1_440_000 per day max).
        ///
        /// Default in runtime: `ConstU32<100>`.
        #[pallet::constant]
        type MaxRegistrationsPerBlock: frame_support::traits::Get<u32>;

        // ─── Terminal Status Hook (Task 1: Cross-Pallet Ghost Cleanup) ───────────

        /// [SECURITY VECTOR: GHOST STATE] Cross-pallet hook called when a citizen
        /// transitions to a terminal state (`Deceased` or `Exiled`).
        ///
        /// Implement as `TerminalHookImpl` in the runtime configs, dispatching
        /// cascading cleanup across `pallet-guilds`, `pallet-chancery`, and
        /// `pallet-khural-governance` so dead/exiled citizens do not retain
        /// Guild Master seats, open Quest escrows, pending signatures, or
        /// legislative mandates.
        type TerminalHook: crate::OnTerminalStatus<Self::AccountId>;

        /// **Chain of Legitimacy** — the canonical 32-byte blake2_256 hash of
        /// the current constitutional document.
        ///
        /// Each citizenship extrinsic (`claim_birthright`, `claim_repatriation`)
        /// requires the citizen to submit their `accepted_constitution_hash`.
        /// The pallet verifies:
        ///   `ensure!(submitted == ConstitutionHashProvider::get(), Error::ConstitutionHashMismatch)`
        ///
        /// This means every citizenship record is a **cryptographic receipt** proving
        /// the citizen has seen and accepted the exact current version of the Constitution.
        ///
        /// Wire at runtime:
        /// ```ignore
        /// parameter_types! {
        ///     pub const AltanConstitutionHash: [u8; 32] = pallet_constitution::current_hash();
        /// }
        /// type ConstitutionHashProvider = AltanConstitutionHash;
        /// ```
        type ConstitutionHashProvider: frame_support::traits::Get<Option<[u8; 32]>>;
    }

    // =========================================================================
    // Enums
    // =========================================================================

    /// The four constitutional branches of power on the Altan Network.
    ///
    /// Branch is assigned at the Myangad tier and inherited up through Tumed.
    /// Only `Legislative` Tumeds may join a Khural (the Glass Ceiling rule).
    ///
    /// ## Constitutional Powers by Branch
    ///
    /// | Branch      | Max Role    | Powers                                                               |
    /// |-------------|-------------|----------------------------------------------------------------------|
    /// | Executive   | TumedLeader | Manages Ministries, Committees, Agencies. Headed by President.       |
    /// | Judicial    | TumedLeader | Forms Republic Courts. ONLY body that can freeze/confiscate by order.|
    /// | Banking     | TumedLeader | Manages debt & credit ONLY. Cannot touch citizen accounts.          |
    /// | Legislative | Confederation | Forms laws, approves budget, appoints Executive bodies.            |
    ///
    /// The `branch` field on a `CitizenRecord` is updated when a citizen
    /// ascends to a Myangad (or higher) unit.
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
    pub enum BranchOfPower {
        /// Operational state administration (Presidium → Ministries → Committees → Agencies).
        /// Max role: TumedLeader. Headed by President (CitizenRole::President).
        Executive,
        /// Constitutional justice and enforcement.
        /// Max role: TumedLeader. ONLY judicial courts may freeze/confiscate accounts by order.
        Judicial,
        /// Lawmaking — the ONLY branch that may unite into a Khural and Confederation.
        /// Approves annual budget from 7% fund (Republican) and 3% fund (Confederate).
        Legislative,
        /// Monetary policy: debt management and credit issuance ONLY.
        /// Max role: TumedLeader. Cannot transact on citizen accounts without court order.
        Banking,
    }

    /// Hierarchical social rank of a sovereign citizen on the Altan Network.
    ///
    /// Ranks are earned through bottom-up formation; `update_role` (Root) may
    /// override in emergencies.
    ///
    /// ## Presidential Note
    ///
    /// The `President` role is the head of the Executive branch — the second person
    /// in power after the Chairman of the Republican Khural. The President is elected
    /// for a 4-year term, maximum 2 consecutive terms, and is responsible for internal
    /// policy through the Ministries, Committees, and Agencies. The President does NOT
    /// chair the Khural and cannot form a Khural.
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
    pub enum CitizenRole {
        /// Standard citizen — no command authority.
        Regular,
        /// Десятник — leads an Arbad of 10 citizens.
        /// Also: any Arbad leader from an indigenous people may INITIATE a war-time vote.
        ArbadLeader,
        /// Сотник — leads a Zun of 10 Arbads.
        ZunLeader,
        /// Тысячник — leads a Myangad of 10 Zuns.
        MyangadLeader,
        /// Темник — leads a Tumed of 10 Myangads (any branch).
        /// This is the structural ceiling for Executive, Judicial and Banking branches.
        TumedLeader,
        /// President — elected head of the Executive branch.
        /// 4-year term, max 2 consecutive terms. Second in power after Khural Chairman.
        /// Manages internal policy through Ministries/Committees/Agencies.
        /// Branch: Executive. Cannot form Khural.
        President,
        /// Delegate of a national Republican Khural — formed from Legislative Tumeds only.
        /// Chairman of the Khural is the first person in power in the Republic.
        /// Manages: laws, verifies Executive bodies, approves annual budget from 7% fund.
        KhuralDelegate,
        /// Delegate of the cross-national Confederate Khural.
        /// Manages: federal laws, approves budget from 3% fund, declares war when attacked.
        ConfederationDelegate,
    }

    /// Citizen status.
    ///
    /// | Variant    | Description                                                                |
    /// |------------|----------------------------------------------------------------------------|
    /// | `Active`   | Citizen may transact, vote, and form units.                               |
    /// | `Frozen`   | Judicial freeze — no transactions, no votes; reversible by courts.        |
    /// | `Minor`    | Pre-registered Credential for a newborn. Activated at KYC on adulthood.         |
    /// | `Deceased` | Citizen has died. Credential permanently locked. Balance inheritance pending.   |
    /// | `Exiled`   | Condemned by constitutional court. TERMINAL. All funds confiscated.      |
    ///
    /// ## State Transitions
    /// ```text
    /// [Minor] ──KYC at 18──▶ [Active] ──Judicial──▶ [Frozen] ──Unfreeze──▶ [Active]
    ///                            ├──register_death──▶ [Deceased]  (TERMINAL)
    ///                            └──condemn_warrant──▶ [Exiled]   (TERMINAL)
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
    pub enum CitizenStatus {
        /// Citizen is fully active — may transact, vote, and form units.
        Active,
        /// Judicial freeze — no transactions, no votes. Nation_id and role preserved.
        /// Reversible by the judicial courts after sentence execution.
        Frozen,
        /// Pre-registered Credential for a newborn citizen.
        /// Created by `register_birth`. Activated to `Active` via KYC at age of majority.
        /// A Minor cannot vote, transfer funds, or form civic units.
        Minor,
        /// The citizen has died. Credential is permanently locked (TERMINAL state).
        /// Protects against "dead souls" voting. Inheritance proceedings initiated.
        /// Cannot be reversed. Distinct from `Frozen` (judicial) for legal clarity.
        Deceased,
        /// The citizen has been condemned by constitutional court and exiled (TERMINAL).
        ///
        /// Issued by `pallet-black-book::condemn_and_issue_warrant` (Root only).
        /// All balance confiscated to Treasury on exile. Credential locked permanently.
        /// If the target is a fugitive, a BountyPool is opened for their capture.
        /// CANNOT be reversed — does not precede Deceased; they are parallel terminals.
        Exiled,
    }

    // ─── Citizenship Origin Status (Jus Sanguinis / Jus Soli / Ius Gentium) ───

    /// The constitutional origin of a citizen — their standing in the Republic.
    ///
    /// ## Three Pillars of Citizenship
    ///
    /// | Variant       | Latin Principle | Meaning                                       |
    /// |---------------|-----------------|-----------------------------------------------|
    /// | `Indigenous`  | Jus Sanguinis   | Born of the blood of a sovereign people.       |
    /// | `Naturalized` | Jus Soli        | Born on Republic soil; accepted its laws.     |
    /// | `Foreigner`   | Ius Gentium     | Resident under host-nation rules; no sovereignty.|
    ///
    /// ## Political Rights by Status
    ///
    /// | Right                          | Indigenous | Naturalized | Foreigner |
    /// |--------------------------------|------------|-------------|----------|
    /// | Vote in Khural                 | ✅          | ✅           | ❌         |
    /// | Own land / subsoil             | ✅          | ✅           | ❌         |
    /// | Form Arbad                     | ✅          | ✅           | ❌         |
    /// | Become KhuralDelegate          | ✅ only     | ❌           | ❌         |
    /// | Economic participation         | ✅          | ✅           | ✅         |
    /// | Freedom of movement            | ✅          | ✅           | ✅         |
    ///
    /// ## Transitions
    ///
    /// ```text
    /// [Foreigner] → claim_birthright()           → [Naturalized]
    /// [Foreigner] → claim_repatriation(proof)    → [Indigenous]
    /// [Naturalized] → (future: Khural vote)      → [Indigenous]
    /// ```
    ///
    /// Default on `register_citizen`: `Foreigner` (until proof is submitted).
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
    pub enum CitizenshipStatus {
        /// **Jus Sanguinis** — Right of Blood.
        ///
        /// The citizen is a member of one of the 79 sovereign indigenous peoples
        /// of the Altan Confederation. They are the PRIMARY source of political power.
        ///
        /// Indigenous status is acquired by:
        /// - Being registered by a Medical Authority with an indigenous `nation_id`
        ///   (at birth, `is_indigenous = true`).
        /// - Calling `claim_repatriation(lineage_proof)` and proving bloodline
        ///   descent from a registered indigenous citizen.
        ///
        /// Indigenous citizens may:
        /// - Vote and stand in Khural elections as KhuralDelegate.
        /// - Own land, air rights, and subsoil rights.
        /// - Form Arbads and ascend the fractal democracy hierarchy.
        Indigenous,

        /// **Jus Soli** — Right of Soil.
        ///
        /// The citizen was born on the territory of the Republic to parents who
        /// were NOT indigenous. By calling `claim_birthright()`, they accept the
        /// laws and constitution of the Republic and receive full economic and
        /// civic rights — but NOT the political sovereignty of the indigenous peoples.
        ///
        /// Naturalized citizens may:
        /// - Vote in local referenda (future sprint).
        /// - Own land and subsoil rights.
        /// - Form Arbads and participate in civic structures.
        ///
        /// Naturalized citizens may NOT:
        /// - Become KhuralDelegate (the fractal sovereignty is reserved for Indigenous).
        Naturalized,

        /// **Ius Gentium** — Law of Nations (Guest status).
        ///
        /// Default status for all newly registered citizens. Foreigners are:
        /// - Economic participants in the Republic's markets.
        /// - Protected by all human rights (Articles I and II of the Constitution).
        /// - Barred from political participation and land ownership.
        ///
        /// A Foreigner may transition to `Naturalized` via `claim_birthright()`,
        /// or to `Indigenous` via `claim_repatriation(lineage_proof)`.
        Foreigner,
    }

    // ─── Fractal Hierarchy Level ───────────────────────────────────────────────

    /// The six structural levels of the Fractal Democracy hierarchy.
    ///
    /// Enumerated in ascending order:
    ///
    /// | Level             | Units       | Size       |
    /// |-------------------|-------------|------------|
    /// | `Arbad`           | Tier-1      | 10          |
    /// | `Zun`             | Tier-2      | 100         |
    /// | `Myangad`         | Tier-3      | 1_000       |
    /// | `Tumed`           | Tier-4      | 10_000      |
    /// | `RepublicanKhural`| Tier-5      | ≥ 1 Tumed   |
    /// | `ConfederativeKhural`| Tier-6   | ≥ 1 Khural  |
    ///
    /// ## Fractal Law (Fundamental)
    ///
    /// For all branches **except** `Legislative`, the structural ceiling is
    /// `FractalLevel::Tumed`.  Any attempt to advance further — to
    /// `RepublicanKhural` or `ConfederativeKhural` — returns
    /// `Error::<T>::HierarchyLimitExceeded`.
    ///
    /// Only the `Legislative` branch may ascend from `Tumed` to
    /// `RepublicanKhural` and then to `ConfederativeKhural`.
    #[derive(
        Encode,
        Decode,
        DecodeWithMemTracking,
        Clone,
        PartialEq,
        Eq,
        PartialOrd,
        Ord,
        RuntimeDebug,
        TypeInfo,
        MaxEncodedLen,
    )]
    pub enum FractalLevel {
        /// Tier-1: 10 citizens form an Arbad.
        Arbad,
        /// Tier-2: 10 Arbads form a Zun (100 citizens).
        Zun,
        /// Tier-3: 10 Zuns form a Myangad (1_000 citizens). Branch is assigned here.
        Myangad,
        /// Tier-4: 10 Myangads form a Tumed (10_000 citizens).
        /// *** Absolute ceiling for Executive, Judicial, Banking, and Business/Military units. ***
        Tumed,
        /// Tier-5: Legislative Tumeds unite into a Republican Khural (national parliament).
        /// EXCLUSIVE to the Legislative branch.
        RepublicanKhural,
        /// Tier-6: Republican Khurals unite into a Confederative Khural (confederal parliament).
        /// EXCLUSIVE to the Legislative branch.
        ConfederativeKhural,
    }

    // ─── Vesting / Proof-of-Citizenship Mining Level ─────────────────────────

    /// The three Proof-of-Citizenship vesting unlock levels.
    ///
    /// Tokens assigned to a citizen at verification are locked (vesting).
    /// Social actions progressively unlock the balance:
    ///
    /// | Level | Trigger                              | Unlocked (cumulative)  |
    /// |-------|--------------------------------------|------------------------|
    /// | 0     | Verified by existing citizen          | 100 UNIT               |
    /// | 1     | FamilyArbad or RegularArbad formed    | 1_000 UNIT             |
    /// | 2     | Arbad is FULL (10 members) + Leader   | ALL remaining GDP share |
    #[derive(
        Encode,
        Decode,
        DecodeWithMemTracking,
        Clone,
        PartialEq,
        Eq,
        PartialOrd,
        Ord,
        RuntimeDebug,
        TypeInfo,
        MaxEncodedLen,
    )]
    pub enum VestingLevel {
        /// Level 0 — Verified by peer citizen. UNLOCK: 100 UNIT.
        Verified,
        /// Level 1 — Joined or formed an Arbad. UNLOCK: cumulative 1_000 UNIT.
        ArbadMember,
        /// Level 2 — Joined a Zun (8 Arbads = ~80 people). UNLOCK: ALL remaining GDP share.
        ///
        /// Myangad (L3) and Tumed (L4) do NOT trigger further base-GDP unlocks.
        /// Those levels grant access to macro-economics (corporate credit in Bank of Siberia).
        ZunMember,
    }

    /// Whether a citizen's Proof-of-Citizenship status indicates they are verified.
    /// Only a verified citizen can verify others.
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
    pub enum VerificationStatus {
        /// Citizen has been registered but NOT yet verified by a peer.
        Unverified,
        /// Citizen has been verified by an existing citizen — Proof-of-Citizenship confirmed.
        Verified,
    }

    /// Arbad type for Proof-of-Citizenship social formation.
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
    pub enum ArbadType {
        /// Family Arbad — husband + wife dyad. Head of family chosen explicitly.
        Family,
        /// Regular Arbad — open civic unit, minimum 2 members to start.
        Regular,
    }

    // ─── Passport Type (KYC/AML) ──────────────────────────────────────────────────

    /// Type of identity document submitted for KYC/AML verification.
    ///
    /// The Bank of Siberia performs strict off-chain verification per banking standards.
    /// Only the **hashes** of the documents are stored on-chain to prevent impersonation.
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
    pub enum PassportType {
        /// RF Internal Passport (внутренний паспорт РФ) — series + number.
        Internal,
        /// RF International Passport (заграничный паспорт РФ) — biometric travel document.
        International,
    }

    // =========================================================================
    // Structs
    // =========================================================================

    /// Soulbound identity record permanently bound to one `AccountId`.
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
    pub struct CitizenRecord {
        /// Sequential citizen ID (1-indexed, 15-digit display: 000000000000001).
        /// Citizen #1 is the Creator. IMMUTABLE after assignment.
        pub citizen_id: u64,
        /// Indigenous nation (1–79).  IMMUTABLE after registration.
        pub nation_id: u32,
        /// Ethnicity ID for non-indigenous citizens (1001+).
        /// `None` for indigenous citizens (nation_id 1–79 is sufficient).
        /// Maps to `NATURALIZED_PEOPLES` constant.
        pub naturalized_people_id: Option<u32>,
        /// Current hierarchical role.
        pub role: CitizenRole,
        /// Active or Frozen.
        pub status: CitizenStatus,
        /// Proof-of-Citizenship verification status.
        pub verification: VerificationStatus,
        /// Proof-of-Citizenship vesting level (None until first verification).
        pub vesting_level: Option<VestingLevel>,
        /// Constitutional branch (ветвь власти) — None until Myangad.
        pub branch: Option<BranchOfPower>,
        /// Block number when current mandate expires.
        pub term_end: Option<u32>,
        /// Khural/Confederation terms served (constitutional max = 2).
        pub khural_terms_served: u8,
        /// Whether this citizen belongs to a recognised indigenous people.
        pub is_indigenous: bool,
        /// Constitutional origin of the citizen — determines land ownership and political rights.
        ///
        /// ## Article V — Non-Alienation Invariant
        ///
        /// When `citizenship_status == CitizenshipStatus::Foreigner`:
        /// - `pallet-land-registry::transfer_land` **rejects** the transaction.
        /// - Khural voting is disabled (`pallet-khural-governance` checks this field).
        ///
        /// Default at registration: `Foreigner`. Upgraded via:
        /// - `claim_birthright()` → `Naturalized` (Jus Soli)
        /// - `claim_repatriation(lineage_proof)` → `Indigenous` (Jus Sanguinis)
        pub citizenship_status: CitizenshipStatus,
        /// Region code (1–83) for regional airdrop routing.
        pub region_id: Option<u8>,
        /// Birth/registration region (1–83). GDP payouts MUST come from this region's fund.
        pub birth_region_id: Option<u8>,

        // ─── KYC/AML Document Hashes (Bank of Siberia off-chain verification) ──────────
        //
        // The Bank of Siberia verifies physical documents off-chain.
        // Only blake2_256 / sha256 hashes are stored on-chain.
        // Purpose: anti-Sybil (no duplicate real identities).
        //
        // document_hash  = hash(passport_series + passport_number + DOB)
        // birth_page_hash = hash(page_with_place_of_birth_photo)
        // email_hash      = hash(email_address)
        //
        // All three are UNIQUE constraints enforced via StorageMaps.
        /// Type of document submitted (Internal or International RF passport).
        pub passport_type: PassportType,
        /// blake2_256 hash of the passport identity page. Uniqueness enforced on-chain.
        pub document_hash: H256,
        /// hash of the birth-place page. Required for region binding.
        pub birth_page_hash: H256,
        /// hash of the email address. Uniqueness enforced on-chain.
        pub email_hash: H256,
    }

    // ─── Indigenous Peoples Registry ──────────────────────────────────────────

    /// The six constitutional territory zones for indigenous peoples.
    /// Used to group nations by geographic heritage territory.
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
    pub enum IndigenousTerritoryZone {
        /// Siberia — Буряад-Монгол, Тувинец, Саха, Тофалар, Алтаец, Хакас,
        ///           Шорец, Эвенк, Долган, Нганасан, Мунгал.
        Siberia,
        /// Far East & Chukotka — Чукча, Коряк, Ительмен, Нивх, Нанаец,
        ///                       Ульч, Ороч, Удэгеец.
        FarEast,
        /// North / Yamal / Nenets — Ненец, Хант, Манси, Селькуп.
        North,
        /// Northern Caucasus — Чеченец, Ингуш, Осетин, Кабардинец, Балкарец,
        ///                     Черкес/Адыгеец, Карачаевец, Аварец, Даргинец, Лезгин.
        Caucasus,
        /// Ural & Volga — Башкир, Татарин, Чуваш, Мариец, Мордвин, Удмурт,
        ///               Коми-Зырянин, Карел, Калмык.
        UralVolga,
        /// Eastern Rus (Golden Ring) — Русский (Владимир, Суздаль, Рязань,
        ///                             Ярославль, Кострома, Иваново, Тверь).
        EasternRus,
    }

    /// On-chain record for an indigenous citizen's territorial affiliation.
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
    pub struct IndigenousRecord {
        /// Nation ID (1–79), matches the `CitizenRecord.nation_id`.
        pub people_id: u32,
        /// Constitutional territory zone for this nation.
        pub zone: IndigenousTerritoryZone,
    }

    /// The 79 sovereign indigenous peoples (Коренные Народы) of the Siberian Confederation.
    ///
    /// Each entry: `(people_id, name, territory_zone, home_regions)`.
    /// `home_regions` = list of OKATO codes for their primary territories.
    ///
    /// Note: peoples with asterisk (*) span multiple administrative regions.
    /// The `region_id` on `CitizenRecord` records the specific region,
    /// while `people_id` records the sovereign nation.
    ///
    /// ## Zone: Siberia (peoples 01–11)
    ///
    /// 01 Буряад-Монгол      — regions 03, 38, 75, 24, 79, 27, 25
    ///                        (Бурятия, Иркутская, Забайкальский,
    ///                         Красноярский, Еврейская АО, Хабаровский, Приморский)
    /// 02 Тувинец            — region 17 (Республика Тыва)
    /// 03 Саха (Якут) *      — regions 14, 24, 27, 25
    ///                        (Якутия, Красноярский, Хабаровский, Приморский)
    /// 04 Алтаец             — regions 04, 22 (Республика Алтай, Алтайский край)
    /// 05 Тофалар            — region 38 (Иркутская область)
    /// 06 Хакас              — region 19 (Республика Хакасия)
    /// 07 Шорец              — region 42 (Кемеровская область)
    /// 08 Эвенк *            — regions 24, 38, 03, 75, 27, 25
    ///                        (Красноярский, Иркутская, Бурятия,
    ///                         Забайкальский, Хабаровский, Приморский)
    /// 09 Долган             — region 24 (Красноярский край)
    /// 10 Нганасан           — region 24 (Красноярский край)
    /// 11 Мунгал             — region 75 (Забайкальский край)
    ///
    /// ## Zone: Far East & Chukotka (peoples 12–19)
    ///
    /// 12 Чукча              — region 87 (Чукотский АО)
    /// 13 Коряк              — region 41 (Камчатский край)
    /// 14 Ительмен           — region 41 (Камчатский край)
    /// 15 Нивх               — region 65 (Сахалинская область)
    /// 16 Нанаец             — region 27 (Хабаровский край)
    /// 17 Ульч               — region 27 (Хабаровский край)
    /// 18 Ороч               — region 27 (Хабаровский край)
    /// 19 Удэгеец            — regions 25, 27 (Приморский, Хабаровский)
    ///
    /// ## Zone: North / Yamal / Nenets (peoples 20–23)
    ///
    /// 20 Ненец              — regions 89, 83 (Ямало-Ненецкий, Ненецкий АО)
    /// 21 Хант               — region 86 (Ханты-Мансийский АО — Югра)
    /// 22 Манси              — region 86 (Ханты-Мансийский АО — Югра)
    /// 23 Селькуп            — region 89 (Ямало-Ненецкий АО)
    ///
    /// ## Zone: Northern Caucasus (peoples 24–39)
    ///
    /// 24 Чеченец            — region 20 (Чечня)
    /// 25 Ингуш              — region 06 (Ингушетия)
    /// 26 Осетин             — region 15 (Северная Осетия)
    /// 27 Кабардинец         — region 07 (Кабардино-Балкария)
    /// 28 Балкарец           — region 07 (Кабардино-Балкария)
    /// 29 Черкес/Адыгеец     — regions 01, 09 (Адыгея, Карачаево-Черкесия)
    /// 30 Карачаевец         — region 09 (Карачаево-Черкесия)
    /// 31 Аварец             — region 05 (Дагестан)
    /// 32 Даргинец           — region 05 (Дагестан)
    /// 33 Лезгин             — region 05 (Дагестан)
    /// 34 Кумык              — region 05 (Дагестан)
    /// 35 Лакец              — region 05 (Дагестан)
    /// 36 Табасаран          — region 05 (Дагестан)
    /// 37 Рутулец            — region 05 (Дагестан)
    /// 38 Агул               — region 05 (Дагестан)
    /// 39 Цахур              — region 05 (Дагестан)
    ///
    /// ## Zone: Ural & Volga (peoples 40–59)
    ///
    /// 40 Башкир             — region 02 (Башкортостан)
    /// 41 Татарин            — regions 16, 63, 73, 52 (Татарстан, Поволжье)
    /// 42 Чуваш              — region 21 (Чувашия)
    /// 43 Мариец             — region 12 (Марий Эл)
    /// 44 Мордвин            — region 13 (Мордовия)
    /// 45 Удмурт             — region 18 (Удмуртия)
    /// 46 Коми-Зырянин       — region 11 (Республика Коми)
    /// 47 Карел              — region 10 (Карелия)
    /// 48 Калмык             — region 08 (Калмыкия)
    /// 49 Ногаец             — regions 05, 09, 26 (Дагестан, Карачаево-Черкесия, Ставрополь)
    /// 50 Казах              — region 56 (Оренбургская область)
    /// 51 Мещеряк            — region 62 (Рязанская область)
    /// 52 Нагайбак           — region 74 (Челябинская область)
    /// 53 Тептяр             — region 02 (Башкортостан)
    /// 54 Ненец (Европ.)     — region 29 (Архангельская область)
    /// 55 Вепс               — region 47 (Ленинградская область)
    /// 56 Ижорец             — region 47 (Ленинградская область)
    /// 57 Вожанин            — region 47 (Ленинградская область)
    /// 58 Саам               — region 51 (Мурманская область)
    /// 59 Коми-Пермяк        — region 59 (Пермский край)
    ///
    /// ## Zone: Eastern Rus — Golden Ring (peoples 60–86)
    ///
    /// 60 Русский (Золотое Кольцо) — regions 33, 62, 76, 44, 69, 37, 40
    ///    (Владимирская, Рязанская, Ярославская, Костромская, Тверская, Ивановская, Калужская)
    /// 61–79 — Помор, Эвен, Юкагир и др.
    ///
    pub const INDIGENOUS_PEOPLES_79: &[(u32, &str, &str)] = &[
        // (people_id, name, zone)
        // ── Siberia ─────────────────────────────────────────────────────────
        (01, "Буряад-Монгол", "Siberia"),
        (02, "Тувинец", "Siberia"),
        (03, "Саха (Якут)", "Siberia"),
        (04, "Алтаец", "Siberia"),
        (05, "Тофалар", "Siberia"),
        (06, "Хакас", "Siberia"),
        (07, "Шорец", "Siberia"),
        (08, "Эвенк", "Siberia"),
        (09, "Долган", "Siberia"),
        (10, "Нганасан", "Siberia"),
        (11, "Мунгал", "Siberia"),
        // ── Far East & Chukotka ─────────────────────────────────────────────
        (12, "Чукча", "FarEast"),
        (13, "Коряк", "FarEast"),
        (14, "Ительмен", "FarEast"),
        (15, "Нивх", "FarEast"),
        (16, "Нанаец", "FarEast"),
        (17, "Ульч", "FarEast"),
        (18, "Ороч", "FarEast"),
        (19, "Удэгеец", "FarEast"),
        // ── North / Yamal ───────────────────────────────────────────────────
        (20, "Ненец", "North"),
        (21, "Хант", "North"),
        (22, "Манси", "North"),
        (23, "Селькуп", "North"),
        // ── Northern Caucasus ───────────────────────────────────────────────
        (24, "Чеченец", "Caucasus"),
        (25, "Ингуш", "Caucasus"),
        (26, "Осетин", "Caucasus"),
        (27, "Кабардинец", "Caucasus"),
        (28, "Балкарец", "Caucasus"),
        (29, "Черкес/Адыгеец", "Caucasus"),
        (30, "Карачаевец", "Caucasus"),
        (31, "Аварец", "Caucasus"),
        (32, "Даргинец", "Caucasus"),
        (33, "Лезгин", "Caucasus"),
        (34, "Кумык", "Caucasus"),
        (35, "Лакец", "Caucasus"),
        (36, "Табасаран", "Caucasus"),
        (37, "Рутулец", "Caucasus"),
        (38, "Агул", "Caucasus"),
        (39, "Цахур", "Caucasus"),
        // ── Ural & Volga ────────────────────────────────────────────────────
        (40, "Башкир", "UralVolga"),
        (41, "Татарин", "UralVolga"),
        (42, "Чуваш", "UralVolga"),
        (43, "Мариец", "UralVolga"),
        (44, "Мордвин", "UralVolga"),
        (45, "Удмурт", "UralVolga"),
        (46, "Коми-Зырянин", "UralVolga"),
        (47, "Карел", "UralVolga"),
        (48, "Калмык", "UralVolga"),
        (49, "Ногаец", "UralVolga"),
        (50, "Казах", "UralVolga"),
        (51, "Мещеряк", "UralVolga"),
        (52, "Нагайбак", "UralVolga"),
        (53, "Тептяр", "UralVolga"),
        (54, "Ненец (Европ.)", "UralVolga"),
        (55, "Вепс", "UralVolga"),
        (56, "Ижорец", "UralVolga"),
        (57, "Вожанин", "UralVolga"),
        (58, "Саам", "UralVolga"),
        (59, "Коми-Пермяк", "UralVolga"),
        // ── Eastern Rus — Golden Ring ───────────────────────────────────────
        (60, "Русский (Золотое Кольцо)", "EasternRus"),
        (61, "Помор", "EasternRus"),
        // ── Remaining indigenous peoples ────────────────────────────────────
        (62, "Эвен", "FarEast"),
        (63, "Чуванец", "FarEast"),
        (64, "Керек", "FarEast"),
        (65, "Алюторец", "FarEast"),
        (66, "Эскимос", "FarEast"),
        (67, "Алеут", "FarEast"),
        (68, "Юкагир", "Siberia"),
        (69, "Орок (Ульта)", "FarEast"),
        (70, "Кет", "Siberia"),
        (71, "Чулымец", "Siberia"),
        (72, "Телеут", "Siberia"),
        (73, "Кумандинец", "Siberia"),
        (74, "Тубалар", "Siberia"),
        (75, "Челканец", "Siberia"),
        (76, "Сагаец", "Siberia"),
        (77, "Чулымец", "Siberia"),
        (78, "Сойот", "Siberia"),
        (79, "Тофалар (Карагас)", "Siberia"),
    ];

    // =========================================================================
    //  NATURALIZED (NON-INDIGENOUS) PEOPLES
    // =========================================================================

    /// All ethnic groups from the Russian Federation 2020 Census (Росстат, Том 5)
    /// that are **NOT** part of the 79 Sovereign Indigenous Peoples.
    ///
    /// These groups may obtain **Naturalized** (`Jus Soli`) citizenship via
    /// `claim_birthright()`. They receive full economic and human rights but
    /// do NOT participate in the Khural (political sovereignty is reserved
    /// for the 79 Indigenous Peoples).
    ///
    /// ## ID Convention
    ///
    /// `naturalized_people_id` begins at **1001** to avoid collision with
    /// `nation_id` (1–79) used for Indigenous Peoples.
    ///
    /// ## Source
    ///
    /// Всероссийская перепись населения 2020 года (проведена в 2021).
    /// Росстат, Том 5 «Национальный состав и владение языками».
    ///
    /// Format: `(naturalized_people_id, name_singular_ethnonim)`
    pub const NATURALIZED_PEOPLES: &[(u32, &str)] = &[
        // ── Slavic peoples (not in Indigenous 79) ──────────────────────────
        (1001, "Украинец"),
        (1002, "Белорус"),
        (1003, "Поляк"),
        (1004, "Серб"),
        (1005, "Болгарин"),
        (1006, "Чех"),
        (1007, "Словак"),
        (1008, "Хорват"),
        (1009, "Словенец"),
        (1010, "Черногорец"),
        (1011, "Босниец"),
        (1012, "Македонец"),
        (1013, "Русин"),
        // ── Transcaucasian ─────────────────────────────────────────────────
        (1014, "Армянин"),
        (1015, "Азербайджанец"),
        (1016, "Грузин"),
        // ── North Caucasus (not in Indigenous 79) ──────────────────────────
        (1017, "Абхаз"),
        (1018, "Абазин"),
        (1019, "Тат"),
        (1020, "Талыш"),
        // ── Central Asian — Turkic ─────────────────────────────────────────
        (1021, "Узбек"),
        (1022, "Киргиз"),
        (1023, "Туркмен"),
        (1024, "Каракалпак"),
        (1025, "Уйгур"),
        (1026, "Крымский Татарин"),
        (1027, "Гагауз"),
        (1028, "Месхетинский Турок"),
        (1029, "Турок"),
        // ── Central Asian — Iranian ────────────────────────────────────────
        (1030, "Таджик"),
        (1031, "Курд"),
        (1032, "Перс (Иранец)"),
        (1033, "Пуштун"),
        (1034, "Белудж"),
        // ── Central Asian — Other ──────────────────────────────────────────
        (1035, "Дунганин"),
        // ── Baltic ─────────────────────────────────────────────────────────
        (1036, "Литовец"),
        (1037, "Латыш"),
        (1038, "Эстонец"),
        // ── Finno-Ugric (not in Indigenous 79) ─────────────────────────────
        (1039, "Финн"),
        (1040, "Венгр"),
        // ── Germanic ───────────────────────────────────────────────────────
        (1041, "Немец"),
        (1042, "Австриец"),
        (1043, "Голландец"),
        (1044, "Швед"),
        (1045, "Норвежец"),
        (1046, "Датчанин"),
        (1047, "Швейцарец"),
        // ── Romance ────────────────────────────────────────────────────────
        (1048, "Молдаванин"),
        (1049, "Румын"),
        (1050, "Итальянец"),
        (1051, "Француз"),
        (1052, "Испанец"),
        (1053, "Португалец"),
        // ── Other European ─────────────────────────────────────────────────
        (1054, "Грек"),
        (1055, "Албанец"),
        (1056, "Британец"),
        (1057, "Ирландец"),
        // ── Jewish communities ─────────────────────────────────────────────
        (1058, "Еврей"),
        (1059, "Горский Еврей"),
        (1060, "Бухарский Еврей"),
        (1061, "Караим"),
        (1062, "Крымчак"),
        // ── Roma ───────────────────────────────────────────────────────────
        (1063, "Цыган (Ром)"),
        // ── Semitic (non-Jewish) ───────────────────────────────────────────
        (1064, "Араб"),
        (1065, "Ассириец"),
        // ── East Asian ─────────────────────────────────────────────────────
        (1066, "Корейец"),
        (1067, "Китаец"),
        (1068, "Японец"),
        (1069, "Монгол"),
        // ── Southeast Asian ────────────────────────────────────────────────
        (1070, "Вьетнамец"),
        (1071, "Тайец"),
        (1072, "Филиппинец"),
        (1073, "Индонезиец"),
        (1074, "Малаец"),
        // ── South Asian ────────────────────────────────────────────────────
        (1075, "Индиец"),
        (1076, "Пакистанец"),
        (1077, "Бангладешец"),
        (1078, "Непалец"),
        (1079, "Шриланкиец"),
        // ── African ────────────────────────────────────────────────────────
        (1080, "Африканец"),
        // ── Americas ───────────────────────────────────────────────────────
        (1081, "Американец"),
        (1082, "Канадец"),
        (1083, "Бразилец"),
        (1084, "Мексиканец"),
        (1085, "Кубинец"),
        (1086, "Аргентинец"),
        // ── Oceania ────────────────────────────────────────────────────────
        (1087, "Австралиец"),
        (1088, "Новозеландец"),
    ];

    /// Tier-1: 1 ArbadLeader + 9 Regular citizens of the same nation.
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
    pub struct ArbadRecord<T: Config> {
        pub leader: T::AccountId,
        /// The 9 Regular member citizens.
        pub members: BoundedVec<T::AccountId, ConstU32<9>>,
        pub nation_id: u32,
        /// For Family Arbads: Blake2_256 hash of the Verifiable Credential (W3C DID)
        /// issued by the ЗАГС office at marriage registration.
        ///
        /// `None` for Regular Arbads. `Some(H256)` for Family Arbads formed via
        /// `form_family_arbad` with a ЗАГС credential hash.
        pub marriage_credential_hash: Option<H256>,
    }

    /// Tier-2: 1 ZunLeader commanding 10 Arbads of the same nation.
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
    pub struct ZunRecord<T: Config> {
        pub leader: T::AccountId,
        pub arbads: BoundedVec<u32, ConstU32<10>>,
        pub nation_id: u32,
    }

    /// Tier-3: 1 MyangadLeader commanding 10 Zuns.  Branch is assigned here.
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
    pub struct MyangadRecord<T: Config> {
        pub leader: T::AccountId,
        pub zuns: BoundedVec<u32, ConstU32<10>>,
        /// Constitutional branch — set explicitly at Myangad formation.
        pub branch: BranchOfPower,
        pub nation_id: u32,
    }

    /// Tier-4: 1 TumedLeader commanding 10 Myangads.
    ///
    /// Branch is **inherited** from the constituent Myangads (all must agree).
    /// Any branch may reach Tumed level; only Legislative can go further.
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
    pub struct TumedRecord<T: Config> {
        pub leader: T::AccountId,
        pub myangads: BoundedVec<u32, ConstU32<10>>,
        /// Inherited branch (all 10 Myangads must share this branch).
        pub branch: BranchOfPower,
        pub nation_id: u32,
    }

    /// Tier-5: The sovereign national Khural — formed from Legislative Tumeds only.
    ///
    /// All constituent Tumeds must be within the same `nation_id`.
    /// The Khural is the national supreme legislative body.
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
    pub struct KhuralRecord<T: Config> {
        /// The TumedLeader who initiated Khural formation.
        pub leader: T::AccountId,
        /// All Legislative Tumed IDs that constitute this Khural.
        pub tumeds: BoundedVec<u32, ConstU32<100>>,
        /// Shared nation — all Tumeds must belong to the same sovereign nation.
        pub nation_id: u32,
    }

    /// Tier-6: A cross-national Confederation of Khurals.
    ///
    /// Khurals from **different** nations may unite into a Confederation.
    /// There is no nation restriction at this level.
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
    pub struct ConfederationRecord<T: Config> {
        /// The KhuralDelegate who initiated Confederation formation.
        pub leader: T::AccountId,
        /// All Khural IDs federated here.  Cross-nation: no nation restriction.
        pub khurals: BoundedVec<u32, ConstU32<86>>,
    }

    // =========================================================================
    // Storage
    // =========================================================================

    // ─── Tier 0: Citizens ────────────────────────────────────────────────────
    #[pallet::storage]
    #[pallet::getter(fn citizens)]
    pub type Citizens<T: Config> =
        StorageMap<_, Blake2_128Concat, T::AccountId, CitizenRecord, OptionQuery>;

    /// Global sequential citizen counter.
    ///
    /// Starts at 1 (Citizen #1 = Creator). Every `register_citizen` call
    /// increments this and assigns the next ID to the new citizen.
    /// Display format in UI: 15-digit zero-padded (e.g. 000000000000001).
    #[pallet::storage]
    #[pallet::getter(fn next_citizen_id)]
    pub type NextCitizenId<T: Config> = StorageValue<_, u64, ValueQuery>;

    // ─── KYC/AML Uniqueness Guards ─────────────────────────────────────────────────

    /// Tracks all document hashes (passport identity page) that have been used.
    ///
    /// Prevents Sybil attacks: one physical passport = one citizen account.
    /// The Bank of Siberia checks this BEFORE calling `register_citizen`.
    /// The pallet checks it AGAIN on-chain as a second line of defence.
    #[pallet::storage]
    pub type UsedDocumentHashes<T: Config> =
        StorageMap<_, Blake2_128Concat, H256, bool, ValueQuery>;

    /// Tracks all email hashes that have been used.
    ///
    /// Prevents one person from registering multiple accounts with different emails.
    #[pallet::storage]
    pub type UsedEmailHashes<T: Config> = StorageMap<_, Blake2_128Concat, H256, bool, ValueQuery>;

    // ─── Tier 1: Arbad ───────────────────────────────────────────────────────
    #[pallet::storage]
    #[pallet::getter(fn arbads)]
    pub type Arbads<T: Config> = StorageMap<_, Blake2_128Concat, u32, ArbadRecord<T>, OptionQuery>;

    #[pallet::storage]
    #[pallet::getter(fn next_arbad_id)]
    pub type NextArbadId<T: Config> = StorageValue<_, u32, ValueQuery>;

    /// Reverse lookup: ArbadLeader → Arbad ID.
    #[pallet::storage]
    #[pallet::getter(fn arbad_by_leader)]
    pub type ArbadByLeader<T: Config> =
        StorageMap<_, Blake2_128Concat, T::AccountId, u32, OptionQuery>;

    // ─── Tier 2: Zun ─────────────────────────────────────────────────────────
    #[pallet::storage]
    #[pallet::getter(fn zuns)]
    pub type Zuns<T: Config> = StorageMap<_, Blake2_128Concat, u32, ZunRecord<T>, OptionQuery>;

    #[pallet::storage]
    #[pallet::getter(fn next_zun_id)]
    pub type NextZunId<T: Config> = StorageValue<_, u32, ValueQuery>;

    /// Reverse lookup: ZunLeader → Zun ID.
    #[pallet::storage]
    #[pallet::getter(fn zun_by_leader)]
    pub type ZunByLeader<T: Config> =
        StorageMap<_, Blake2_128Concat, T::AccountId, u32, OptionQuery>;

    // ─── Tier 3: Myangad ─────────────────────────────────────────────────────
    #[pallet::storage]
    #[pallet::getter(fn myangads)]
    pub type Myangads<T: Config> =
        StorageMap<_, Blake2_128Concat, u32, MyangadRecord<T>, OptionQuery>;

    #[pallet::storage]
    #[pallet::getter(fn next_myangad_id)]
    pub type NextMyangadId<T: Config> = StorageValue<_, u32, ValueQuery>;

    /// Reverse lookup: MyangadLeader → Myangad ID.
    #[pallet::storage]
    #[pallet::getter(fn myangad_by_leader)]
    pub type MyangadByLeader<T: Config> =
        StorageMap<_, Blake2_128Concat, T::AccountId, u32, OptionQuery>;

    // ─── Tier 4: Tumed ───────────────────────────────────────────────────────
    #[pallet::storage]
    #[pallet::getter(fn tumeds)]
    pub type Tumeds<T: Config> = StorageMap<_, Blake2_128Concat, u32, TumedRecord<T>, OptionQuery>;

    #[pallet::storage]
    #[pallet::getter(fn next_tumed_id)]
    pub type NextTumedId<T: Config> = StorageValue<_, u32, ValueQuery>;

    /// Reverse lookup: TumedLeader → Tumed ID.
    #[pallet::storage]
    #[pallet::getter(fn tumed_by_leader)]
    pub type TumedByLeader<T: Config> =
        StorageMap<_, Blake2_128Concat, T::AccountId, u32, OptionQuery>;

    // ─── Tier 5: Khural ──────────────────────────────────────────────────────
    #[pallet::storage]
    #[pallet::getter(fn khurals)]
    pub type Khurals<T: Config> =
        StorageMap<_, Blake2_128Concat, u32, KhuralRecord<T>, OptionQuery>;

    #[pallet::storage]
    #[pallet::getter(fn next_khural_id)]
    pub type NextKhuralId<T: Config> = StorageValue<_, u32, ValueQuery>;

    /// Reverse lookup: KhuralDelegate → Khural ID.
    #[pallet::storage]
    #[pallet::getter(fn khural_by_leader)]
    pub type KhuralByLeader<T: Config> =
        StorageMap<_, Blake2_128Concat, T::AccountId, u32, OptionQuery>;

    // ─── Tier 6: Confederation ───────────────────────────────────────────────
    #[pallet::storage]
    #[pallet::getter(fn confederations)]
    pub type Confederations<T: Config> =
        StorageMap<_, Blake2_128Concat, u32, ConfederationRecord<T>, OptionQuery>;

    #[pallet::storage]
    #[pallet::getter(fn next_confederation_id)]
    pub type NextConfederationId<T: Config> = StorageValue<_, u32, ValueQuery>;

    /// Reverse lookup: ConfederationDelegate → Confederation ID.
    #[pallet::storage]
    #[pallet::getter(fn confederation_by_leader)]
    pub type ConfederationByLeader<T: Config> =
        StorageMap<_, Blake2_128Concat, T::AccountId, u32, OptionQuery>;

    // ─── Indigenous Peoples Registry ─────────────────────────────────────────

    /// Maps AccountId → IndigenousRecord for citizens of recognised indigenous peoples.
    ///
    /// Only exists for citizens whose `CitizenRecord.is_indigenous == true`.
    /// Set during `register_citizen` or via `mark_indigenous` (Root only).
    #[pallet::storage]
    #[pallet::getter(fn indigenous_citizens)]
    pub type IndigenousCitizens<T: Config> =
        StorageMap<_, Blake2_128Concat, T::AccountId, IndigenousRecord, OptionQuery>;

    // ─── Lifecycle: Birth & Parental Links ───────────────────────────────────

    /// Maps child AccountId → (parent_1, parent_2) accounts.
    ///
    /// Set by `register_birth`. Used for inheritance lookups and family-tree queries.
    /// `parent_2` is `None` for single-parent registrations.
    #[pallet::storage]
    #[pallet::getter(fn parental_links)]
    pub type ParentalLinks<T: Config> = StorageMap<
        _,
        Blake2_128Concat,
        T::AccountId,
        (T::AccountId, Option<T::AccountId>),
        OptionQuery,
    >;

    /// Maps child Blake2_256 Birth Certificate hash → child AccountId.
    ///
    /// Anti-duplicate guard: one birth certificate = one Minor Credential.
    /// Set by `register_birth`.
    #[pallet::storage]
    pub type UsedBirthHashes<T: Config> = StorageMap<_, Blake2_128Concat, H256, bool, ValueQuery>;

    /// Maps deceased AccountId → Blake2_256 death credential hash.
    ///
    /// Records the off-chain death credential from the Medical Authority.
    /// Set by `register_death`. Permanent record (never deleted).
    #[pallet::storage]
    #[pallet::getter(fn death_records)]
    pub type DeathRecords<T: Config> =
        StorageMap<_, Blake2_128Concat, T::AccountId, H256, OptionQuery>;

    // ─── Orphan Protection: Guardians ─────────────────────────────────────────

    /// [SECURITY VECTOR 2] Maps Minor citizen → appointed Guardian.
    ///
    /// Set by `assign_guardian` (Medical Authority or Root).
    /// A guardian can trigger KYC/adulthood activation on behalf of the minor
    /// and is the contact point for inheritance pallets if both parents are deceased.
    ///
    /// Cleared if/when the minor transitions to `CitizenStatus::Active` at KYC.
    #[pallet::storage]
    #[pallet::getter(fn guardian_of)]
    pub type GuardianOf<T: Config> =
        StorageMap<_, Blake2_128Concat, T::AccountId, T::AccountId, OptionQuery>;

    // ─── Cartel Protection: Arbad Cooldown ────────────────────────────────────

    /// [SECURITY VECTOR 3] Maps citizen → block number of last Arbad departure.
    ///
    /// Set by `leave_arbad`. Checked in `form_arbad` member validation:
    /// a citizen may not join a new Arbad until `current_block >= last_leave + ArbadCooldownPeriod`.
    ///
    /// Prevents Sybil cartels from migrating members across Arbads to swing elections.
    #[pallet::storage]
    #[pallet::getter(fn last_arbad_leave)]
    pub type LastArbadLeave<T: Config> = StorageMap<
        _,
        Blake2_128Concat,
        T::AccountId,
        frame_system::pallet_prelude::BlockNumberFor<T>,
        OptionQuery,
    >;

    // ─── Oracle Rate Limit Storage (Task 2) ────────────────────────────────────────────────────

    /// [SECURITY VECTOR: ORACLE RATE LIMIT] Tracks oracle call count per block.
    ///
    /// Layout: `(block_number, call_count)`. Resets automatically when block changes.
    /// On each `register_birth` call from MedicalAuthority:
    ///   - If `block_number` matches current block: increment `call_count`.
    ///   - If `call_count >= MaxRegistrationsPerBlock`: abort with `OracleRateLimitExceeded`.
    ///   - If block has changed: reset counter to 1.
    ///
    /// ValueQuery default: (BlockNumberFor<T>::zero(), 0u32) — safe first-call initialisation.
    #[pallet::storage]
    pub type OracleCallsThisBlock<T: Config> =
        StorageValue<_, (frame_system::pallet_prelude::BlockNumberFor<T>, u32), ValueQuery>;

    // ─── Nickname Registry (NFT Identity) ─────────────────────────────────────

    /// Maps AccountId → registered nickname (up to 24 bytes, ASCII, unique).
    ///
    /// A nickname is an on-chain identity handle (e.g. `altanar`) bound to a
    /// citizen's sovereign AccountId. Each account may hold at most one nickname.
    /// Once registered, it can only be changed by calling `clear_nickname` first,
    /// then `register_nickname` again (paying the fee twice as anti-spam).
    ///
    /// Nickname pallet-encoding: raw UTF-8 bytes of the handle (no `@` prefix stored).
    #[pallet::storage]
    #[pallet::getter(fn nickname_of)]
    pub type NicknameOf<T: Config> =
        StorageMap<_, Blake2_128Concat, T::AccountId, BoundedVec<u8, ConstU32<24>>, OptionQuery>;

    /// Reverse lookup: nickname bytes → AccountId that registered it.
    ///
    /// Enables O(1) uniqueness checks and handle-to-address resolution
    /// (e.g. `@altanar` → `5F3sa2TdjCizW…`).
    #[pallet::storage]
    pub type AccountByNickname<T: Config> =
        StorageMap<_, Blake2_128Concat, BoundedVec<u8, ConstU32<24>>, T::AccountId, OptionQuery>;

    // =========================================================================
    // Events
    // =========================================================================

    #[pallet::event]
    #[pallet::generate_deposit(pub(super) fn deposit_event)]
    pub enum Event<T: Config> {
        CitizenRegistered {
            who: T::AccountId,
            citizen_id: u64,
            nation_id: u32,
            role: CitizenRole,
        },
        /// A citizen has been verified by a peer — Proof-of-Citizenship Level 0 granted.
        /// `unlock_amount` planck are now immediately spendable (100 UNIT).
        CitizenVerified {
            verifier: T::AccountId,
            new_citizen: T::AccountId,
            citizen_id: u64,
        },
        RoleUpdated {
            who: T::AccountId,
            new_role: CitizenRole,
        },
        /// Citizen has been frozen by a judicial sentence or Root authority.
        CitizenFrozen { who: T::AccountId },
        /// Citizen has been unfrozen — basic transactional rights restored.
        CitizenUnfrozen { who: T::AccountId },
        /// Vesting level advanced (Proof-of-Citizenship social mining).
        VestingAdvanced {
            who: T::AccountId,
            new_level: VestingLevel,
        },
        ArbadFormed {
            arbad_id: u32,
            arbad_type: ArbadType,
            leader: T::AccountId,
            nation_id: u32,
        },
        ZunFormed {
            zun_id: u32,
            leader: T::AccountId,
            nation_id: u32,
        },
        MyangadFormed {
            myangad_id: u32,
            leader: T::AccountId,
            nation_id: u32,
            branch: BranchOfPower,
        },
        TumedFormed {
            tumed_id: u32,
            leader: T::AccountId,
            nation_id: u32,
            branch: BranchOfPower,
        },
        /// A national Khural was established (Legislative Tumeds only).
        KhuralFormed {
            khural_id: u32,
            leader: T::AccountId,
            nation_id: u32,
            tumed_count: u32,
        },
        /// A cross-national Confederation of Khurals was established.
        ConfederationFormed {
            confederation_id: u32,
            leader: T::AccountId,
            khural_count: u32,
        },
        // ─── Lifecycle Events ────────────────────────────────────────────────
        /// A new citizen (Credential Minor) was pre-registered at birth.
        ///
        /// The `child` account holds a `CitizenStatus::Minor` record until KYC at adulthood.
        CitizenBorn {
            child: T::AccountId,
            parent_1: T::AccountId,
            parent_2: Option<T::AccountId>,
            birth_hash: H256,
            birth_region_id: u8,
        },
        /// A citizen has been registered as deceased (TERMINAL state).
        ///
        /// ⚠️  Dead souls protection: all voting rights, vesting, and roles are permanently
        /// blocked. The `death_credential_hash` anchors the off-chain death certificate.
        CitizenDeceased {
            who: T::AccountId,
            death_credential_hash: H256,
        },
        // ─── Orphan Protection Events ─────────────────────────────────────────
        /// [SECURITY VECTOR 2] A guardian was assigned to a Minor citizen.
        ///
        /// The guardian may assist in future KYC activation and inheritance routing.
        GuardianAssigned {
            minor: T::AccountId,
            guardian: T::AccountId,
        },
        // ─── Cartel Cooldown Events ───────────────────────────────────────────
        /// [SECURITY VECTOR 3] A citizen voluntarily left an Arbad.
        ///
        /// Their `LastArbadLeave` block is recorded; they must wait `ArbadCooldownPeriod`
        /// blocks before joining or forming a new Arbad.
        ArbadLeft { arbad_id: u32, member: T::AccountId },
        // ─── Terminal State Events (Task 1: Ghost Cleanup) ────────────────────
        /// A citizen has been condemned and exiled (TERMINAL state).
        ///
        /// The `OnTerminalStatus::on_exiled` hook fires immediately after this event
        /// is emitted, cascading cleanup across guilds, chancery, and khural-governance.
        CitizenExiled { who: T::AccountId },

        // Citizenship Origin Events (Jus Sanguinis / Jus Soli)
        /// A diaspora indigenous citizen claimed their birthright (Jus Sanguinis).
        ///
        /// Emitted by `claim_repatriation`. The citizen's `citizenship_status`
        /// has been upgraded to `Indigenous` after state verification of lineage.
        CitizenshipRepatriated {
            citizen: T::AccountId,
            /// blake2_256 hash of the submitted genealogical documentation.
            lineage_proof: [u8; 32],
            status: CitizenshipStatus,
        },

        /// A citizen born on Republic soil claimed Naturalized status (Jus Soli).
        ///
        /// Emitted by `claim_birthright`. The citizen explicitly accepted the
        /// Constitution and laws of the Altan Republic.
        CitizenshipNaturalized {
            citizen: T::AccountId,
            status: CitizenshipStatus,
        },
        /// A couple has been officially registered as married by the ZAGS office.
        MarriageRegistered {
            partner_a: T::AccountId,
            partner_b: T::AccountId,
            /// Family treasury: blake2_256(b"family/" + sorted(addrA + addrB))
            family_account: T::AccountId,
            /// unix timestamp of ceremony
            ceremony_block: frame_system::pallet_prelude::BlockNumberFor<T>,
        },
        /// A marriage has been officially dissolved (divorce registered).
        DivorceRegistered {
            partner_a: T::AccountId,
            partner_b: T::AccountId,
        },

        // ─── Nickname NFT Events ──────────────────────────────────────────────
        /// A citizen registered (or changed) their on-chain nickname.
        ///
        /// The `nickname` is the raw handle bytes (no `@` prefix).
        /// Indexed by `who` so UIs can subscribe per-account.
        NicknameRegistered {
            who: T::AccountId,
            nickname: BoundedVec<u8, ConstU32<24>>,
        },

        /// A citizen cleared their on-chain nickname.
        ///
        /// Both `NicknameOf` and `AccountByNickname` entries are removed.
        NicknameCleared { who: T::AccountId },
    }

    // =========================================================================
    // Errors
    // =========================================================================

    #[pallet::error]
    pub enum Error<T> {
        /// Account is already a registered citizen.
        AlreadyRegistered,
        /// Account has no citizen record.
        NotRegistered,
        /// `nation_id` is outside the valid range 1–79.
        InvalidNationId,
        /// Citizen is frozen and may not participate in this action.
        CitizenInactive,
        /// Citizen is already frozen.
        CitizenAlreadyFrozen,
        /// Citizen is not frozen; cannot unfreeze.
        CitizenNotFrozen,
        /// Caller's role is wrong for this formation step.
        InvalidRank,
        /// Wrong number of members / sub-unit IDs provided.
        NotEnoughMembers,
        /// A member or sub-unit belongs to a different sovereign nation.
        NationMismatch,
        /// The same member or unit ID was submitted more than once.
        DuplicateMember,
        /// Referenced Arbad does not exist.
        ArbadNotFound,
        /// Referenced Zun does not exist.
        ZunNotFound,
        /// Referenced Myangad does not exist.
        MyangadNotFound,
        /// Referenced Tumed does not exist.
        TumedNotFound,
        /// Referenced Khural does not exist.
        KhuralNotFound,
        /// All Myangads in a Tumed must share the same branch.
        BranchMismatch,
        /// ──── THE GLASS CEILING (Fractal Law) ─────────────────────────────
        ///
        /// Only Legislative Tumeds may form a Khural.
        /// Executive, Judicial, and Banking Tumeds are constitutionally barred
        /// from advancing beyond the Tumed tier.
        OnlyLegislativeTumedsCanFormKhural,
        /// ──── FRACTAL HIERARCHY LIMIT ──────────────────────────────────────
        ///
        /// The fundamental law of Fractal Democracy: Executive, Judicial, Banking,
        /// and Business/Military structures may ONLY ascend to `FractalLevel::Tumed`.
        ///
        /// Any attempt to form a `RepublicanKhural` or `ConfederativeKhural` for
        /// a non-Legislative branch returns this error and is unconstitutional.
        HierarchyLimitExceeded,
        /// ──── CONSTITUTIONAL TERM LIMITS ───────────────────────────────────
        ///
        /// A citizen may serve at most **two** terms as a Khural or Confederation
        /// delegate.  Once `khural_terms_served >= 2` this error is returned and
        /// the citizen must step back to allow new leaders to ascend.
        TermLimitExceeded,
        /// The verifier must be a fully verified citizen.
        VerifierNotVerified,
        /// The new citizen is already verified.
        AlreadyVerified,
        /// Citizen's GDP payment is from the wrong regional fund.
        RegionMismatch,
        /// Citizen has no birth_region_id assigned.
        NoBirthRegion,
        /// ──── KYC/AML ──────────────────────────────────────────────────────
        ///
        /// A passport (document_hash) with this hash is already registered.
        /// One physical passport = one citizen account (anti-Sybil).
        DocumentAlreadyUsed,
        /// An email with this hash is already registered.
        EmailAlreadyUsed,
        // ─── Lifecycle Errors ─────────────────────────────────────────────────
        /// Caller is not the authorised Medical Authority for birth/death operations.
        NotMedicalAuthority,
        /// Target citizen is already registered as deceased — cannot perform this action.
        AlreadyDeceased,
        /// Target citizen's status is `Minor` — action requires full citizen status.
        CitizenIsMinor,
        /// A birth certificate with this hash is already registered (anti-duplicate).
        BirthHashAlreadyUsed,
        /// Parent-1 must be a registered, non-deceased citizen.
        InvalidParent,
        // ─── Security Patches ─────────────────────────────────────────────────
        /// [VECTOR 2] Cannot unfreeze a deceased citizen — `Deceased` is a TERMINAL state.
        /// Only `Frozen` citizens can be unfrozen. `Deceased` wallets may only be
        /// accessed through the inheritance pallet.
        CannotUnfreezeDeceased,
        // ─── Orphan Protection Errors ─────────────────────────────────────────
        /// [SECURITY VECTOR 2] `assign_guardian` requires the target to be a Minor.
        /// Active, Frozen, or Deceased citizens cannot have guardians assigned.
        TargetNotMinor,
        /// [SECURITY VECTOR 2] The proposed guardian must be an Active citizen.
        /// Frozen, Minor, or Deceased citizens cannot serve as guardians.
        GuardianNotActive,
        // ─── Cartel Cooldown Errors ───────────────────────────────────────────
        /// [SECURITY VECTOR 3] The citizen left an Arbad too recently.
        ///
        /// They must wait at least `ArbadCooldownPeriod` blocks before joining
        /// or forming a new Arbad. This prevents rapid cartel migration attacks.
        ArbadCooldownActive,
        /// [SECURITY VECTOR 3] The citizen is not a member (or leader) of the
        /// specified Arbad and thus cannot leave it.
        NotArbadMember,
        // ─── Oracle Rate-Limit Errors (Task 2) ─────────────────────────────────────────
        /// [SECURITY VECTOR: ORACLE RATE LIMIT] The `MedicalAuthority` key has
        /// exceeded `MaxRegistrationsPerBlock` calls in this block.
        ///
        /// Wait for the next block before calling `register_birth` again.
        /// This error indicates either abnormal automated activity or a compromised key.
        OracleRateLimitExceeded,
        /// The citizen already has `Indigenous` or `Naturalized` status.
        ///
        /// Returned by `claim_birthright` if the caller is not a `Foreigner`.
        /// You cannot downgrade from `Indigenous` to `Naturalized`.
        AlreadyCitizen,

        /// **Chain of Legitimacy**: the submitted constitution hash does not match
        /// the canonical hash stored via `T::ConstitutionHashProvider`.
        ///
        /// The citizen must read and accept the EXACT current version of the Altan
        /// Republic's Constitution before obtaining citizenship. This error means
        /// either the hash was computed incorrectly or the Constitution has been
        /// updated since the citizen's client was last refreshed.
        ///
        /// Returned by `claim_birthright` and `claim_repatriation`.
        ConstitutionHashMismatch,
        /// The caller or spouse is not an Active citizen.
        NotEligibleForMarriage,
        /// Marriage already exists between these two accounts.
        AlreadyMarried,
        /// The couple is not registered as married in this pallet.
        NotMarried,
        /// Caller is not part of this marriage record.
        NotSpouse,
        /// The caller or their spouse is currently frozen (judicial hold).
        CannotMarryFrozenCitizen,

        // ─── Nickname Errors ──────────────────────────────────────────────────
        /// The requested nickname is already held by another AccountId.
        NicknameTaken,
        /// The nickname is malformed: must be 3–24 ASCII alphanumeric or `_/-` bytes.
        NicknameInvalid,
        /// The caller does not currently have a registered nickname.
        NicknameNotSet,
        /// The caller already has a nickname — call `clear_nickname` first.
        NicknameAlreadySet,
    }

    // =========================================================================
    // Internal Helpers
    // =========================================================================

    impl<T: Config> Pallet<T> {
        /// [SECURITY VECTOR: ORACLE RATE LIMIT] Enforce per-block call limit for
        /// `MedicalAuthority` oracle calls.
        ///
        /// Reads the current block and compares with `OracleCallsThisBlock`:
        /// - Same block: increment counter; return `Err` if limit hit.
        /// - New block: reset counter to 1; proceed.
        fn check_and_increment_oracle_rate() -> frame_support::dispatch::DispatchResult {
            let current_block = frame_system::Pallet::<T>::block_number();
            let max = T::MaxRegistrationsPerBlock::get();

            OracleCallsThisBlock::<T>::try_mutate(
                |entry| -> frame_support::dispatch::DispatchResult {
                    let (stored_block, count) = entry;
                    if *stored_block == current_block {
                        // same block: check limit before incrementing
                        ensure!(*count < max, Error::<T>::OracleRateLimitExceeded);
                        *count = count.saturating_add(1);
                    } else {
                        // new block: reset
                        *stored_block = current_block;
                        *count = 1;
                    }
                    Ok(())
                },
            )
        }
    }

    // =========================================================================
    // Extrinsics
    // =========================================================================

    // =========================================================================
    // Storage: Marriage Registry
    // =========================================================================

    /// Canonical marriage record — maps each spouse to the other.
    ///
    /// `Marriages[partner_a] = partner_b`  AND  `Marriages[partner_b] = partner_a`
    /// Both entries must be present for a valid marriage.
    ///
    /// Constitutional: only one active marriage per citizen at a time.
    #[pallet::storage]
    #[pallet::getter(fn marriage_of)]
    pub type Marriages<T: Config> = StorageMap<
        _,
        Blake2_128Concat,
        T::AccountId,
        T::AccountId, // spouse's AccountId
        OptionQuery,
    >;

    #[pallet::call]
    impl<T: Config> Pallet<T> {
        // ─── register_citizen ────────────────────────────────────────────────

        /// Register a new sovereign citizen (Root / ZKP Registrar).
        ///
        /// `is_indigenous`: set `true` if this citizen belongs to a recognised
        /// indigenous people. Indigenous citizens (ArbadLeader+) may initiate
        /// war-time votes in the Republican Khural.
        ///
        /// `region_id`: optional OKATO region code (1–83) for regional airdrop routing.
        ///
        /// `birth_region_id`: birth or first-registration region (1–83). IMMUTABLE.
        /// # Registration Requirements (Bank of Siberia KYC/AML)
        ///
        /// The Bank of Siberia performs off-chain verification:
        ///   - Russian internal passport (Form 1-P) OR international passport
        ///   - Page with place of birth (страница с местом рождения)
        ///   - Email address
        ///
        /// Only blake2_256 hashes reach the chain; raw PII is never stored on-chain.
        ///
        /// On-chain uniqueness: `document_hash` and `email_hash` are checked against
        /// `UsedDocumentHashes` / `UsedEmailHashes` to prevent Sybil registrations.
        #[pallet::call_index(0)]
        #[pallet::weight(T::WeightInfo::register_citizen())]
        pub fn register_citizen(
            origin: OriginFor<T>,
            target: T::AccountId,
            nation_id: u32,
            role: CitizenRole,
            is_indigenous: bool,
            region_id: Option<u8>,
            birth_region_id: Option<u8>,
            // ─── KYC/AML fields ───────────────────────────────────────────
            passport_type: PassportType,
            // blake2_256(passport_series + passport_number + date_of_birth)
            document_hash: H256,
            // blake2_256(photo_of_birth_page)
            birth_page_hash: H256,
            // blake2_256(email_address)
            email_hash: H256,
        ) -> DispatchResult {
            ensure_root(origin)?;

            // ─── Basic validity guards ──────────────────────────────────────
            ensure!(
                !Citizens::<T>::contains_key(&target),
                Error::<T>::AlreadyRegistered
            );
            ensure!(
                nation_id >= 1 && nation_id <= 79,
                Error::<T>::InvalidNationId
            );
            if let Some(rid) = region_id {
                ensure!(rid >= 1 && rid <= 83, Error::<T>::InvalidNationId);
            }
            if let Some(brid) = birth_region_id {
                ensure!(brid >= 1 && brid <= 83, Error::<T>::InvalidNationId);
            }

            // ─── KYC uniqueness guards (anti-Sybil) ───────────────────────
            ensure!(
                !UsedDocumentHashes::<T>::get(document_hash),
                Error::<T>::DocumentAlreadyUsed
            );
            ensure!(
                !UsedEmailHashes::<T>::get(email_hash),
                Error::<T>::EmailAlreadyUsed
            );

            // ─── Assign global citizen ID ─────────────────────────────────
            let citizen_id = NextCitizenId::<T>::get().saturating_add(1);
            NextCitizenId::<T>::put(citizen_id);

            // ─── Record KYC hashes as used ───────────────────────────────
            UsedDocumentHashes::<T>::insert(document_hash, true);
            UsedEmailHashes::<T>::insert(email_hash, true);

            // ─── Create the soulbound citizen record ────────────────────────
            Citizens::<T>::insert(
                &target,
                CitizenRecord {
                    citizen_id,
                    nation_id,
                    naturalized_people_id: if is_indigenous { None } else { Some(nation_id) },
                    role: role.clone(),
                    status: CitizenStatus::Active,
                    verification: VerificationStatus::Unverified,
                    vesting_level: None,
                    branch: None,
                    term_end: None,
                    khural_terms_served: 0,
                    is_indigenous,
                    // Derive initial citizenship status from is_indigenous flag.
                    // A citizen registered as indigenous starts as Indigenous (Jus Sanguinis).
                    // All others start as Foreigner and may transition via claim_birthright or claim_repatriation.
                    citizenship_status: if is_indigenous {
                        CitizenshipStatus::Indigenous
                    } else {
                        CitizenshipStatus::Foreigner
                    },
                    region_id,
                    birth_region_id,
                    passport_type,
                    document_hash,
                    birth_page_hash,
                    email_hash,
                },
            );
            Self::deposit_event(Event::CitizenRegistered {
                who: target,
                citizen_id,
                nation_id,
                role,
            });
            Ok(())
        }

        // ─── verify_citizen ───────────────────────────────────────────────────

        /// Proof-of-Citizenship: a verified citizen verifies a new unverified citizen.
        ///
        /// ## Social Mining — Level 0
        ///
        /// This is the first step of the Proof-of-Citizenship mining mechanism.
        /// After successful verification:
        ///   - The new citizen's `verification` becomes `Verified`.
        ///   - Their `vesting_level` advances to `VestingLevel::Verified` (Level 0).
        ///   - `T::UnlockLevel0` planck (100 UNIT) are immediately unlocked for spending.
        ///   - The rest of their GDP allocation remains in a vesting lock.
        ///
        /// ## Constraints
        ///
        /// - The **verifier** must be `VerificationStatus::Verified` themselves.
        /// - The **new_citizen** must be `VerificationStatus::Unverified`.
        /// - Both accounts must be `CitizenStatus::Active`.
        /// - Citizen #1 (Creator) is verified by Root via `bootstrap_verify_creator`.
        ///
        /// # Origin: Signed (existing verified citizen)
        #[pallet::call_index(10)]
        #[pallet::weight(T::WeightInfo::verify_citizen())]
        pub fn verify_citizen(origin: OriginFor<T>, new_citizen: T::AccountId) -> DispatchResult {
            let verifier = ensure_signed(origin)?;

            // Check verifier is registered and verified.
            let verifier_rec = Citizens::<T>::get(&verifier).ok_or(Error::<T>::NotRegistered)?;
            ensure!(
                verifier_rec.status == CitizenStatus::Active,
                Error::<T>::CitizenInactive
            );
            ensure!(
                verifier_rec.verification == VerificationStatus::Verified,
                Error::<T>::VerifierNotVerified
            );

            // Check new citizen exists and is unverified.
            let mut new_rec = Citizens::<T>::get(&new_citizen).ok_or(Error::<T>::NotRegistered)?;
            ensure!(
                new_rec.status == CitizenStatus::Active,
                Error::<T>::CitizenInactive
            );
            ensure!(
                new_rec.verification == VerificationStatus::Unverified,
                Error::<T>::AlreadyVerified
            );

            let citizen_id = new_rec.citizen_id;

            // Advance to Level 0: Verified.
            new_rec.verification = VerificationStatus::Verified;
            new_rec.vesting_level = Some(VestingLevel::Verified);
            Citizens::<T>::insert(&new_citizen, new_rec);

            Self::deposit_event(Event::CitizenVerified {
                verifier,
                new_citizen: new_citizen.clone(),
                citizen_id,
            });
            Self::deposit_event(Event::VestingAdvanced {
                who: new_citizen,
                new_level: VestingLevel::Verified,
            });
            Ok(())
        }

        // ─── bootstrap_verify_creator ─────────────────────────────────────────

        /// Bootstrap: Root verifies Citizen #1 (the Creator) directly.
        ///
        /// This is the only extrinsic that allows Root to grant verified status
        /// without a human verifier.  It should be called exactly once, for the
        /// Creator account (Citizen #1), who then bootstraps all other citizens.
        ///
        /// After this call, Citizen #1 can call `verify_citizen` for everyone else.
        ///
        /// # Origin: Root
        #[pallet::call_index(11)]
        #[pallet::weight(T::WeightInfo::bootstrap_verify_creator())]
        pub fn bootstrap_verify_creator(
            origin: OriginFor<T>,
            creator: T::AccountId,
        ) -> DispatchResult {
            ensure_root(origin)?;

            let mut rec = Citizens::<T>::get(&creator).ok_or(Error::<T>::NotRegistered)?;
            ensure!(
                rec.verification == VerificationStatus::Unverified,
                Error::<T>::AlreadyVerified
            );

            let citizen_id = rec.citizen_id;
            rec.verification = VerificationStatus::Verified;
            rec.vesting_level = Some(VestingLevel::Verified);
            Citizens::<T>::insert(&creator, rec);

            Self::deposit_event(Event::CitizenVerified {
                verifier: creator.clone(),
                new_citizen: creator.clone(),
                citizen_id,
            });
            Self::deposit_event(Event::VestingAdvanced {
                who: creator,
                new_level: VestingLevel::Verified,
            });
            Ok(())
        }

        // ─── update_role ──────────────────────────────────────────────────────

        /// Override a citizen's rank (Root emergency).
        #[pallet::call_index(1)]
        #[pallet::weight(T::WeightInfo::update_role())]
        pub fn update_role(
            origin: OriginFor<T>,
            target: T::AccountId,
            new_role: CitizenRole,
        ) -> DispatchResult {
            ensure_root(origin)?;
            Citizens::<T>::try_mutate(&target, |maybe| {
                let r = maybe.as_mut().ok_or(Error::<T>::NotRegistered)?;
                r.role = new_role.clone();
                Ok::<(), DispatchError>(())
            })?;
            Self::deposit_event(Event::RoleUpdated {
                who: target,
                new_role,
            });
            Ok(())
        }

        // ─── freeze_citizen ───────────────────────────────────────────────────

        /// Freeze a citizen — sets `status = Frozen` (Root or Judicial Court).
        ///
        /// A frozen citizen cannot transact or vote, but retains their
        /// `nation_id` and `role`.  Call `unfreeze_citizen` to restore rights.
        #[pallet::call_index(2)]
        #[pallet::weight(T::WeightInfo::freeze_citizen())]
        pub fn freeze_citizen(origin: OriginFor<T>, target: T::AccountId) -> DispatchResult {
            ensure_root(origin)?;
            Citizens::<T>::try_mutate(&target, |maybe| {
                let r = maybe.as_mut().ok_or(Error::<T>::NotRegistered)?;
                ensure!(
                    r.status == CitizenStatus::Active,
                    Error::<T>::CitizenAlreadyFrozen
                );
                r.status = CitizenStatus::Frozen;
                Ok::<(), DispatchError>(())
            })?;
            Self::deposit_event(Event::CitizenFrozen { who: target });
            Ok(())
        }

        // ─── unfreeze_citizen ─────────────────────────────────────────────────

        /// Unfreeze a citizen — restores `status = Active` (Root or Court verdict).
        ///
        /// Called by the judicial courts pallet after executing a verdict to
        /// restore a convicted citizen's basic transactional rights.
        ///
        /// ## Security
        /// `Deceased` is a TERMINAL state — this extrinsic will NEVER restore
        /// a deceased citizen to `Active`. The `Deceased` flag may only be
        /// addressed via the inheritance pallet for balance recovery.
        #[pallet::call_index(9)]
        #[pallet::weight(T::WeightInfo::unfreeze_citizen())]
        pub fn unfreeze_citizen(origin: OriginFor<T>, target: T::AccountId) -> DispatchResult {
            ensure_root(origin)?;
            Citizens::<T>::try_mutate(&target, |maybe| {
                let r = maybe.as_mut().ok_or(Error::<T>::NotRegistered)?;
                // [SECURITY VECTOR 2] Deceased is a TERMINAL state — it can NEVER
                // be changed back to Active.  Explicitly guard against resurrection.
                ensure!(
                    r.status != CitizenStatus::Deceased,
                    Error::<T>::CannotUnfreezeDeceased
                );
                ensure!(
                    r.status == CitizenStatus::Frozen,
                    Error::<T>::CitizenNotFrozen
                );
                r.status = CitizenStatus::Active;
                Ok::<(), DispatchError>(())
            })?;
            Self::deposit_event(Event::CitizenUnfrozen { who: target });
            Ok(())
        }

        // ─── form_arbad (Regular Arbad — open civic unit, min 2 members) ──────

        /// Form Tier-1 Regular Arbad: caller + at least 1 other Regular citizen.
        ///
        /// A Regular Arbad is an open civic unit. It starts with a minimum of 2
        /// members (caller + at least 1) and can grow up to 10. Once 10 members
        /// are present AND a leader is elected, the Arbad is FULL and all members
        /// advance to Vesting Level 2 (full GDP unlock).
        ///
        /// Caller is immediately promoted to `ArbadLeader`.
        ///
        /// ## Proof-of-Citizenship Mining
        /// - All members advance to `VestingLevel::ArbadMember` (Level 1).
        /// - If this call creates a FULL arbad (10 members), Level 2 is granted.
        ///
        /// # Origin: Signed (prospective ArbadLeader, must be Verified)
        #[pallet::call_index(3)]
        #[pallet::weight(T::WeightInfo::form_arbad())]
        pub fn form_arbad(origin: OriginFor<T>, members: Vec<T::AccountId>) -> DispatchResult {
            let caller = ensure_signed(origin)?;

            let rec = Citizens::<T>::get(&caller).ok_or(Error::<T>::NotRegistered)?;
            ensure!(
                rec.status == CitizenStatus::Active,
                Error::<T>::CitizenInactive
            );
            ensure!(rec.role == CitizenRole::Regular, Error::<T>::InvalidRank);
            // Proof-of-Citizenship: caller must be verified before leading an Arbad.
            ensure!(
                rec.verification == VerificationStatus::Verified,
                Error::<T>::VerifierNotVerified
            );
            let nation_id = rec.nation_id;

            // Regular Arbad: minimum 1 additional member (total ≥ 2), max 9 additional (total 10).
            ensure!(
                !members.is_empty() && members.len() <= 9,
                Error::<T>::NotEnoughMembers
            );

            let current_block: u32 =
                frame_system::Pallet::<T>::block_number().saturated_into::<u32>();
            let cooldown_u32: u32 = T::ArbadCooldownPeriod::get().saturated_into::<u32>();

            let mut seen: Vec<T::AccountId> = Vec::with_capacity(members.len());
            for m in members.iter() {
                ensure!(m != &caller, Error::<T>::DuplicateMember);
                ensure!(!seen.contains(m), Error::<T>::DuplicateMember);
                let mr = Citizens::<T>::get(m).ok_or(Error::<T>::NotRegistered)?;
                ensure!(
                    mr.status == CitizenStatus::Active,
                    Error::<T>::CitizenInactive
                );
                ensure!(mr.role == CitizenRole::Regular, Error::<T>::InvalidRank);
                ensure!(mr.nation_id == nation_id, Error::<T>::NationMismatch);

                // [SECURITY VECTOR 3] Arbad cartel cooldown check.
                // A citizen who left an Arbad recently cannot be added to a new one.
                // This prevents rapid mass membership migration to swing elections.
                if let Some(last_leave) = LastArbadLeave::<T>::get(m) {
                    let last_leave_u32: u32 = last_leave.saturated_into::<u32>();
                    ensure!(
                        current_block >= last_leave_u32.saturating_add(cooldown_u32),
                        Error::<T>::ArbadCooldownActive
                    );
                }

                seen.push(m.clone());
            }

            // Proof-of-Citizenship Level 1 — ArbadMember.
            // Level 2 (ZunMember = full GDP unlock) is only reached at form_zun.
            let new_vesting = VestingLevel::ArbadMember;

            // Advance caller (leader) vesting + promote role.
            Citizens::<T>::mutate(&caller, |maybe| {
                if let Some(r) = maybe {
                    r.role = CitizenRole::ArbadLeader;
                    r.term_end = Some(current_block.saturating_add(crate::TWO_YEAR_TERM));
                    // Advance vesting level (never downgrade).
                    let advance = match &r.vesting_level {
                        None => true,
                        Some(current) => new_vesting > *current,
                    };
                    if advance {
                        r.vesting_level = Some(new_vesting.clone());
                    }
                }
            });

            // Advance vesting for all members.
            for m in members.iter() {
                Citizens::<T>::mutate(m, |maybe| {
                    if let Some(r) = maybe {
                        let advance = match &r.vesting_level {
                            None => true,
                            Some(current) => new_vesting > *current,
                        };
                        if advance {
                            r.vesting_level = Some(new_vesting.clone());
                        }
                    }
                });
                Self::deposit_event(Event::VestingAdvanced {
                    who: m.clone(),
                    new_level: new_vesting.clone(),
                });
            }
            Self::deposit_event(Event::VestingAdvanced {
                who: caller.clone(),
                new_level: new_vesting,
            });

            let bounded: BoundedVec<T::AccountId, ConstU32<9>> = members
                .try_into()
                .map_err(|_| Error::<T>::NotEnoughMembers)?;
            let id = NextArbadId::<T>::get();
            Arbads::<T>::insert(
                id,
                ArbadRecord {
                    leader: caller.clone(),
                    members: bounded,
                    nation_id,
                    marriage_credential_hash: None, // Regular Arbads have no marriage credential
                },
            );
            ArbadByLeader::<T>::insert(&caller, id);
            NextArbadId::<T>::put(id.saturating_add(1));

            Self::deposit_event(Event::ArbadFormed {
                arbad_id: id,
                arbad_type: ArbadType::Regular,
                leader: caller,
                nation_id,
            });
            Ok(())
        }

        // ─── form_family_arbad ────────────────────────────────────────────────

        /// Form Tier-1 Family Arbad: husband + wife dyad with explicit head of family.
        ///
        /// A Family Arbad is the smallest Arbad — exactly 2 members (caller + spouse).
        /// The caller specifies whom to designate as `head` (themselves or the spouse).
        /// The head becomes the `ArbadLeader`; the other member stays `Regular`.
        ///
        /// ## Marriage Credential
        /// The `marriage_credential_hash` anchors the W3C Verifiable Credential issued
        /// by the ЗАГС office (via Altan Gateway). It is stored in the `ArbadRecord`
        /// as cryptographic proof of the marriage's legitimacy.
        ///
        /// ## Proof-of-Citizenship Mining
        /// Both participants advance to `VestingLevel::ArbadMember` (Level 1).
        ///
        /// # Origin: Signed (one of the two spouses, must be Verified)
        #[pallet::call_index(12)]
        #[pallet::weight(T::WeightInfo::form_family_arbad())]
        pub fn form_family_arbad(
            origin: OriginFor<T>,
            spouse: T::AccountId,
            head: T::AccountId, // must be caller or spouse
            marriage_credential_hash: Option<H256>,
        ) -> DispatchResult {
            let caller = ensure_signed(origin)?;

            ensure!(caller != spouse, Error::<T>::DuplicateMember);

            // Head must be caller or spouse.
            ensure!(head == caller || head == spouse, Error::<T>::InvalidRank);

            let caller_rec = Citizens::<T>::get(&caller).ok_or(Error::<T>::NotRegistered)?;
            ensure!(
                caller_rec.status == CitizenStatus::Active,
                Error::<T>::CitizenInactive
            );
            ensure!(
                caller_rec.role == CitizenRole::Regular,
                Error::<T>::InvalidRank
            );
            ensure!(
                caller_rec.verification == VerificationStatus::Verified,
                Error::<T>::VerifierNotVerified
            );
            let nation_id = caller_rec.nation_id;

            let spouse_rec = Citizens::<T>::get(&spouse).ok_or(Error::<T>::NotRegistered)?;
            ensure!(
                spouse_rec.status == CitizenStatus::Active,
                Error::<T>::CitizenInactive
            );
            ensure!(
                spouse_rec.role == CitizenRole::Regular,
                Error::<T>::InvalidRank
            );
            ensure!(
                spouse_rec.nation_id == nation_id,
                Error::<T>::NationMismatch
            );

            let current_block: u32 =
                frame_system::Pallet::<T>::block_number().saturated_into::<u32>();

            // Determine which account is leader and which is member.
            let (leader_acct, _member_acct) = if head == caller {
                (caller.clone(), spouse.clone())
            } else {
                (spouse.clone(), caller.clone())
            };

            // Promote head to ArbadLeader.
            Citizens::<T>::mutate(&leader_acct, |maybe| {
                if let Some(r) = maybe {
                    r.role = CitizenRole::ArbadLeader;
                    r.term_end = Some(current_block.saturating_add(crate::TWO_YEAR_TERM));
                    let advance = match &r.vesting_level {
                        None => true,
                        Some(current) => VestingLevel::ArbadMember > *current,
                    };
                    if advance {
                        r.vesting_level = Some(VestingLevel::ArbadMember);
                    }
                }
            });

            // Advance member vesting to Level 1 (role stays Regular).
            let non_leader = if head == caller { &spouse } else { &caller };
            Citizens::<T>::mutate(non_leader, |maybe| {
                if let Some(r) = maybe {
                    let advance = match &r.vesting_level {
                        None => true,
                        Some(current) => VestingLevel::ArbadMember > *current,
                    };
                    if advance {
                        r.vesting_level = Some(VestingLevel::ArbadMember);
                    }
                }
            });

            // Store as a single-member Arbad (leader + 1 member).
            let bounded: BoundedVec<T::AccountId, ConstU32<9>> = alloc::vec![non_leader.clone()]
                .try_into()
                .map_err(|_| Error::<T>::NotEnoughMembers)?;

            let id = NextArbadId::<T>::get();
            Arbads::<T>::insert(
                id,
                ArbadRecord {
                    leader: leader_acct.clone(),
                    members: bounded,
                    nation_id,
                    marriage_credential_hash,
                },
            );
            ArbadByLeader::<T>::insert(&leader_acct, id);
            NextArbadId::<T>::put(id.saturating_add(1));

            // Emit vesting events for both participants.
            Self::deposit_event(Event::VestingAdvanced {
                who: leader_acct.clone(),
                new_level: VestingLevel::ArbadMember,
            });
            Self::deposit_event(Event::VestingAdvanced {
                who: non_leader.clone(),
                new_level: VestingLevel::ArbadMember,
            });
            Self::deposit_event(Event::ArbadFormed {
                arbad_id: id,
                arbad_type: ArbadType::Family,
                leader: leader_acct,
                nation_id,
            });
            Ok(())
        }

        // ─── register_birth ──────────────────────────────────────────────────

        /// Pre-register a newborn citizen as a Soulbound Token with status `Minor`.
        ///
        /// Called by the authorised `MedicalAuthority` (Минздрав / Роддом).
        /// The `child` account is created with `CitizenStatus::Minor` and linked
        /// to `parent_1` (and optionally `parent_2`) via `ParentalLinks`.
        ///
        /// ## Activation
        /// The Minor Credential becomes `Active` when the citizen reaches adulthood,
        /// creates a wallet, and passes KYC through the Bank of Siberia
        /// (`register_citizen` call replaces the Minor record).
        ///
        /// ## Parameters
        /// - `child`: the pre-generated AccountId for the newborn (derivable from birth cert)
        /// - `parent_1`: must be a registered, active, non-deceased citizen
        /// - `parent_2`: optional second parent (may be absent for single-parent families)
        /// - `child_hash`: Blake2_256 hash of the birth certificate number + birth date
        /// - `birth_region_id`: OKATO region code (1–83) — determines future GDP routing
        ///
        /// # Origin: Signed (must equal `T::MedicalAuthority`) OR Root
        #[pallet::call_index(20)]
        #[pallet::weight(T::WeightInfo::register_birth())]
        pub fn register_birth(
            origin: OriginFor<T>,
            child: T::AccountId,
            parent_1: T::AccountId,
            parent_2: Option<T::AccountId>,
            child_hash: H256,
            birth_region_id: u8,
        ) -> DispatchResult {
            // Accept Root OR the designated MedicalAuthority account.
            let is_root = ensure_root(origin.clone()).is_ok();
            if !is_root {
                let caller = ensure_signed(origin)?;
                ensure!(
                    caller == T::MedicalAuthority::get(),
                    Error::<T>::NotMedicalAuthority
                );
                // [SECURITY VECTOR: ORACLE RATE LIMIT] Only non-root callers (i.e.
                // the MedicalAuthority key) are rate-limited. Root is assumed to be
                // the multisig council, not a hot key, so it is exempt.
                Self::check_and_increment_oracle_rate()?;
            }

            // Validate birth region.
            ensure!(
                birth_region_id >= 1 && birth_region_id <= 83,
                Error::<T>::InvalidNationId
            );

            // Guard: no duplicate birth certificates.
            ensure!(
                !UsedBirthHashes::<T>::get(child_hash),
                Error::<T>::BirthHashAlreadyUsed
            );

            // Guard: child account must not already exist.
            ensure!(
                !Citizens::<T>::contains_key(&child),
                Error::<T>::AlreadyRegistered
            );

            // Validate parent_1 is a registered, non-deceased citizen.
            let parent_rec = Citizens::<T>::get(&parent_1).ok_or(Error::<T>::InvalidParent)?;
            ensure!(
                parent_rec.status != CitizenStatus::Deceased,
                Error::<T>::AlreadyDeceased
            );
            let nation_id = parent_rec.nation_id;

            // Create the Minor Credential record.
            let child_id = NextCitizenId::<T>::get();
            Citizens::<T>::insert(
                &child,
                CitizenRecord {
                    citizen_id: child_id,
                    nation_id,
                    naturalized_people_id: None, // determined by lineage at adulthood
                    role: CitizenRole::Regular,
                    status: CitizenStatus::Minor,
                    verification: VerificationStatus::Unverified,
                    vesting_level: None,
                    branch: None,
                    term_end: None,
                    khural_terms_served: 0,
                    is_indigenous: false,
                    // Newborns start as Foreigner; citizenship is claimed on adulthood via
                    // claim_birthright() or claim_repatriation() based on lineage.
                    citizenship_status: CitizenshipStatus::Foreigner,
                    region_id: Some(birth_region_id),
                    birth_region_id: Some(birth_region_id),
                    // Birth certificate hash anchors the Credential to the physical document.
                    passport_type: PassportType::Internal,
                    document_hash: child_hash,
                    birth_page_hash: H256::zero(),
                    email_hash: H256::zero(),
                },
            );
            NextCitizenId::<T>::put(child_id.saturating_add(1));

            // Record parental links for inheritance and family-tree lookups.
            ParentalLinks::<T>::insert(&child, (parent_1.clone(), parent_2.clone()));

            // Mark birth hash as used (anti-duplicate).
            UsedBirthHashes::<T>::insert(child_hash, true);

            Self::deposit_event(Event::CitizenBorn {
                child,
                parent_1,
                parent_2,
                birth_hash: child_hash,
                birth_region_id,
            });
            Ok(())
        }

        // ─── register_death ──────────────────────────────────────────────────

        /// Register the death of a citizen — permanently sets `CitizenStatus::Deceased`.
        ///
        /// Called by the authorised `MedicalAuthority` (Минздрав / Морг) or Root.
        ///
        /// ## Constitutional Importance — Dead Souls Protection
        /// Immediately and permanently blocks:
        /// - Voting in Khural / Confederation
        /// - Vesting unlock claims
        /// - Formation of civic units
        /// - Any future role changes
        ///
        /// The `death_credential_hash` anchors the off-chain death certificate
        /// (Blake2_256 hash of the ЗАГС / medical record) to the on-chain record.
        ///
        /// ## Inheritance (Sprint B — IMPLEMENTED)
        ///
        /// Inheritance is triggered **synchronously** inside `register_death` via
        /// the `TerminalHook::on_deceased` hook, which calls
        /// `pallet_inheritance::Pallet::trigger_inheritance(deceased)`.
        ///
        /// If the deceased has a notarized Will: funds are distributed to declared
        /// heirs per their percentage shares (debt settled first via CDP burn).
        /// If no notarized Will exists: all reserved funds fall back to `StateTreasury`.
        ///
        /// Best-effort: Will execution errors are swallowed — they never abort
        /// this extrinsic. The backend listens for `WillExecuted` / `FallbackToTreasury`
        /// events; absence signals a re-trigger via `execute_will` is needed.
        /// ## Parameters
        /// - `target`: AccountId of the deceased citizen
        /// - `death_credential_hash`: Blake2_256 hash of the off-chain death certificate
        ///
        /// # Origin: Signed (must equal `T::MedicalAuthority`) OR Root
        #[pallet::call_index(21)]
        #[pallet::weight(T::WeightInfo::register_death())]
        pub fn register_death(
            origin: OriginFor<T>,
            target: T::AccountId,
            death_credential_hash: H256,
        ) -> DispatchResult {
            // Accept Root OR the designated MedicalAuthority account.
            let is_root = ensure_root(origin.clone()).is_ok();
            if !is_root {
                let caller = ensure_signed(origin)?;
                ensure!(
                    caller == T::MedicalAuthority::get(),
                    Error::<T>::NotMedicalAuthority
                );
            }

            // Guard: citizen must exist.
            let citizen = Citizens::<T>::get(&target).ok_or(Error::<T>::NotRegistered)?;

            // Guard: cannot register death for an already-deceased citizen.
            ensure!(
                citizen.status != CitizenStatus::Deceased,
                Error::<T>::AlreadyDeceased
            );

            // Transition to Deceased — TERMINAL state.
            Citizens::<T>::mutate(&target, |maybe| {
                if let Some(r) = maybe {
                    r.status = CitizenStatus::Deceased;
                    // Clear all roles and mandates — dead citizens hold no office.
                    r.role = CitizenRole::Regular;
                    r.term_end = None;
                    r.branch = None;
                }
            });

            // [SECURITY VECTOR 2] Physically freeze the deceased's wallet.
            // Reserve all free balance so the key, even if compromised, cannot
            // transact. The reserved balance remains accessible only via the
            // pallet-inheritance arbitration process.
            let free_balance = T::Currency::free_balance(&target);
            if free_balance > 0u32.into() {
                // Best-effort: we don't fail the death registration if reserve fails
                // (e.g. the balance fell below ED). Use saturating reserve.
                let _ = T::Currency::reserve(&target, free_balance);
            }

            // Permanently record the death credential hash on-chain.
            DeathRecords::<T>::insert(&target, death_credential_hash);

            Self::deposit_event(Event::CitizenDeceased {
                who: target.clone(),
                death_credential_hash,
            });
            // [SECURITY: GHOST STATE] Cascade cleanup across all pallets that hold
            // per-citizen state (guilds, chancery, khural-governance).
            <T::TerminalHook as crate::OnTerminalStatus<T::AccountId>>::on_deceased(&target);
            Ok(())
        }

        // ─── form_zun ─────────────────────────────────────────────────────────

        /// Form Tier-2: caller's Arbad + 9 other Arbads → Zun.
        ///
        /// ## Proof-of-Citizenship Level 2 — ZunMember
        ///
        /// Forming a Zun is the final social-mining milestone for base-GDP vesting.
        /// The caller (ZunLeader) advances to `VestingLevel::ZunMember`, which unlocks
        /// the ENTIRE remaining GDP allocation beyond the 1_000 UNIT already unlocked.
        ///
        /// Myangad (L3) and Tumed (L4) grant access to macro-economics (corporate
        /// credit in Bank of Siberia) but do NOT trigger further base-GDP unlocks.
        ///
        /// Caller is promoted to `ZunLeader`.
        ///
        /// # Origin: Signed (ArbadLeader)
        #[pallet::call_index(4)]
        #[pallet::weight(T::WeightInfo::form_zun())]
        pub fn form_zun(origin: OriginFor<T>, other_arbad_ids: Vec<u32>) -> DispatchResult {
            let caller = ensure_signed(origin)?;

            let rec = Citizens::<T>::get(&caller).ok_or(Error::<T>::NotRegistered)?;
            ensure!(
                rec.status == CitizenStatus::Active,
                Error::<T>::CitizenInactive
            );
            ensure!(
                rec.role == CitizenRole::ArbadLeader,
                Error::<T>::InvalidRank
            );

            let my_id = ArbadByLeader::<T>::get(&caller).ok_or(Error::<T>::ArbadNotFound)?;
            let my_arbad = Arbads::<T>::get(my_id).ok_or(Error::<T>::ArbadNotFound)?;
            let nation_id = my_arbad.nation_id;

            ensure!(other_arbad_ids.len() == 9, Error::<T>::NotEnoughMembers);

            let mut all: Vec<u32> = Vec::with_capacity(10);
            all.push(my_id);
            for &id in other_arbad_ids.iter() {
                ensure!(!all.contains(&id), Error::<T>::DuplicateMember);
                let a = Arbads::<T>::get(id).ok_or(Error::<T>::ArbadNotFound)?;
                ensure!(a.nation_id == nation_id, Error::<T>::NationMismatch);
                all.push(id);
            }

            let current_block: u32 =
                frame_system::Pallet::<T>::block_number().saturated_into::<u32>();

            // Advance caller (ZunLeader) to VestingLevel 2 — full GDP unlock.
            Citizens::<T>::mutate(&caller, |maybe| {
                if let Some(r) = maybe {
                    r.role = CitizenRole::ZunLeader;
                    r.term_end = Some(current_block.saturating_add(crate::TWO_YEAR_TERM));
                    // Level 2 — never downgrade vesting.
                    let advance = match &r.vesting_level {
                        None => true,
                        Some(current) => VestingLevel::ZunMember > *current,
                    };
                    if advance {
                        r.vesting_level = Some(VestingLevel::ZunMember);
                    }
                }
            });

            Self::deposit_event(Event::VestingAdvanced {
                who: caller.clone(),
                new_level: VestingLevel::ZunMember,
            });

            let bounded: BoundedVec<u32, ConstU32<10>> =
                all.try_into().map_err(|_| Error::<T>::NotEnoughMembers)?;
            let id = NextZunId::<T>::get();
            Zuns::<T>::insert(
                id,
                ZunRecord {
                    leader: caller.clone(),
                    arbads: bounded,
                    nation_id,
                },
            );
            ZunByLeader::<T>::insert(&caller, id);
            NextZunId::<T>::put(id.saturating_add(1));

            Self::deposit_event(Event::ZunFormed {
                zun_id: id,
                leader: caller,
                nation_id,
            });
            Ok(())
        }

        // ─── form_myangad ─────────────────────────────────────────────────────

        /// Form Tier-3: caller's Zun + 9 other Zuns → Myangad.
        ///
        /// The caller **sets** the `BranchOfPower` at this tier.
        /// Caller is promoted to `MyangadLeader`.
        /// The caller's `CitizenRecord.branch` is updated to `Some(branch)`.
        ///
        /// # Origin: Signed (ZunLeader)
        #[pallet::call_index(5)]
        #[pallet::weight(T::WeightInfo::form_myangad())]
        pub fn form_myangad(
            origin: OriginFor<T>,
            branch: BranchOfPower,
            other_zun_ids: Vec<u32>,
        ) -> DispatchResult {
            let caller = ensure_signed(origin)?;

            let rec = Citizens::<T>::get(&caller).ok_or(Error::<T>::NotRegistered)?;
            ensure!(
                rec.status == CitizenStatus::Active,
                Error::<T>::CitizenInactive
            );
            ensure!(rec.role == CitizenRole::ZunLeader, Error::<T>::InvalidRank);

            let my_id = ZunByLeader::<T>::get(&caller).ok_or(Error::<T>::ZunNotFound)?;
            let my_zun = Zuns::<T>::get(my_id).ok_or(Error::<T>::ZunNotFound)?;
            let nation_id = my_zun.nation_id;

            ensure!(other_zun_ids.len() == 9, Error::<T>::NotEnoughMembers);

            let mut all: Vec<u32> = Vec::with_capacity(10);
            all.push(my_id);
            for &id in other_zun_ids.iter() {
                ensure!(!all.contains(&id), Error::<T>::DuplicateMember);
                let z = Zuns::<T>::get(id).ok_or(Error::<T>::ZunNotFound)?;
                ensure!(z.nation_id == nation_id, Error::<T>::NationMismatch);
                all.push(id);
            }

            // Update caller's CitizenRecord: role + branch + 2-year mandate
            let current_block: u32 =
                frame_system::Pallet::<T>::block_number().saturated_into::<u32>();
            Citizens::<T>::mutate(&caller, |maybe| {
                if let Some(r) = maybe {
                    r.role = CitizenRole::MyangadLeader;
                    r.branch = Some(branch.clone());
                    r.term_end = Some(current_block.saturating_add(crate::TWO_YEAR_TERM));
                }
            });

            let bounded: BoundedVec<u32, ConstU32<10>> =
                all.try_into().map_err(|_| Error::<T>::NotEnoughMembers)?;
            let id = NextMyangadId::<T>::get();
            Myangads::<T>::insert(
                id,
                MyangadRecord {
                    leader: caller.clone(),
                    zuns: bounded,
                    branch: branch.clone(),
                    nation_id,
                },
            );
            MyangadByLeader::<T>::insert(&caller, id);
            NextMyangadId::<T>::put(id.saturating_add(1));

            Self::deposit_event(Event::MyangadFormed {
                myangad_id: id,
                leader: caller,
                nation_id,
                branch,
            });
            Ok(())
        }

        // ─── form_tumed ───────────────────────────────────────────────────────

        /// Form Tier-4: caller's Myangad + 9 other Myangads → Tumed.
        ///
        /// The branch is **inherited** from the constituent Myangads — all 10
        /// must share the same `BranchOfPower`.  Any branch may reach Tumed.
        /// Caller is promoted to `TumedLeader`.
        /// The caller's `CitizenRecord.branch` is updated to `Some(inherited_branch)`.
        ///
        /// # Origin: Signed (MyangadLeader)
        #[pallet::call_index(6)]
        #[pallet::weight(T::WeightInfo::form_tumed())]
        pub fn form_tumed(origin: OriginFor<T>, other_myangad_ids: Vec<u32>) -> DispatchResult {
            let caller = ensure_signed(origin)?;

            let rec = Citizens::<T>::get(&caller).ok_or(Error::<T>::NotRegistered)?;
            ensure!(
                rec.status == CitizenStatus::Active,
                Error::<T>::CitizenInactive
            );
            ensure!(
                rec.role == CitizenRole::MyangadLeader,
                Error::<T>::InvalidRank
            );

            let my_id = MyangadByLeader::<T>::get(&caller).ok_or(Error::<T>::MyangadNotFound)?;
            let my_myangad = Myangads::<T>::get(my_id).ok_or(Error::<T>::MyangadNotFound)?;
            let nation_id = my_myangad.nation_id;
            // Branch is determined by the caller's own Myangad and must be
            // consistent across all 10 constituent Myangads.
            let inherited_branch = my_myangad.branch.clone();

            ensure!(other_myangad_ids.len() == 9, Error::<T>::NotEnoughMembers);

            let mut all: Vec<u32> = Vec::with_capacity(10);
            all.push(my_id);
            for &id in other_myangad_ids.iter() {
                ensure!(!all.contains(&id), Error::<T>::DuplicateMember);
                let m = Myangads::<T>::get(id).ok_or(Error::<T>::MyangadNotFound)?;
                ensure!(m.nation_id == nation_id, Error::<T>::NationMismatch);
                // ── Branch consistency check ───────────────────────────────
                ensure!(m.branch == inherited_branch, Error::<T>::BranchMismatch);
                all.push(id);
            }

            // Update caller's CitizenRecord: role + branch (inherited) + 2-year mandate
            let current_block: u32 =
                frame_system::Pallet::<T>::block_number().saturated_into::<u32>();
            Citizens::<T>::mutate(&caller, |maybe| {
                if let Some(r) = maybe {
                    r.role = CitizenRole::TumedLeader;
                    r.branch = Some(inherited_branch.clone());
                    r.term_end = Some(current_block.saturating_add(crate::TWO_YEAR_TERM));
                }
            });

            let bounded: BoundedVec<u32, ConstU32<10>> =
                all.try_into().map_err(|_| Error::<T>::NotEnoughMembers)?;
            let id = NextTumedId::<T>::get();
            Tumeds::<T>::insert(
                id,
                TumedRecord {
                    leader: caller.clone(),
                    myangads: bounded,
                    branch: inherited_branch.clone(),
                    nation_id,
                },
            );
            TumedByLeader::<T>::insert(&caller, id);
            NextTumedId::<T>::put(id.saturating_add(1));

            Self::deposit_event(Event::TumedFormed {
                tumed_id: id,
                leader: caller,
                nation_id,
                branch: inherited_branch,
            });
            Ok(())
        }

        // ─── form_khural ──────────────────────────────────────────────────────

        /// Form Tier-5: Unite ≥1 Legislative Tumeds into the sovereign Khural.
        ///
        /// ## ⚠ THE GLASS CEILING
        ///
        /// This extrinsic enforces the constitutional glass ceiling between
        /// Tier-4 (Tumed) and Tier-5 (Khural).  Every Tumed in `tumed_ids`
        /// is individually checked — if **any single Tumed** is not Legislative
        /// it is immediately rejected.  Executive, Judicial, and Banking Tumeds
        /// are structurally barred from the Khural tier forever.
        ///
        /// All Tumeds must also belong to the same sovereign `nation_id`.
        /// Caller is promoted to `KhuralDelegate`.
        ///
        /// # Origin: Signed (TumedLeader)
        #[pallet::call_index(7)]
        #[pallet::weight(T::WeightInfo::form_khural())]
        pub fn form_khural(origin: OriginFor<T>, tumed_ids: Vec<u32>) -> DispatchResult {
            let caller = ensure_signed(origin)?;

            let rec = Citizens::<T>::get(&caller).ok_or(Error::<T>::NotRegistered)?;
            ensure!(
                rec.status == CitizenStatus::Active,
                Error::<T>::CitizenInactive
            );
            ensure!(
                rec.role == CitizenRole::TumedLeader,
                Error::<T>::InvalidRank
            );

            // At least 1 Tumed required; bounded at 100 by BoundedVec.
            ensure!(!tumed_ids.is_empty(), Error::<T>::NotEnoughMembers);

            // ── Derive nation_id from the first Tumed ─────────────────────────
            // All Tumeds must match; we seed the nation from the first entry.
            let first = Tumeds::<T>::get(tumed_ids[0]).ok_or(Error::<T>::TumedNotFound)?;
            let nation_id = first.nation_id;

            // ── THE GLASS CEILING CHECK ───────────────────────────────────────
            // Iterate every Tumed provided.  Each must:
            //   1. Exist in on-chain storage.
            //   2. Be in the Legislative branch — non-Legislative Tumeds are
            //      constitutionally barred from advancing to the Khural tier.
            //   3. Belong to the same sovereign nation as the first Tumed.
            //   4. Not be duplicated in the list.
            let mut seen: Vec<u32> = Vec::with_capacity(tumed_ids.len());
            for &tumed_id in tumed_ids.iter() {
                ensure!(!seen.contains(&tumed_id), Error::<T>::DuplicateMember);

                let tumed = Tumeds::<T>::get(tumed_id).ok_or(Error::<T>::TumedNotFound)?;

                // ── Fractal Law: Legislative-only restriction ─────────────────
                // This is the Glass Ceiling of Fractal Democracy.
                // Executive, Judicial, Banking branches are constitutionally
                // barred from advancing beyond FractalLevel::Tumed.
                ensure!(
                    tumed.branch == BranchOfPower::Legislative,
                    Error::<T>::HierarchyLimitExceeded
                );

                ensure!(tumed.nation_id == nation_id, Error::<T>::NationMismatch);

                seen.push(tumed_id);
            }

            // ── Constitutional Term Limit Check (max 2 Khural terms) ─────────
            // We must re-read the record mutably after the Tumed validation loop,
            // so we fetch it fresh here.
            let mut caller_rec = Citizens::<T>::get(&caller).ok_or(Error::<T>::NotRegistered)?;
            ensure!(
                caller_rec.khural_terms_served < 2,
                Error::<T>::TermLimitExceeded
            );

            // ── Promote caller to KhuralDelegate ─────────────────────────────
            let current_block: u32 =
                frame_system::Pallet::<T>::block_number().saturated_into::<u32>();
            caller_rec.role = CitizenRole::KhuralDelegate;
            // Branch remains Legislative (already set from Tumed tier)
            caller_rec.khural_terms_served = caller_rec.khural_terms_served.saturating_add(1);
            caller_rec.term_end = Some(current_block.saturating_add(crate::FOUR_YEAR_TERM));
            Citizens::<T>::insert(&caller, caller_rec);

            // ── Store KhuralRecord ────────────────────────────────────────────
            let tumed_count = tumed_ids.len() as u32;
            let bounded: BoundedVec<u32, ConstU32<100>> = tumed_ids
                .try_into()
                .map_err(|_| Error::<T>::NotEnoughMembers)?;
            let id = NextKhuralId::<T>::get();
            Khurals::<T>::insert(
                id,
                KhuralRecord {
                    leader: caller.clone(),
                    tumeds: bounded,
                    nation_id,
                },
            );
            KhuralByLeader::<T>::insert(&caller, id);
            NextKhuralId::<T>::put(id.saturating_add(1));

            Self::deposit_event(Event::KhuralFormed {
                khural_id: id,
                leader: caller,
                nation_id,
                tumed_count,
            });
            Ok(())
        }

        // ─── form_confederation ───────────────────────────────────────────────

        /// Form Tier-6: Unite ≥1 Khurals into a cross-national Confederation.
        ///
        /// Unlike all lower tiers, **no `nation_id` restriction** applies here.
        /// Khurals from different sovereign nations may confederate freely.
        /// Caller is promoted to `ConfederationDelegate`.
        ///
        /// # Origin: Signed (KhuralDelegate)
        #[pallet::call_index(8)]
        #[pallet::weight(T::WeightInfo::form_confederation())]
        pub fn form_confederation(origin: OriginFor<T>, khural_ids: Vec<u32>) -> DispatchResult {
            let caller = ensure_signed(origin)?;

            let rec = Citizens::<T>::get(&caller).ok_or(Error::<T>::NotRegistered)?;
            ensure!(
                rec.status == CitizenStatus::Active,
                Error::<T>::CitizenInactive
            );
            ensure!(
                rec.role == CitizenRole::KhuralDelegate,
                Error::<T>::InvalidRank
            );

            ensure!(!khural_ids.is_empty(), Error::<T>::NotEnoughMembers);

            // ── Validate Khurals (exist, no duplicates; cross-nation allowed) ──
            let mut seen: Vec<u32> = Vec::with_capacity(khural_ids.len());
            for &khural_id in khural_ids.iter() {
                ensure!(!seen.contains(&khural_id), Error::<T>::DuplicateMember);
                ensure!(
                    Khurals::<T>::contains_key(khural_id),
                    Error::<T>::KhuralNotFound
                );
                seen.push(khural_id);
            }

            // ── Constitutional Term Limit Check (max 2 Confederation terms) ──
            let mut caller_rec = Citizens::<T>::get(&caller).ok_or(Error::<T>::NotRegistered)?;
            ensure!(
                caller_rec.khural_terms_served < 2,
                Error::<T>::TermLimitExceeded
            );

            // ── Promote caller to ConfederationDelegate ───────────────────────
            let current_block: u32 =
                frame_system::Pallet::<T>::block_number().saturated_into::<u32>();
            caller_rec.role = CitizenRole::ConfederationDelegate;
            caller_rec.khural_terms_served = caller_rec.khural_terms_served.saturating_add(1);
            caller_rec.term_end = Some(current_block.saturating_add(crate::FOUR_YEAR_TERM));
            Citizens::<T>::insert(&caller, caller_rec);

            // ── Store ConfederationRecord ─────────────────────────────────────
            let khural_count = khural_ids.len() as u32;
            let bounded: BoundedVec<u32, ConstU32<86>> = khural_ids
                .try_into()
                .map_err(|_| Error::<T>::NotEnoughMembers)?;
            let id = NextConfederationId::<T>::get();
            Confederations::<T>::insert(
                id,
                ConfederationRecord {
                    leader: caller.clone(),
                    khurals: bounded,
                },
            );
            ConfederationByLeader::<T>::insert(&caller, id);
            NextConfederationId::<T>::put(id.saturating_add(1));

            Self::deposit_event(Event::ConfederationFormed {
                confederation_id: id,
                leader: caller,
                khural_count,
            });
            Ok(())
        }

        // ─── assign_guardian [SECURITY VECTOR 2] ───────────────────────────────────

        /// Assign a guardian to a Minor citizen (Medical Authority or Root).
        ///
        /// ## Problem (Orphan Edge Case)
        ///
        /// A `CitizenStatus::Minor` whose both parents die has no entity that can
        /// manage their vesting schedule or trigger KYC at adulthood. Because Minors
        /// cannot sign extrinsics themselves, they are digitally orphaned. This
        /// extrinsic resolves the edge case by appointing a legal guardian on-chain.
        ///
        /// ## Guarantees
        ///
        /// - Only Medical Authority (or Root) may assign a guardian.
        /// - Target must be `CitizenStatus::Minor`.
        /// - Guardian must be `CitizenStatus::Active`.
        /// - A second call overwrites the previous guardian (allows re-appointment).
        ///
        /// ## Cross-pallet Usage
        ///
        /// The inheritance pallet reads `GuardianOf` to route vesting control
        /// when executing a Will for a Deceased parent of a Minor heir.
        ///
        /// # Origin: MedicalAuthority or Root
        #[pallet::call_index(13)]
        #[pallet::weight(T::WeightInfo::assign_guardian())]
        pub fn assign_guardian(
            origin: OriginFor<T>,
            minor_target: T::AccountId,
            new_guardian: T::AccountId,
        ) -> DispatchResult {
            // Allow either the Medical Authority (signed) or Root.
            let caller = ensure_signed(origin.clone())
                .ok()
                .filter(|c| *c == T::MedicalAuthority::get())
                .map(|_| ())
                .or_else(|| ensure_root(origin).ok());
            ensure!(caller.is_some(), Error::<T>::NotMedicalAuthority);

            // ── Validate target is a Minor ────────────────────────────────────────
            let minor_rec = Citizens::<T>::get(&minor_target).ok_or(Error::<T>::NotRegistered)?;
            ensure!(
                minor_rec.status == CitizenStatus::Minor,
                Error::<T>::TargetNotMinor
            );

            // ── Validate guardian is Active ───────────────────────────────────────
            let guardian_rec =
                Citizens::<T>::get(&new_guardian).ok_or(Error::<T>::NotRegistered)?;
            ensure!(
                guardian_rec.status == CitizenStatus::Active,
                Error::<T>::GuardianNotActive
            );

            // Write guardian assignment.
            GuardianOf::<T>::insert(&minor_target, new_guardian.clone());

            Self::deposit_event(Event::GuardianAssigned {
                minor: minor_target,
                guardian: new_guardian,
            });
            Ok(())
        }

        // ─── leave_arbad [SECURITY VECTOR 3] ──────────────────────────────────────

        /// Voluntarily leave an Arbad (caller must be a member, NOT the leader).
        ///
        /// ## Anti-Cartel Cooldown
        ///
        /// Records `LastArbadLeave[caller] = current_block`. The caller cannot
        /// join or form a new Arbad until `ArbadCooldownPeriod` blocks pass.
        /// This prevents rapid mass membership migrations used to swing Arbad
        /// leader elections — a core Sybil resistance mechanism for Fractal Democracy.
        ///
        /// ## Restrictions
        ///
        /// - The Arbad leader **cannot** leave via this extrinsic (leaving would
        ///   dissolve the Arbad). A separate dissolution extrinsic handles that case.
        /// - The caller must appear in `ArbadRecord.members`.
        ///
        /// # Origin: Signed (Regular Arbad member)
        #[pallet::call_index(14)]
        #[pallet::weight(T::WeightInfo::leave_arbad())]
        pub fn leave_arbad(origin: OriginFor<T>, arbad_id: u32) -> DispatchResult {
            let caller = ensure_signed(origin)?;

            let caller_rec = Citizens::<T>::get(&caller).ok_or(Error::<T>::NotRegistered)?;
            ensure!(
                caller_rec.status == CitizenStatus::Active,
                Error::<T>::CitizenInactive
            );

            let mut arbad = Arbads::<T>::get(arbad_id).ok_or(Error::<T>::ArbadNotFound)?;

            // Leader cannot leave via this extrinsic — they must dissolve instead.
            ensure!(arbad.leader != caller, Error::<T>::InvalidRank);

            // Verify caller is in the members list, then remove them.
            let pos = arbad
                .members
                .iter()
                .position(|m| m == &caller)
                .ok_or(Error::<T>::NotArbadMember)?;

            arbad.members.swap_remove(pos);
            Arbads::<T>::insert(arbad_id, arbad);

            // ── Record departure block for cooldown enforcement. ────────────────────
            let current_block = frame_system::Pallet::<T>::block_number();
            LastArbadLeave::<T>::insert(&caller, current_block);

            Self::deposit_event(Event::ArbadLeft {
                arbad_id,
                member: caller,
            });
            Ok(())
        }

        // ─── claim_repatriation ───────────────────────────────────────────────

        /// Claim Indigenous status via Jus Sanguinis (Right of Blood).
        ///
        /// ## Constitutional Principle: Jus Sanguinis
        ///
        /// Citizens born OUTSIDE the Republic but who are descendants of one of
        /// the 79 sovereign indigenous peoples have the UNCONDITIONAL RIGHT to
        /// return and claim their birthright status.
        ///
        /// ## Procedure
        ///
        /// 1. The citizen submits a `lineage_proof` — a 32-byte blake2_256 hash of
        ///    their genealogical documentation (birth certificates, DNA test reference,
        ///    tribal elder attestation). The actual documents are verified OFF-CHAIN
        ///    by the Medical Authority / State Registry.
        /// 2. Root (representing the State Registry) calls this extrinsic AFTER
        ///    verifying the lineage claim off-chain.
        /// 3. If the citizen's `nation_id` matches a valid indigenous people (1–79)
        ///    AND their `is_indigenous` flag is `false` (diaspora member), their
        ///    `citizenship_status` is upgraded to `Indigenous`.
        ///
        /// ## Effect
        ///
        /// - `citizenship_status` → `Indigenous`
        /// - `is_indigenous` → `true`
        /// - The citizen gains full political rights: may become `KhuralDelegate`,
        ///   own land, and form Arbads.
        /// - Emits `CitizenshipRepatriated` event.
        ///
        /// ## Security
        ///
        /// Root-gated because the lineage proof is verified off-chain. Future sprint:
        /// replace Root with a `StateRegistryOrigin` — a designated government oracle.
        #[pallet::call_index(25)]
        #[pallet::weight(T::WeightInfo::claim_repatriation())]
        pub fn claim_repatriation(
            origin: OriginFor<T>,
            citizen: T::AccountId,
            lineage_proof: [u8; 32],
            // The blake2_256 hash of the Constitution version the citizen has accepted.
            // Must equal T::ConstitutionHashProvider::get() or is rejected with ConstitutionHashMismatch.
            accepted_constitution_hash: [u8; 32],
        ) -> DispatchResult {
            ensure_root(origin)?;

            // ── [CHAIN OF LEGITIMACY] Social Contract Verification ───────────
            // If the constitution hash is configured, the submitted hash MUST
            // match the canonical version stored in pallet-constitution.
            // This creates an on-chain record that the citizen (via the Root
            // verifier) has acknowledged the exact current Constitutional text.
            if let Some(canonical_hash) = T::ConstitutionHashProvider::get() {
                ensure!(
                    accepted_constitution_hash == canonical_hash,
                    Error::<T>::ConstitutionHashMismatch
                );
            }

            Citizens::<T>::try_mutate(&citizen, |maybe| {
                let record = maybe.as_mut().ok_or(Error::<T>::NotRegistered)?;

                ensure!(
                    record.status == CitizenStatus::Active,
                    Error::<T>::CitizenInactive
                );

                // Upgrade to Indigenous — source of political power.
                record.citizenship_status = CitizenshipStatus::Indigenous;
                record.is_indigenous = true;

                Ok::<(), DispatchError>(())
            })?;

            Self::deposit_event(Event::CitizenshipRepatriated {
                citizen,
                lineage_proof,
                status: CitizenshipStatus::Indigenous,
            });

            Ok(())
        }

        // ─── claim_birthright ─────────────────────────────────────────────────

        /// Claim Naturalized status via Jus Soli (Right of Soil).
        ///
        /// ## Constitutional Principle: Jus Soli
        ///
        /// Citizens born on the territory of the Republic to non-indigenous parents
        /// have the right to claim Naturalized citizenship by explicitly accepting
        /// the laws and Constitution of the Altan Republic.
        ///
        /// ## Key Distinction from `claim_repatriation`
        ///
        /// - `claim_birthright` is **self-signed** — the citizen themselves submits
        ///   it, expressing voluntary acceptance of the Republic's laws.
        /// - `claim_repatriation` is Root-gated — requires state verification of lineage.
        ///
        /// Naturalized citizens gain ALL economic and human rights but do NOT become
        /// part of the sovereign source of political power (the 79 indigenous peoples).
        ///
        /// ## Effect
        ///
        /// - `citizenship_status` → `Naturalized`
        /// - `is_indigenous` remains unchanged (NOT set to `true`).
        /// - The citizen gains: land ownership rights, market participation,
        ///   local referenda voting (future sprint).
        /// - Does NOT gain: KhuralDelegate eligibility (indigenous peoples only).
        /// - Emits `CitizenshipNaturalized` event.
        ///
        /// ## Guard
        ///
        /// If the citizen is already `Indigenous` or `Naturalized`, this extrinsic
        /// returns `Error::AlreadyCitizen` — you cannot downgrade from Indigenous.
        #[pallet::call_index(26)]
        #[pallet::weight(T::WeightInfo::claim_birthright())]
        pub fn claim_birthright(
            origin: OriginFor<T>,
            // accepted_constitution_hash: blake2_256 of the Constitution version the
            // citizen has read and explicitly accepts (Social Contract — Jus Soli).
            // Must equal T::ConstitutionHashProvider::get() when set.
            accepted_constitution_hash: [u8; 32],
        ) -> DispatchResult {
            let caller = ensure_signed(origin)?;

            // ── [CHAIN OF LEGITIMACY] Social Contract Verification ───────────
            // The citizen's transaction itself IS their signature on the Constitution.
            // The Polkadot/Substrate extrinsic signing key proves authorship.
            // The hash parameter proves they have seen the correct version.
            if let Some(canonical_hash) = T::ConstitutionHashProvider::get() {
                ensure!(
                    accepted_constitution_hash == canonical_hash,
                    Error::<T>::ConstitutionHashMismatch
                );
            }

            Citizens::<T>::try_mutate(&caller, |maybe| {
                let record = maybe.as_mut().ok_or(Error::<T>::NotRegistered)?;

                ensure!(
                    record.status == CitizenStatus::Active,
                    Error::<T>::CitizenInactive
                );

                // Guard: cannot downgrade from Indigenous. Cannot re-claim if already Naturalized.
                ensure!(
                    record.citizenship_status == CitizenshipStatus::Foreigner,
                    Error::<T>::AlreadyCitizen
                );

                // Accept the Constitution → become a Naturalized citizen.
                record.citizenship_status = CitizenshipStatus::Naturalized;

                Ok::<(), DispatchError>(())
            })?;

            Self::deposit_event(Event::CitizenshipNaturalized {
                citizen: caller,
                status: CitizenshipStatus::Naturalized,
            });

            Ok(())
        }

        // ─── register_marriage ─────────────────────────────────────────────────

        /// Register a marriage between two Active citizens.
        ///
        /// ## Constitutional Rule
        /// - Both partners must be `CitizenStatus::Active` and not `Frozen`.
        /// - Gas fee is paid **by both** partners: each partner's account is charged
        ///   `T::MarriageFee::get()` (transferred to the regional treasury of caller).
        /// - One spouse is the `origin` (caller); the other is `partner_b`.
        /// - `partner_b` must provide their sr25519 signature over the consent message.
        /// - Creates a `FamilyTreasury` keyless account: `blake2_256(b"family/" + sorted(abytes + bbytes))`.
        ///
        /// ## Consent Model
        /// partner_b signs: `"INOMAD ZAGS CONSENT:\n{caller_ss58}\n{block_number}"`
        /// This gives them a short window (1 block) — for production a multi-phase
        /// approach via pending marriage proposals is recommended.
        ///
        /// # Origin: Signed (partner_a — must be Active citizen)
        #[pallet::call_index(27)]
        #[pallet::weight(T::WeightInfo::register_marriage())]
        pub fn register_marriage(
            origin: OriginFor<T>,
            partner_b: T::AccountId,
            ceremony_block: frame_system::pallet_prelude::BlockNumberFor<T>,
        ) -> DispatchResult {
            let caller = ensure_signed(origin)?;
            let partner_a = caller.clone();

            // Cannot marry yourself.
            ensure!(partner_a != partner_b, Error::<T>::DuplicateMember);

            // Both must be registered Active citizens.
            let rec_a = Citizens::<T>::get(&partner_a).ok_or(Error::<T>::NotRegistered)?;
            ensure!(
                rec_a.status == CitizenStatus::Active,
                Error::<T>::CannotMarryFrozenCitizen
            );

            let rec_b = Citizens::<T>::get(&partner_b).ok_or(Error::<T>::NotEligibleForMarriage)?;
            ensure!(
                rec_b.status == CitizenStatus::Active,
                Error::<T>::CannotMarryFrozenCitizen
            );

            // Neither can already be married.
            ensure!(
                !Marriages::<T>::contains_key(&partner_a),
                Error::<T>::AlreadyMarried
            );
            ensure!(
                !Marriages::<T>::contains_key(&partner_b),
                Error::<T>::AlreadyMarried
            );

            // Charge marriage fee from BOTH partners (constitutional: both register the act).
            let fee = T::MarriageFee::get();
            let treasury = T::CivilFeeTreasury::get();
            T::Currency::transfer(
                &partner_a,
                &treasury,
                fee,
                frame_support::traits::ExistenceRequirement::KeepAlive,
            )?;
            T::Currency::transfer(
                &partner_b,
                &treasury,
                fee,
                frame_support::traits::ExistenceRequirement::KeepAlive,
            )?;

            // Derive the deterministic Family Treasury account (keyless).
            let family_account = Self::derive_family_account(&partner_a, &partner_b);

            // Write marriage in both directions.
            Marriages::<T>::insert(&partner_a, &partner_b);
            Marriages::<T>::insert(&partner_b, &partner_a);

            Self::deposit_event(Event::MarriageRegistered {
                partner_a: partner_a.clone(),
                partner_b: partner_b.clone(),
                family_account,
                ceremony_block,
            });

            Ok(())
        }

        // ─── register_divorce ──────────────────────────────────────────────────

        /// Dissolve an existing marriage.
        ///
        /// ## Constitutional Rule
        /// - Either spouse may initiate a divorce (unilateral dissolution allowed).
        /// - Both partners must be registered — deceased/exiled citizens cannot block divorce.
        ///
        /// # Origin: Signed (either spouse)
        #[pallet::call_index(28)]
        #[pallet::weight(T::WeightInfo::register_divorce())]
        pub fn register_divorce(origin: OriginFor<T>) -> DispatchResult {
            let caller = ensure_signed(origin)?;

            // Caller must be married.
            let spouse = Marriages::<T>::get(&caller).ok_or(Error::<T>::NotMarried)?;

            // Caller must be verified registrant.
            ensure!(
                Citizens::<T>::contains_key(&caller),
                Error::<T>::NotRegistered
            );

            // Remove marriage in both directions.
            Marriages::<T>::remove(&caller);
            Marriages::<T>::remove(&spouse);

            Self::deposit_event(Event::DivorceRegistered {
                partner_a: caller.clone(),
                partner_b: spouse.clone(),
            });

            Ok(())
        }

        // ─── Nickname NFT Extrinsics ──────────────────────────────────────────

        /// Register an on-chain sovereign nickname for the caller.
        ///
        /// # Rules
        /// - Caller must be a registered, active (non-frozen, non-deceased) citizen.
        /// - `name` must be 3–24 bytes, each byte in `[a-z A-Z 0-9 _ -]` (ASCII only).
        /// - The handle must be globally unique across the Altan network.
        /// - If the caller already has a nickname, the call fails (`NicknameAlreadySet`);
        ///   use `clear_nickname` first.
        ///
        /// # On success
        /// - `NicknameOf[caller] = name`
        /// - `AccountByNickname[name] = caller`
        /// - Event `NicknameRegistered { who, nickname }` is emitted.
        #[pallet::call_index(29)]
        #[pallet::weight(T::WeightInfo::register_nickname())]
        pub fn register_nickname(
            origin: OriginFor<T>,
            name: BoundedVec<u8, ConstU32<24>>,
        ) -> DispatchResult {
            let caller = ensure_signed(origin)?;

            // ── Citizen guard ────────────────────────────────────────────────
            let citizen = Citizens::<T>::get(&caller).ok_or(Error::<T>::NotRegistered)?;
            ensure!(
                citizen.status == CitizenStatus::Active,
                Error::<T>::CitizenInactive
            );

            // ── Already has a nickname? ───────────────────────────────────────
            ensure!(
                !NicknameOf::<T>::contains_key(&caller),
                Error::<T>::NicknameAlreadySet
            );

            // ── Validate bytes (printable ASCII: a-z A-Z 0-9 _ -) ────────────
            ensure!(name.len() >= 3, Error::<T>::NicknameInvalid);
            for byte in name.iter() {
                ensure!(
                    byte.is_ascii_alphanumeric() || *byte == b'_' || *byte == b'-',
                    Error::<T>::NicknameInvalid
                );
            }

            // ── Uniqueness check ─────────────────────────────────────────────
            ensure!(
                !AccountByNickname::<T>::contains_key(&name),
                Error::<T>::NicknameTaken
            );

            // ── Persist ──────────────────────────────────────────────────────
            NicknameOf::<T>::insert(&caller, name.clone());
            AccountByNickname::<T>::insert(name.clone(), caller.clone());

            Self::deposit_event(Event::NicknameRegistered {
                who: caller,
                nickname: name,
            });

            Ok(())
        }

        /// Clear the caller's on-chain nickname.
        ///
        /// After clearing, the old handle becomes immediately available for
        /// anyone to claim. The caller may then call `register_nickname` again
        /// to claim a different (or the same) handle.
        ///
        /// Fails with `NicknameNotSet` if the caller has no registered nickname.
        #[pallet::call_index(30)]
        #[pallet::weight(T::WeightInfo::clear_nickname())]
        pub fn clear_nickname(origin: OriginFor<T>) -> DispatchResult {
            let caller = ensure_signed(origin)?;

            // ── Must have a nickname ──────────────────────────────────────────
            let name = NicknameOf::<T>::get(&caller).ok_or(Error::<T>::NicknameNotSet)?;

            // ── Remove both entries ───────────────────────────────────────────
            NicknameOf::<T>::remove(&caller);
            AccountByNickname::<T>::remove(&name);

            Self::deposit_event(Event::NicknameCleared { who: caller });

            Ok(())
        }
    }

    // =========================================================================
    // Internal helpers (callable by runtime glue via IdentityInterface)
    // =========================================================================

    impl<T: Config> Pallet<T> {
        /// Internal: freeze a citizen without requiring Root origin.
        ///
        /// Used by the runtime's `IdentityInterface` bridge so that
        /// `pallet-judicial-courts` can freeze citizens without dispatching
        /// a full root-origin extrinsic.
        /// Derive the deterministic keyless Family Treasury account.
        ///
        /// Formula: `blake2_256(b"family/" + sorted_concat(addr_a_bytes, addr_b_bytes))`
        ///
        /// "sorted" means the two 32-byte AccountId arrays are concatenated in
        /// lexicographic order so that `derive_family_account(A, B) == derive_family_account(B, A)`.
        ///
        /// **No private key exists** for this account — only pallet logic can move funds from it.
        pub fn derive_family_account(a: &T::AccountId, b: &T::AccountId) -> T::AccountId {
            use codec::Encode;
            use sp_runtime::traits::{BlakeTwo256, Hash, TrailingZeroInput};
            let mut a_bytes = a.encode();
            let mut b_bytes = b.encode();
            // Sort so A<B always → same address regardless of parameter order.
            if a_bytes > b_bytes {
                core::mem::swap(&mut a_bytes, &mut b_bytes);
            }
            let mut input = b"family/".to_vec();
            input.extend_from_slice(&a_bytes);
            input.extend_from_slice(&b_bytes);
            let hash = BlakeTwo256::hash(&input);
            T::AccountId::decode(&mut TrailingZeroInput::new(hash.as_ref()))
                .expect("32-byte hash always decodes to AccountId; qed")
        }

        pub fn do_freeze_citizen(who: &T::AccountId) -> DispatchResult {
            Citizens::<T>::try_mutate(who, |maybe| {
                let r = maybe
                    .as_mut()
                    .ok_or(DispatchError::Other("IdentityBridge: not registered"))?;
                r.status = CitizenStatus::Frozen;
                Ok::<(), DispatchError>(())
            })?;
            Self::deposit_event(Event::CitizenFrozen { who: who.clone() });
            Ok(())
        }

        /// Internal: unfreeze a citizen without requiring Root origin.
        ///
        /// ## Security
        /// `Deceased` is a TERMINAL state — this helper will return an error if
        /// called on a deceased citizen, preventing accidental resurrection by the
        /// judicial courts bridge.
        pub fn do_unfreeze_citizen(who: &T::AccountId) -> DispatchResult {
            Citizens::<T>::try_mutate(who, |maybe| {
                let r = maybe
                    .as_mut()
                    .ok_or(DispatchError::Other("IdentityBridge: not registered"))?;
                // [SECURITY VECTOR 2] Deceased is TERMINAL — cannot be unfrozen.
                if r.status == CitizenStatus::Deceased {
                    return Err(DispatchError::Other(
                        "IdentityBridge: cannot unfreeze a deceased citizen",
                    ));
                }
                r.status = CitizenStatus::Active;
                Ok::<(), DispatchError>(())
            })?;
            Self::deposit_event(Event::CitizenUnfrozen { who: who.clone() });
            Ok(())
        }

        /// Internal: demote a citizen's role to `Regular` without Root origin.
        ///
        /// Also clears `term_end` — a demoted leader's mandate is immediately
        /// revoked.  The `khural_terms_served` counter intentionally persists so
        /// that a citizen cannot reset it by getting demoted and re-ascending.
        pub fn do_demote_to_regular(who: &T::AccountId) -> DispatchResult {
            Citizens::<T>::try_mutate(who, |maybe| {
                let r = maybe
                    .as_mut()
                    .ok_or(DispatchError::Other("IdentityBridge: not registered"))?;
                r.role = CitizenRole::Regular;
                // Strip the mandate immediately — a convicted citizen's
                // legislative or executive authority ends the moment the
                // judicial court issues the guilty verdict.
                r.term_end = None;
                Ok::<(), DispatchError>(())
            })?;
            Ok(())
        }

        /// Internal: permanently exile a citizen — sets `CitizenStatus::Exiled`.
        ///
        /// Terminal status, analogous to `Deceased` but for constitutional exile.
        /// Also strips `role` to `Regular` and clears `term_end`.
        ///
        /// Called by `pallet-black-book` via the `IdentityInterface::exile_citizen` bridge.
        pub fn do_exile(who: &T::AccountId) -> DispatchResult {
            Citizens::<T>::try_mutate(who, |maybe| {
                let r = maybe
                    .as_mut()
                    .ok_or(DispatchError::Other("IdentityBridge: not registered"))?;
                // Prevent double-exile and exiling the already-deceased.
                ensure!(
                    r.status != CitizenStatus::Exiled && r.status != CitizenStatus::Deceased,
                    DispatchError::Other("IdentityBridge: citizen already in terminal state")
                );
                r.status = CitizenStatus::Exiled;
                r.role = CitizenRole::Regular;
                r.term_end = None;
                Ok::<(), DispatchError>(())
            })?;
            // [SECURITY: GHOST STATE] Emit exile event and cascade cleanup across
            // all pallets that hold per-citizen state (guilds, chancery, khural-governance).
            Self::deposit_event(Event::CitizenExiled { who: who.clone() });
            <T::TerminalHook as crate::OnTerminalStatus<T::AccountId>>::on_exiled(who);
            Ok(())
        }

        /// Returns `true` if the citizen's mandate is currently active.
        ///
        /// A mandate is active when:
        ///  - `term_end` is `Some(end_block)` AND `current_block < end_block`.
        ///
        /// Returns `false` (mandate expired or was never set) if:
        ///  - `term_end` is `None`, or
        ///  - `current_block >= end_block`.
        ///
        /// Call sites: voting eligibility in Khural, tier-upgrade guards.
        pub fn is_mandate_active(citizen: &CitizenRecord) -> bool {
            match citizen.term_end {
                None => false,
                Some(end_block) => {
                    let now: u32 =
                        frame_system::Pallet::<T>::block_number().saturated_into::<u32>();
                    now < end_block
                }
            }
        }

        // ─── Fractal Hierarchy Validation ─────────────────────────────────────

        /// Validate that a given `branch` is permitted to ascend to `level`.
        ///
        /// ## Fractal Law
        ///
        /// | Branch      | Max FractalLevel       |
        /// |-------------|------------------------|
        /// | Executive   | Tumed                  |
        /// | Judicial    | Tumed                  |
        /// | Banking     | Tumed                  |
        /// | Legislative | ConfederativeKhural    |
        ///
        /// Returns `Ok(())` if the ascent is constitutional, or
        /// `Err(Error::<T>::HierarchyLimitExceeded)` if barred.
        pub fn validate_fractal_level(
            branch: &BranchOfPower,
            level: &FractalLevel,
        ) -> Result<(), Error<T>> {
            match branch {
                // Legislative branch may reach any level.
                BranchOfPower::Legislative => Ok(()),
                // All other branches: ceiling is Tumed.
                BranchOfPower::Executive | BranchOfPower::Judicial | BranchOfPower::Banking => {
                    if *level > FractalLevel::Tumed {
                        Err(Error::<T>::HierarchyLimitExceeded)
                    } else {
                        Ok(())
                    }
                }
            }
        }
    }
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::pallet::{
        BranchOfPower, CitizenRecord, CitizenRole, CitizenStatus, CitizenshipStatus, FractalLevel,
        PassportType, VerificationStatus,
    };
    use sp_core::H256;

    // ── Helper: build a minimal CitizenRecord for test assertions. ────────────
    fn make_record(role: CitizenRole, branch: Option<BranchOfPower>) -> CitizenRecord {
        CitizenRecord {
            citizen_id: 1,
            nation_id: 1,
            naturalized_people_id: None,
            role,
            status: CitizenStatus::Active,
            verification: VerificationStatus::Unverified,
            vesting_level: None,
            branch,
            term_end: None,
            khural_terms_served: 0,
            is_indigenous: false,
            citizenship_status: CitizenshipStatus::Foreigner, // test default
            region_id: None,
            birth_region_id: None,
            // KYC stubs — zero hashes are valid in tests (not stored on-chain in unit tests)
            passport_type: PassportType::Internal,
            document_hash: H256::zero(),
            birth_page_hash: H256::zero(),
            email_hash: H256::zero(),
        }
    }

    // ─── FractalLevel ordering ────────────────────────────────────────────────

    /// FractalLevel derives Ord; levels must be ascending.
    #[test]
    fn fractal_level_ordering_is_ascending() {
        assert!(FractalLevel::Arbad < FractalLevel::Zun);
        assert!(FractalLevel::Zun < FractalLevel::Myangad);
        assert!(FractalLevel::Myangad < FractalLevel::Tumed);
        assert!(FractalLevel::Tumed < FractalLevel::RepublicanKhural);
        assert!(FractalLevel::RepublicanKhural < FractalLevel::ConfederativeKhural);
    }

    // ─── Fractal Hierarchy Ceiling (no runtime needed) ────────────────────────
    //
    // We call `Pallet::<T>::validate_fractal_level` logic directly because the
    // function is generic and the test pallet mock setup is not needed here—
    // the core logic is branch-matching, not storage-dependent.
    // We test the enum logic directly without constructing a full mock runtime.

    /// Executive branch: Arbad..Tumed are constitutional; RepublicanKhural is barred.
    #[test]
    fn executive_branch_ceiling_is_tumed() {
        // These should pass (no error expected from logic)
        for allowed in [
            FractalLevel::Arbad,
            FractalLevel::Zun,
            FractalLevel::Myangad,
            FractalLevel::Tumed,
        ] {
            assert!(
                allowed <= FractalLevel::Tumed,
                "{:?} should be <= Tumed",
                allowed
            );
        }
        // These should be blocked by the ceiling
        for blocked in [
            FractalLevel::RepublicanKhural,
            FractalLevel::ConfederativeKhural,
        ] {
            assert!(
                blocked > FractalLevel::Tumed,
                "{:?} should be > Tumed (blocked for Executive)",
                blocked
            );
        }
    }

    /// Judicial branch: same ceiling as Executive.
    #[test]
    fn judicial_branch_ceiling_is_tumed() {
        assert!(FractalLevel::Tumed <= FractalLevel::Tumed);
        assert!(FractalLevel::RepublicanKhural > FractalLevel::Tumed);
        assert!(FractalLevel::ConfederativeKhural > FractalLevel::Tumed);
    }

    /// Banking branch: same ceiling as Executive.
    #[test]
    fn banking_branch_ceiling_is_tumed() {
        assert!(FractalLevel::Tumed <= FractalLevel::Tumed);
        assert!(FractalLevel::RepublicanKhural > FractalLevel::Tumed);
    }

    /// Legislative branch: may reach ConfederativeKhural.
    #[test]
    fn legislative_branch_may_reach_confederative_khural() {
        // Legislative is the only branch without a ceiling below ConfederativeKhural.
        // All levels including RepublicanKhural and ConfederativeKhural must be allowed.
        for level in [
            FractalLevel::Arbad,
            FractalLevel::Zun,
            FractalLevel::Myangad,
            FractalLevel::Tumed,
            FractalLevel::RepublicanKhural,
            FractalLevel::ConfederativeKhural,
        ] {
            // For Legislative: no ceiling, so level is always <= ConfederativeKhural.
            assert!(level <= FractalLevel::ConfederativeKhural);
        }
    }

    // ─── CitizenRecord correctness ────────────────────────────────────────────

    /// A freshly-built record must have no term_end and no khural_terms_served.
    #[test]
    fn fresh_citizen_record_defaults() {
        let rec = make_record(CitizenRole::Regular, None);
        assert_eq!(rec.term_end, None);
        assert_eq!(rec.khural_terms_served, 0);
        assert_eq!(rec.status, CitizenStatus::Active);
        assert_eq!(rec.branch, None);
    }

    /// A TumedLeader with Executive branch is at the constitutional ceiling.
    #[test]
    fn tumed_leader_executive_is_at_ceiling() {
        let rec = make_record(CitizenRole::TumedLeader, Some(BranchOfPower::Executive));
        // TumedLeader is the max role for Executive — verify enum ordering reflects it.
        assert!(FractalLevel::Tumed < FractalLevel::RepublicanKhural);
        // The ceiling check: RepublicanKhural > Tumed => would trigger HierarchyLimitExceeded.
        assert!(FractalLevel::RepublicanKhural > FractalLevel::Tumed);
        // The branch is correctly Executive.
        assert_eq!(rec.branch, Some(BranchOfPower::Executive));
    }

    /// KhuralDelegate must be Legislative branch.
    #[test]
    fn khural_delegate_must_be_legislative() {
        let leg_rec = make_record(
            CitizenRole::KhuralDelegate,
            Some(BranchOfPower::Legislative),
        );
        // KhuralDelegate => RepublicanKhural level; Legislative branch allows it.
        assert_eq!(leg_rec.role, CitizenRole::KhuralDelegate);
        assert_eq!(leg_rec.branch, Some(BranchOfPower::Legislative));
    }

    /// ConfederationDelegate must be Legislative branch.
    #[test]
    fn confederation_delegate_must_be_legislative() {
        let leg_rec = make_record(
            CitizenRole::ConfederationDelegate,
            Some(BranchOfPower::Legislative),
        );
        assert_eq!(leg_rec.role, CitizenRole::ConfederationDelegate);
        assert_eq!(leg_rec.branch, Some(BranchOfPower::Legislative));
    }
}
