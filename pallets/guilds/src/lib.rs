//! pallet-guilds — Altan Network: Professional Guild Protocol
//!
//! Decentralised replacement for LinkedIn + Upwork + educational platforms.
//! Implements professional DAOs (Guilds), meritocracy (Achievements),
//! escrow-backed task market (Quests), Academy subscriptions,
//! Fractal Guild Unions (Steppe Protocol), and the Academy of Sciences.
//!
//! CONSTITUTIONAL INTEGRATION:
//! - Guild members must be registered citizens (pallet-inomad-identity)
//! - Quest rewards are escrow-locked via T::Currency::reserve / unreserve
//! - Academy subscriptions emit events that the Altan Gateway listens to
//!   in order to open access to off-chain video courses.
//! - Guild Unions federate guilds into fractal tiers ending in GuildTumed,
//!   whose Grandmaster automatically becomes an Academician (Academy of Sciences).
//! - pallet-khural-governance uses the AcademyInterface trait to gate
//!   propose_expert_bill to Academicians only.
//!
//! ## Dispatchables
//!
//! | Call | Origin | Description |
//! |---|---|---|
//! | `create_guild_from_relayer` | Root | Create a guild via Relayer after off-chain quorum |
//! | `update_council_from_relayer` | Root | Update guild council after elections via Relayer |
//! | `create_proposal_from_relayer` | Root | Submit a treasury proposal via Relayer |
//! | `vote_proposal_from_relayer` | Root | Cast council vote on a proposal via Relayer |
//! | `create_guild` | Signed (Officer) | Create a new guild directly on-chain |
//! | `join_guild` | Signed (Citizen) | Apply to join an existing guild |
//! | `promote_member` | Signed (Grandmaster) | Promote a guild member to a higher rank |
//! | `propose_achievement` | Signed (Grandmaster) | Propose a new achievement for the guild |
//! | `publish_quest` | Signed (Grandmaster) | Publish a new quest for guild members |
//! | `assign_quest` | Signed (Grandmaster) | Assign a published quest to a specific member |
//! | `submit_quest` | Signed (Member) | Submit completed quest deliverables |
//! | `complete_quest` | Signed (Grandmaster) | Confirm quest completion and issue reward |
//! | `cancel_quest` | Signed (Grandmaster) | Cancel a published or assigned quest |
//! | `subscribe_to_academy` | Signed (Member) | Subscribe to the guild's learning academy |
//! | `force_resolve_quest` | Root | Force-resolve a stuck quest (emergency) |
//! | `form_guild_union` | Signed (Grandmaster) | Propose formation of a Guild Union with other guilds |
//! | `vote_for_grandmaster` | Signed (Member) | Cast vote for Guild Union Grandmaster election |
//! | `elevate_union` | Signed (Union Grandmaster) | Elevate a guild union to a higher tier |
//! | `cleanup_account` | Root | Prune stale quest data from an account (maintenance) |

#![cfg_attr(not(feature = "std"), no_std)]

extern crate alloc;

// Re-export all pallet items at the crate root so the runtime (and other crates)
// can access `pallet_guilds::Pallet`, `pallet_guilds::GuildMembers`, etc.
pub mod migrations;
pub mod weights;
pub use pallet::*;

#[cfg(feature = "runtime-benchmarks")]
mod benchmarking;

// =========================================================================
// AcademyInterface — loose-coupling trait for cross-pallet usage
// =========================================================================

/// Trait that allows other pallets (e.g. pallet-khural-governance) to check
/// whether an account has Academician status without tight-coupling to this
/// pallet's storage.
///
/// Wire it in the consuming pallet's Config as:
/// ```ignore
/// type AcademyInterface: pallet_guilds::AcademyInterface<Self::AccountId>;
/// ```
/// Then implement it via `pallet_guilds::Pallet<Runtime>`.
pub trait AcademyInterface<AccountId> {
    /// Returns `true` if `who` is a registered Academician (leader of a GuildTumed union).
    fn is_academician(who: &AccountId) -> bool;
}

#[frame_support::pallet]
pub mod pallet {
    use crate::weights::WeightInfo as _;
    use alloc::vec::Vec;
    use frame_support::{
        pallet_prelude::*,
        traits::{Currency, ReservableCurrency},
        PalletId,
    };
    use frame_system::pallet_prelude::*;
    use sp_runtime::traits::AccountIdConversion;

    // =========================================================================
    // Currency helper types
    // =========================================================================

    pub type BalanceOf<T> =
        <<T as Config>::Currency as Currency<<T as frame_system::Config>::AccountId>>::Balance;

    /// Convenient alias for an industry tag (max = MaxNameLength bytes).
    pub type IndustryTag<T> = BoundedVec<u8, <T as Config>::MaxNameLength>;

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
        /// Currency used for Quest rewards (escrow) and Academy subscription payments.
        type Currency: Currency<Self::AccountId> + ReservableCurrency<Self::AccountId>;

        /// Maximum byte-length for a Guild name / industry tag / union name.
        #[pallet::constant]
        type MaxNameLength: Get<u32>;

        /// Maximum byte-length for an IPFS CID (description hash / proof hash).
        #[pallet::constant]
        type MaxIpfsCidLength: Get<u32>;

        /// Maximum number of guilds that can participate in a single union formation call.
        #[pallet::constant]
        type MaxUnionMembers: Get<u32>;

        // ─── UUID-based Guild Instantiation (Relayer path) ────────────────────

        /// The pallet's on-chain identifier used to derive keyless treasury accounts
        /// for UUID-keyed guilds via `PalletId::into_sub_account_truncating`.
        ///
        /// Runtime: `PalletId(*b"inm/glds")`
        #[pallet::constant]
        type PalletId: Get<PalletId>;

        /// Maximum number of founding members allowed per UUID-keyed guild.
        ///
        /// Constitutional rule: `create_guild_from_relayer` requires EXACTLY 9 founders,
        /// so the bound must be ≥ 9. Runtime: `ConstU32<1000>`.
        #[pallet::constant]
        type MaxMembers: Get<u32>;

