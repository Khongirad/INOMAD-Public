//! # INOMAD Elections Pallet
//!
//! **Altan Network — Sovereign L1 Blockchain**
//! **Central Election Commission (ЦИК) | Bottom-Up Hierarchical Meritocracy + Peak Governance**
//!
//! ## Hierarchy Overview
//!
//! ```text
//! Arbad  (10)   citizens ──vote──▶  Arbad  Leader  (Аравт)
//! Zun    (100)  leaders  ──vote──▶  Zun    Leader  (Зуут)
//! Myangad(1000) leaders  ──vote──▶  Myangad Leader (Мянгат)
//! Tumed  (10k)  leaders  ──vote──▶  [PEAK GOVERNANCE — four branches]
//!
//! ┌──── PEAK GOVERNANCE (Tumed Leaders vote) ──────────────────────────────┐
//! │  Executive  ──▶  BranchCouncil[9]  ──▶  SupremeLeader (Head of State) │
//! │  Judicial   ──▶  BranchCouncil[9]  ──▶  SupremeLeader (Chief Justice) │
//! │  Banking    ──▶  BranchCouncil[9]  ──▶  SupremeLeader (Central Banker)│
//! │  Legislative──▶  KhuralChairman (+ original Arbad ID stored)          │
//! └────────────────────────────────────────────────────────────────────────┘
//! ```
//!
//! ## Election Level Hierarchy
//!
//! | Level | Unit Size | Who Can Vote | Who Gets Elected |
//! |-------|-----------|-------------|-----------------|
//! | `Arbad` | ~10 | Any citizen with a `CitizenArbad` entry | Arbad Leader |
//! | `Zun` | ~100 | `ElectedLeaders ≥ Zun` | Zun Leader |
//! | `Myangad` | ~1 000 | `ElectedLeaders ≥ Myangad` | Myangad Leader |
//! | `Tumed` | ~10 000 | `ElectedLeaders = Tumed` | Confederation Delegate |
//!
//! ## Peak Governance (Constitutional Branch Separation)
//!
//! | Extrinsic | Voters | Outcome |
//! |-----------|--------|---------|
//! | `elect_branch_council` | All Tumed leaders | Top 9 candidates → `BranchCouncils` |
//! | `elect_supreme_leader` | The 9 council members of that branch | 1 → `SupremeLeaders` |
//! | `elect_khural_chairman` | All Tumed leaders | 1 winner + their Arbad ID → `KhuralChairman` |
//!
//! ## Storage at a Glance
//!
//! | Storage | Key | Value |
//! |---------|-----|-------|
//! | `CitizenArbad` | `AccountId` | `arbad_id: u32` |
//! | `ElectedLeaders` | `AccountId` | `ElectionLevel` |
//! | `Elections` | `election_id: u32` | `Election<T>` |
//! | `Votes` | `(election_id, voter)` | `candidate` |
//! | `VoteCounts` | `(election_id, candidate)` | `u32` |
//! | `BranchCouncils` | `GovernmentBranch` | `BoundedVec<AccountId, 9>` |
//! | `BranchVotes` | `(GovernmentBranch, voter)` | `candidate` |
//! | `BranchVoteCounts` | `(GovernmentBranch, candidate)` | `u32` |
//! | `SupremeLeaders` | `GovernmentBranch` | `AccountId` |
//! | `SupremeLeaderVotes` | `(GovernmentBranch, council_member)` | `candidate` |
//! | `SupremeLeaderVoteCounts` | `(GovernmentBranch, candidate)` | `u32` |
//! | `KhuralChairman` | — | `(AccountId, arbad_id: u32)` |
//! | `KhuralVotes` | `voter` | `candidate` |
//! | `KhuralVoteCounts` | `candidate` | `u32` |
//!
//! ## Dispatchables
//!
//! | Call | Origin | Description |
//! |---|---|---|
//! | `cast_vote` | Signed (Citizen) | Cast a vote in the active Arbad-level election |
//! | `add_citizen_to_arbad` | Root | Enrol a citizen into their geographic Arbad for elections |
//! | `promote_leader` | Signed (Officer) | Promote an Arbad leader to the Tumed-level pool |
//! | `create_election` | Root | Create a new election cycle for a branch or level |
//! | `elect_branch_council` | Root | Tally Tumed votes and elect the Branch Council |
//! | `confirm_branch_council` | Root | Confirm and seat the elected Branch Council |
//! | `elect_supreme_leader` | Root | Tally Branch Council votes and elect the Supreme Leader |
//! | `confirm_supreme_leader` | Root | Confirm and install the elected Supreme Leader |
//! | `elect_khural_chairman` | Root | Tally Branch Council votes and elect the Khural Chairman |
//! | `confirm_khural_chairman` | Root | Confirm and install the elected Khural Chairman |
//! | `reset_ballot` | Root | Clear ballot data after an election cycle concludes |

#![cfg_attr(not(feature = "std"), no_std)]

extern crate alloc;

pub mod migrations;
pub mod weights;
pub use pallet::*;

#[cfg(test)]
mod mock;
#[cfg(test)]
mod tests;

#[cfg(feature = "runtime-benchmarks")]
mod benchmarking;

#[frame_support::pallet]
pub mod pallet {
    use crate::weights::WeightInfo as _;
    use alloc::vec::Vec;
    use frame_support::pallet_prelude::*;
    use frame_system::pallet_prelude::*;

    // =========================================================================
    // Constants
    // =========================================================================

    /// Constitutional size of each Branch Council.
    /// The Rule of Nine applies to Executive, Judicial, and Banking branches.
    pub const COUNCIL_SIZE: u32 = 9;

    // =========================================================================
    // Pallet Struct
    // =========================================================================

    #[pallet::pallet]
    pub struct Pallet<T>(_);

    // =========================================================================
    // Types — Enums
    // =========================================================================

    /// The four tiers of the authentic Mongol Decimal (аравт систем) hierarchy.
    ///
    /// Variant order **must not change** — `PartialOrd`/`Ord` derives ordinal
    /// ordering from declaration order: `Arbad < Zun < Myangad < Tumed`.
    /// This is used in `cast_vote` to check `voter_level >= election.level`.
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
    pub enum ElectionLevel {
        /// Аравт — base Arbad community vote (~10 citizens per unit).
        Arbad,
        /// Зуут — Arbad Leaders vote to elect Zun Leaders (~100 unit).
        Zun,
        /// Мянгат — Zun Leaders vote to elect Myangad Leaders (~1 000 unit).
        Myangad,
        /// Түмэт — Myangad Leaders vote to elect Tumed / Confederation Delegates (~10 000 unit).
        Tumed,
    }

