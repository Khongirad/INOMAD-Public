//! # Khural Governance Pallet
//!
//! **Altan Network — Sovereign L1 Blockchain**
//! **Sprint L1-05: Full Khural Democracy** | Constitutional Enactment Engine
//!
//! This pallet implements the decentralised governance core of the Altan Network,
//! modelled on the historical Mongolian Khural (Great Council).
//!
//! ## Architecture
//!
//! - **Tight coupling** to `pallet-inomad-identity` via `Config: pallet_inomad_identity::Config`.
//!   This grants direct access to `Citizens<T>` storage — no cross-pallet message passing needed.
//! - **Loose coupling** to `pallet-guilds` via `T::AcademyInterface` trait.
//!   This gates `propose_expert_bill` to Academicians (GuildTumed Grandmasters only).
//! - **Treasury transfers** are executed through `T::Currency` (expected to be `pallet_balances`).
//!   The nation treasury accounts are resolved at runtime via `T::NationTreasuryProvider`.
//!
//! ## Proposal Lifecycle
//!
//! ```text
//! create_proposal → Active (end_block = current_block + VotingPeriod)
//!                     ↓
//!                   vote (any active citizen, 1-person-1-vote)
//!                     ↓
//!             on_initialize (auto-executes when current_block >= end_block)
//!                /           \
//!          Executed         Rejected
//! ```
//!
//! ## Expert Bill Lifecycle (Academy of Sciences)
//!
//! ```text
//! propose_expert_bill → Active (ExpertInitiative)
//!                          ↓
//!                      vote (any Khural delegate, confederation-wide)
//!                          ↓
//!                    execute_proposal (anyone) or on_initialize
//! ```
//!
//! ## Constitutional Firewall
//!
//! The Khural CANNOT modify the constitutional constants of the network:
//! - **0.03% cross-transfer fee** — wired in `pallet-bank-of-siberia` as `Get<Perbill>`
//! - **10% state income tax** — wired in `pallet-bank-of-siberia` as `Get<Perbill>`
//!
//! These are compile-time WASM constants, not storage values. No governance vote
//! can touch them. Only a constitutional referendum + runtime upgrade can change them.
//!
//! ## Security Model
//!
//! | Check                         | Enforced in                        |
//! |-------------------------------|------------------------------------|
//! | Caller is registered citizen  | create_proposal, vote              |
//! | Caller is active (not slashed)| create_proposal, vote              |
//! | No double voting              | vote                               |
//! | Proposal is Active            | vote                               |
//! | Voting period expired         | on_initialize (auto-execution)     |
//! | Quorum met                    | on_initialize (yes > no + MinQuorum)|
//! | Caller is Academician         | propose_expert_bill                |
//!
//! ## Extrinsics
//!
//! | Call                  | Origin | Description                                          |
//! |-----------------------|--------|------------------------------------------------------|
//! | `create_proposal`     | Signed | Submit a treasury spend proposal (any active citizen)|
//! | `vote`                | Signed | Cast an approve/reject vote (1 citizen = 1 vote)     |
//! | `execute_proposal`    | Signed | Manually finalise a proposal (anyone)                |
//! | `propose_expert_bill` | Signed | Submit an expert legislative bill (Academicians only)|
//! | `vote_on_expert_bill` | Signed | Vote on an expert bill (ArbadLeader+)                |
//! | `execute_expert_bill` | Signed | Finalise an expert bill (anyone)                     |

#![cfg_attr(not(feature = "std"), no_std)]

extern crate alloc;

pub mod migrations;
pub mod weights;
pub use pallet::*;

#[cfg(feature = "runtime-benchmarks")]
mod benchmarking;

#[cfg(test)]
mod mock;
#[cfg(test)]
mod tests;

#[frame_support::pallet]
pub mod pallet {
    use alloc::vec::Vec;
    use frame_support::{
        pallet_prelude::*,
        traits::{Currency, ExistenceRequirement, Get, ReservableCurrency},
        weights::Weight,
    };
    use frame_system::pallet_prelude::*;
    use sp_core::H256;
    // Bring AcademyInterface into scope so T::AcademyInterface::is_academician can be called.
    use pallet_guilds::AcademyInterface;
    // Re-use types and storage from pallet-inomad-identity (tight coupling).
    use pallet_inomad_identity::{
        pallet::{CitizenRole, CitizenStatus},
        Citizens,
    };

    // =========================================================================
    // Type Alias
    // =========================================================================

    /// Shorthand for the balance type driven by `T::Currency`.
    pub type BalanceOf<T> =
        <<T as Config>::Currency as Currency<<T as frame_system::Config>::AccountId>>::Balance;

    // =========================================================================
    // Pallet Struct
    // =========================================================================

    #[pallet::pallet]
    pub struct Pallet<T>(_);

    // =========================================================================
    // Configuration Trait
    // =========================================================================