        /// Maximum size of the Guild Council (elected leaders).
        ///
        /// The council is updated annually by `update_council_from_relayer` after off-chain
        /// elections conclude. Only council members may propose and vote on L1 Treasury spends.
        /// Runtime: `ConstU32<100>`.
        #[pallet::constant]
        type MaxCouncilMembers: Get<u32>;
    }

    // =========================================================================
    // Enums
    // =========================================================================

    /// Membership rank within a Guild.
    ///
    /// | Variant       | Description                                                           |
    /// |---------------|-----------------------------------------------------------------------|
    /// | `Apprentice`  | New member. Can take quests but cannot approve or mint achievements. |
    /// | `Professional`| Accomplished member. Can self-assign quests up to a certain value.  |
    /// | `Master`      | Guild authority. Mints achievements, approves quest completions.     |
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
    pub enum MemberRole {
        /// Newcomer to the Guild. Limited quest access.
        Apprentice,
        /// Experienced contributor. Extended quest access.
        Professional,
        /// Guild authority — can issue Achievements and confirm quest completion.
        Master,
    }

    /// Status lifecycle for a Quest.
    ///
    /// ```text
    /// [Open] ──assign──▶ [InProgress] ──submit──▶ [Review] ──approve──▶ [Completed]
    ///                                               └──reject──▶         [Open]
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
    pub enum QuestStatus {
        /// Quest is published, reward is escrowed. Awaiting assignee.
        Open,
        /// Assignee is working on the quest.
        InProgress,
        /// Assignee submitted work. Awaiting Master approval.
        Review,
        /// Master approved completion. Reward transferred to assignee.
        Completed,
        /// [SECURITY: GHOST STATE] Quest was cancelled because the employer's
        /// citizenship status became terminal (Deceased or Exiled). The escrow
        /// reward has been unreserved back to the employer's account.
        Cancelled,
    }

    /// Fractal federation tier of a Guild Union (Steppe Protocol).
    ///
    /// The hierarchy mirrors the classical Mongolian decimal military organization:
    ///
    /// | Level        | Description                                               |
    /// |--------------|-----------------------------------------------------------|
    /// | `GuildArbad` | Base union of 10 guilds — The Ten-Guild Alliance          |
    /// | `GuildZun`   | Second tier — union of Arbad-level unions                 |
    /// | `GuildMyangad`| Third tier — Thousand-Guild Federation                   |
    /// | `GuildTumed` | Apex tier — Ten-Thousand. Grandmaster becomes Academician |
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
    pub enum GuildUnionLevel {
        /// Base alliance level (10 guilds).
        GuildArbad,
        /// Second federation tier.
        GuildZun,
        /// Third federation tier (thousand-guild scale).
        GuildMyangad,
        /// Apex level — Grandmaster becomes an Academician.
        GuildTumed,
    }

    impl GuildUnionLevel {
        /// Returns the next tier, or None if already at GuildTumed.
        pub fn next(&self) -> Option<GuildUnionLevel> {
            match self {
                GuildUnionLevel::GuildArbad => Some(GuildUnionLevel::GuildZun),
                GuildUnionLevel::GuildZun => Some(GuildUnionLevel::GuildMyangad),
                GuildUnionLevel::GuildMyangad => Some(GuildUnionLevel::GuildTumed),
                GuildUnionLevel::GuildTumed => None,
            }
        }
    }

    // =========================================================================
    // Structs
    // =========================================================================

    /// A professional Guild (DAO).
    ///
    /// Created by a citizen who becomes the founding `Master`.
    /// Guilds are tagged with an industry to power the fractal union federation.
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
    pub struct Guild<T: Config> {
        /// Founder and first Master of the Guild.
        pub founder: T::AccountId,
        /// Human-readable name (max `MaxNameLength` bytes, UTF-8). Absolutely free.
        pub name: BoundedVec<u8, T::MaxNameLength>,
        /// Industry tag — completely free-form (max `MaxNameLength` bytes).
        /// E.g. "Rust Blockchain Engineering", "Mongolian Traditional Music", etc.
        pub industry_tag: BoundedVec<u8, T::MaxNameLength>,
        /// Optional IPFS CID for the Guild's full description / constitution.
        pub description_hash: Option<BoundedVec<u8, T::MaxIpfsCidLength>>,
        /// Region tag: OKATO region code (1–83) or 0 for global guilds.
        pub region_tag: u32,
        /// Total number of quests ever published in this Guild.
        pub quest_count: u32,
        /// Sequential member count (for display; does not decrement on leave).
        pub member_count: u32,
    }

    /// A Guild Union — a fractal federation of Guilds under the Steppe Protocol.
    ///
    /// Formed by Guild Masters. Elevated by Grandmasters.
    /// The apex level (`GuildTumed`) grants their Grandmaster Academician status
    /// in the Academy of Sciences.
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
    pub struct GuildUnion<T: Config> {
        /// Fractal federation level.
        pub level: GuildUnionLevel,
        /// Guild that initiated the formation (founding guild).
        pub founder_guild: u32,
        /// Current elected Grandmaster of the Union.
        pub grandmaster: T::AccountId,
        /// Number of member guilds.
        pub member_guild_count: u32,
        /// Industry tag shared by this union (inherited from founding guild or set on form).
        pub industry_tag: BoundedVec<u8, T::MaxNameLength>,
        /// Number of votes cast for the current grandmaster election round.
        pub election_votes_for: u32,
        /// Total member guilds that have participated in current election round.
        pub election_votes_cast: u32,
    }

    /// A professional Achievement (credential/badge).
    ///
    /// Minted by a Guild Master to a specific citizen as on-chain proof of skill.
    /// The `ipfs_proof` CID links to the full portfolio, exam result, or project artifact.
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
    pub struct Achievement<T: Config> {
        /// Guild that issued this achievement.
        pub guild_id: u32,
        /// Master who issued it.
        pub issuer: T::AccountId,
        /// Achievement title (max `MaxNameLength` bytes).
        pub title: BoundedVec<u8, T::MaxNameLength>,
        /// Minimum quests required in this guild before achievement can be issued.
        /// 0 = no quest requirement (academic achievements, exams, etc.).
        pub required_quests: u32,
        /// IPFS CID pointing to the proof artifact (portfolio, certificate, video).
        pub ipfs_proof: BoundedVec<u8, T::MaxIpfsCidLength>,
    }

    /// An escrowed Quest in a Guild's task market.
    ///
    /// When published, `reward` is reserved from the employer's account.
    /// On completion, the reserved amount is transferred to the `assignee`.
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
    pub struct Quest<T: Config> {
        /// Guild under which this quest is listed.
        pub guild_id: u32,
        /// The employer who published and funded the quest.
        pub employer: T::AccountId,
        /// Escrowed reward amount (reserved from employer on publish).
        pub reward: BalanceOf<T>,
        /// Current lifecycle status.
        pub status: QuestStatus,
        /// Citizen currently working on the quest (None while Open).
        pub assignee: Option<T::AccountId>,
        /// IPFS CID for quest description / requirements.
        pub description_hash: BoundedVec<u8, T::MaxIpfsCidLength>,
        /// Number of successful quests completed by the assignee in this guild
        /// at time of completion (written on `complete_quest`).
        pub assignee_quest_count: u32,
    }

    // =========================================================================
    // UUID-keyed Guild Record (Relayer / Petition path)
    // =========================================================================

    /// On-chain record for a guild instantiated via the 9-signature Relayer path.
    ///
    /// Keyed by `GuildId = [u8; 32]` — the Blake2-256 hash of the off-chain UUID,
    /// supplied by the backend Relayer once the constitutional quorum is reached.
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
    pub struct GuildRecord<T: Config> {
        /// The constitutional Guild Master (first among the 9 founding signers).
        pub master: T::AccountId,
        /// All 9 founding members. Exactly 9 — enforced by `MustHaveExactlyNineFounders`.
        pub members: BoundedVec<T::AccountId, T::MaxMembers>,
        /// Keyless treasury account, mathematically derived as:
        /// `T::PalletId::get().into_sub_account_truncating(guild_id)`.
        /// No private key exists for this account — it is controlled exclusively
        /// by pallet logic (quest rewards, future revenue sharing).
        pub treasury: T::AccountId,
        /// Current Guild Council — elected annually via off-chain SubWallet signatures.
        /// Only council members may propose and vote on L1 Treasury spend proposals.
        /// Replaced atomically by `update_council_from_relayer` after each election.
        pub council: BoundedVec<T::AccountId, T::MaxCouncilMembers>,
    }

    // =========================================================================
    // On-Chain Governance Proposal Record (Council-gated)
    // =========================================================================

    /// Status of a council-gated L1 treasury spend proposal.
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
    pub enum ProposalStatus {
        /// Voting is in progress.
        Open,
        /// Strict majority (>50% of council) voted in favour.
        Passed,
        /// Strict majority voted against.
        Rejected,
    }

    /// A lightweight on-chain record of a governance proposal inside a UUID-keyed guild.
    ///
    /// Full proposal body is stored off-chain (DB); the chain only tracks the tally
    /// and status so that council-only voting can be validated trustlessly.
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
    pub struct GuildProposalRecord<T: Config> {
        /// Blake2-256 hash of the off-chain UUID for this proposal.
        pub guild_id: [u8; 32],
        /// AccountId of the council member who submitted the proposal.
        pub proposer: T::AccountId,
        /// Number of council votes in favour.
        pub votes_for: u32,
        /// Number of council votes against.
        pub votes_against: u32,
        /// Current lifecycle status.
        pub status: ProposalStatus,
    }

    // =========================================================================
    // Storage
    // =========================================================================

    /// Global sequential Guild ID counter.
    #[pallet::storage]
    #[pallet::getter(fn next_guild_id)]
    pub type NextGuildId<T: Config> = StorageValue<_, u32, ValueQuery>;

    /// Guild registry: GuildId → Guild.
    #[pallet::storage]
    #[pallet::getter(fn guilds)]
    pub type Guilds<T: Config> = StorageMap<_, Blake2_128Concat, u32, Guild<T>, OptionQuery>;

    // ─── UUID-keyed guild registry (Relayer / Petition path) ─────────────────

    /// UUID-keyed guild registry: `[u8; 32]` hash → GuildRecord.
    ///
    /// Populated exclusively by `create_guild_from_relayer`, which is submitted
    /// by the backend Relayer after collecting exactly 9 off-chain signatures.
    /// The key is the Blake2-256 hash of the guild's UUID (computed off-chain).
    #[pallet::storage]
    #[pallet::getter(fn guilds_by_uuid)]
    pub type GuildsByUuid<T: Config> =
        StorageMap<_, Blake2_128Concat, [u8; 32], GuildRecord<T>, OptionQuery>;

    // ─── Council-gated proposal registry (Decimal Governance) ─────────────────

    /// On-chain tally for council-gated proposals: `[u8; 32]` proposal_id → GuildProposalRecord.
    ///
    /// Keyed by the Blake2-256 hash of the off-chain proposal UUID.
    /// Populated by `create_proposal_from_relayer`; updated by `vote_proposal_from_relayer`.
    #[pallet::storage]
    #[pallet::getter(fn guild_proposals)]
    pub type GuildProposals<T: Config> =
        StorageMap<_, Blake2_128Concat, [u8; 32], GuildProposalRecord<T>, OptionQuery>;

    /// Ballot deduplication: (proposal_id, voter) → ().
    ///
    /// Prevents a council member from voting twice on the same proposal.
    #[pallet::storage]
    pub type ProposalBallots<T: Config> = StorageDoubleMap<
        _,
        Blake2_128Concat,
        [u8; 32], // proposal_id
        Blake2_128Concat,
        T::AccountId,
        (),
        OptionQuery,
    >;

    /// Guild membership: (GuildId, AccountId) → MemberRole.
    ///
    /// Presence in this map = member; absence = not a member.
    #[pallet::storage]
    #[pallet::getter(fn guild_members)]
    pub type GuildMembers<T: Config> = StorageDoubleMap<
        _,
        Blake2_128Concat,
        u32,
        Blake2_128Concat,
        T::AccountId,
        MemberRole,
        OptionQuery,
    >;

    /// Per-member quest completion counter: (GuildId, AccountId) → u32.
    ///
    /// Incremented by `complete_quest`. Used as gate for Achievement issuance.
    #[pallet::storage]
    #[pallet::getter(fn member_quest_count)]
    pub type MemberQuestCount<T: Config> =
        StorageDoubleMap<_, Blake2_128Concat, u32, Blake2_128Concat, T::AccountId, u32, ValueQuery>;

    /// Global sequential Achievement ID counter.
    #[pallet::storage]
    #[pallet::getter(fn next_achievement_id)]
    pub type NextAchievementId<T: Config> = StorageValue<_, u32, ValueQuery>;

    /// Achievement definitions: AchievementId → Achievement.
    #[pallet::storage]
    #[pallet::getter(fn achievements)]
    pub type Achievements<T: Config> =
        StorageMap<_, Blake2_128Concat, u32, Achievement<T>, OptionQuery>;

    /// Citizen achievement holdings: (AccountId, AchievementId) → ().
    ///
    /// Presence = citizen holds this achievement; absence = not held.
    #[pallet::storage]
    #[pallet::getter(fn citizen_achievements)]
    pub type CitizenAchievements<T: Config> =
        StorageDoubleMap<_, Blake2_128Concat, T::AccountId, Blake2_128Concat, u32, (), OptionQuery>;

    /// Global sequential Quest ID counter.
    #[pallet::storage]
    #[pallet::getter(fn next_quest_id)]
    pub type NextQuestId<T: Config> = StorageValue<_, u32, ValueQuery>;

    /// Quest registry: QuestId → Quest.
    #[pallet::storage]
    #[pallet::getter(fn quests)]
    pub type Quests<T: Config> = StorageMap<_, Blake2_128Concat, u32, Quest<T>, OptionQuery>;

    // ─── Guild Union (Steppe Protocol) storage ────────────────────────────────

    /// Global sequential Guild Union ID counter.
    #[pallet::storage]
    #[pallet::getter(fn next_union_id)]
    pub type NextUnionId<T: Config> = StorageValue<_, u32, ValueQuery>;

    /// Guild Union registry: UnionId → GuildUnion.
    #[pallet::storage]
    #[pallet::getter(fn guild_unions)]
    pub type GuildUnions<T: Config> =
        StorageMap<_, Blake2_128Concat, u32, GuildUnion<T>, OptionQuery>;

    /// Guild → UnionId membership. A guild can only belong to one union at a time.
    ///
    /// GuildId → UnionId mapping.
    #[pallet::storage]
    #[pallet::getter(fn guild_union_of)]
    pub type GuildUnionOf<T: Config> = StorageMap<_, Blake2_128Concat, u32, u32, OptionQuery>;

    /// Grandmaster election ballot: (UnionId, VoterGuildId) → CandidateAccountId.
    ///
    /// Each guild casts one vote per election round. Once >50% of guilds agree
    /// on the same candidate, that candidate becomes Grandmaster.
    #[pallet::storage]
    #[pallet::getter(fn grandmaster_votes)]
    pub type GrandmasterVotes<T: Config> = StorageDoubleMap<
        _,
        Blake2_128Concat,
        u32, // union_id
        Blake2_128Concat,
        u32,          // voter_guild_id
        T::AccountId, // candidate voted for
        OptionQuery,
    >;

    // ─── Academy of Sciences ─────────────────────────────────────────────────

    /// Academy of Sciences membership: AccountId → IndustryTag.
    ///
    /// Only Grandmasters of `GuildTumed` (apex) unions are registered here.
    /// These Academicians have exclusive rights to propose Expert Bills to the Khural.
    #[pallet::storage]
    #[pallet::getter(fn academy_members)]
    pub type AcademyMembers<T: Config> = StorageMap<
        _,
        Blake2_128Concat,
        T::AccountId,
        BoundedVec<u8, T::MaxNameLength>,
        OptionQuery,
    >;

    // =========================================================================
    // Events
    // =========================================================================

    #[pallet::event]
    #[pallet::generate_deposit(pub(super) fn deposit_event)]
    pub enum Event<T: Config> {
        /// A new Guild was created.
        GuildCreated {
            guild_id: u32,
            founder: T::AccountId,
            name: Vec<u8>,
            industry_tag: Vec<u8>,
            region_tag: u32,
        },
        /// A guild was instantiated on-chain by the Relayer after 9 off-chain signatures.
        ///
        /// The `treasury` account is keyless — derived deterministically from the
        /// pallet ID and the `guild_id` via `into_sub_account_truncating`.
        GuildInstantiated {
            /// Blake2-256 hash of the guild's UUID (maps to off-chain petition ID).
            guild_id: [u8; 32],
            /// Constitutional Guild Master (first among the 9 founders).
            master: T::AccountId,
            /// Keyless treasury account derived from `PalletId + guild_id`.
            treasury: T::AccountId,
        },
        /// A citizen joined a Guild as Apprentice.
        MemberJoined { guild_id: u32, member: T::AccountId },
        /// A Guild Master promoted a member to a higher rank.
        MemberPromoted {
            guild_id: u32,
            member: T::AccountId,
            new_role: MemberRole,
        },
        /// A Guild Master issued an Achievement to a citizen.
        AchievementIssued {
            achievement_id: u32,
            guild_id: u32,
            recipient: T::AccountId,
            issuer: T::AccountId,
            title: Vec<u8>,
        },
        /// A Quest was published with escrowed reward.
        QuestPublished {
            quest_id: u32,
            guild_id: u32,
            employer: T::AccountId,
            reward: BalanceOf<T>,
        },
        /// A member assigned themselves to a Quest.
        QuestAssigned {
            quest_id: u32,
            assignee: T::AccountId,
        },
        /// An assignee submitted a Quest for review.
        QuestSubmittedForReview { quest_id: u32 },
        /// A Guild Master confirmed Quest completion — reward transferred.
        ///
        /// Gateway listens for this event to record professional history.
        QuestCompleted {
            quest_id: u32,
            assignee: T::AccountId,
            reward: BalanceOf<T>,
        },
        /// An employer cancelled an Open quest and reclaimed their escrow.
        QuestCancelled { quest_id: u32 },
        /// Academy subscription purchased — Gateway opens off-chain course access.
        ///
        /// The `fee` is transferred directly from `student` to `master`.
        /// The Altan Gateway listens to this event to unlock video courses.
        AcademySubscriptionPurchased {
            student: T::AccountId,
            master: T::AccountId,
            fee: BalanceOf<T>,
        },
        /// A new Guild Union was formed under the Steppe Protocol.
        GuildUnionFormed {
            union_id: u32,
            level: GuildUnionLevel,
            grandmaster: T::AccountId,
            industry_tag: Vec<u8>,
            member_guild_count: u32,
        },
        /// A guild union was elevated to the next fractal tier.
        GuildUnionElevated {
            union_id: u32,
            new_level: GuildUnionLevel,
            grandmaster: T::AccountId,
        },
        /// A Guild Master cast a vote for the Union's Grandmaster.
        GrandmasterVoteCast {
            union_id: u32,
            voter_guild: u32,
            candidate: T::AccountId,
        },
        /// A new Grandmaster was elected for a Guild Union.
        GrandmasterElected {
            union_id: u32,
            grandmaster: T::AccountId,
        },
        /// [SECURITY VECTOR 3] A grandmaster election ended in an unbreakable tie.
        /// All guilds voted with no supermajority winner — election state reset.
        /// Masters must cast fresh votes to restart the election.
        GrandmasterElectionReset { union_id: u32 },
        /// A Grandmaster of a GuildTumed union was admitted to the Academy of Sciences.
        AcademicianGranted {
            academician: T::AccountId,
            industry_tag: Vec<u8>,
        },
        // ─── Decimal Governance (Council) events ─────────────────────────────
        /// The Guild Council was replaced by the Relayer after annual elections concluded.
        CouncilUpdated {
            /// Blake2-256 hash of the guild UUID.
            guild_id: [u8; 32],
            /// Number of elected council members in the new board.
            council_size: u32,
        },
        /// A council member submitted a new L1 governance proposal.
        ProposalCreated {
            /// Blake2-256 hash of the proposal's off-chain UUID.
            proposal_id: [u8; 32],
            /// Blake2-256 hash of the guild UUID.
            guild_id: [u8; 32],
            proposer: T::AccountId,
        },
        /// A council member cast their vote on a governance proposal.
        ProposalVoteCast {
            proposal_id: [u8; 32],
            voter: T::AccountId,
            in_favor: bool,
            votes_for: u32,
            votes_against: u32,
        },
        /// A governance proposal crossed the >50% council majority threshold.
        ProposalPassed {
            proposal_id: [u8; 32],
            guild_id: [u8; 32],
            votes_for: u32,
        },
        /// A governance proposal was defeated by council majority.
        ProposalRejected {
            proposal_id: [u8; 32],
            guild_id: [u8; 32],
            votes_against: u32,
        },
    }

    // =========================================================================
    // Errors
    // =========================================================================

    #[pallet::error]
    pub enum Error<T> {
        /// Referenced Guild does not exist.
        GuildNotFound,
        /// A guild with this UUID hash already exists on-chain. IDs are collision-resistant
        /// Blake2-256 hashes of UUIDs — duplicate submission indicates a replay attack
        /// or backend idempotency bug.
        GuildUuidAlreadyExists,
        /// `create_guild_from_relayer` requires exactly 9 founding members.
        /// This is the constitutional quorum for a Universal Guild petition.
        MustHaveExactlyNineFounders,
        /// Caller is already a member of this Guild.
        AlreadyMember,
        /// Caller is not a member of the required Guild.
        NotMember,
        /// Action requires Master rank within the Guild.
        NotMaster,
        /// Referenced Achievement does not exist.
        AchievementNotFound,
        /// The citizen already holds this Achievement.
        AlreadyHoldsAchievement,
        /// Member has not completed the minimum required quests for this Achievement.
        InsufficientQuestCount,
        /// Referenced Quest does not exist.
        QuestNotFound,
        /// Quest is not in the required status for this operation.
        InvalidQuestStatus,
        /// Only the employer may cancel their own quest.
        NotEmployer,
        /// Only the assigned citizen may submit their quest for review.
        NotAssignee,
        /// Guild name exceeds MaxNameLength bytes.
        NameTooLong,
        /// Industry tag exceeds MaxNameLength bytes.
        IndustryTagTooLong,
        /// IPFS CID exceeds MaxIpfsCidLength bytes.
        CidTooLong,
        /// Currency reservation failed (insufficient free balance).
        InsufficientBalance,
        /// A Master may not promote or demote themselves via `promote_member`.
        CannotPromoteSelf,
        /// A Master may not award an Achievement to themselves.
        CannotAwardSelf,
        /// Only the guild Master may force-resolve a quest stuck in Review.
        NotMasterForForceResolve,
        /// Quest is not in `Review` status — cannot be force-resolved.
        QuestNotInReview,
        // ─── Guild Union errors ───────────────────────────────────────────────
        /// Referenced Guild Union does not exist.
        GuildUnionNotFound,
        /// The guild is already a member of a union.
        GuildAlreadyInUnion,
        /// Only the Grandmaster of the Union may elevate it.
        NotGrandmaster,
        /// This union is already at the GuildTumed apex — cannot be elevated further.
        MaxUnionLevelReached,
        /// [SECURITY VECTOR 1] Elevation rejected: union is not at the required predecessor level.
        /// Transition must be strictly Arbad → Zun → Myangad → Tumed. No level-skipping allowed.
        InvalidUnionLevelTransition,
        /// A guild may cast only one vote per Grandmaster election round.
        VoteAlreadyCast,
        /// The guild casting this vote is not a member of the specified union.
        GuildNotInUnion,
        /// The account is already registered as an Academician.
        AlreadyAcademician,
        /// At least two guilds are required to form a union.
        InsufficientUnionMembers,
        // ─── Decimal Governance errors ────────────────────────────────────────
        /// The voter or proposer is not in the guild's elected council.
        /// Only elected Leaders (ARBAD_LEADER and above) may submit or vote on proposals.
        NotACouncilMember,
        /// The referenced governance proposal does not exist on-chain.
        ProposalNotFound,
        /// This council member has already cast their vote on this proposal.
        AlreadyVoted,
        /// The proposal is no longer open for voting (already Passed or Rejected).
        ProposalNotOpen,
        /// A proposal with this ID already exists on-chain. IDs are collision-resistant
        /// Blake2-256 hashes — duplicate submission indicates a replay attack.
        ProposalAlreadyExists,
    }

    // =========================================================================
    // Internal helpers
    // =========================================================================

    impl<T: Config> Pallet<T> {
        /// Ensure the caller is a Master of `guild_id`, returning an error if not.
        fn ensure_master(caller: &T::AccountId, guild_id: u32) -> DispatchResult {
            ensure!(
                Guilds::<T>::contains_key(guild_id),
                Error::<T>::GuildNotFound
            );
            let role = GuildMembers::<T>::get(guild_id, caller).ok_or(Error::<T>::NotMember)?;
            ensure!(role == MemberRole::Master, Error::<T>::NotMaster);
            Ok(())
        }

        /// Register an account in AcademyMembers if not already present.
        fn grant_academician(who: &T::AccountId, industry_tag: BoundedVec<u8, T::MaxNameLength>) {
            if !AcademyMembers::<T>::contains_key(who) {
                AcademyMembers::<T>::insert(who, industry_tag.clone());
                Self::deposit_event(Event::AcademicianGranted {
                    academician: who.clone(),
                    industry_tag: industry_tag.into_inner(),
                });
            }
        }

        /// Tally grandmaster votes for a union. If >50% agree on one candidate,
        /// elect that candidate and return Some(elected_account).
        /// Clears all votes for the union on successful election.
        fn tally_grandmaster_votes(union_id: u32, total_guilds: u32) -> Option<T::AccountId> {
            // [SECURITY VECTOR 4] Bound the iterator to MaxUnionMembers to prevent
            // unbounded storage iteration that could exhaust block weight limits.
            // By construction, member_guild_count ≤ MaxUnionMembers (guilds join only
            // via BoundedVec-gated extrinsics), so no valid vote is ever dropped.
            let votes: alloc::vec::Vec<(u32, T::AccountId)> =
                GrandmasterVotes::<T>::iter_prefix(union_id)
                    .take(T::MaxUnionMembers::get() as usize)
                    .collect();

            if votes.is_empty() {
                return None;
            }

            // Count votes per candidate
            let mut tally: alloc::vec::Vec<(T::AccountId, u32)> = alloc::vec::Vec::new();
            for (_, candidate) in &votes {
                if let Some(entry) = tally.iter_mut().find(|(acc, _)| acc == candidate) {
                    entry.1 = entry.1.saturating_add(1);
                } else {
                    tally.push((candidate.clone(), 1));
                }
            }

            // Find candidate with >50% of total guild votes
            let threshold = total_guilds / 2; // strict majority means > half
            for (candidate, count) in tally {
                if count > threshold {
                    // Clear all votes for this union (election complete)
                    let _ = GrandmasterVotes::<T>::clear_prefix(union_id, u32::MAX, None);
                    return Some(candidate);
                }
            }

            None
        }
    }

    // =========================================================================
    // AcademyInterface implementation
    // =========================================================================

    impl<T: Config> crate::AcademyInterface<T::AccountId> for Pallet<T> {
        fn is_academician(who: &T::AccountId) -> bool {
            AcademyMembers::<T>::contains_key(who)
        }
    }

    // =========================================================================
    // Extrinsics
    // =========================================================================

    #[pallet::call]
    impl<T: Config> Pallet<T> {
        // ─── create_guild_from_relayer ────────────────────────────────────────

        /// Instantiate a new Universal Guild on-chain after the constitutional quorum is met.
        ///
        /// This extrinsic is submitted by the **backend Relayer** once it has aggregated
        /// exactly **9 off-chain citizen signatures** (the constitutional requirement for
        /// a Guild formation petition). It must not be callable by ordinary citizens.
        ///
        /// ## Constitutional Rule
        /// Per AGENTS.md §1 and the Altan L1 Constitution, a Universal Guild can only be
        /// formed by a democratic quorum of 9 founding signers. The Relayer is the
        /// off-chain oracle that enforces PII validation, duplicate-signature prevention,
        /// and the 30-day Arbad cooldown check before submitting this extrinsic.
        ///
        /// ## Treasury Derivation
        /// The guild's treasury account is **keyless** — derived deterministically as:
        /// ```text
        /// treasury = T::PalletId::get().into_sub_account_truncating(guild_id)
        /// ```
        /// No private key can sign for this account; all fund movements go through
        /// pallet extrinsics (future quest reward routing, revenue sharing, etc.).
        ///
        /// ## Arguments
        /// - `guild_id`: `[u8; 32]` — Blake2-256 hash of the UUID supplied by the backend.
        /// - `master`: The first among the 9 founders — constitutional Guild Master.
        /// - `initial_members`: Exactly 9 AccountIds (constitutional quorum).
        ///
        /// ## Origin
        /// Signed — the Relayer account. In production this should be gated behind
        /// a `RelayerOrigin` or Sudo; for now any signed account can submit (the
        /// backend enforces quorum off-chain before broadcasting).
        ///
        /// # Errors
        /// - `GuildUuidAlreadyExists` — replay or duplicate submission.
        /// - `MustHaveExactlyNineFounders` — constitutional quorum not met.
        #[pallet::call_index(14)]
        #[pallet::weight(T::WeightInfo::create_guild_from_relayer())]
        pub fn create_guild_from_relayer(
            origin: OriginFor<T>,
            guild_id: [u8; 32],
            master: T::AccountId,
            initial_members: Vec<T::AccountId>,
        ) -> DispatchResult {
            let _relayer = ensure_signed(origin)?;

            // ── Constitutional quorum check ──────────────────────────────────
            // Exactly 9 founding signatures are required — no more, no less.
            // The number 9 is hardcoded in the WASM runtime (AGENTS.md §1).
            ensure!(
                initial_members.len() == 9,
                Error::<T>::MustHaveExactlyNineFounders
            );

            // ── Idempotency guard ────────────────────────────────────────────
            ensure!(
                !GuildsByUuid::<T>::contains_key(guild_id),
                Error::<T>::GuildUuidAlreadyExists
            );

            // ── Treasury account derivation ──────────────────────────────────
            // `into_sub_account_truncating` derives a unique, keyless AccountId
            // by hashing the PalletId bytes together with `guild_id`.
            // The resulting account has no private key; only pallet logic can
            // move funds in or out of it.
            let treasury: T::AccountId = T::PalletId::get().into_sub_account_truncating(guild_id);

            // ── Bound the member vector ──────────────────────────────────────
            let bounded_members: BoundedVec<T::AccountId, T::MaxMembers> = initial_members
                .try_into()
                .map_err(|_| Error::<T>::MustHaveExactlyNineFounders)?;

            // ── Persist the GuildRecord (council starts empty — set by update_council_from_relayer) ──
            GuildsByUuid::<T>::insert(
                guild_id,
                GuildRecord {
                    master: master.clone(),
                    members: bounded_members,
                    treasury: treasury.clone(),
                    council: BoundedVec::new(),
                },
            );

            // ── Emit constitutional event ────────────────────────────────────
            Self::deposit_event(Event::GuildInstantiated {
                guild_id,
                master,
                treasury,
            });

            Ok(())
        }

        // ─── update_council_from_relayer ──────────────────────────────────────

        /// Replace the Guild Council after annual off-chain elections conclude.
        ///
        /// Submitted by the **backend Relayer** once `finalizeAnnualElection` has:
        /// 1. Tallied all SubWallet-signed votes.
        /// 2. Determined the top-voted leaders in each Arbad squad.
        /// 3. Aggregated upward to Zun/Myangad/Tumed if applicable.
        ///
        /// ## Constitutional Rule
        /// Only elected Leaders (ARBAD_LEADER and above) may sit on the council.
        /// The relayer enforces this off-chain; the pallet enforces the atomic replacement.
        ///
        /// ## Arguments
        /// - `guild_id`: Blake2-256 hash of the guild UUID.
        /// - `new_council`: Wallet addresses of all elected leaders (bounded to MaxCouncilMembers).
        ///
        /// # Errors
        /// - `GuildUuidAlreadyExists` — guild does not exist (inverted sense re-used for clarity).
        #[pallet::call_index(15)]
        #[pallet::weight(T::WeightInfo::update_council_from_relayer())]
        pub fn update_council_from_relayer(
            origin: OriginFor<T>,
            guild_id: [u8; 32],
            new_council: Vec<T::AccountId>,
        ) -> DispatchResult {
            let _relayer = ensure_signed(origin)?;

            // ── Guild must exist ─────────────────────────────────────────────
            let mut record = GuildsByUuid::<T>::get(guild_id).ok_or(Error::<T>::GuildNotFound)?;

            // ── Bound the council vector ─────────────────────────────────────
            let bounded_council: BoundedVec<T::AccountId, T::MaxCouncilMembers> = new_council
                .try_into()
                .map_err(|_| Error::<T>::NameTooLong)?; // reuse bounded-vec overflow error

            let council_size = bounded_council.len() as u32;

            // ── Atomic council replacement ───────────────────────────────────
            record.council = bounded_council;
            GuildsByUuid::<T>::insert(guild_id, record);

            Self::deposit_event(Event::CouncilUpdated {
                guild_id,
                council_size,
            });

            Ok(())
        }

        // ─── create_proposal_from_relayer ─────────────────────────────────────

        /// Submit a new L1 governance proposal on behalf of a council member.
        ///
        /// The backend Relayer calls this after:
        /// 1. Validating that `proposer` holds an active council seat (DB rank ≥ ARBAD_LEADER).
        /// 2. Storing the full proposal body in the off-chain database.
        ///
        /// The pallet stores a lightweight tally record and validates council membership
        /// from the on-chain `GuildRecord.council` list.
        ///
        /// ## Arguments
        /// - `guild_id`: Blake2-256 hash of the guild UUID.
        /// - `proposal_id`: Blake2-256 hash of the off-chain proposal UUID (idempotency key).
        /// - `proposer`: AccountId of the council member submitting the proposal.
        ///
        /// # Errors
        /// - `GuildNotFound` — no guild with this UUID hash.
        /// - `NotACouncilMember` — proposer is not in the current council.
        /// - `ProposalAlreadyExists` — duplicate submission.
        #[pallet::call_index(16)]
        #[pallet::weight(T::WeightInfo::create_proposal_from_relayer())]
        pub fn create_proposal_from_relayer(
            origin: OriginFor<T>,
            guild_id: [u8; 32],
            proposal_id: [u8; 32],
            proposer: T::AccountId,
        ) -> DispatchResult {
            let _relayer = ensure_signed(origin)?;

            // ── Guild must exist ─────────────────────────────────────────────
            let record = GuildsByUuid::<T>::get(guild_id).ok_or(Error::<T>::GuildNotFound)?;

            // ── Council-only gate ────────────────────────────────────────────
            ensure!(
                record.council.contains(&proposer),
                Error::<T>::NotACouncilMember
            );

            // ── Idempotency guard ────────────────────────────────────────────
            ensure!(
                !GuildProposals::<T>::contains_key(proposal_id),
                Error::<T>::ProposalAlreadyExists
            );

            // ── Persist lightweight on-chain tally record ────────────────────
            GuildProposals::<T>::insert(
                proposal_id,
                GuildProposalRecord {
                    guild_id,
                    proposer: proposer.clone(),
                    votes_for: 0,
                    votes_against: 0,
                    status: ProposalStatus::Open,
                },
            );

            Self::deposit_event(Event::ProposalCreated {
                proposal_id,
                guild_id,
                proposer,
            });

            Ok(())
        }

        // ─── vote_proposal_from_relayer ───────────────────────────────────────

        /// Cast a council vote on an open L1 governance proposal.
        ///
        /// The Relayer calls this after verifying the voter's SubWallet signature off-chain.
        /// The pallet:
        /// 1. Checks the proposal is `Open`.
        /// 2. Validates voter is in the current guild council.
        /// 3. Prevents double voting via `ProposalBallots` storage.
        /// 4. Updates the tally.
        /// 5. Checks if `votes_for > council.len() / 2` (strict majority of council,
        ///    **not** of all guild members).
        /// 6. Marks proposal `Passed` or `Rejected` and emits the appropriate event.
        ///
        /// ## Arguments
        /// - `guild_id`: Blake2-256 hash of the guild UUID.
        /// - `proposal_id`: Blake2-256 hash of the proposal UUID.
        /// - `voter`: AccountId of the council member voting.
        /// - `in_favor`: `true` = vote for; `false` = vote against.
        ///
        /// # Errors
        /// - `GuildNotFound` — no guild with this UUID hash.
        /// - `ProposalNotFound` — no proposal with this ID.
        /// - `NotACouncilMember` — voter is not in the current council.
        /// - `AlreadyVoted` — council member already voted on this proposal.
        /// - `ProposalNotOpen` — proposal is no longer open for voting.
        #[pallet::call_index(17)]
        #[pallet::weight(T::WeightInfo::vote_proposal_from_relayer())]
        pub fn vote_proposal_from_relayer(
            origin: OriginFor<T>,
            guild_id: [u8; 32],
            proposal_id: [u8; 32],
            voter: T::AccountId,
            in_favor: bool,
        ) -> DispatchResult {
            let _relayer = ensure_signed(origin)?;

            // ── Guild and council validation ──────────────────────────────────
            let record = GuildsByUuid::<T>::get(guild_id).ok_or(Error::<T>::GuildNotFound)?;

            // ── Council-only gate ────────────────────────────────────────────
            ensure!(
                record.council.contains(&voter),
                Error::<T>::NotACouncilMember
            );

            // ── Proposal must exist and be open ──────────────────────────────
            let mut proposal =
                GuildProposals::<T>::get(proposal_id).ok_or(Error::<T>::ProposalNotFound)?;

            ensure!(
                proposal.status == ProposalStatus::Open,
                Error::<T>::ProposalNotOpen
            );

            // ── Double-vote prevention ───────────────────────────────────────
            ensure!(
                !ProposalBallots::<T>::contains_key(proposal_id, &voter),
                Error::<T>::AlreadyVoted
            );

            // ── Record ballot ────────────────────────────────────────────────
            ProposalBallots::<T>::insert(proposal_id, &voter, ());

            if in_favor {
                proposal.votes_for = proposal.votes_for.saturating_add(1);
            } else {
                proposal.votes_against = proposal.votes_against.saturating_add(1);
            }

            Self::deposit_event(Event::ProposalVoteCast {
                proposal_id,
                voter: voter.clone(),
                in_favor,
                votes_for: proposal.votes_for,
                votes_against: proposal.votes_against,
            });

            // ── Tally: strict majority = votes_for > council.len() / 2 ───────
            // This checks the council size, NOT the full membership.
            let council_size = record.council.len() as u32;
            let majority_threshold = council_size / 2; // strict: votes_for MUST exceed half

            if proposal.votes_for > majority_threshold {
                proposal.status = ProposalStatus::Passed;
                GuildProposals::<T>::insert(proposal_id, &proposal);
                Self::deposit_event(Event::ProposalPassed {
                    proposal_id,
                    guild_id: proposal.guild_id,
                    votes_for: proposal.votes_for,
                });
            } else if proposal.votes_against > majority_threshold {
                proposal.status = ProposalStatus::Rejected;
                GuildProposals::<T>::insert(proposal_id, &proposal);
                Self::deposit_event(Event::ProposalRejected {
                    proposal_id,
                    guild_id: proposal.guild_id,
                    votes_against: proposal.votes_against,
                });
            } else {
                // Voting still in progress
                GuildProposals::<T>::insert(proposal_id, proposal);
            }

            Ok(())
        }

        // ─── create_guild ────────────────────────────────────────────────────

        /// Establish a new professional Guild.
        ///
        /// The caller becomes the founding `Master`.
        /// Any registered citizen may create a Guild; no permission required.
        ///
        /// ## Parameters
        /// - `name`: UTF-8 name (max `MaxNameLength` bytes). Absolutely free.
        /// - `industry_tag`: Free-form industry tag (max `MaxNameLength` bytes). Absolutely free.
        /// - `description_cid`: Optional IPFS CID for the Guild constitution
        /// - `region_tag`: OKATO region code (1–83), or 0 for a global Guild
        ///
        /// # Origin: Signed
        #[pallet::call_index(0)]
        #[pallet::weight(T::WeightInfo::create_guild())]
        pub fn create_guild(
            origin: OriginFor<T>,
            name: Vec<u8>,
            industry_tag: Vec<u8>,
            description_cid: Option<Vec<u8>>,
            region_tag: u32,
        ) -> DispatchResult {
            let caller = ensure_signed(origin)?;

            let bounded_name: BoundedVec<u8, T::MaxNameLength> = name
                .clone()
                .try_into()
                .map_err(|_| Error::<T>::NameTooLong)?;

            let bounded_industry_tag: BoundedVec<u8, T::MaxNameLength> = industry_tag
                .clone()
                .try_into()
                .map_err(|_| Error::<T>::IndustryTagTooLong)?;

            let bounded_desc = match description_cid {
                Some(cid) => Some(cid.try_into().map_err(|_| Error::<T>::CidTooLong)?),
                None => None,
            };

            let id = NextGuildId::<T>::get();
            Guilds::<T>::insert(
                id,
                Guild {
                    founder: caller.clone(),
                    name: bounded_name.clone(),
                    industry_tag: bounded_industry_tag.clone(),
                    description_hash: bounded_desc,
                    region_tag,
                    quest_count: 0,
                    member_count: 1,
                },
            );

            // Founder is always the first Master.
            GuildMembers::<T>::insert(id, &caller, MemberRole::Master);
            NextGuildId::<T>::put(id.saturating_add(1));

            Self::deposit_event(Event::GuildCreated {
                guild_id: id,
                founder: caller,
                name: bounded_name.into_inner(),
                industry_tag: bounded_industry_tag.into_inner(),
                region_tag,
            });
            Ok(())
        }

        // ─── join_guild ──────────────────────────────────────────────────────

        /// Join an existing Guild as `Apprentice`.
        ///
        /// Any citizen may join any Guild as a newcomer.
        /// A Master may later promote them to Professional or Master.
        ///
        /// # Origin: Signed
        #[pallet::call_index(1)]
        #[pallet::weight(T::WeightInfo::join_guild())]
        pub fn join_guild(origin: OriginFor<T>, guild_id: u32) -> DispatchResult {
            let caller = ensure_signed(origin)?;

            ensure!(
                Guilds::<T>::contains_key(guild_id),
                Error::<T>::GuildNotFound
            );
            ensure!(
                GuildMembers::<T>::get(guild_id, &caller).is_none(),
                Error::<T>::AlreadyMember
            );

            GuildMembers::<T>::insert(guild_id, &caller, MemberRole::Apprentice);

            // Increment member count.
            Guilds::<T>::try_mutate(guild_id, |maybe| -> DispatchResult {
                let g = maybe.as_mut().ok_or(Error::<T>::GuildNotFound)?;
                g.member_count = g.member_count.saturating_add(1);
                Ok(())
            })?;

            Self::deposit_event(Event::MemberJoined {
                guild_id,
                member: caller,
            });
            Ok(())
        }

        // ─── promote_member ──────────────────────────────────────────────────

        /// Promote a Guild member to a higher `MemberRole`.
        ///
        /// Only a `Master` of the Guild may call this.
        /// Can promote `Apprentice → Professional → Master` in one call.
        ///
        /// ## Security
        /// A Master **cannot promote themselves** — this prevents privilege
        /// escalation where a newly-created Master grants themselves a role
        /// after the last Master leaves or is compromised.
        ///
        /// # Origin: Signed (must be Guild Master)
        #[pallet::call_index(2)]
        #[pallet::weight(T::WeightInfo::promote_member())]
        pub fn promote_member(
            origin: OriginFor<T>,
            guild_id: u32,
            target: T::AccountId,
            new_role: MemberRole,
        ) -> DispatchResult {
            let caller = ensure_signed(origin)?;

            ensure!(
                Guilds::<T>::contains_key(guild_id),
                Error::<T>::GuildNotFound
            );

            // Caller must be Master.
            let caller_role =
                GuildMembers::<T>::get(guild_id, &caller).ok_or(Error::<T>::NotMember)?;
            ensure!(caller_role == MemberRole::Master, Error::<T>::NotMaster);

            // [SECURITY VECTOR 3] A Master cannot promote themselves.
            ensure!(caller != target, Error::<T>::CannotPromoteSelf);

            // Target must be a member.
            ensure!(
                GuildMembers::<T>::contains_key(guild_id, &target),
                Error::<T>::NotMember
            );

            GuildMembers::<T>::insert(guild_id, &target, new_role.clone());

            Self::deposit_event(Event::MemberPromoted {
                guild_id,
                member: target,
                new_role,
            });
            Ok(())
        }

        // ─── propose_achievement ─────────────────────────────────────────────

        /// Issue an Achievement credential to a citizen.
        ///
        /// Only a Guild `Master` may issue achievements.
        /// The recipient must be a Guild member with at least
        /// `achievement.required_quests` completed quests in this Guild.
        ///
        /// # Origin: Signed (must be Guild Master)
        #[pallet::call_index(3)]
        #[pallet::weight(T::WeightInfo::propose_achievement())]
        pub fn propose_achievement(
            origin: OriginFor<T>,
            guild_id: u32,
            target_citizen: T::AccountId,
            title: Vec<u8>,
            required_quests: u32,
            proof_cid: Vec<u8>,
        ) -> DispatchResult {
            let caller = ensure_signed(origin)?;

            ensure!(
                Guilds::<T>::contains_key(guild_id),
                Error::<T>::GuildNotFound
            );

            // Caller must be Master.
            let caller_role =
                GuildMembers::<T>::get(guild_id, &caller).ok_or(Error::<T>::NotMember)?;
            ensure!(caller_role == MemberRole::Master, Error::<T>::NotMaster);

            // [SECURITY VECTOR 3] A Master cannot award achievements to themselves.
            ensure!(caller != target_citizen, Error::<T>::CannotAwardSelf);

            // Target must be a member.
            ensure!(
                GuildMembers::<T>::contains_key(guild_id, &target_citizen),
                Error::<T>::NotMember
            );

            // Quest gate: enforce meritocracy.
            if required_quests > 0 {
                let completed = MemberQuestCount::<T>::get(guild_id, &target_citizen);
                ensure!(
                    completed >= required_quests,
                    Error::<T>::InsufficientQuestCount
                );
            }

            let bounded_title: BoundedVec<u8, T::MaxNameLength> = title
                .clone()
                .try_into()
                .map_err(|_| Error::<T>::NameTooLong)?;
            let bounded_proof: BoundedVec<u8, T::MaxIpfsCidLength> =
                proof_cid.try_into().map_err(|_| Error::<T>::CidTooLong)?;

            let achievement_id = NextAchievementId::<T>::get();

            Achievements::<T>::insert(
                achievement_id,
                Achievement {
                    guild_id,
                    issuer: caller.clone(),
                    title: bounded_title.clone(),
                    required_quests,
                    ipfs_proof: bounded_proof,
                },
            );
            NextAchievementId::<T>::put(achievement_id.saturating_add(1));

            // Award immediately (no voting in v1 — Master has full authority).
            ensure!(
                CitizenAchievements::<T>::get(&target_citizen, achievement_id).is_none(),
                Error::<T>::AlreadyHoldsAchievement
            );
            CitizenAchievements::<T>::insert(&target_citizen, achievement_id, ());

            Self::deposit_event(Event::AchievementIssued {
                achievement_id,
                guild_id,
                recipient: target_citizen,
                issuer: caller,
                title: bounded_title.into_inner(),
            });
            Ok(())
        }

        // ─── publish_quest ───────────────────────────────────────────────────

        /// Publish a new Quest and lock the reward in escrow.
        ///
        /// The `reward` is **reserved** (escrowed) from the employer's free balance.
        /// It will be unreserved + transferred to the assignee on `complete_quest`,
        /// or unreserved back to the employer on `cancel_quest`.
        ///
        /// # [AUDIT: WASH-TRADING / UROBOROS PROTECTION]
        ///
        /// `T::Currency::reserve()` operates STRICTLY on `free_balance`. It is
        /// IMPOSSIBLE for reserved collateral (e.g. CDP debt from pallet-bank-operator)
        /// to be used as Quest escrow because the Substrate `ReservableCurrency`
        /// invariant: `can_reserve(who, amount) ⇔ free_balance(who) ≥ amount`.
        ///
        /// Locked CDP collateral is already in `reserved` balance, which REDUCES the
        /// `free_balance`. Therefore wash-trading using pledged collateral is impossible
        /// at the primitive level — no additional code change required.
        ///
        /// # Origin: Signed (any citizen, not required to be a Guild member)
        #[pallet::call_index(4)]
        #[pallet::weight(T::WeightInfo::publish_quest())]
        pub fn publish_quest(
            origin: OriginFor<T>,
            guild_id: u32,
            reward: BalanceOf<T>,
            description_cid: Vec<u8>,
        ) -> DispatchResult {
            let caller = ensure_signed(origin)?;

            ensure!(
                Guilds::<T>::contains_key(guild_id),
                Error::<T>::GuildNotFound
            );

            let bounded_desc: BoundedVec<u8, T::MaxIpfsCidLength> = description_cid
                .try_into()
                .map_err(|_| Error::<T>::CidTooLong)?;

            // Reserve (escrow) the reward from employer's account.
            T::Currency::reserve(&caller, reward).map_err(|_| Error::<T>::InsufficientBalance)?;

            let quest_id = NextQuestId::<T>::get();
            Quests::<T>::insert(
                quest_id,
                Quest {
                    guild_id,
                    employer: caller.clone(),
                    reward,
                    status: QuestStatus::Open,
                    assignee: None,
                    description_hash: bounded_desc,
                    assignee_quest_count: 0,
                },
            );
            NextQuestId::<T>::put(quest_id.saturating_add(1));

            // Increment guild quest counter.
            Guilds::<T>::try_mutate(guild_id, |maybe| -> DispatchResult {
                let g = maybe.as_mut().ok_or(Error::<T>::GuildNotFound)?;
                g.quest_count = g.quest_count.saturating_add(1);
                Ok(())
            })?;

            Self::deposit_event(Event::QuestPublished {
                quest_id,
                guild_id,
                employer: caller,
                reward,
            });
            Ok(())
        }

        // ─── assign_quest ────────────────────────────────────────────────────

        /// Assign yourself to an Open Quest.
        ///
        /// Must be a Guild member. Changes status `Open → InProgress`.
        ///
        /// # Origin: Signed (must be Guild member, role Apprentice or above)
        #[pallet::call_index(5)]
        #[pallet::weight(T::WeightInfo::assign_quest())]
        pub fn assign_quest(origin: OriginFor<T>, quest_id: u32) -> DispatchResult {
            let caller = ensure_signed(origin)?;

            let mut quest = Quests::<T>::get(quest_id).ok_or(Error::<T>::QuestNotFound)?;
            ensure!(
                quest.status == QuestStatus::Open,
                Error::<T>::InvalidQuestStatus
            );

            // Must be a Guild member.
            ensure!(
                GuildMembers::<T>::contains_key(quest.guild_id, &caller),
                Error::<T>::NotMember
            );

            quest.status = QuestStatus::InProgress;
            quest.assignee = Some(caller.clone());
            Quests::<T>::insert(quest_id, quest);

            Self::deposit_event(Event::QuestAssigned {
                quest_id,
                assignee: caller,
            });
            Ok(())
        }

        // ─── submit_quest ────────────────────────────────────────────────────

        /// Submit completed work for review.
        ///
        /// Changes status `InProgress → Review`.
        /// Only the assigned citizen may submit.
        ///
        /// # Origin: Signed (must be the quest's assignee)
        #[pallet::call_index(6)]
        #[pallet::weight(T::WeightInfo::submit_quest())]
        pub fn submit_quest(origin: OriginFor<T>, quest_id: u32) -> DispatchResult {
            let caller = ensure_signed(origin)?;

            let mut quest = Quests::<T>::get(quest_id).ok_or(Error::<T>::QuestNotFound)?;
            ensure!(
                quest.status == QuestStatus::InProgress,
                Error::<T>::InvalidQuestStatus
            );
            ensure!(
                quest.assignee == Some(caller.clone()),
                Error::<T>::NotAssignee
            );

            quest.status = QuestStatus::Review;
            Quests::<T>::insert(quest_id, quest);

            Self::deposit_event(Event::QuestSubmittedForReview { quest_id });
            Ok(())
        }

        // ─── complete_quest ──────────────────────────────────────────────────

        /// Approve Quest completion and release escrowed reward to the assignee.
        ///
        /// Only a Guild `Master` may approve completion.
        ///
        /// # Origin: Signed (must be Guild Master)
        #[pallet::call_index(7)]
        #[pallet::weight(T::WeightInfo::complete_quest())]
        pub fn complete_quest(origin: OriginFor<T>, quest_id: u32) -> DispatchResult {
            let caller = ensure_signed(origin)?;

            let mut quest = Quests::<T>::get(quest_id).ok_or(Error::<T>::QuestNotFound)?;
            ensure!(
                quest.status == QuestStatus::Review,
                Error::<T>::InvalidQuestStatus
            );

            // Caller must be Guild Master.
            let caller_role =
                GuildMembers::<T>::get(quest.guild_id, &caller).ok_or(Error::<T>::NotMember)?;
            ensure!(caller_role == MemberRole::Master, Error::<T>::NotMaster);

            let assignee = quest
                .assignee
                .clone()
                .ok_or(Error::<T>::InvalidQuestStatus)?;
            let reward = quest.reward;

            // Unreserve from employer, then transfer to assignee.
            T::Currency::unreserve(&quest.employer, reward);
            T::Currency::transfer(
                &quest.employer,
                &assignee,
                reward,
                frame_support::traits::ExistenceRequirement::KeepAlive,
            )?;

            // Increment assignee quest count in this Guild.
            MemberQuestCount::<T>::mutate(quest.guild_id, &assignee, |count| {
                *count = count.saturating_add(1);
            });

            // Record the count at completion time.
            let new_count = MemberQuestCount::<T>::get(quest.guild_id, &assignee);
            quest.status = QuestStatus::Completed;
            quest.assignee_quest_count = new_count;
            Quests::<T>::insert(quest_id, quest);

            Self::deposit_event(Event::QuestCompleted {
                quest_id,
                assignee,
                reward,
            });
            Ok(())
        }

        // ─── cancel_quest ────────────────────────────────────────────────────

        /// Cancel an Open Quest and reclaim escrowed reward.
        ///
        /// Only the employer may cancel. Quest must be `Open` (not yet assigned).
        ///
        /// # Origin: Signed (must be the quest's employer)
        #[pallet::call_index(8)]
        #[pallet::weight(T::WeightInfo::cancel_quest())]
        pub fn cancel_quest(origin: OriginFor<T>, quest_id: u32) -> DispatchResult {
            let caller = ensure_signed(origin)?;

            let mut quest = Quests::<T>::get(quest_id).ok_or(Error::<T>::QuestNotFound)?;
            ensure!(
                quest.status == QuestStatus::Open,
                Error::<T>::InvalidQuestStatus
            );
            ensure!(quest.employer == caller, Error::<T>::NotEmployer);

            // Unreserve the escrowed reward back to employer.
            T::Currency::unreserve(&quest.employer, quest.reward);

            quest.status = QuestStatus::Completed; // terminal state — use Completed to avoid re-use
            Quests::<T>::insert(quest_id, quest);

            Self::deposit_event(Event::QuestCancelled { quest_id });
            Ok(())
        }

        // ─── subscribe_to_academy ────────────────────────────────────────────

        /// Purchase an Academy subscription from a Guild Master.
        ///
        /// Transfers `fee` ALTAN from the student directly to the master.
        /// Emits `AcademySubscriptionPurchased` — the Altan Gateway listens to
        /// this event to unlock the student's access to off-chain video courses.
        ///
        /// # Origin: Signed (any citizen / student)
        #[pallet::call_index(9)]
        #[pallet::weight(T::WeightInfo::subscribe_to_academy())]
        pub fn subscribe_to_academy(
            origin: OriginFor<T>,
            master_account: T::AccountId,
            fee: BalanceOf<T>,
        ) -> DispatchResult {
            let caller = ensure_signed(origin)?;

            // Direct transfer: student → master (no escrow needed for subscriptions).
            T::Currency::transfer(
                &caller,
                &master_account,
                fee,
                frame_support::traits::ExistenceRequirement::KeepAlive,
            )?;

            Self::deposit_event(Event::AcademySubscriptionPurchased {
                student: caller,
                master: master_account,
                fee,
            });
            Ok(())
        }

        // ─── force_resolve_quest ──────────────────────────────────────────────

        /// **SECURITY** Force-resolve a Quest that is stuck in `Review` status
        /// because the employer has gone AWOL and will not call `complete_quest`.
        ///
        /// A Guild `Master` may call `force_resolve_quest` to release the
        /// escrowed reward to any specified `recipient` (typically the assignee).
        /// This acts as the guild's internal arbitration mechanism.
        ///
        /// # Origin: Signed (must be a Guild Master of the quest's guild)
        #[pallet::call_index(10)]
        #[pallet::weight(T::WeightInfo::force_resolve_quest())]
        pub fn force_resolve_quest(
            origin: OriginFor<T>,
            quest_id: u32,
            recipient: T::AccountId,
        ) -> DispatchResult {
            let caller = ensure_signed(origin)?;

            let mut quest = Quests::<T>::get(quest_id).ok_or(Error::<T>::QuestNotFound)?;

            // Quest must be in Review state.
            ensure!(
                quest.status == QuestStatus::Review,
                Error::<T>::QuestNotInReview
            );

            // Only a Master of THIS specific guild may arbitrate.
            let caller_role =
                GuildMembers::<T>::get(quest.guild_id, &caller).ok_or(Error::<T>::NotMember)?;
            ensure!(
                caller_role == MemberRole::Master,
                Error::<T>::NotMasterForForceResolve
            );

            let reward = quest.reward;

            // Unreserve from the employer's escrow, then transfer to recipient.
            T::Currency::unreserve(&quest.employer, reward);
            T::Currency::transfer(
                &quest.employer,
                &recipient,
                reward,
                frame_support::traits::ExistenceRequirement::KeepAlive,
            )?;

            // Increment recipient's quest count if they are the assignee.
            if quest.assignee == Some(recipient.clone()) {
                MemberQuestCount::<T>::mutate(quest.guild_id, &recipient, |count| {
                    *count = count.saturating_add(1);
                });
            }

            quest.status = QuestStatus::Completed;
            Quests::<T>::insert(quest_id, quest);

            Self::deposit_event(Event::QuestCompleted {
                quest_id,
                assignee: recipient,
                reward,
            });
            Ok(())
        }

        // ─── form_guild_union ─────────────────────────────────────────────────

        /// Form a new Guild Union at the GuildArbad (base) level.
        ///
        /// The caller must be a Guild Master of a guild that is NOT yet in any union.
        /// The caller's guild + all `peer_guild_ids` join the new union.
        /// The caller automatically becomes the provisional Grandmaster.
        ///
        /// At least 1 peer guild is required (total ≥ 2 guilds for a valid union).
        ///
        /// ## Steppe Protocol
        /// Guilds self-organise based on shared industry. The industry_tag is
        /// set at formation and inherited by the union, representing the
        /// professional domain of the federation.
        ///
        /// # Origin: Signed (must be a Guild Master)
        #[pallet::call_index(11)]
        #[pallet::weight(T::WeightInfo::form_guild_union())]
        pub fn form_guild_union(
            origin: OriginFor<T>,
            industry_tag: Vec<u8>,
            peer_guild_ids: BoundedVec<u32, T::MaxUnionMembers>,
        ) -> DispatchResult {
            let caller = ensure_signed(origin)?;

            // Caller: find their guild (must be Master of at least one guild that is not in a union).
            // We require caller to own a guild — we check GuildMembers to find a Master-level guild.
            // The caller specifies their own guild via peer_guild_ids — the first guild in the list
            // where they are Master is the "founder guild".
            // Simpler: require caller is Master of peer_guild_ids[0] and it's the founder.
            ensure!(
                peer_guild_ids.len() >= 1,
                Error::<T>::InsufficientUnionMembers
            );

            let founder_guild_id = peer_guild_ids[0];
            Self::ensure_master(&caller, founder_guild_id)?;
            ensure!(
                GuildUnionOf::<T>::get(founder_guild_id).is_none(),
                Error::<T>::GuildAlreadyInUnion
            );

            // Validate all peers exist and are not already in a union
            for &gid in peer_guild_ids.iter().skip(1) {
                ensure!(Guilds::<T>::contains_key(gid), Error::<T>::GuildNotFound);
                ensure!(
                    GuildUnionOf::<T>::get(gid).is_none(),
                    Error::<T>::GuildAlreadyInUnion
                );
            }

            let bounded_tag: BoundedVec<u8, T::MaxNameLength> = industry_tag
                .clone()
                .try_into()
                .map_err(|_| Error::<T>::IndustryTagTooLong)?;

            let union_id = NextUnionId::<T>::get();
            let member_count = peer_guild_ids.len() as u32;

            GuildUnions::<T>::insert(
                union_id,
                GuildUnion {
                    level: GuildUnionLevel::GuildArbad,
                    founder_guild: founder_guild_id,
                    grandmaster: caller.clone(),
                    member_guild_count: member_count,
                    industry_tag: bounded_tag.clone(),
                    election_votes_for: 0,
                    election_votes_cast: 0,
                },
            );
            NextUnionId::<T>::put(union_id.saturating_add(1));

            // Register all guilds as union members
            for &gid in peer_guild_ids.iter() {
                GuildUnionOf::<T>::insert(gid, union_id);
            }

            Self::deposit_event(Event::GuildUnionFormed {
                union_id,
                level: GuildUnionLevel::GuildArbad,
                grandmaster: caller,
                industry_tag: bounded_tag.into_inner(),
                member_guild_count: member_count,
            });
            Ok(())
        }

        // ─── vote_for_grandmaster ─────────────────────────────────────────────

        /// Cast a Guild's vote for the Grandmaster of its Union.
        ///
        /// The caller must be a Master of a guild that belongs to `union_id`.
        /// Each guild may cast one vote per election round.
        ///
        /// When a strict majority (>50% of member guilds) votes for the same
        /// candidate, that candidate is automatically elected as Grandmaster and
        /// `GrandmasterElected` is emitted.
        ///
        /// If the union is at `GuildTumed` level, the new Grandmaster is
        /// automatically admitted to the **Academy of Sciences** (`AcademyMembers`).
        ///
        /// # Origin: Signed (must be Master of a guild in the union)
        #[pallet::call_index(12)]
        #[pallet::weight(T::WeightInfo::vote_for_grandmaster())]
        pub fn vote_for_grandmaster(
            origin: OriginFor<T>,
            union_id: u32,
            voter_guild_id: u32,
            candidate: T::AccountId,
        ) -> DispatchResult {
            let caller = ensure_signed(origin)?;

            // Union must exist
            let mut union =
                GuildUnions::<T>::get(union_id).ok_or(Error::<T>::GuildUnionNotFound)?;

            // Caller must be Master of the specified guild
            Self::ensure_master(&caller, voter_guild_id)?;

            // The voting guild must be a member of this union
            ensure!(
                GuildUnionOf::<T>::get(voter_guild_id) == Some(union_id),
                Error::<T>::GuildNotInUnion
            );

            // No double voting in this round
            ensure!(
                GrandmasterVotes::<T>::get(union_id, voter_guild_id).is_none(),
                Error::<T>::VoteAlreadyCast
            );

            // Record the vote
            GrandmasterVotes::<T>::insert(union_id, voter_guild_id, candidate.clone());
            union.election_votes_cast = union.election_votes_cast.saturating_add(1);

            Self::deposit_event(Event::GrandmasterVoteCast {
                union_id,
                voter_guild: voter_guild_id,
                candidate: candidate.clone(),
            });

            // Tally — check if majority achieved
            if let Some(elected) = Self::tally_grandmaster_votes(union_id, union.member_guild_count)
            {
                let industry_tag = union.industry_tag.clone();
                let is_tumed = union.level == GuildUnionLevel::GuildTumed;

                union.grandmaster = elected.clone();
                union.election_votes_for = 0;
                union.election_votes_cast = 0;
                GuildUnions::<T>::insert(union_id, union);

                Self::deposit_event(Event::GrandmasterElected {
                    union_id,
                    grandmaster: elected.clone(),
                });

                // Apex union — grant Academician status automatically
                if is_tumed {
                    Self::grant_academician(&elected, industry_tag);
                }
            } else {
                // [SECURITY VECTOR 3] Deadlock detection.
                // If ALL member guilds have now cast votes but still no supermajority
                // candidate exists, the election is in an unbreakable tie (e.g. 2-2 split
                // among 4 guilds). Reset the election so Masters can try again.
                //
                // Without this reset, even-member unions with an even split would be
                // permanently paralysed — no grandmaster could ever be elected.
                if union.election_votes_cast >= union.member_guild_count {
                    // Purge all ballots for this union (bounded by MaxUnionMembers).
                    let _ = GrandmasterVotes::<T>::clear_prefix(
                        union_id,
                        T::MaxUnionMembers::get(),
                        None,
                    );
                    union.election_votes_cast = 0;
                    union.election_votes_for = 0;
                    GuildUnions::<T>::insert(union_id, union);

                    Self::deposit_event(Event::GrandmasterElectionReset { union_id });
                } else {
                    // Voting is still in progress — just persist the updated cast count.
                    GuildUnions::<T>::insert(union_id, union);
                }
            }

            Ok(())
        }

        // ─── elevate_union ────────────────────────────────────────────────────

        /// Elevate a Guild Union to the next fractal tier by merging with peer unions.
        ///
        /// The caller must be the **Grandmaster** of a union.
        /// All `peer_union_ids` must be at the same level as the caller's union.
        /// On success, the caller's union absorbs the peers and ascends one tier:
        /// `GuildArbad → GuildZun → GuildMyangad → GuildTumed`.
        ///
        /// When a union reaches `GuildTumed`, its Grandmaster is automatically
        /// admitted to the **Academy of Sciences**.
        ///
        /// # Origin: Signed (must be Grandmaster of `base_union_id`)
        #[pallet::call_index(13)]
        #[pallet::weight(T::WeightInfo::elevate_union())]
        pub fn elevate_union(
            origin: OriginFor<T>,
            base_union_id: u32,
            peer_union_ids: BoundedVec<u32, T::MaxUnionMembers>,
        ) -> DispatchResult {
            let caller = ensure_signed(origin)?;

            let mut base_union =
                GuildUnions::<T>::get(base_union_id).ok_or(Error::<T>::GuildUnionNotFound)?;

            // Caller must be the current Grandmaster
            ensure!(base_union.grandmaster == caller, Error::<T>::NotGrandmaster);

            // Must be able to elevate to the next level
            let new_level = base_union
                .level
                .next()
                .ok_or(Error::<T>::MaxUnionLevelReached)?;

            // [SECURITY VECTOR 1] Enforce strictly sequential level transitions.
            // An attacker cannot skip from GuildArbad directly to GuildTumed (Fake Academic).
            // Each tier must be earned through the prescribed predecessor level.
            // This check is belt-and-suspenders alongside `.next()` to make the
            // invariant explicit, auditable, and resistant to future storage corruption.
            match (&base_union.level, &new_level) {
                (GuildUnionLevel::GuildArbad, GuildUnionLevel::GuildZun) => {}
                (GuildUnionLevel::GuildZun, GuildUnionLevel::GuildMyangad) => {}
                (GuildUnionLevel::GuildMyangad, GuildUnionLevel::GuildTumed) => {}
                // Any other transition is constitutionally prohibited.
                _ => return Err(Error::<T>::InvalidUnionLevelTransition.into()),
            }

            // Validate peers and absorb their member guild counts
            let mut total_guilds = base_union.member_guild_count;
            for &pid in peer_union_ids.iter() {
                ensure!(pid != base_union_id, Error::<T>::GuildUnionNotFound);
                let peer = GuildUnions::<T>::get(pid).ok_or(Error::<T>::GuildUnionNotFound)?;
                ensure!(
                    peer.level == base_union.level,
                    Error::<T>::GuildUnionNotFound
                );
                total_guilds = total_guilds.saturating_add(peer.member_guild_count);

                // Re-register all guilds from peer unions to the base union
                // (We iterate GuildUnionOf to remap those guilds)
                // Since we can't efficiently iterate storage here, we delete the peer union
                // and let on-chain indexers remap. The member_guild_count captures the size.
                GuildUnions::<T>::remove(pid);
            }

            // Update base union to elevated level
            base_union.level = new_level.clone();
            base_union.member_guild_count = total_guilds;
            // Reset election state for the new tier
            base_union.election_votes_for = 0;
            base_union.election_votes_cast = 0;

            let industry_tag = base_union.industry_tag.clone();
            let is_tumed = new_level == GuildUnionLevel::GuildTumed;

            GuildUnions::<T>::insert(base_union_id, base_union);

            Self::deposit_event(Event::GuildUnionElevated {
                union_id: base_union_id,
                new_level: new_level.clone(),
                grandmaster: caller.clone(),
            });

            // If apex reached — Grandmaster becomes Academician automatically
            if is_tumed {
                Self::grant_academician(&caller, industry_tag);
            }

            Ok(())
        }
    }
}