    /// The four constitutional branches of the Altan State.
    ///
    /// Power is constitutionally separated. No branch may perform the functions
    /// of another (see AGENTS.md §2.6).
    ///
    /// | Branch | Head | Elected by |
    /// |--------|------|------------|
    /// | `Executive` | Supreme Leader (Head of State) | 9-member Executive Council |
    /// | `Judicial` | Chief Justice | 9-member Judicial Council |
    /// | `Banking` | Central Banker | 9-member Banking Council |
    /// | `Legislative` | Khural Chairman | All Tumed leaders (direct vote) |
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
    pub enum GovernmentBranch {
        /// Гүйцэтгэх засаглал — Executive branch.
        Executive,
        /// Шүүх засаглал — Judicial branch.
        Judicial,
        /// Банкны салбар — Banking/Central Bank branch.
        Banking,
        /// Хурал — Legislative branch (Great Assembly).
        Legislative,
    }

    // =========================================================================
    // Types — Structs
    // =========================================================================

    /// A standard decimal-hierarchy election record (Arbad → Tumed).
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
    pub struct Election<T: Config> {
        /// The decimal-hierarchy tier at which this election operates.
        pub level: ElectionLevel,
        /// Whether the election is still accepting votes.
        pub is_active: bool,
        /// Whitelist of accounts that may receive votes.
        pub candidates: BoundedVec<T::AccountId, T::MaxCandidates>,
    }

    // =========================================================================
    // Configuration Trait
    // =========================================================================

    #[pallet::config]
    pub trait Config: frame_system::Config<RuntimeEvent: From<Event<Self>>> {
        /// Weight information for extrinsics in this pallet.
        type WeightInfo: crate::weights::WeightInfo;
        /// Maximum candidates per standard decimal-hierarchy election.
        /// Set to `1 024` at runtime for production.
        #[pallet::constant]
        type MaxCandidates: Get<u32>;

        /// Maximum candidates for a Branch Council election.
        /// Must be >= `COUNCIL_SIZE` (9). Set to `100` in production.
        #[pallet::constant]
        type MaxBranchCandidates: Get<u32>;

        /// Constitutional minimum: an Arbad must have this many CITIZENS
        /// before its leader can be promoted to Zun level.
        /// = 10 (9 citizens + 1 leader = full Arbad).
        #[pallet::constant]
        type MinArbadSize: Get<u32>;

        /// Constitutional minimum TOTAL CITIZENS in a Zun zone.
        /// = 100 (10 Arbads × 10 citizens each).
        /// Stored for documentation and future member-count checks.
        #[pallet::constant]
        type MinZunSize: Get<u32>;

        /// Constitutional minimum ARBAD LEADERS in a Zun zone before
        /// one of them can be elected and promoted to Myangad level.
        /// = 10 (one elected leader from each of the 10 Arbads in the Zun).
        /// Check: `ZunLeaderCount[zun_id] ≥ MinZunLeaders` before promote_leader(Myangad).
        #[pallet::constant]
        type MinZunLeaders: Get<u32>;

        /// Constitutional minimum TOTAL CITIZENS in a Myangad zone.
        /// = 1 000 (10 Zuns × 100 citizens each).
        /// Stored for documentation and future member-count checks.
        #[pallet::constant]
        type MinMyangadSize: Get<u32>;

        /// Constitutional minimum ZUN LEADERS in a Myangad zone before
        /// one of them can be elected and promoted to Tumed level.
        /// = 10 (one elected leader from each of the 10 Zuns in the Myangad).
        /// Check: `MyangadLeaderCount[myangad_id] ≥ MinMyangadLeaders` before promote_leader(Tumed).
        #[pallet::constant]
        type MinMyangadLeaders: Get<u32>;

        /// Hook to verify whether an AccountId is an indigenous citizen.
        ///
        /// Used to enforce the constitutional rule that only indigenous people
        /// (`CitizenshipStatus::Indigenous`) participate in the Legislative branch
        /// (Хурал) at all levels.
        ///
        /// In production this should query `pallet-inomad-identity`.
        /// For tests a mock that returns `true` for designated accounts is used.
        type IsIndigenous: IsIndigenousCitizen<Self::AccountId>;
    }

    // ── Indigenous citizenship trait ─────────────────────────────────────────

    /// Hook that allows the elections pallet to verify indigenous status
    /// without creating a hard circular dependency on pallet-inomad-identity.
    pub trait IsIndigenousCitizen<AccountId> {
        /// Returns `true` if the account holds `CitizenshipStatus::Indigenous`.
        fn is_indigenous(who: &AccountId) -> bool;
    }

    /// Noop implementation — always returns `true`.
    /// Use this in test mocks where all accounts are treated as indigenous.
    pub struct AlwaysIndigenous;
    impl<AccountId> IsIndigenousCitizen<AccountId> for AlwaysIndigenous {
        fn is_indigenous(_who: &AccountId) -> bool {
            true
        }
    }

    // =========================================================================
    // Storage — Decimal Hierarchy (unchanged from V2)
    // =========================================================================

    /// Maps `AccountId` → `arbad_id` (u32).
    /// Citizens not present cannot vote at `Arbad` level.
    #[pallet::storage]
    #[pallet::getter(fn citizen_arbad)]
    pub type CitizenArbad<T: Config> =
        StorageMap<_, Blake2_128Concat, T::AccountId, u32, OptionQuery>;

    /// Maps `AccountId` → highest `ElectionLevel` the account may vote IN.
    /// Absence = regular Arbad citizen only.
    #[pallet::storage]
    #[pallet::getter(fn elected_leaders)]
    pub type ElectedLeaders<T: Config> =
        StorageMap<_, Blake2_128Concat, T::AccountId, ElectionLevel, OptionQuery>;

    /// Maps `election_id` → `Election<T>`.
    #[pallet::storage]
    #[pallet::getter(fn elections)]
    pub type Elections<T: Config> = StorageMap<_, Blake2_128Concat, u32, Election<T>, OptionQuery>;