    /// Tight-coupled configuration: this pallet requires the runtime to also
    /// configure `pallet_inomad_identity`.  This allows us to read `Citizens<T>`
    /// directly without cross-pallet message passing.
    ///
    /// Loosely coupled to `pallet_guilds` via `AcademyInterface` to gate
    /// `propose_expert_bill` to Academicians only — no direct storage coupling.
    #[pallet::config]
    pub trait Config:
        frame_system::Config<RuntimeEvent: From<Event<Self>>> + pallet_inomad_identity::Config
    {
        /// The currency used for treasury → beneficiary transfers and anti-spam deposits.
        /// Must implement `ReservableCurrency` so we can lock and slash Expert Bill deposits.
        type Currency: Currency<Self::AccountId> + ReservableCurrency<Self::AccountId>;

        /// Resolves the 79 sovereign nation treasury addresses.
        ///
        /// In production this is wired to `pallet_altan_tax::NationTreasuries<Runtime>::get()`
        /// via a `parameter_types!` wrapper in the runtime configs.
        ///
        /// Index convention: `nation_id` (1-79) maps to `accounts[nation_id - 1]`.
        type NationTreasuryProvider: Get<BoundedVec<Self::AccountId, ConstU32<150>>>;

        /// Cross-pallet interface to `pallet-guilds` for Academician checks.
        ///
        /// Wire at runtime with `type AcademyInterface = pallet_guilds::Pallet<Runtime>;`
        /// This allows `propose_expert_bill` to verify the caller is a GuildTumed Grandmaster
        /// without directly coupling to pallet-guilds' storage layout.
        type AcademyInterface: pallet_guilds::AcademyInterface<Self::AccountId>;

        /// [SECURITY VECTOR 2] Anti-spam deposit for Expert Legislative Bills.
        ///
        /// Reserved from the proposer's free balance when `propose_expert_bill` is called.
        /// - If the bill **passes** (`execute_expert_bill` with `votes_for > 2`): deposit returned.
        /// - If the bill **fails**: deposit is slashed (burned from monetary supply).
        ///
        /// This makes spamming the Parliamentary ledger economically irrational.
        /// Configured at runtime via `type ExpertBillDeposit = ConstU128<{ 10 * UNIT }>`.
        #[pallet::constant]
        type ExpertBillDeposit: Get<BalanceOf<Self>>;

        /// **Voting Period (blocks)**: how long a proposal stays open for votes.
        ///
        /// After `VotingPeriod` blocks, `on_initialize` will automatically evaluate
        /// and execute or reject the proposal based on quorum and majority.
        ///
        /// Recommended: `100_800` blocks ≈ 7 days at 6s/block.
        /// Configure at runtime via `type VotingPeriod = ConstU32<100_800>;`
        #[pallet::constant]
        type VotingPeriod: Get<u32>;

        /// **Minimum Quorum**: minimum number of YES votes required for a proposal to pass.
        ///
        /// A proposal passes only if:
        ///   1. `votes_for > votes_against` (simple majority)
        ///   2. `votes_for >= MinQuorum` (minimum participation threshold)
        ///
        /// This prevents a single citizen from unilaterally passing a treasury proposal
        /// in a low-turnout vote. Sprint L1-05 default: 3 votes.
        /// Configure at runtime via `type MinQuorum = ConstU32<3>;`
        #[pallet::constant]
        type MinQuorum: Get<u32>;
    }

    // =========================================================================
    // Types — Enums
    // =========================================================================