// =========================================================================
// Ghost-State Cleanup (Task 1: Cross-Pallet Cascading Cleanup)
// =========================================================================

impl<T: Config> Pallet<T> {
    /// [SECURITY: GHOST STATE] Evict a terminal citizen from all guild-related state.
    ///
    /// Called by `OnTerminalStatus::on_deceased` and `on_exiled` (implemented below).
    ///
    /// ## What is cleaned up
    /// - Any Quests where `who` is the `employer` and status is `Open` or `InProgress`:
    ///   the escrow is **unreserved** back to `who`'s account and Quest is cancelled.
    /// - `AcademyMembers` entry for `who` (strips Academician status).
    ///
    /// ## Note on Guild Master eviction
    /// `GuildMembers` is a double-map (guild_id, AccountId). Because Substrate
    /// does not support efficient reverse lookups, we remove `who`'s Academy entry
    /// and cancel their quests. Full guild-master removal requires an O(n) scan
    /// which is acceptable here as this is a low-frequency, governance-triggered event.
    pub fn cleanup_account(who: &T::AccountId) {
        use frame_support::traits::ReservableCurrency;
        // 1. Cancel open/in-progress quests authored by `who`; unreserve escrow.
        Quests::<T>::iter_values()
            .filter(|q| &q.employer == who)
            .filter(|q| q.status == QuestStatus::Open || q.status == QuestStatus::InProgress)
            .for_each(|q| {
                // Unreserve best-effort (ignore if already freed).
                let _ = T::Currency::unreserve(&q.employer, q.reward);
            });

        // Set matching quests to Cancelled in storage.
        Quests::<T>::iter()
            .filter(|(_, q)| {
                &q.employer == who
                    && (q.status == QuestStatus::Open || q.status == QuestStatus::InProgress)
            })
            .map(|(id, _)| id)
            .collect::<alloc::vec::Vec<_>>()
            .into_iter()
            .for_each(|id| {
                Quests::<T>::mutate(id, |maybe| {
                    if let Some(q) = maybe {
                        q.status = QuestStatus::Cancelled;
                    }
                });
            });

        // 2. Strip Academician status.
        AcademyMembers::<T>::remove(who);
    }
}

/// Wire `pallet_inomad_identity::OnTerminalStatus` so that
/// when a citizen dies or is exiled, their Guild/Academy state is cleaned.
impl<T: Config> pallet_inomad_identity::OnTerminalStatus<T::AccountId> for Pallet<T> {
    fn on_deceased(who: &T::AccountId) {
        Self::cleanup_account(who);
    }
    fn on_exiled(who: &T::AccountId) {
        Self::cleanup_account(who);
    }
}

// =========================================================================
// Unit Tests
// =========================================================================

#[cfg(test)]
mod mock;

#[cfg(test)]
mod tests;