    /// DoubleMap: `election_id` × `voter` → `candidate_voted_for`.
    #[pallet::storage]
    #[pallet::getter(fn votes)]
    pub type Votes<T: Config> = StorageDoubleMap<
        _,
        Blake2_128Concat,
        u32,
        Blake2_128Concat,
        T::AccountId,
        T::AccountId,
        OptionQuery,
    >;

    /// DoubleMap: `election_id` × `candidate` → vote tally.
    #[pallet::storage]
    #[pallet::getter(fn vote_counts)]
    pub type VoteCounts<T: Config> =
        StorageDoubleMap<_, Blake2_128Concat, u32, Blake2_128Concat, T::AccountId, u32, ValueQuery>;

    // =========================================================================
    // Storage — Peak Governance (new in V3)
    // =========================================================================

    /// The confirmed Council of 9 for each branch (Executive / Judicial / Banking).
    ///
    /// Constitutional constraint: exactly 9 members per council.
    /// `BoundedVec<_, 9>` is enforced at write time in `elect_branch_council`.
    /// The `Legislative` branch does not use a council — it has `KhuralChairman`.
    #[pallet::storage]
    #[pallet::getter(fn branch_councils)]
    pub type BranchCouncils<T: Config> = StorageMap<
        _,
        Blake2_128Concat,
        GovernmentBranch,
        BoundedVec<T::AccountId, ConstU32<9>>,
        OptionQuery,
    >;

    /// DoubleMap: `GovernmentBranch` × `Tumed voter` → `candidate_voted_for`
    /// in the branch council election phase.
    ///
    /// This is the first ballot: all Tumed leaders vote for up to N candidates;
    /// the top 9 by `BranchVoteCounts` become the council.
    #[pallet::storage]
    #[pallet::getter(fn branch_votes)]
    pub type BranchVotes<T: Config> = StorageDoubleMap<
        _,
        Blake2_128Concat,
        GovernmentBranch,
        Blake2_128Concat,
        T::AccountId, // voter
        T::AccountId, // candidate
        OptionQuery,
    >;

    /// DoubleMap: `GovernmentBranch` × `candidate` → vote tally (branch council phase).
    #[pallet::storage]
    #[pallet::getter(fn branch_vote_counts)]
    pub type BranchVoteCounts<T: Config> = StorageDoubleMap<
        _,
        Blake2_128Concat,
        GovernmentBranch,
        Blake2_128Concat,
        T::AccountId,
        u32,
        ValueQuery,
    >;

    /// The single Supreme Leader of each branch (Executive → Head of State,
    /// Judicial → Chief Justice, Banking → Central Banker).
    ///
    /// `Legislative` uses `KhuralChairman` instead.
    #[pallet::storage]
    #[pallet::getter(fn supreme_leaders)]
    pub type SupremeLeaders<T: Config> =
        StorageMap<_, Blake2_128Concat, GovernmentBranch, T::AccountId, OptionQuery>;

    /// DoubleMap: `GovernmentBranch` × `council_member` → `candidate_voted_for`
    /// in the Supreme Leader election phase.
    ///
    /// Only the 9 council members of a branch may vote here (second ballot).
    #[pallet::storage]
    #[pallet::getter(fn supreme_leader_votes)]
    pub type SupremeLeaderVotes<T: Config> = StorageDoubleMap<
        _,
        Blake2_128Concat,
        GovernmentBranch,
        Blake2_128Concat,
        T::AccountId, // council member (voter)
        T::AccountId, // candidate
        OptionQuery,
    >;

    /// DoubleMap: `GovernmentBranch` × `candidate` → vote tally (Supreme Leader phase).
    #[pallet::storage]
    #[pallet::getter(fn supreme_leader_vote_counts)]
    pub type SupremeLeaderVoteCounts<T: Config> = StorageDoubleMap<
        _,
        Blake2_128Concat,
        GovernmentBranch,
        Blake2_128Concat,
        T::AccountId,
        u32,
        ValueQuery,
    >;

    /// The elected Khural Chairman AND their original Arbad ID.
    ///
    /// Constitutional significance: the Chairman's Arbad officially represents
    /// the entire nation at the Confederation level. Storing `arbad_id` alongside
    /// the `AccountId` makes this constitutional fact immutable on-chain.
    ///
    /// Set by `elect_khural_chairman` after tallying votes from all Tumed leaders.
    #[pallet::storage]
    #[pallet::getter(fn khural_chairman)]
    pub type KhuralChairman<T: Config> = StorageValue<_, (T::AccountId, u32), OptionQuery>;

    /// Maps `voter (Tumed leader)` → `candidate_voted_for` in the Khural Chairman election.
    #[pallet::storage]
    #[pallet::getter(fn khural_votes)]
    pub type KhuralVotes<T: Config> =
        StorageMap<_, Blake2_128Concat, T::AccountId, T::AccountId, OptionQuery>;

    /// Maps `candidate` → tally in the Khural Chairman election.
    #[pallet::storage]
    #[pallet::getter(fn khural_vote_counts)]
    pub type KhuralVoteCounts<T: Config> =
        StorageMap<_, Blake2_128Concat, T::AccountId, u32, ValueQuery>;

    /// H-1 fix: Counts the number of **distinct** candidates who have received
    /// at least one vote in a given branch council election.
    ///
    /// This is incremented the **first** time a candidate receives any vote via
    /// `elect_branch_council`. If the count would exceed `T::MaxBranchCandidates`,
    /// the extrinsic returns `TooManyCandidates`, preventing an unbounded-growth
    /// DOS on `BranchVoteCounts`.
    ///
    /// Reset to 0 by `reset_ballot`.
    #[pallet::storage]
    #[pallet::getter(fn branch_candidate_count)]
    pub type BranchCandidateCount<T: Config> =
        StorageMap<_, Blake2_128Concat, GovernmentBranch, u32, ValueQuery>;

    // ── Constitutional size counters ─────────────────────────────────────────

    /// Counts registered citizens in each Arbad.
    ///
    /// Incremented by `add_citizen_to_arbad`.
    /// An Arbad must reach `T::MinArbadSize` (≥ 10) before its leader can be
    /// promoted to Zun via `promote_leader`.
    #[pallet::storage]
    #[pallet::getter(fn arbad_member_count)]
    pub type ArbadMemberCount<T: Config> =
        StorageMap<_, Blake2_128Concat, u32, u32, ValueQuery>;