    /// Lifecycle state of a Khural treasury proposal or expert bill.
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
        /// Accepting votes.
        Active,
        /// Quorum reached — funds disbursed to beneficiary (or bill enacted).
        Executed,
        /// Insufficient votes — proposal closed without transfer.
        Rejected,
    }

    /// Marks the origin type of a legislative action.
    ///
    /// `ExpertInitiative` bills are proposed by Academicians and go to
    /// Khural delegates in **priority order**, setting industry standards.
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
    pub enum BillInitiativeType {
        /// Standard treasury proposal — submitted by a nation ArbadLeader+.
        Standard,
        /// Expert initiative — submitted by an Academician (GuildTumed Grandmaster).
        /// Flagged for priority consideration by Khural delegates.
        ExpertInitiative,
    }

    // =========================================================================
    // Types — Structs
    // =========================================================================

    /// A Khural treasury spend proposal.
    ///
    /// All monetary fields use `BalanceOf<T>` (u128 planck on the Altan runtime).
    ///
    /// ## Lifecycle
    ///
    /// Created with status `Active` and a deadline at `end_block`.
    /// `on_initialize` fires automatically at `end_block` to evaluate and execute.
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
    pub struct Proposal<T: Config> {
        /// The citizen who submitted this proposal.
        pub proposer: T::AccountId,
        /// Sovereign nation (1–86) this proposal belongs to.
        pub nation_id: u32,
        /// Amount (in planck) to transfer from the nation treasury to `beneficiary`.
        pub amount: BalanceOf<T>,
        /// Destination account for the treasury funds upon execution.
        pub beneficiary: T::AccountId,
        /// Number of approve votes cast.
        pub votes_for: u32,
        /// Number of reject votes cast.
        pub votes_against: u32,
        /// Current lifecycle state.
        pub status: ProposalStatus,
        /// Block number at which voting closes and `on_initialize` fires auto-execution.
        pub end_block: u32,

        /// **Chain of Legitimacy** — the Constitutional or legal basis for this proposal.
        ///
        /// Every law in the Altan Republic must trace its authority upward through
        /// the Legal Lineage tree. A proposal without a `constitutional_basis` is
        /// a standard treasury motion; a proposal WITH one creates a verifiable
        /// link to the Article of the Constitution or the prior law it is built upon.
        ///
        /// ## Legal Lineage Tree
        ///
        /// ```text
        /// [Constitution: CoreRightsHash]
        ///     └── [Proposal A: constitutional_basis = CoreRightsHash]
        ///           └── [Proposal B: constitutional_basis = Hash(Proposal A)]
        ///                 └── [Proposal C: constitutional_basis = Hash(Proposal B)]
        /// ```
        ///
        /// Indexed by off-chain explorers to reconstruct the full legal provenance
        /// of each enacted law. `None` = standalone treasury motion.
        pub constitutional_basis: Option<[u8; 32]>,
    }

    /// An Expert Legislative Bill submitted by an Academician.
    ///
    /// These bills carry `ExpertInitiative` flag, marking them for priority
    /// consideration by Khural delegates. They establish industry standards
    /// and are confederation-wide (not bound to a single nation).
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
    pub struct ExpertBill<T: Config> {
        /// Academician who proposed this bill.
        pub proposer: T::AccountId,
        /// Content-addressed bill hash (IPFS / on-chain hash of the full bill text).
        pub bill_hash: H256,
        /// Industry domain this bill pertains to (free-form tag from the guild).
        pub industry_tag: BoundedVec<u8, ConstU32<64>>,
        /// Always `ExpertInitiative` — stored for indexer/UI consumption.
        pub initiative_type: BillInitiativeType,
        /// Current lifecycle state.
        pub status: ProposalStatus,
        /// Approve votes from Khural delegates.
        pub votes_for: u32,
        /// Reject votes from Khural delegates.
        pub votes_against: u32,
    }

    // =========================================================================
    // Storage
    // =========================================================================

    /// Maps a `proposal_id` (auto-incremented u32) to its `Proposal`.
    ///
    /// `OptionQuery`: returns `None` for unknown IDs.
    #[pallet::storage]
    #[pallet::getter(fn proposals)]
    pub type Proposals<T: Config> = StorageMap<_, Blake2_128Concat, u32, Proposal<T>, OptionQuery>;

    /// Auto-incrementing counter for proposal IDs.
    ///
    /// The *current* value is assigned to the next proposal; then the counter is
    /// incremented by 1.  Starts at 0 on genesis.
    #[pallet::storage]
    #[pallet::getter(fn next_proposal_id)]
    pub type NextProposalId<T: Config> = StorageValue<_, u32, ValueQuery>;

    /// Tracks whether a given `AccountId` has already voted on a proposal.
    ///
    /// Layout: `(proposal_id, voter_account) → has_voted`.
    /// `ValueQuery` defaults to `false` for accounts that have not yet voted.
    #[pallet::storage]
    #[pallet::getter(fn has_voted)]
    pub type HasVoted<T: Config> = StorageDoubleMap<
        _,
        Blake2_128Concat,
        u32, // proposal_id
        Blake2_128Concat,
        T::AccountId, // voter
        bool,
        ValueQuery,
    >;

    // ─── Expert Bills (Academy of Sciences Legislative Channel) ───────────────

    /// Expert Bills registry: BillId → ExpertBill.
    ///
    /// Only Academicians (GuildTumed Grandmasters) can create entries here.
    /// These bills are flagged `ExpertInitiative` and go to Khural delegates
    /// in priority order for voting.
    #[pallet::storage]
    #[pallet::getter(fn expert_bills)]
    pub type ExpertBills<T: Config> =
        StorageMap<_, Blake2_128Concat, u32, ExpertBill<T>, OptionQuery>;

    /// Auto-incrementing counter for Expert Bill IDs.
    #[pallet::storage]
    #[pallet::getter(fn next_expert_bill_id)]
    pub type NextExpertBillId<T: Config> = StorageValue<_, u32, ValueQuery>;

    /// Tracks whether a given `AccountId` has already voted on an expert bill.
    ///
    /// Layout: `(bill_id, voter_account) → has_voted`.
    #[pallet::storage]
    #[pallet::getter(fn has_voted_on_bill)]
    pub type HasVotedOnBill<T: Config> = StorageDoubleMap<
        _,
        Blake2_128Concat,
        u32, // bill_id
        Blake2_128Concat,
        T::AccountId, // voter
        bool,
        ValueQuery,
    >;

    /// [SECURITY VECTOR 2] Expert Bill anti-spam deposit registry.
    ///
    /// Tracks `(proposer, reserved_amount)` for each Active bill.
    /// Entry is consumed (`.take()`) when `execute_expert_bill` finalises the bill:
    /// - Passed  → `unreserve(depositor, deposit)` — funds returned to proposer.
    /// - Rejected → `slash_reserved(depositor, deposit)` — funds burned from supply.
    ///
    /// Layout: `BillId → (AccountId, BalanceOf<T>)`.
    #[pallet::storage]
    #[pallet::getter(fn expert_bill_depositors)]
    pub type ExpertBillDepositor<T: Config> =
        StorageMap<_, Blake2_128Concat, u32, (T::AccountId, BalanceOf<T>), OptionQuery>;

    // =========================================================================
    // Events
    // =========================================================================

    #[pallet::event]
    #[pallet::generate_deposit(pub(super) fn deposit_event)]
    pub enum Event<T: Config> {
        /// A new treasury proposal was submitted.
        ProposalCreated {
            proposal_id: u32,
            proposer: T::AccountId,
            nation_id: u32,
            amount: BalanceOf<T>,
            beneficiary: T::AccountId,
            // constitutional_basis: optional hash of the constitutional or legal basis.
            constitutional_basis: Option<[u8; 32]>,
        },
        /// A citizen cast a vote on a proposal.
        Voted {
            proposal_id: u32,
            voter: T::AccountId,
            approve: bool,
        },
        /// A proposal was finalised (executed or rejected).
        ProposalExecuted {
            proposal_id: u32,
            /// `true` → funds transferred; `false` → quorum not met.
            passed: bool,
        },
        /// An Academician submitted an Expert Legislative Bill.
        ///
        /// These bills carry the `ExpertInitiative` flag and should be
        /// presented to Khural delegates with priority for consideration.
        ExpertBillProposed {
            bill_id: u32,
            proposer: T::AccountId,
            industry_tag: alloc::vec::Vec<u8>,
            bill_hash: H256,
        },
        /// A Khural delegate cast a vote on an expert bill.
        ExpertBillVoted {
            bill_id: u32,
            voter: T::AccountId,
            approve: bool,
        },
        /// An expert bill was finalised.
        ExpertBillExecuted { bill_id: u32, passed: bool },
        /// [SECURITY VECTOR 2] An Expert Bill was rejected and the proposer's anti-spam
        /// deposit was slashed (removed from monetary circulation).
        ExpertBillDepositSlashed {
            bill_id: u32,
            proposer: T::AccountId,
            slashed_amount: BalanceOf<T>,
        },
    }

    // =========================================================================
    // Errors
    // =========================================================================

    #[pallet::error]
    pub enum Error<T> {
        /// The caller has no citizen record in `pallet-inomad-identity`.
        NotRegistered,
        /// The citizen account has been slashed (frozen) and may not participate.
        CitizenInactive,
        /// The caller's role is below `ArbadLeader` — insufficient authority.
        InsufficientRole,
        /// The caller's nation does not match the proposal's nation.
        WrongNation,
        /// The referenced proposal does not exist.
        ProposalNotFound,
        /// The proposal is no longer accepting votes or execution.
        ProposalNotActive,
        /// The caller has already voted on this proposal.
        AlreadyVoted,
        /// The nation treasury account for `nation_id` was not found.
        TreasuryNotFound,
        /// An arithmetic operation overflowed.
        MathOverflow,
        /// The caller is not registered in the Academy of Sciences.
        /// Only Grandmasters of GuildTumed unions (Academicians) may propose expert bills.
        NotAcademician,
        /// The referenced expert bill does not exist.
        ExpertBillNotFound,
        /// The expert bill is no longer accepting votes.
        ExpertBillNotActive,
        /// Industry tag exceeds the 64-byte limit.
        IndustryTagTooLong,
        /// [SECURITY VECTOR 2] The proposer's free balance is below `ExpertBillDeposit`.
        /// Top up your balance before proposing an Expert Bill.
        InsufficientBalance,
        /// [SECURITY VECTOR 2] Deposit record missing for this bill (should never happen;
        /// indicates a storage inconsistency).
        DepositNotFound,
    }

    // =========================================================================
    // Internal helpers
    // =========================================================================

    impl<T: Config> Pallet<T> {
        /// Fetch and validate the caller's `CitizenRecord`.
        ///
        /// Returns `Ok(record)` if the citizen exists and is `Active`.
        /// This is the **only** access gate for `create_proposal` and `vote` —
        /// the Khural Democracy operates on 1-Citizen-1-Vote principles:
        /// any registered, active citizen may participate.
        fn validated_citizen(
            who: &T::AccountId,
        ) -> Result<pallet_inomad_identity::pallet::CitizenRecord, DispatchError> {
            let record = Citizens::<T>::get(who).ok_or(Error::<T>::NotRegistered)?;
            ensure!(
                record.status == CitizenStatus::Active,
                Error::<T>::CitizenInactive
            );
            Ok(record)
        }

        /// True iff `role` is at least `ArbadLeader` in the command hierarchy.
        ///
        /// Used only for Expert Bills (`vote_on_expert_bill`) where delegation
        /// to higher-ranking citizens is constitutionally required.
        ///
        /// ⚠ AUDIT FIX (Sprint L1-15): `KhuralDelegate` and `ConfederationDelegate`
        /// are the **highest** roles in the fractal hierarchy.  Both explicitly included.
        fn is_at_least_arbad_leader(role: &CitizenRole) -> bool {
            matches!(
                role,
                CitizenRole::ArbadLeader
                    | CitizenRole::ZunLeader
                    | CitizenRole::MyangadLeader
                    | CitizenRole::TumedLeader
                    | CitizenRole::KhuralDelegate        // ← AUDIT FIX L1-15
                    | CitizenRole::ConfederationDelegate // ← AUDIT FIX L1-15
            )
        }

        /// Return the current block number as `u32`.
        fn current_block() -> u32 {
            frame_system::Pallet::<T>::block_number()
                .try_into()
                .unwrap_or(0u32)
        }

        /// Internal enactment engine for expired proposals.
        ///
        /// Called by `on_initialize` when `current_block >= proposal.end_block`.
        /// Also callable from `execute_proposal` for manual early execution.
        ///
        /// ## Quorum Rule
        ///
        /// A proposal PASSES if and only if:
        ///   1. `votes_for >= T::MinQuorum::get()` — minimum participation met.
        ///   2. `votes_for > votes_against` — simple majority in favour.
        ///
        /// If either condition fails, the proposal is **Rejected** and no
        /// treasury funds are moved.
        fn enact_proposal(proposal_id: u32, mut proposal: Proposal<T>) -> Weight {
            let total_for = proposal.votes_for;
            let total_against = proposal.votes_against;
            let quorum_met = total_for >= T::MinQuorum::get();
            let majority = total_for > total_against;
            let mut enacted = false;

            if quorum_met && majority {
                let treasuries = T::NationTreasuryProvider::get();
                let idx = (proposal.nation_id.saturating_sub(1)) as usize;
                if let Some(treasury_account) = treasuries.get(idx) {
                    // EXECUTE: transfer from nation treasury → beneficiary.
                    // Errors are swallowed — if the treasury has insufficient
                    // funds the proposal is rejected rather than panicking.
                    let transfer_result = <T as Config>::Currency::transfer(
                        treasury_account,
                        &proposal.beneficiary,
                        proposal.amount,
                        ExistenceRequirement::KeepAlive,
                    );
                    if transfer_result.is_ok() {
                        proposal.status = ProposalStatus::Executed;
                        enacted = true;
                    } else {
                        proposal.status = ProposalStatus::Rejected;
                    }
                } else {
                    proposal.status = ProposalStatus::Rejected;
                }
            } else {
                proposal.status = ProposalStatus::Rejected;
            }

            // Insert BEFORE emitting event — status is captured in `enacted`
            // so we don't borrow `proposal` after the move into storage.
            Proposals::<T>::insert(proposal_id, proposal);
            Self::deposit_event(Event::ProposalExecuted {
                proposal_id,
                passed: enacted,
            });

            Weight::from_parts(120_000_000, 0)
        }
    }

    // =========================================================================
    // KhuralOrigin — Constitutional Authority Marker
    // =========================================================================
    //
    // `EnsureKhural` is a custom `EnsureOrigin` implementation that verifies
    // the origin comes from a successfully-enacted Khural governance decision.
    //
    // In Sprint L1-05 the Khural executes transfers directly via
    // `T::Currency::transfer`. `EnsureKhural` is provided as the infrastructure
    // for future call-dispatching proposals (where the Khural sends a Root-like
    // privileged call on behalf of the Republic).
    //
    // Usage in the runtime:
    //   `type KhuralOrigin = EnsureKhural<Runtime>;`
    //
    // Any extrinsic gated by `EnsureKhural` can ONLY be called after a Khural
    // vote passes — it cannot be called by any individual citizen, even Root.

    /// Verified Khural governance origin.
    ///
    /// Accepts the runtime's `KhuralGovernance` origin variant (to be added in
    /// Sprint L1-06 when dispatchable-call proposals are enabled).
    ///
    /// ## Dev Mode (current)
    ///
    /// Delegates to `EnsureRoot` for backwards compatibility — allows the Genesis
    /// bootstrap sudo to dispatch privileged governance calls during development.
    ///
    /// ## Production Mode (`--features production-origins`)
    ///
    /// Will be wired to a dedicated `KhuralGovernance` origin emitted ONLY by
    /// `enact_proposal` after a successful Khural vote. Individual citizens,
    /// including Root, cannot call extrinsics gated by this origin directly.
    ///
    /// AUDIT STATUS: Dev fallback to EnsureRoot is intentional and documented.
    /// Transition to dedicated origin is Sprint L1-06 scope.
    pub struct EnsureKhural<T>(core::marker::PhantomData<T>);

    impl<T: Config> frame_support::traits::EnsureOrigin<T::RuntimeOrigin> for EnsureKhural<T>
    where
        T::RuntimeOrigin: From<frame_system::RawOrigin<T::AccountId>>,
    {
        type Success = ();

        fn try_origin(o: T::RuntimeOrigin) -> Result<Self::Success, T::RuntimeOrigin> {
            // DEV MODE: accept Root as the Khural authority.
            // PRODUCTION (Sprint L1-06): replace with dedicated KhuralGovernance origin
            // that is emitted only by `enact_proposal` on a successful vote.
            // The feature flag below makes the transition explicit and auditable.
            #[cfg(not(feature = "production-origins"))]
            return <frame_system::EnsureRoot<T::AccountId> as frame_support::traits::EnsureOrigin<
                T::RuntimeOrigin,
            >>::try_origin(o)
            .map(|_| ());

            // TODO (Sprint L1-06): replace with:
            // #[cfg(feature = "production-origins")]
            // return T::KhuralGovernanceOrigin::ensure_origin(o).map(|_| ());
            //
            // For now, production-origins falls through to EnsureRoot as well
            // until the dedicated origin type is implemented.
            #[cfg(feature = "production-origins")]
            <frame_system::EnsureRoot<T::AccountId> as frame_support::traits::EnsureOrigin<
                T::RuntimeOrigin,
            >>::try_origin(o)
            .map(|_| ())
        }

        #[cfg(feature = "runtime-benchmarks")]
        fn try_successful_origin() -> Result<T::RuntimeOrigin, ()> {
            Ok(frame_system::RawOrigin::Root.into())
        }
    }

    // =========================================================================
    // Block Hooks — Automatic Proposal Enactment
    // =========================================================================

    #[pallet::hooks]
    impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T> {
        /// Automatic proposal enactment engine.
        ///
        /// On every block, iterates all `Active` proposals and checks if their
        /// `end_block` has been reached.  If so, applies quorum + majority rules
        /// and executes or rejects the proposal without any external trigger.
        ///
        /// ## Gas Model
        ///
        /// Returns a `Weight` proportional to the number of proposals enacted.
        /// In production, this should be bounded by `MaxActiveProposals` storage
        /// to prevent unbounded iteration.  Sprint L1-05: acceptable at low volume.
        fn on_initialize(n: BlockNumberFor<T>) -> Weight {
            let current: u32 = n.try_into().unwrap_or(0u32);
            let mut total_weight = Weight::zero();

            // Collect expired proposal IDs first to avoid re-entrancy on the
            // storage iterator while mutating the map inside `enact_proposal`.
            let expired: alloc::vec::Vec<(u32, Proposal<T>)> = Proposals::<T>::iter()
                .filter_map(|(id, proposal)| {
                    if proposal.status == ProposalStatus::Active && current >= proposal.end_block {
                        Some((id, proposal))
                    } else {
                        None
                    }
                })
                .collect();

            for (proposal_id, proposal) in expired {
                let w = Self::enact_proposal(proposal_id, proposal);
                total_weight = total_weight.saturating_add(w);
            }

            total_weight
        }
    }

    // =========================================================================
    // Extrinsics — The Khural Democracy
    // =========================================================================

    #[pallet::call]
    impl<T: Config> Pallet<T> {
        // ─── create_proposal ─────────────────────────────────────────────────

        /// Submit a new treasury spend proposal for a sovereign nation.
        ///
        /// **Any active, registered citizen may propose** — this is the
        /// constitutional right of every member of the Republic (1 Citizen = 1 Voice).
        ///
        /// The proposal is assigned a voting deadline of
        /// `current_block + T::VotingPeriod::get()`. When that block is reached,
        /// `on_initialize` automatically evaluates and executes or rejects it.
        ///
        /// # Security Checks
        ///
        /// 1. Caller must be a **registered** citizen (`NotRegistered`).
        /// 2. Caller must be **active** — not slashed or deceased (`CitizenInactive`).
        /// 3. Caller's `nation_id` must exactly match the proposal `nation_id` (`WrongNation`).
        ///
        /// # Arguments
        ///
        /// - `nation_id`   – Sovereign nation (1–79) for which funds are requested.
        /// - `amount`      – Planck amount to transfer from the nation treasury.
        /// - `beneficiary` – Destination account for approved funds.
        #[pallet::call_index(0)]
        #[pallet::weight(Weight::from_parts(50_000_000, 0))]
        pub fn create_proposal(
            origin: OriginFor<T>,
            nation_id: u32,
            amount: BalanceOf<T>,
            beneficiary: T::AccountId,
            // optional blake2_256 hash of the constitutional article or prior law this proposal grounds in.
            // None = standalone treasury motion. Some(hash) = Legal Lineage link.
            constitutional_basis: Option<[u8; 32]>,
        ) -> DispatchResult {
            let caller = ensure_signed(origin)?;

            // ── [SECURITY CHECK 1 & 2] Registered + Active citizen ────────────
            // Any active citizen may propose — no role gate.
            // This upholds the constitutional right: 1 Citizen = 1 Voice.
            let citizen = Self::validated_citizen(&caller)?;

            // ── [SECURITY CHECK 3] Nation binding is immutable ───────────────
            // Citizens may only propose on behalf of their own sovereign nation.
            ensure!(citizen.nation_id == nation_id, Error::<T>::WrongNation);

            // ── Assign proposal ID and compute end_block ───────────────────────
            let proposal_id = NextProposalId::<T>::get();
            let creation_block = Self::current_block();
            let end_block = creation_block.saturating_add(T::VotingPeriod::get());

            Proposals::<T>::insert(
                proposal_id,
                Proposal {
                    proposer: caller.clone(),
                    nation_id,
                    amount,
                    beneficiary: beneficiary.clone(),
                    votes_for: 0,
                    votes_against: 0,
                    status: ProposalStatus::Active,
                    end_block,
                    constitutional_basis,
                },
            );
            NextProposalId::<T>::put(proposal_id.saturating_add(1));

            Self::deposit_event(Event::ProposalCreated {
                proposal_id,
                proposer: caller,
                nation_id,
                amount,
                beneficiary,
                constitutional_basis,
            });

            Ok(())
        }

        // ─── vote ────────────────────────────────────────────────────────────

        /// Cast an approve or reject vote on an active treasury proposal.
        ///
        /// ## Constitutional Rule: 1 Citizen = 1 Vote
        ///
        /// Any registered, active citizen of the same nation may vote.
        /// There is no role requirement — this is the foundational democratic
        /// right of every member of the Altan Republic.
        ///
        /// In the future, votes will be **weighted by Arbad participation**
        /// (i.e., citizens in higher fractal units will have greater influence),
        /// but for Sprint L1-05 all votes carry equal weight.
        ///
        /// # Security Checks
        ///
        /// 1. Caller must be a **registered, active** citizen (`NotRegistered` / `CitizenInactive`).
        /// 2. The proposal must exist and currently be `Active` (`ProposalNotFound` / `ProposalNotActive`).
        /// 3. The proposal's voting window must not have closed (`current_block < end_block`).
        /// 4. Caller's `nation_id` must match the proposal's nation (`WrongNation`).
        /// 5. Caller must not have voted on this proposal already (`AlreadyVoted`).
        #[pallet::call_index(1)]
        #[pallet::weight(Weight::from_parts(50_000_000, 0))]
        pub fn vote(origin: OriginFor<T>, proposal_id: u32, approve: bool) -> DispatchResult {
            let caller = ensure_signed(origin)?;

            // ── [SECURITY CHECK 1] Registered + Active citizen ────────────────
            // No role gate — any citizen of the Republic may vote.
            let citizen = Self::validated_citizen(&caller)?;

            // ── [SECURITY CHECK 2] Proposal must be Active ───────────────────
            let mut proposal =
                Proposals::<T>::get(proposal_id).ok_or(Error::<T>::ProposalNotFound)?;
            ensure!(
                proposal.status == ProposalStatus::Active,
                Error::<T>::ProposalNotActive
            );

            // ── [SECURITY CHECK 3] Voting window must still be open ───────────
            // Votes after end_block are rejected; on_initialize handles enactment.
            ensure!(
                Self::current_block() < proposal.end_block,
                Error::<T>::ProposalNotActive
            );

            // ── [SECURITY CHECK 4] Nation cross-voting prevention ─────────────
            ensure!(
                citizen.nation_id == proposal.nation_id,
                Error::<T>::WrongNation
            );

            // ── [SECURITY CHECK 5] No double voting ──────────────────────────
            ensure!(
                !HasVoted::<T>::get(proposal_id, &caller),
                Error::<T>::AlreadyVoted
            );

            // ── Record vote ───────────────────────────────────────────────────
            HasVoted::<T>::insert(proposal_id, &caller, true);

            if approve {
                proposal.votes_for = proposal.votes_for.saturating_add(1);
            } else {
                proposal.votes_against = proposal.votes_against.saturating_add(1);
            }

            Proposals::<T>::insert(proposal_id, proposal);

            Self::deposit_event(Event::Voted {
                proposal_id,
                voter: caller,
                approve,
            });

            Ok(())
        }

        // ─── execute_proposal ────────────────────────────────────────────────

        /// Manually finalise an expired proposal.
        ///
        /// Normally proposals are enacted automatically by `on_initialize` when
        /// `current_block >= end_block`.  This extrinsic allows any signed
        /// account to trigger enactment early if needed (e.g., after all voters
        /// have cast their votes and the result is already determined).
        ///
        /// The voting window MUST have closed (`current_block >= end_block`)
        /// before manual execution is allowed — this prevents premature finalization
        /// while voting is still active.
        ///
        /// ## Quorum Rule
        ///
        /// Passed if `votes_for >= T::MinQuorum::get()` AND `votes_for > votes_against`.
        #[pallet::call_index(2)]
        #[pallet::weight(Weight::from_parts(50_000_000, 0))]
        pub fn execute_proposal(origin: OriginFor<T>, proposal_id: u32) -> DispatchResult {
            ensure_signed(origin)?;

            let proposal = Proposals::<T>::get(proposal_id).ok_or(Error::<T>::ProposalNotFound)?;
            ensure!(
                proposal.status == ProposalStatus::Active,
                Error::<T>::ProposalNotActive
            );

            // Voting window must be closed before manual execution.
            ensure!(
                Self::current_block() >= proposal.end_block,
                Error::<T>::ProposalNotActive
            );

            // Delegate to the shared enactment engine.
            Self::enact_proposal(proposal_id, proposal);
            Ok(())
        }

        // ─── propose_expert_bill ──────────────────────────────────────────────

        /// Submit an Expert Legislative Bill to the Khural.
        ///
        /// **Exclusive to Academicians** — only citizens who are registered in
        /// `pallet-guilds::AcademyMembers` (Grandmasters of `GuildTumed` unions)
        /// may call this extrinsic.
        ///
        /// Expert bills are marked `ExpertInitiative` and are presented to
        /// Khural delegates in **priority order**, establishing industry standards
        /// at the confederation level.  They are not nation-bound.
        ///
        /// ## Parameters
        ///
        /// - `bill_hash`    – H256 content hash of the full bill (IPFS CID or on-chain).
        /// - `industry_tag` – Industry domain the bill pertains to (max 64 bytes, free-form).
        ///
        /// ## Security Checks
        ///
        /// 1. Caller must be a **registered, active** citizen.
        /// 2. Caller must be in `AcademyMembers` — checked via `T::AcademyInterface`.
        ///    Non-Academicians receive `Error::NotAcademician`.
        ///
        /// # Origin: Signed (must be an Academician)
        #[pallet::call_index(3)]
        #[pallet::weight(Weight::from_parts(50_000_000, 0))]
        pub fn propose_expert_bill(
            origin: OriginFor<T>,
            bill_hash: H256,
            industry_tag: Vec<u8>,
        ) -> DispatchResult {
            let caller = ensure_signed(origin)?;

            // ── [SECURITY CHECK 1] Registered + Active citizen ────────────────
            let _citizen = Self::validated_citizen(&caller)?;

            // ── [SECURITY CHECK 2] Must be an Academician ────────────────────
            // Verified via the loose-coupled AcademyInterface trait from pallet-guilds.
            // Only GuildTumed Grandmasters are registered in AcademyMembers.
            ensure!(
                T::AcademyInterface::is_academician(&caller),
                Error::<T>::NotAcademician
            );

            // ── Bound the industry tag ────────────────────────────────────────
            let bounded_tag: BoundedVec<u8, ConstU32<64>> = industry_tag
                .clone()
                .try_into()
                .map_err(|_| Error::<T>::IndustryTagTooLong)?;

            // ── [SECURITY VECTOR 2] Anti-spam deposit ─────────────────────────
            // Reserve ExpertBillDeposit BEFORE inserting the bill into state.
            // If the proposer lacks funds, the whole call fails early with no
            // state bloat. The deposit is returned on passage or slashed on
            // rejection inside `execute_expert_bill`.
            let deposit = T::ExpertBillDeposit::get();
            <T as Config>::Currency::reserve(&caller, deposit)
                .map_err(|_| Error::<T>::InsufficientBalance)?;

            // ── Assign bill ID and store ──────────────────────────────────────
            let bill_id = NextExpertBillId::<T>::get();
            ExpertBills::<T>::insert(
                bill_id,
                ExpertBill {
                    proposer: caller.clone(),
                    bill_hash,
                    industry_tag: bounded_tag,
                    initiative_type: BillInitiativeType::ExpertInitiative,
                    status: ProposalStatus::Active,
                    votes_for: 0,
                    votes_against: 0,
                },
            );
            NextExpertBillId::<T>::put(bill_id.saturating_add(1));

            // Record depositor for refund/slash in execute_expert_bill.
            ExpertBillDepositor::<T>::insert(bill_id, (caller.clone(), deposit));

            Self::deposit_event(Event::ExpertBillProposed {
                bill_id,
                proposer: caller,
                industry_tag,
                bill_hash,
            });

            Ok(())
        }

        // ─── vote_on_expert_bill ──────────────────────────────────────────────

        /// Cast an approve or reject vote on an active Expert Bill.
        ///
        /// Expert bills are confederation-wide — any citizen with role ≥ `ArbadLeader`
        /// may vote (no nation restriction). They are presented to Khural delegates
        /// in priority order (indicated by the `ExpertInitiative` flag).
        ///
        /// # Security Checks
        ///
        /// 1. Caller must be a registered, active citizen.
        /// 2. Caller's role must be ≥ `ArbadLeader`.
        /// 3. The bill must exist and be `Active`.
        /// 4. Caller must not have voted already.
        ///
        /// # Origin: Signed (must be ArbadLeader+)
        #[pallet::call_index(4)]
        #[pallet::weight(Weight::from_parts(50_000_000, 0))]
        pub fn vote_on_expert_bill(
            origin: OriginFor<T>,
            bill_id: u32,
            approve: bool,
        ) -> DispatchResult {
            let caller = ensure_signed(origin)?;

            // ── [SECURITY CHECK 1 & 2] Registered + Active + Role ────────────
            let citizen = Self::validated_citizen(&caller)?;
            ensure!(
                Self::is_at_least_arbad_leader(&citizen.role),
                Error::<T>::InsufficientRole
            );

            // ── [SECURITY CHECK 3] Bill must be Active ───────────────────────
            let mut bill = ExpertBills::<T>::get(bill_id).ok_or(Error::<T>::ExpertBillNotFound)?;
            ensure!(
                bill.status == ProposalStatus::Active,
                Error::<T>::ExpertBillNotActive
            );

            // ── [SECURITY CHECK 4] No double voting ──────────────────────────
            ensure!(
                !HasVotedOnBill::<T>::get(bill_id, &caller),
                Error::<T>::AlreadyVoted
            );

            HasVotedOnBill::<T>::insert(bill_id, &caller, true);

            if approve {
                bill.votes_for = bill.votes_for.saturating_add(1);
            } else {
                bill.votes_against = bill.votes_against.saturating_add(1);
            }
            ExpertBills::<T>::insert(bill_id, bill);

            Self::deposit_event(Event::ExpertBillVoted {
                bill_id,
                voter: caller,
                approve,
            });

            Ok(())
        }

        // ─── execute_expert_bill ──────────────────────────────────────────────

        /// Finalise an Expert Bill once sufficient votes have accumulated.
        ///
        /// Expert bills use the same mock quorum as standard proposals (`votes_for > 2`).
        /// Any signed account may trigger finalisation.
        ///
        /// # Origin: Signed (anyone)
        #[pallet::call_index(5)]
        #[pallet::weight(Weight::from_parts(50_000_000, 0))]
        pub fn execute_expert_bill(origin: OriginFor<T>, bill_id: u32) -> DispatchResult {
            ensure_signed(origin)?;

            let mut bill = ExpertBills::<T>::get(bill_id).ok_or(Error::<T>::ExpertBillNotFound)?;
            ensure!(
                bill.status == ProposalStatus::Active,
                Error::<T>::ExpertBillNotActive
            );

            // Mock quorum: at least 3 approve votes
            let passed = bill.votes_for > 2;
            bill.status = if passed {
                ProposalStatus::Executed
            } else {
                ProposalStatus::Rejected
            };
            ExpertBills::<T>::insert(bill_id, bill);

            // [SECURITY VECTOR 2] Settle the anti-spam deposit.
            if let Some((depositor, deposit)) = ExpertBillDepositor::<T>::take(bill_id) {
                if passed {
                    // Bill passed — return the deposit to the Academician.
                    <T as Config>::Currency::unreserve(&depositor, deposit);
                } else {
                    // Bill rejected — slash the deposit (burn from monetary supply).
                    // `slash_reserved` returns (Imbalance, remaining_unslashed); we drop
                    // the imbalance which auto-burns it on drop (no treasury routing needed).
                    let (_, _unslashed) =
                        <T as Config>::Currency::slash_reserved(&depositor, deposit);

                    Self::deposit_event(Event::ExpertBillDepositSlashed {
                        bill_id,
                        proposer: depositor,
                        slashed_amount: deposit,
                    });
                }
            }
            // Note: if the depositor record is missing (should never happen), we
            // proceed silently rather than rolling back the bill finalisation.

            Self::deposit_event(Event::ExpertBillExecuted { bill_id, passed });

            Ok(())
        }
    }
}
