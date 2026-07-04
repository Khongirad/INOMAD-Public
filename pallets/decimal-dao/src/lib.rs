//! # pallet-decimal-dao
//!
//! **Altan Network — Generic Decimal DAO Core**
//!
//! Powers all 4 branches of the INOMAD State (Legislative, Executive,
//! Judicial, Audit) and any arbitrary organization (Guilds, Enterprises).
//!
//! ## Architecture
//!
//! - `org_id: [u8; 32]` — Blake2-256 hash of the off-chain UUID (from the
//!   backend Relayer after forming quorum).
//! - Councils are stored per `org_id` and managed entirely by the Relayer.
//! - Keyless treasuries are derived deterministically:
//!   `T::PalletId::get().into_sub_account_truncating(org_id)`
//! - The Relayer calls `sync_council` after each annual Arbad→Tumed election
//!   to atomically replace the on-chain council with the top-N elected leaders.
//!
//! ## Constitutional Bounds
//!
//! - Treasury transfers use `KeepAlive` (Existential Deposit protected).
//! - No `force_set_balance` / `force_transfer`.
//! - State accounts (Central Bank, Confederation Treasury) are handled
//!   in `pallet-altan-tax`; this pallet handles org-level DAOs only.
//!
//! ## Dispatchables
//!
//! | Call | Origin | Description |
//! |---|---|---|
//! | `instantiate_org` | Root | Create org with initial council and keyless treasury |
//! | `sync_council` | Root | Replace council after annual elections |
//! | `create_proposal` | Root | Register treasury-spend proposal on-chain |
//! | `vote_proposal` | Root | Cast council vote on open proposal |
//! | `execute_proposal` | Root | Transfer funds after proposal passes |

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
    use frame_support::{
        pallet_prelude::*,
        traits::{Currency, ExistenceRequirement, ReservableCurrency},
        PalletId,
    };
    use frame_system::pallet_prelude::*;
    use sp_runtime::traits::AccountIdConversion;

    // =========================================================================
    // Currency alias
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
        /// Currency used for proposal treasury transfers.
        type Currency: Currency<Self::AccountId> + ReservableCurrency<Self::AccountId>;

        /// PalletId used to derive keyless treasury accounts for each org.
        ///
        /// `treasury = T::PalletId::get().into_sub_account_truncating(org_id)`
        ///
        /// Runtime: `PalletId(*b"inm/ddao")`
        #[pallet::constant]
        type PalletId: Get<PalletId>;

        /// Maximum number of council members per org.
        ///
        /// Covers GuildTumed apex tier (~100 squads across 10,000 members).
        /// Runtime: `ConstU32<100>`
        #[pallet::constant]
        type MaxCouncilMembers: Get<u32>;
    }

    // =========================================================================
    // Structs
    // =========================================================================

    /// Lifecycle status of an on-chain DAO proposal.
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
        /// Council voting is open.
        Open,
        /// Strict majority (>50%) voted in favour.
        Passed,
        /// Strict majority voted against.
        Rejected,
    }

    /// Lightweight on-chain record for a council-gated treasury proposal.
    ///
    /// Full proposal body is stored off-chain (DB); the chain tracks only the
    /// tally and status so council-only voting can be validated trustlessly.
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
    pub struct ProposalRecord<T: Config> {
        /// Blake2-256 hash of the org UUID — links proposal to its council.
        pub org_id: [u8; 32],
        /// Council member who submitted the proposal.
        pub proposer: T::AccountId,
        /// ALTAN amount to transfer from the keyless treasury.
        pub amount: BalanceOf<T>,
        /// Beneficiary account.
        pub beneficiary: T::AccountId,
        /// Votes cast in favour.
        pub votes_for: u32,
        /// Votes cast against.
        pub votes_against: u32,
        /// Current lifecycle status.
        pub status: ProposalStatus,
    }

    // =========================================================================
    // Storage
    // =========================================================================

    /// Per-org elected council: `org_id → BoundedVec<AccountId, MaxCouncilMembers>`.
    ///
    /// Populated by `instantiate_org`; replaced atomically by `sync_council`
    /// after annual Arbad→Tumed elections conclude off-chain.
    #[pallet::storage]
    #[pallet::getter(fn councils)]
    pub type Councils<T: Config> = StorageMap<
        _,
        Blake2_128Concat,
        [u8; 32], // org_id
        BoundedVec<T::AccountId, T::MaxCouncilMembers>,
        OptionQuery,
    >;

    /// Per-org keyless treasury account: `org_id → AccountId`.
    ///
    /// Derived deterministically on `instantiate_org` as:
    ///   `T::PalletId::get().into_sub_account_truncating(org_id)`
    /// No private key; controlled exclusively by pallet logic.
    #[pallet::storage]
    #[pallet::getter(fn org_treasuries)]
    pub type OrgTreasuries<T: Config> = StorageMap<
        _,
        Blake2_128Concat,
        [u8; 32], // org_id
        T::AccountId,
        OptionQuery,
    >;

    /// On-chain tally for council-gated proposals: `prop_id → ProposalRecord`.
    ///
    /// Keyed by Blake2-256 hash of the off-chain proposal UUID.
    #[pallet::storage]
    #[pallet::getter(fn proposals)]
    pub type Proposals<T: Config> = StorageMap<
        _,
        Blake2_128Concat,
        [u8; 32], // prop_id
        ProposalRecord<T>,
        OptionQuery,
    >;

    /// Vote deduplication: `(prop_id, voter) → ()`.
    ///
    /// Prevents a council member from voting twice on the same proposal.
    #[pallet::storage]
    pub type ProposalBallots<T: Config> = StorageDoubleMap<
        _,
        Blake2_128Concat,
        [u8; 32], // prop_id
        Blake2_128Concat,
        T::AccountId,
        (),
        OptionQuery,
    >;

    // =========================================================================
    // Events
    // =========================================================================

    #[pallet::event]
    #[pallet::generate_deposit(pub(super) fn deposit_event)]
    pub enum Event<T: Config> {
        /// A new org was instantiated with its initial council and treasury.
        OrgInstantiated {
            org_id: [u8; 32],
            treasury: T::AccountId,
            council_size: u32,
        },
        /// The org's elected council was replaced by the Relayer after annual elections.
        CouncilSynced { org_id: [u8; 32], council_size: u32 },
        /// A council member submitted a new governance proposal.
        ProposalCreated {
            prop_id: [u8; 32],
            org_id: [u8; 32],
            proposer: T::AccountId,
            amount: BalanceOf<T>,
        },
        /// A council member cast their vote on a governance proposal.
        ProposalVoteCast {
            prop_id: [u8; 32],
            voter: T::AccountId,
            in_favor: bool,
            votes_for: u32,
            votes_against: u32,
        },
        /// A governance proposal crossed the >50% council majority threshold.
        ProposalPassed {
            prop_id: [u8; 32],
            org_id: [u8; 32],
            votes_for: u32,
        },
        /// A governance proposal was defeated by council majority.
        ProposalRejected {
            prop_id: [u8; 32],
            org_id: [u8; 32],
            votes_against: u32,
        },
        /// A passed proposal was executed — funds transferred from keyless treasury.
        ProposalExecuted {
            prop_id: [u8; 32],
            org_id: [u8; 32],
            amount: BalanceOf<T>,
            beneficiary: T::AccountId,
        },
    }

    // =========================================================================
    // Errors
    // =========================================================================

    #[pallet::error]
    pub enum Error<T> {
        /// An org with this ID is already instantiated on-chain.
        OrgAlreadyExists,
        /// Referenced org does not exist on-chain.
        OrgNotFound,
        /// The council list exceeds `MaxCouncilMembers`.
        CouncilTooLarge,
        /// The caller / voter is not in the org's elected council.
        NotACouncilMember,
        /// Referenced proposal does not exist.
        ProposalNotFound,
        /// A proposal with this ID already exists on-chain.
        ProposalAlreadyExists,
        /// This council member has already voted on this proposal.
        AlreadyVoted,
        /// The proposal is no longer open for voting.
        ProposalNotOpen,
        /// The proposal has not yet reached >50% majority — cannot execute.
        ProposalNotPassed,
        /// Treasury balance insufficient to execute the proposal.
        InsufficientTreasuryBalance,
    }

    // =========================================================================
    // Internal Helpers
    // =========================================================================

    impl<T: Config> Pallet<T> {
        /// Derive the keyless treasury AccountId for `org_id`.
        ///
        /// `treasury = T::PalletId::get().into_sub_account_truncating(org_id)`
        pub fn treasury_account(org_id: &[u8; 32]) -> T::AccountId {
            T::PalletId::get().into_sub_account_truncating(org_id)
        }

        /// Check that `who` is in `Councils[org_id]`.
        fn ensure_council_member(org_id: &[u8; 32], who: &T::AccountId) -> DispatchResult {
            let council = Councils::<T>::get(org_id).ok_or(Error::<T>::OrgNotFound)?;
            ensure!(council.contains(who), Error::<T>::NotACouncilMember);
            Ok(())
        }
    }

    // =========================================================================
    // Extrinsics
    // =========================================================================

    #[pallet::call]
    impl<T: Config> Pallet<T> {
        // ─── instantiate_org ─────────────────────────────────────────────────

        /// Register a new org on-chain with an initial council and a keyless treasury.
        ///
        /// Called by the Relayer (via Sudo) once the off-chain quorum is reached.
        ///
        /// - Derives `treasury = PalletId.into_sub_account_truncating(org_id)`.
        /// - Stores `initial_council` in `Councils[org_id]`.
        /// - Errors if `org_id` already exists.
        #[pallet::call_index(0)]
        #[pallet::weight(T::WeightInfo::instantiate_org())]
        pub fn instantiate_org(
            origin: OriginFor<T>,
            org_id: [u8; 32],
            initial_council: Vec<T::AccountId>,
        ) -> DispatchResult {
            ensure_root(origin)?;

            // Guard: no duplicate org IDs.
            ensure!(
                !Councils::<T>::contains_key(&org_id),
                Error::<T>::OrgAlreadyExists
            );

            // Bound the council vec.
            let council: BoundedVec<T::AccountId, T::MaxCouncilMembers> = initial_council
                .try_into()
                .map_err(|_| Error::<T>::CouncilTooLarge)?;

            // Derive the keyless treasury.
            let treasury = Self::treasury_account(&org_id);

            // Persist.
            Councils::<T>::insert(&org_id, &council);
            OrgTreasuries::<T>::insert(&org_id, &treasury);

            Self::deposit_event(Event::OrgInstantiated {
                org_id,
                treasury,
                council_size: council.len() as u32,
            });

            Ok(())
        }

        // ─── sync_council ────────────────────────────────────────────────────

        /// Replace the org's council with the newly-elected members.
        ///
        /// Called by the Relayer (via Sudo) after annual off-chain elections conclude.
        /// Atomically replaces the entire council; old council authority is revoked.
        ///
        /// - Errors if `org_id` is not instantiated.
        /// - Errors if `new_council.len() > MaxCouncilMembers`.
        #[pallet::call_index(1)]
        #[pallet::weight(T::WeightInfo::sync_council())]
        pub fn sync_council(
            origin: OriginFor<T>,
            org_id: [u8; 32],
            new_council: Vec<T::AccountId>,
        ) -> DispatchResult {
            ensure_root(origin)?;

            ensure!(
                Councils::<T>::contains_key(&org_id),
                Error::<T>::OrgNotFound
            );

            let council: BoundedVec<T::AccountId, T::MaxCouncilMembers> = new_council
                .try_into()
                .map_err(|_| Error::<T>::CouncilTooLarge)?;

            Councils::<T>::insert(&org_id, &council);

            Self::deposit_event(Event::CouncilSynced {
                org_id,
                council_size: council.len() as u32,
            });

            Ok(())
        }

        // ─── create_proposal ────────────────────────────────────────────────

        /// Register a new treasury-spend proposal on-chain.
        ///
        /// Only callable by the Relayer (Sudo) after verifying the proposer is a
        /// council member off-chain (SubWallet signature verified in DaoGovernanceService).
        ///
        /// - `proposer` must be in `Councils[org_id]`.
        /// - Errors if `prop_id` already exists.
        #[pallet::call_index(2)]
        #[pallet::weight(T::WeightInfo::create_proposal())]
        pub fn create_proposal(
            origin: OriginFor<T>,
            org_id: [u8; 32],
            prop_id: [u8; 32],
            proposer: T::AccountId,
            amount: BalanceOf<T>,
            beneficiary: T::AccountId,
        ) -> DispatchResult {
            ensure_root(origin)?;

            // Verify proposer is a council member.
            Self::ensure_council_member(&org_id, &proposer)?;

            // Guard: no duplicate proposal IDs.
            ensure!(
                !Proposals::<T>::contains_key(&prop_id),
                Error::<T>::ProposalAlreadyExists
            );

            Proposals::<T>::insert(
                &prop_id,
                ProposalRecord {
                    org_id,
                    proposer: proposer.clone(),
                    amount,
                    beneficiary,
                    votes_for: 0,
                    votes_against: 0,
                    status: ProposalStatus::Open,
                },
            );

            Self::deposit_event(Event::ProposalCreated {
                prop_id,
                org_id,
                proposer,
                amount,
            });

            Ok(())
        }

        // ─── vote_proposal ───────────────────────────────────────────────────

        /// Cast a council vote on an open proposal.
        ///
        /// - `voter` must be in `Councils[proposal.org_id]`.
        /// - One vote per council member per proposal (dedup via `ProposalBallots`).
        /// - If votes_for > 50% of council size, proposal transitions to `Passed`.
        /// - If votes_against > 50% of council size, proposal transitions to `Rejected`.
        #[pallet::call_index(3)]
        #[pallet::weight(T::WeightInfo::vote_proposal())]
        pub fn vote_proposal(
            origin: OriginFor<T>,
            prop_id: [u8; 32],
            voter: T::AccountId,
            support: bool,
        ) -> DispatchResult {
            ensure_root(origin)?;

            let mut proposal = Proposals::<T>::get(&prop_id).ok_or(Error::<T>::ProposalNotFound)?;

            ensure!(
                proposal.status == ProposalStatus::Open,
                Error::<T>::ProposalNotOpen
            );

            // Verify voter is in this org's council.
            Self::ensure_council_member(&proposal.org_id, &voter)?;

            // Dedup guard.
            ensure!(
                !ProposalBallots::<T>::contains_key(&prop_id, &voter),
                Error::<T>::AlreadyVoted,
            );
            ProposalBallots::<T>::insert(&prop_id, &voter, ());

            // Tally.
            if support {
                proposal.votes_for = proposal.votes_for.saturating_add(1);
            } else {
                proposal.votes_against = proposal.votes_against.saturating_add(1);
            }

            // Determine if threshold crossed. Council size needed for >50%.
            let council_size = Councils::<T>::get(&proposal.org_id)
                .map(|c| c.len() as u32)
                .unwrap_or(0);

            let majority = council_size / 2 + 1; // >50%, rounds up

            if proposal.votes_for >= majority {
                proposal.status = ProposalStatus::Passed;
                Self::deposit_event(Event::ProposalPassed {
                    prop_id,
                    org_id: proposal.org_id,
                    votes_for: proposal.votes_for,
                });
            } else if proposal.votes_against >= majority {
                proposal.status = ProposalStatus::Rejected;
                Self::deposit_event(Event::ProposalRejected {
                    prop_id,
                    org_id: proposal.org_id,
                    votes_against: proposal.votes_against,
                });
            }

            let votes_for = proposal.votes_for;
            let votes_against = proposal.votes_against;

            Proposals::<T>::insert(&prop_id, proposal);

            Self::deposit_event(Event::ProposalVoteCast {
                prop_id,
                voter,
                in_favor: support,
                votes_for,
                votes_against,
            });

            Ok(())
        }

        // ─── execute_proposal ────────────────────────────────────────────────

        /// Execute a passed proposal — transfer funds from the keyless treasury.
        ///
        /// - Proposal must be in `Passed` status.
        /// - Funds transferred from `OrgTreasuries[org_id]` to `beneficiary`.
        /// - Constitutional rule: `KeepAlive` — Existential Deposit preserved.
        ///
        /// Callable by Sudo/Relayer after the proposer requests execution.
        #[pallet::call_index(4)]
        #[pallet::weight(T::WeightInfo::execute_proposal())]
        pub fn execute_proposal(origin: OriginFor<T>, prop_id: [u8; 32]) -> DispatchResult {
            ensure_root(origin)?;

            let proposal = Proposals::<T>::get(&prop_id).ok_or(Error::<T>::ProposalNotFound)?;

            ensure!(
                proposal.status == ProposalStatus::Passed,
                Error::<T>::ProposalNotPassed
            );

            let treasury =
                OrgTreasuries::<T>::get(&proposal.org_id).ok_or(Error::<T>::OrgNotFound)?;

            // Constitutional rule (AGENTS.md §1.1): use transfer, never force_set_balance.
            // KeepAlive ensures Existential Deposit is preserved in the treasury.
            T::Currency::transfer(
                &treasury,
                &proposal.beneficiary,
                proposal.amount,
                ExistenceRequirement::KeepAlive,
            )
            .map_err(|_| Error::<T>::InsufficientTreasuryBalance)?;

            // Mark executed.
            let updated = proposal;
            // We map Passed → a terminal state by removing it (or keep as record).
            // Keep the record but mark as effective execution via event.
            Proposals::<T>::remove(&prop_id);

            Self::deposit_event(Event::ProposalExecuted {
                prop_id,
                org_id: updated.org_id,
                amount: updated.amount,
                beneficiary: updated.beneficiary,
            });

            Ok(())
        }
    }

    // =========================================================================
    // Public Query API (for runtime / other pallets)
    // =========================================================================

    impl<T: Config> Pallet<T> {
        /// Returns the list of current council members for `org_id`, or `None`.
        pub fn get_council(org_id: &[u8; 32]) -> Option<Vec<T::AccountId>> {
            Councils::<T>::get(org_id).map(|bv| bv.into_inner())
        }

        /// Returns `true` if `who` is a current council member of `org_id`.
        pub fn is_council_member(org_id: &[u8; 32], who: &T::AccountId) -> bool {
            Councils::<T>::get(org_id)
                .map(|c| c.contains(who))
                .unwrap_or(false)
        }
    }
}