    /// Counts Arbad leaders registered in each Zun-level zone (zun_id).
    ///
    /// A Zun must reach `T::MinZunSize` (≥ 100) before its leader can be
    /// promoted to Myangad via `promote_leader`.
    #[pallet::storage]
    #[pallet::getter(fn zun_leader_count)]
    pub type ZunLeaderCount<T: Config> =
        StorageMap<_, Blake2_128Concat, u32, u32, ValueQuery>;

    /// Counts Zun leaders registered in each Myangad-level zone (myangad_id).
    ///
    /// A Myangad must reach `T::MinMyangadSize` (≥ 1 000) before its leader
    /// can be promoted to Tumed via `promote_leader`.
    #[pallet::storage]
    #[pallet::getter(fn myangad_leader_count)]
    pub type MyangadLeaderCount<T: Config> =
        StorageMap<_, Blake2_128Concat, u32, u32, ValueQuery>;

    /// Maps each citizen to the Zun zone they belong to (set on leader promotion).
    #[pallet::storage]
    #[pallet::getter(fn citizen_zun)]
    pub type CitizenZun<T: Config> =
        StorageMap<_, Blake2_128Concat, T::AccountId, u32, OptionQuery>;

    /// Maps each citizen to the Myangad zone they belong to.
    #[pallet::storage]
    #[pallet::getter(fn citizen_myangad)]
    pub type CitizenMyangad<T: Config> =
        StorageMap<_, Blake2_128Concat, T::AccountId, u32, OptionQuery>;

    // =========================================================================
    // Events
    // =========================================================================

    #[pallet::event]
    #[pallet::generate_deposit(pub(super) fn deposit_event)]
    pub enum Event<T: Config> {
        // ── Decimal Hierarchy ────────────────────────────────────────────────
        /// A vote was successfully cast in a standard decimal-hierarchy election.
        VoteCast {
            election_id: u32,
            voter: T::AccountId,
            candidate: T::AccountId,
        },
        /// A citizen was enrolled in an Arbad.
        CitizenAddedToArbad {
            citizen: T::AccountId,
            arbad_id: u32,
        },
        /// A leader was promoted — their voting rights were elevated.
        LeaderPromoted {
            leader: T::AccountId,
            new_level: ElectionLevel,
        },

        // ── Peak Governance ─────────────────────────────────────────────────
        /// A Tumed leader voted in a Branch Council election.
        BranchVoteCast {
            branch: GovernmentBranch,
            voter: T::AccountId,
            candidate: T::AccountId,
        },
        /// The Branch Council of 9 has been confirmed for a branch.
        BranchCouncilElected {
            branch: GovernmentBranch,
            council: BoundedVec<T::AccountId, ConstU32<9>>,
        },
        /// A council member voted in the Supreme Leader election.
        SupremeLeaderVoteCast {
            branch: GovernmentBranch,
            voter: T::AccountId,
            candidate: T::AccountId,
        },
        /// A Supreme Leader has been elected for a branch.
        SupremeLeaderElected {
            branch: GovernmentBranch,
            leader: T::AccountId,
        },
        /// A Tumed leader cast their vote for the Khural Chairman.
        KhuralVoteCast {
            voter: T::AccountId,
            candidate: T::AccountId,
        },
        /// The Khural Chairman has been confirmed — their Arbad ID is stored.
        KhuralChairmanElected {
            chairman: T::AccountId,
            arbad_id: u32,
        },
        /// All ballot data for a branch has been purged — a new election cycle can begin.
        ///
        /// For `Executive`/`Judicial`/`Banking`: clears BranchVotes, BranchVoteCounts,
        /// SupremeLeaderVotes, SupremeLeaderVoteCounts, BranchCouncils, SupremeLeaders.
        /// For `Legislative`: clears KhuralVotes, KhuralVoteCounts, KhuralChairman.
        BallotReset { branch: GovernmentBranch },
    }

    // =========================================================================
    // Errors
    // =========================================================================

    #[pallet::error]
    pub enum Error<T> {
        // ── Decimal Hierarchy ────────────────────────────────────────────────
        /// The referenced election does not exist.
        ElectionNotFound,
        /// The election is no longer accepting votes.
        ElectionClosed,
        /// The candidate is not in the election's whitelist.
        CandidateNotFound,
        /// The voter has already cast a vote in this election.
        AlreadyVoted,
        /// The caller is not a member of any Arbad.
        NotInArbad,
        /// The caller's leadership level is insufficient for this tier.
        NotAuthorizedForLevel,
        /// Arithmetic overflow in vote count.
        MathOverflow,

        // ── Peak Governance ─────────────────────────────────────────────────
        /// Caller is not a Tumed-level leader.
        NotATumedLeader,
        /// The branch council for this branch has not been elected yet.
        CouncilNotElected,
        /// Caller is not a member of the specified branch council.
        NotACouncilMember,
        /// The winner holds no Arbad membership — `CitizenArbad` lookup failed.
        WinnerHasNoArbad,
        /// The candidate list provided is too long for the council size.
        TooManyCandidates,
        /// Not enough candidates were provided to fill the council.
        InsufficientCandidates,
        /// The Legislative branch does not use `elect_branch_council`.
        LegislativeUsesKhuralPath,
        /// The Khural Chairman election is only for the Legislative branch.
        NotLegislativeBranch,
        /// A proposed council winner has zero on-chain votes — Root supplied an
        /// invalid winner list (C-1 audit finding). Every winner must have at
        /// least one entry in `BranchVoteCounts[branch][winner]`.
        WinnerDidNotReceiveVotes,
        /// The election already exists — overwriting is forbidden (H-4).
        /// Use `reset_ballot` to clear the state and start a new cycle.
        ElectionAlreadyExists,
        /// The nominated branch council candidate is not a Tumed-level leader (H-2).
        /// Only leaders who have reached the peak of the decimal hierarchy
        /// may stand for election to a Branch Council.
        CandidateNotATumedLeader,

        // ── Constitutional size requirements ────────────────────────────────
        /// The Arbad has fewer than `MinArbadSize` (10) citizens.
        /// An Arbad must be full before its leader can represent it at Zun level.
        ArbadTooSmall,
        /// The Zun zone has fewer than `MinZunSize` (100) Arbad leaders.
        /// A Zun must be full before its leader can represent it at Myangad level.
        ZunTooSmall,
        /// The Myangad zone has fewer than `MinMyangadSize` (1 000) Zun leaders.
        /// A Myangad must be full before its leader can represent it at Tumed level.
        MyangadTooSmall,
        /// The Zun or Myangad zone ID was not provided when required for promotion.
        MissingZoneId,

        // ── Indigenous constraint (Legislative branch) ────────────────────
        /// The Legislative branch (Хурал) is restricted to indigenous citizens.
        /// At Zun level and above, knowledge of traditions and language is required.
        /// Only citizens whose father OR mother is indigenous may participate.
        NotIndigenous,
    }

    // =========================================================================
    // Helper: Tumed check
    // =========================================================================

    impl<T: Config> Pallet<T> {
        /// Returns `Ok(())` if `who` is a Tumed-level leader, else `NotATumedLeader`.
        fn ensure_tumed(who: &T::AccountId) -> DispatchResult {
            let level = ElectedLeaders::<T>::get(who).ok_or(Error::<T>::NotATumedLeader)?;
            ensure!(level >= ElectionLevel::Tumed, Error::<T>::NotATumedLeader);
            Ok(())
        }
    }

    // =========================================================================
    // Extrinsics
    // =========================================================================

    #[pallet::call]
    impl<T: Config> Pallet<T> {
        // ─── [0] cast_vote — Decimal Hierarchy ───────────────────────────────

        /// Cast a vote in an active decimal-hierarchy election (Arbad/Zun/Myangad/Tumed).
        ///
        /// Authorization is level-gated:
        /// - `Arbad`: voter must be in `CitizenArbad`.
        /// - `Zun/Myangad/Tumed`: voter must have `ElectedLeaders >= election.level`.
        #[pallet::call_index(0)]
        #[pallet::weight(T::WeightInfo::cast_vote())]
        pub fn cast_vote(
            origin: OriginFor<T>,
            election_id: u32,
            candidate: T::AccountId,
        ) -> DispatchResult {
            let voter = ensure_signed(origin)?;

            let election = Elections::<T>::get(election_id).ok_or(Error::<T>::ElectionNotFound)?;
            ensure!(election.is_active, Error::<T>::ElectionClosed);
            ensure!(
                election.candidates.contains(&candidate),
                Error::<T>::CandidateNotFound
            );
            ensure!(
                Votes::<T>::get(election_id, &voter).is_none(),
                Error::<T>::AlreadyVoted
            );

            match election.level {
                ElectionLevel::Arbad => {
                    ensure!(
                        CitizenArbad::<T>::contains_key(&voter),
                        Error::<T>::NotInArbad
                    );
                }
                ref required_level => {
                    let voter_level = ElectedLeaders::<T>::get(&voter)
                        .ok_or(Error::<T>::NotAuthorizedForLevel)?;
                    ensure!(
                        voter_level >= *required_level,
                        Error::<T>::NotAuthorizedForLevel
                    );
                }
            }

            Votes::<T>::insert(election_id, &voter, &candidate);
            VoteCounts::<T>::try_mutate(election_id, &candidate, |count| -> DispatchResult {
                *count = count.checked_add(1).ok_or(Error::<T>::MathOverflow)?;
                Ok(())
            })?;

            Self::deposit_event(Event::VoteCast {
                election_id,
                voter,
                candidate,
            });
            Ok(())
        }

        // ─── [1] add_citizen_to_arbad ─────────────────────────────────────────

        /// Register a citizen into a specific Arbad (Root-gated).
        ///
        /// Also increments the `ArbadMemberCount` for that arbad_id.
        /// When count reaches `T::MinArbadSize` (10), the Arbad is eligible
        /// to promote its elected leader to Zun.
        #[pallet::call_index(1)]
        #[pallet::weight(T::WeightInfo::add_citizen_to_arbad())]
        pub fn add_citizen_to_arbad(
            origin: OriginFor<T>,
            citizen: T::AccountId,
            arbad_id: u32,
        ) -> DispatchResult {
            ensure_root(origin)?;
            CitizenArbad::<T>::insert(&citizen, arbad_id);
            // Increment constitutional size counter.
            ArbadMemberCount::<T>::try_mutate(arbad_id, |count| -> DispatchResult {
                *count = count.checked_add(1).ok_or(Error::<T>::MathOverflow)?;
                Ok(())
            })?;
            Self::deposit_event(Event::CitizenAddedToArbad { citizen, arbad_id });
            Ok(())
        }

        // ─── [2] promote_leader ───────────────────────────────────────────────

        /// Grant elevated voting rights to an election winner (Root-gated).
        ///
        /// ## Constitutional size requirements
        ///
        /// Before a leader can be promoted, their unit must reach the minimum
        /// constitutional size:
        ///
        /// | Target level | Requirement | Field |
        /// |---|---|---|
        /// | `Zun` | Arbad has ≥ 10 citizens | `ArbadMemberCount[arbad_id]` |
        /// | `Myangad` | Zun zone has ≥ 100 Arbad leaders | `ZunLeaderCount[zun_id]` |
        /// | `Tumed` | Myangad zone has ≥ 1 000 Zun leaders | `MyangadLeaderCount[myangad_id]` |
        ///
        /// `zone_id` is required for Myangad and Tumed promotions.
        #[pallet::call_index(2)]
        #[pallet::weight(T::WeightInfo::promote_leader())]
        pub fn promote_leader(
            origin: OriginFor<T>,
            leader: T::AccountId,
            new_level: ElectionLevel,
            // Zone ID required for Myangad (zun_id) and Tumed (myangad_id) promotions.
            zone_id: Option<u32>,
        ) -> DispatchResult {
            ensure_root(origin)?;

            match &new_level {
                ElectionLevel::Zun => {
                    // Constitutional check: Arbad must be full (≥ MinArbadSize citizens).
                    // = 10 citizens must be registered in this Arbad before its leader
                    // can represent it at Zun level.
                    let arbad_id = CitizenArbad::<T>::get(&leader).ok_or(Error::<T>::NotInArbad)?;
                    let member_count = ArbadMemberCount::<T>::get(arbad_id);
                    ensure!(member_count >= T::MinArbadSize::get(), Error::<T>::ArbadTooSmall);

                    // Track this Arbad Leader in their Zun zone.
                    ZunLeaderCount::<T>::try_mutate(arbad_id, |c| -> DispatchResult {
                        *c = c.checked_add(1).ok_or(Error::<T>::MathOverflow)?;
                        Ok(())
                    })?;
                    CitizenZun::<T>::insert(&leader, arbad_id);
                }
                ElectionLevel::Myangad => {
                    // Constitutional check — DUAL requirement:
                    //
                    //  1. Zun zone must have ≥ MinZunLeaders (10) ELECTED Arbad Leaders.
                    //     (10 Arbads each elected their leader → 10 Arbad Leaders in this Zun)
                    //
                    //  2. Because each of those 10 Arbad Leaders required their Arbad to
                    //     have ≥ 10 citizens, the Zun implicitly has ≥ 100 total citizens
                    //     (MinZunSize = 100). No separate member-count storage is needed.
                    //
                    // Together: Зун имеет ≥ 100 членов И 10 Лидеров Арбадов.
                    let zun_id = zone_id.ok_or(Error::<T>::MissingZoneId)?;
                    let leader_count = ZunLeaderCount::<T>::get(zun_id);
                    ensure!(
                        leader_count >= T::MinZunLeaders::get(),
                        Error::<T>::ZunTooSmall
                    );

                    MyangadLeaderCount::<T>::try_mutate(zun_id, |c| -> DispatchResult {
                        *c = c.checked_add(1).ok_or(Error::<T>::MathOverflow)?;
                        Ok(())
                    })?;
                    CitizenMyangad::<T>::insert(&leader, zun_id);
                }
                ElectionLevel::Tumed => {
                    // Constitutional check — DUAL requirement:
                    //
                    //  1. Myangad zone must have ≥ MinMyangadLeaders (10) ELECTED Zun Leaders.
                    //     (10 Zuns each elected their leader → 10 Zun Leaders in this Myangad)
                    //
                    //  2. Because each of those 10 Zun Leaders required their Zun to have
                    //     ≥ 10 Arbad Leaders and ≥ 100 citizens, the Myangad implicitly has
                    //     ≥ 1 000 total citizens (MinMyangadSize = 1 000).
                    //
                    // Together: Мянгад имеет ≥ 1 000 членов И 10 Лидеров Зунов.
                    let myangad_id = zone_id.ok_or(Error::<T>::MissingZoneId)?;
                    let leader_count = MyangadLeaderCount::<T>::get(myangad_id);
                    ensure!(
                        leader_count >= T::MinMyangadLeaders::get(),
                        Error::<T>::MyangadTooSmall
                    );
                }
                ElectionLevel::Arbad => {
                    // No size requirement for Arbad-level registration.
                }
            }

            ElectedLeaders::<T>::insert(&leader, &new_level);
            Self::deposit_event(Event::LeaderPromoted { leader, new_level });
            Ok(())
        }

        // ─── [3] create_election ─────────────────────────────────────────────

        /// Create and open a new decimal-hierarchy election (Root-gated, V1 mock).
        ///
        /// H-4 fix: Returns `ElectionAlreadyExists` if the `election_id` is already
        /// in use. Prevents silent state overwrite which could be exploited to
        /// invalidate in-progress vote tallies. Use `reset_ballot` (for Peak
        /// Governance) or a future `close_election` extrinsic (for decimal-hierarchy)
        /// to clear state before reopening.
        #[pallet::call_index(3)]
        #[pallet::weight(T::WeightInfo::create_election())]
        pub fn create_election(
            origin: OriginFor<T>,
            election_id: u32,
            level: ElectionLevel,
            candidates: Vec<T::AccountId>,
        ) -> DispatchResult {
            ensure_root(origin)?;

            // H-4: block overwrite of existing elections.
            ensure!(
                !Elections::<T>::contains_key(election_id),
                Error::<T>::ElectionAlreadyExists
            );

            let bounded: BoundedVec<T::AccountId, T::MaxCandidates> = candidates
                .try_into()
                .map_err(|_| DispatchError::Other("too many candidates"))?;
            Elections::<T>::insert(
                election_id,
                Election {
                    level,
                    is_active: true,
                    candidates: bounded,
                },
            );
            Ok(())
        }

        // ─── [4] elect_branch_council ─────────────────────────────────────────

        /// Phase 1 – Branch Council Election (Executive / Judicial / Banking only).
        ///
        /// All Tumed-level leaders of the branch vote.
        /// Candidates can be ANY citizen of that branch — not just Tumed leaders.
        /// The TOP-9 vote-getters become the Branch Council.
        /// Call `confirm_branch_council` after voting closes to seat the top-9.
        ///
        /// Role: `Legislative` is ineligible here — use `elect_khural_chairman`.
        ///
        /// Checks:
        /// 1. Branch is not `Legislative`.
        /// 2. **Candidate** is a Tumed-level leader (H-2 — must have proven merit).
        /// 3. Caller holds `ElectedLeaders = Tumed`.
        /// 4. Caller has not already voted for this branch.
        /// 5. Distinct candidate count does not exceed `T::MaxBranchCandidates` (H-1).
        #[pallet::call_index(4)]
        #[pallet::weight(T::WeightInfo::elect_branch_council())]
        pub fn elect_branch_council(
            origin: OriginFor<T>,
            branch: GovernmentBranch,
            candidate: T::AccountId,
        ) -> DispatchResult {
            let voter = ensure_signed(origin)?;

            // Legislative uses the Khural path, not Branch Council.
            ensure!(
                branch != GovernmentBranch::Legislative,
                Error::<T>::LegislativeUsesKhuralPath
            );

            // H-2: Only Tumed-level leaders may stand as candidates.
            // Guards against nominating arbitrary accounts that could never
            // be confirmed (they would fail the `WinnerDidNotReceiveVotes`
            // check anyway, but this blocks the vote from being recorded at all).
            let candidate_level =
                ElectedLeaders::<T>::get(&candidate).ok_or(Error::<T>::CandidateNotATumedLeader)?;
            ensure!(
                candidate_level >= ElectionLevel::Tumed,
                Error::<T>::CandidateNotATumedLeader
            );

            // Only Tumed leaders may vote at the Peak.
            Self::ensure_tumed(&voter)?;

            // No double-voting per branch.
            ensure!(
                BranchVotes::<T>::get(&branch, &voter).is_none(),
                Error::<T>::AlreadyVoted
            );

            // H-1: Enforce MaxBranchCandidates to prevent unbounded storage growth.
            // Increment the count only when this candidate is seeing their FIRST vote.
            let is_new_candidate = BranchVoteCounts::<T>::get(&branch, &candidate) == 0;
            if is_new_candidate {
                let current = BranchCandidateCount::<T>::get(&branch);
                let new_count = current.checked_add(1).ok_or(Error::<T>::MathOverflow)?;
                ensure!(
                    new_count <= T::MaxBranchCandidates::get(),
                    Error::<T>::TooManyCandidates
                );
                BranchCandidateCount::<T>::insert(&branch, new_count);
            }

            BranchVotes::<T>::insert(&branch, &voter, &candidate);
            BranchVoteCounts::<T>::try_mutate(&branch, &candidate, |count| -> DispatchResult {
                *count = count.checked_add(1).ok_or(Error::<T>::MathOverflow)?;
                Ok(())
            })?;

            Self::deposit_event(Event::BranchVoteCast {
                branch,
                voter,
                candidate,
            });
            Ok(())
        }

        // ─── [5] confirm_branch_council ───────────────────────────────────────

        /// Phase 1 — Finalise: seat the top-9 vote-getters as the Branch Council.
        ///
        /// Root-gated in V1 (production: auto-finalise via `on_finalize` or relayer).
        /// Caller supplies the ordered list of winners (the relayer computes this
        /// from `BranchVoteCounts`). The pallet enforces exactly 9 members.
        ///
        /// Overrides any existing council (allows re-election).
        #[pallet::call_index(5)]
        #[pallet::weight(T::WeightInfo::confirm_branch_council())]
        pub fn confirm_branch_council(
            origin: OriginFor<T>,
            branch: GovernmentBranch,
            winners: Vec<T::AccountId>,
        ) -> DispatchResult {
            ensure_root(origin)?;

            ensure!(
                branch != GovernmentBranch::Legislative,
                Error::<T>::LegislativeUsesKhuralPath
            );
            ensure!(
                winners.len() == COUNCIL_SIZE as usize,
                Error::<T>::InsufficientCandidates
            );

            // ── C-1 fix: on-chain validation of vote tallies ─────────────────
            //
            // Root supplies the intended winner list. We verify each winner
            // actually received ≥ 1 on-chain vote.  This prevents Root from
            // arbitrarily seating an account that was never nominated.
            //
            // Production hardening note: a stronger check would verify that
            // `winners` contains the true top-9 by vote count (no omissions).
            // That requires sorting `BranchVoteCounts` on-chain, which is
            // O(N) over all candidates — safe only after `MaxBranchCandidates`
            // is enforced (H-1).  For V1 we validate ≥ 1 vote; a relayer
            // must supply the correct top-9 ordering.
            for winner in &winners {
                let votes = BranchVoteCounts::<T>::get(&branch, winner);
                ensure!(votes > 0, Error::<T>::WinnerDidNotReceiveVotes);
            }

            let council: BoundedVec<T::AccountId, ConstU32<9>> = winners
                .try_into()
                .map_err(|_| Error::<T>::TooManyCandidates)?;

            BranchCouncils::<T>::insert(&branch, council.clone());

            Self::deposit_event(Event::BranchCouncilElected { branch, council });
            Ok(())
        }

        // ─── [6] elect_supreme_leader ─────────────────────────────────────────

        /// Phase 2 — Supreme Leader Election.
        ///
        /// **Only the 9 members of the branch's council** may vote here.
        /// Each member casts one vote for a candidate; the candidate with the
        /// most votes becomes `SupremeLeaders[branch]`.
        ///
        /// This is a **live ballot** — votes are recorded and the leader is
        /// updated any time a new majority emerges. In V1, Root finalises the
        /// winner by calling `confirm_supreme_leader` after the voting window.
        ///
        /// Checks:
        /// 1. Branch council exists for this branch.
        /// 2. Caller is a member of that council.
        /// 3. Caller has not already voted (single vote per council member).
        #[pallet::call_index(6)]
        #[pallet::weight(T::WeightInfo::elect_supreme_leader())]
        pub fn elect_supreme_leader(
            origin: OriginFor<T>,
            branch: GovernmentBranch,
            candidate: T::AccountId,
        ) -> DispatchResult {
            let voter = ensure_signed(origin)?;

            // Constitutional separation (AGENTS.md §2.6):
            // The Khural (Legislative) has a Chairman, NOT a Supreme Leader.
            ensure!(
                branch != GovernmentBranch::Legislative,
                Error::<T>::LegislativeUsesKhuralPath
            );

            // Council must be confirmed first.
            let council = BranchCouncils::<T>::get(&branch).ok_or(Error::<T>::CouncilNotElected)?;

            // Voter must be a council member.
            ensure!(council.contains(&voter), Error::<T>::NotACouncilMember);

            // No double-voting.
            ensure!(
                SupremeLeaderVotes::<T>::get(&branch, &voter).is_none(),
                Error::<T>::AlreadyVoted
            );

            SupremeLeaderVotes::<T>::insert(&branch, &voter, &candidate);
            SupremeLeaderVoteCounts::<T>::try_mutate(
                &branch,
                &candidate,
                |count| -> DispatchResult {
                    *count = count.checked_add(1).ok_or(Error::<T>::MathOverflow)?;
                    Ok(())
                },
            )?;

            Self::deposit_event(Event::SupremeLeaderVoteCast {
                branch,
                voter,
                candidate,
            });
            Ok(())
        }

        // ─── [7] confirm_supreme_leader ───────────────────────────────────────

        /// Phase 2 — Finalise: record the council-elected Supreme Leader on-chain.
        ///
        /// Root-gated in V1. Caller supplies the winning `AccountId`; the pallet
        /// writes it to `SupremeLeaders[branch]` and emits `SupremeLeaderElected`.
        #[pallet::call_index(7)]
        #[pallet::weight(T::WeightInfo::confirm_supreme_leader())]
        pub fn confirm_supreme_leader(
            origin: OriginFor<T>,
            branch: GovernmentBranch,
            leader: T::AccountId,
        ) -> DispatchResult {
            ensure_root(origin)?;

            ensure!(
                branch != GovernmentBranch::Legislative,
                Error::<T>::LegislativeUsesKhuralPath
            );

            SupremeLeaders::<T>::insert(&branch, &leader);

            Self::deposit_event(Event::SupremeLeaderElected { branch, leader });
            Ok(())
        }

        // ─── [8] elect_khural_chairman ────────────────────────────────────────

        /// Legislative branch ballot — All Tumed leaders vote directly for the
        /// Khural Chairman.
        ///
        /// This is a **running ballot**: votes accumulate in `KhuralVoteCounts`.
        /// After the voting window closes, Root calls `confirm_khural_chairman`
        /// to finalise and store `(winner_AccountId, winner_arbad_id)`.
        ///
        /// Checks:
        /// 1. Caller holds `ElectedLeaders = Tumed`.
        /// 2. Caller has not already voted.
        #[pallet::call_index(8)]
        #[pallet::weight(T::WeightInfo::elect_khural_chairman())]
        pub fn elect_khural_chairman(
            origin: OriginFor<T>,
            candidate: T::AccountId,
        ) -> DispatchResult {
            let voter = ensure_signed(origin)?;

            // ── Constitutional: Legislative branch is indigenous-only ──────────
            // Отец ИЛИ мать должен быть коренным (Jus Sanguinis).
            // Уровень Зун и выше: знание языка и традиций не ниже разговорного.
            ensure!(
                T::IsIndigenous::is_indigenous(&voter),
                Error::<T>::NotIndigenous
            );
            ensure!(
                T::IsIndigenous::is_indigenous(&candidate),
                Error::<T>::NotIndigenous
            );

            // Only Tumed leaders vote for the Khural Chairman.
            Self::ensure_tumed(&voter)?;

            // No double-voting.
            ensure!(
                KhuralVotes::<T>::get(&voter).is_none(),
                Error::<T>::AlreadyVoted
            );

            KhuralVotes::<T>::insert(&voter, &candidate);
            KhuralVoteCounts::<T>::try_mutate(&candidate, |count| -> DispatchResult {
                *count = count.checked_add(1).ok_or(Error::<T>::MathOverflow)?;
                Ok(())
            })?;

            Self::deposit_event(Event::KhuralVoteCast { voter, candidate });
            Ok(())
        }

        // ─── [9] confirm_khural_chairman ──────────────────────────────────────

        /// Legislative branch — Finalise: record the Khural Chairman on-chain.
        ///
        /// **Constitutional requirement**: the winner's original `arbad_id` is
        /// fetched from `CitizenArbad` and stored alongside their `AccountId`.
        /// That Arbad officially represents the nation at the Confederation level.
        ///
        /// Fails with `WinnerHasNoArbad` if the winner has no Arbad registration.
        /// Root-gated in V1; production will wire this to `on_finalize`.
        #[pallet::call_index(9)]
        #[pallet::weight(T::WeightInfo::confirm_khural_chairman())]
        pub fn confirm_khural_chairman(
            origin: OriginFor<T>,
            chairman: T::AccountId,
        ) -> DispatchResult {
            ensure_root(origin)?;

            // Fetch the winner's Arbad ID — constitutionally required.
            let arbad_id = CitizenArbad::<T>::get(&chairman).ok_or(Error::<T>::WinnerHasNoArbad)?;

            KhuralChairman::<T>::put((&chairman, arbad_id));

            Self::deposit_event(Event::KhuralChairmanElected { chairman, arbad_id });
            Ok(())
        }

        // ─── [10] reset_ballot ────────────────────────────────────────────────

        /// Purge all voting state for a branch, enabling a new election cycle.
        ///
        /// This is the constitutionally required mechanism between election terms.
        /// Without calling this, all voters remain in `AlreadyVoted` state forever
        /// and no second election cycle is possible (see audit finding C-2).
        ///
        /// **For `Executive` / `Judicial` / `Banking`** — clears:
        /// - `BranchVotes[branch]` (all voter → candidate entries)
        /// - `BranchVoteCounts[branch]` (all candidate tallies)
        /// - `SupremeLeaderVotes[branch]` (all council member votes)
        /// - `SupremeLeaderVoteCounts[branch]` (all SL candidate tallies)
        /// - `BranchCouncils[branch]` (the seated council)
        /// - `SupremeLeaders[branch]` (the confirmed leader)
        ///
        /// **For `Legislative`** — clears:
        /// - `KhuralVotes` (all Tumed voter → candidate entries)
        /// - `KhuralVoteCounts` (all chairman candidate tallies)
        /// - `KhuralChairman` (the confirmed chairman + arbad_id)
        ///
        /// Root-gated in V1. Must be called between election cycles.
        ///
        /// > ⚠️  Weight is a V1 estimate. This extrinsic requires benchmarking
        /// > before mainnet since `remove_prefix` cost scales with storage size.
        #[pallet::call_index(10)]
        #[pallet::weight(T::WeightInfo::reset_ballot())]
        pub fn reset_ballot(origin: OriginFor<T>, branch: GovernmentBranch) -> DispatchResult {
            ensure_root(origin)?;

            match &branch {
                GovernmentBranch::Legislative => {
                    // Clear all Khural Chairman ballot data.
                    // `clear(u32::MAX, None)` removes all entries in the map.
                    let _ = KhuralVotes::<T>::clear(u32::MAX, None);
                    let _ = KhuralVoteCounts::<T>::clear(u32::MAX, None);
                    KhuralChairman::<T>::kill();
                }
                b => {
                    // Clear branch council ballot (phase 1).
                    // `clear_prefix(k1, limit)` removes all DoubleMap entries
                    // whose first key == `b`. u32::MAX = no limit.
                    let _ = BranchVotes::<T>::clear_prefix(b, u32::MAX, None);
                    let _ = BranchVoteCounts::<T>::clear_prefix(b, u32::MAX, None);

                    // H-1: reset the distinct-candidate counter for this branch.
                    BranchCandidateCount::<T>::remove(b);

                    // Clear supreme leader ballot (phase 2).
                    let _ = SupremeLeaderVotes::<T>::clear_prefix(b, u32::MAX, None);
                    let _ = SupremeLeaderVoteCounts::<T>::clear_prefix(b, u32::MAX, None);

                    // Remove the confirmed council and leader.
                    BranchCouncils::<T>::remove(b);
                    SupremeLeaders::<T>::remove(b);
                }
            }

            Self::deposit_event(Event::BallotReset { branch });
            Ok(())
        }
    }
}
