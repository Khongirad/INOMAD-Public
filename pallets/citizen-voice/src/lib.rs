//! pallet-citizen-voice — Altan Network: Citizen Voice Protocol (Голос Гражданина)
//!
//! Decentralised civic feedback system. Citizens can file:
//!   - Complaints  (Жалоба)    — against any target
//!   - Suggestions (Предложение)— ideas and improvements
//!   - Whistleblower (Сигнал) — anonymous corruption reports
//!
//! ANTI-SPAM MECHANISM:
//!   Every ticket submission reserves `TicketDeposit` (1 ALTAN) from the caller.
//!   - If the responsible party resolves the ticket as "helpful" → deposit returned.
//!   - If the ticket is fraudulent / slander → deposit is slashed (burned).
//!
//! CONSTITUTIONAL INTEGRATION:
//!   - `FeedbackTarget::Guild(id)` → resolved only by that Guild's Master.
//!     Verified via `T::GuildsChecker::is_guild_master` (bridge to pallet-guilds).
//!   - `FeedbackTarget::Government(_)` → resolved by Root/Sudo.
//!   - `FeedbackTarget::Entity(account)` → resolved by the account holder.
//!
//! ANTI-CORRUPTION (Sting Operations — Оперативный Эксперимент):
//!   A citizen can initiate a controlled sting operation against a suspect official.
//!   The Prosecutor (Root) approves immunity; the citizen makes a real transfer to
//!   the target; Root then reveals the hash and springs the trap (exile + reward).
//!
//! ESCALATION PROTECTION:
//!   A ticket author may escalate a ticket, preventing the accused from closing it.
//!   Only Root can resolve Escalated tickets; deposit is NOT burned on escalation.
//!
//! GUARANTEED DELIVERY:
//!   `TargetTickets` double-map lets any manager enumerate their open tickets O(1)
//!   without scanning the full `Tickets` map. Nothing can be silently dropped.
//!
//! ## Dispatchables
//!
//! | Call | Origin | Description |
//! |---|---|---|
//! | `submit_ticket` | Signed (Citizen) | Submit a complaint or grievance ticket |
//! | `resolve_ticket` | Signed (Officer) | Close a ticket as resolved |
//! | `mark_in_review` | Signed (Officer) | Mark a ticket as under active review |
//! | `escalate_ticket` | Signed (Senior Officer) | Escalate a ticket to the next authority level |
//! | `request_sting_operation` | Signed (Officer) | Request an undercover sting on a reported issue |
//! | `approve_sting` | Signed (Chief) | Approve a requested sting operation |
//! | `reveal_and_spring_trap` | Signed (Officer) | Conclude a sting operation and record outcome |

#![cfg_attr(not(feature = "std"), no_std)]

extern crate alloc;

pub mod migrations;
pub mod weights;
pub use pallet::*;

#[cfg(test)]
mod mock;
#[cfg(test)]
mod tests;

// =========================================================================
// GuildsInterface — loose-coupling trait for cross-pallet guild authority check
// =========================================================================

/// Trait that allows `pallet-citizen-voice` to verify guild authority without
/// tight-coupling to `pallet-guilds` storage.
pub trait GuildsInterface<AccountId> {
    /// Returns `true` if `who` is a `Master` (or higher) of the given `guild_id`.
    fn is_guild_master(guild_id: u32, who: &AccountId) -> bool;
}

// =========================================================================
// BlackBookBridgeInterface — loose-coupling trait to exile a sting target
// =========================================================================

/// Allows `pallet-citizen-voice` to exile a target citizen identified during a
/// sting operation, without tight-coupling to `pallet-inomad-identity` or
/// `pallet-black-book`.
///
/// Wire at runtime with `pallet_inomad_identity::Pallet::<Runtime>` or a glue struct.
pub trait BlackBookBridgeInterface<AccountId> {
    /// Set `who`'s status to `CitizenStatus::Exiled` (terminal).
    fn exile_citizen(who: &AccountId) -> frame_support::dispatch::DispatchResult;
}

#[cfg(feature = "runtime-benchmarks")]
mod benchmarking;
#[frame_support::pallet]
pub mod pallet {
    use crate::weights::WeightInfo as _;
    use frame_support::{
        pallet_prelude::*,
        traits::{Currency, ExistenceRequirement, ReservableCurrency},
    };
    use frame_system::pallet_prelude::*;
    use sp_core::{blake2_256, H256};
    // Bring the cross-pallet traits into scope.
    use crate::BlackBookBridgeInterface;
    use crate::GuildsInterface;

    // =========================================================================
    // Currency helper type
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
        /// Currency used to reserve anti-spam deposits and whistleblower rewards.
        type Currency: Currency<Self::AccountId> + ReservableCurrency<Self::AccountId>;

        /// Bridge to `pallet-guilds` for Guild Master authority checks.
        type GuildsChecker: crate::GuildsInterface<Self::AccountId>;

        /// Bridge to exile a sting target (wired to pallet-inomad-identity in runtime).
        type BlackBookBridge: crate::BlackBookBridgeInterface<Self::AccountId>;

        /// The state treasury account — source of Whistleblower Rewards.
        #[pallet::constant]
        type StateTreasury: Get<Self::AccountId>;

        /// Amount reserved from the submitter as an anti-spam deposit.
        /// Returned if the ticket is "helpful"; burned if it is fraudulent.
        #[pallet::constant]
        type TicketDeposit: Get<BalanceOf<Self>>;

        /// Whistleblower reward percentage of confiscated funds (e.g. 20 = 20%).
        #[pallet::constant]
        type WhistleblowerRewardPercent: Get<u8>;
    }

    // =========================================================================
    // Enums
    // =========================================================================

    /// The branch of government that can handle a Government-targeted ticket.
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
    pub enum BranchType {
        /// The Khural — the legislative assembly.
        Khural,
        /// The Academy of Sciences — expert advisory body.
        Academy,
        /// The Judicial Courts — rule-of-law enforcement.
        JudicialCourt,
        /// The Banking Authority — monetary policy branch.
        BankingAuthority,
        /// The Medical / Civil Registry — births, deaths, identity.
        CivilRegistry,
    }

    /// The target of a citizen's feedback ticket.
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
    pub enum FeedbackTarget<AccountId> {
        /// A professional Guild (resolved by Guild Master).
        Guild(u32),
        /// A branch of government (resolved by Root/Sudo).
        Government(BranchType),
        /// A company or individual identified by their AccountId.
        Entity(AccountId),
    }

    /// The category of the citizen's feedback.
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
    pub enum FeedbackType {
        /// Formal complaint — alleges harm or service failure.
        Complaint,
        /// Constructive suggestion — proposes an improvement.
        Suggestion,
        /// Whistleblower signal — alleges corruption or serious misconduct.
        Whistleblower,
    }

    /// Lifecycle status of a feedback Ticket.
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
    pub enum TicketStatus {
        /// Newly submitted — awaiting review by the target's responsible party.
        Open,
        /// Acknowledged — the responsible party has started reviewing it.
        InReview,
        /// Escalated — author requested constitutional review; only Root can resolve.
        /// Deposit is NOT burned while in this state.
        Escalated,
        /// Resolved — confirmed helpful; deposit returned (+ possible bonus).
        Resolved,
        /// Rejected — determined to be spam or false; deposit burned.
        Rejected,
    }

    /// Lifecycle status of a Sting Operation (Оперативный Эксперимент).
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
    pub enum StingStatus {
        /// Citizen filed the request; awaiting Prosecutor approval.
        Pending,
        /// Prosecutor approved — citizen has immunity, operation is live.
        Approved,
        /// Trap was sprung — target exiled, citizen rewarded.
        Sprung,
    }

    // =========================================================================
    // Structs
    // =========================================================================

    /// A civic feedback ticket submitted by a citizen.
    ///
    /// `content_hash` is an H256 linking to an IPFS document.
    /// Only the hash is stored on-chain — full content lives off-chain.
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
    pub struct Ticket<T: Config> {
        /// The citizen who submitted this ticket.
        pub author: T::AccountId,
        /// Who this ticket is directed at.
        pub target: FeedbackTarget<T::AccountId>,
        /// The type of feedback.
        pub feedback_type: FeedbackType,
        /// H256 hash of the IPFS document containing the full content.
        pub content_hash: H256,
        /// Current lifecycle status.
        pub status: TicketStatus,
        /// Anti-spam deposit reserved from `author` on submission.
        pub deposit: BalanceOf<T>,
    }

    /// A Sting Operation (Оперативный Эксперимент) record.
    ///
    /// Implements a commit-reveal scheme. The `commit_hash` is blake2_256(amount ++ salt).
    /// The full `amount` is only revealed to the chain when `reveal_and_spring_trap` is called.
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
    pub struct StingOperation<T: Config> {
        /// The citizen who filed the sting request (whistleblower).
        pub citizen: T::AccountId,
        /// The suspected corrupt official being targeted.
        pub target: T::AccountId,
        /// The committed bribe amount (reserved from citizen on request).
        pub amount: BalanceOf<T>,
        /// Commit hash: blake2_256(amount_le_bytes ++ secret_salt).
        pub commit_hash: [u8; 32],
        /// Current operation status.
        pub status: StingStatus,
    }

    // =========================================================================
    // Storage
    // =========================================================================

    /// Global sequential Ticket ID counter.
    #[pallet::storage]
    #[pallet::getter(fn next_ticket_id)]
    pub type NextTicketId<T: Config> = StorageValue<_, u32, ValueQuery>;

    /// Ticket registry: TicketId → Ticket.
    #[pallet::storage]
    #[pallet::getter(fn tickets)]
    pub type Tickets<T: Config> = StorageMap<_, Blake2_128Concat, u32, Ticket<T>, OptionQuery>;

    /// Target ticket index: (FeedbackTarget, TicketId) → ().
    ///
    /// Allows managers (Guild Masters, Government delegates, Entity owners) to
    /// enumerate all tickets addressed to them.
    /// Nothing can be silently "lost" — every ticket is indexed here on creation.
    #[pallet::storage]
    #[pallet::getter(fn target_tickets)]
    pub type TargetTickets<T: Config> = StorageDoubleMap<
        _,
        Blake2_128Concat,
        FeedbackTarget<T::AccountId>,
        Blake2_128Concat,
        u32,
        (),
        OptionQuery,
    >;

    /// Global sequential Sting Operation ID counter.
    #[pallet::storage]
    #[pallet::getter(fn next_sting_id)]
    pub type NextStingId<T: Config> = StorageValue<_, u64, ValueQuery>;

    /// Sting Operation registry: StingId → StingOperation.
    #[pallet::storage]
    #[pallet::getter(fn sting_operations)]
    pub type StingOperations<T: Config> =
        StorageMap<_, Blake2_128Concat, u64, StingOperation<T>, OptionQuery>;

    // =========================================================================
    // Events
    // =========================================================================

    #[pallet::event]
    #[pallet::generate_deposit(pub(super) fn deposit_event)]
    pub enum Event<T: Config> {
        // ── Ticket Events ─────────────────────────────────────────────────────
        /// A citizen submitted a new feedback ticket.
        TicketSubmitted {
            ticket_id: u32,
            author: T::AccountId,
            target: FeedbackTarget<T::AccountId>,
            feedback_type: FeedbackType,
            content_hash: H256,
            deposit: BalanceOf<T>,
        },
        /// The responsible party began reviewing a ticket.
        /// `reviewer` is `None` when acknowledged by Root/Sudo (no AccountId available).
        TicketInReview {
            ticket_id: u32,
            reviewer: Option<T::AccountId>,
        },
        /// A ticket was resolved as helpful — deposit returned to citizen.
        /// `resolver` is `None` when the resolution was performed by Root/Sudo.
        TicketResolved {
            ticket_id: u32,
            resolver: Option<T::AccountId>,
            resolution_hash: H256,
            deposit_returned: BalanceOf<T>,
        },
        /// A ticket was rejected as spam or slander — deposit burned.
        /// `resolver` is `None` when the rejection was performed by Root/Sudo.
        TicketRejected {
            ticket_id: u32,
            resolver: Option<T::AccountId>,
            resolution_hash: H256,
            deposit_slashed: BalanceOf<T>,
        },
        /// [VECTOR 2] A ticket was escalated by its author for constitutional review.
        /// Only Root can now resolve it. Deposit is safe.
        TicketEscalated {
            ticket_id: u32,
            author: T::AccountId,
        },
        // ── Sting Operation Events ────────────────────────────────────────────
        /// [VECTOR 0] A citizen filed a sting operation request.
        StingRequested {
            sting_id: u64,
            citizen: T::AccountId,
            target: T::AccountId,
            commit_hash: [u8; 32],
        },
        /// [VECTOR 0] Prosecutor approved the sting — citizen now has immunity.
        StingApproved {
            sting_id: u64,
            citizen: T::AccountId,
            target: T::AccountId,
        },
        /// [VECTOR 0] Trap sprung — target exiled, whistleblower rewarded.
        TrapSprung {
            sting_id: u64,
            target: T::AccountId,
            whistleblower: T::AccountId,
            amount_returned: BalanceOf<T>,
            whistleblower_reward: BalanceOf<T>,
        },
    }

    // =========================================================================
    // Errors
    // =========================================================================

    #[pallet::error]
    pub enum Error<T> {
        // ── Ticket Errors ──────────────────────────────────────────────────────
        /// Referenced ticket does not exist.
        TicketNotFound,
        /// Ticket is not in a state that allows this operation.
        InvalidTicketStatus,
        /// Caller is not authorised to resolve this ticket.
        /// (Must be Guild Master, Entity owner, or Root for Government targets.)
        NotAuthorised,
        /// Currency reservation failed — insufficient free balance.
        InsufficientBalance,
        /// Ticket deposit amount is zero (configuration error).
        ZeroDeposit,
        /// Only the ticket's author may escalate it.
        OnlyAuthorCanEscalate,
        /// [VECTOR 2] Ticket is already in Escalated state.
        TicketAlreadyEscalated,
        /// [VECTOR 2] Escalated tickets can only be resolved by Root.
        EscalatedTicketRequiresRoot,
        // ── Sting Operation Errors ─────────────────────────────────────────────
        /// Referenced sting operation does not exist.
        StingNotFound,
        /// The sting operation has already been approved.
        StingAlreadyApproved,
        /// The sting operation has already been sprung.
        StingAlreadySprung,
        /// The sting operation has not yet been approved by the Prosecutor.
        StingNotApproved,
        /// The reveal hash does not match the original commit hash.
        InvalidReveal,
        /// Treasury has insufficient funds for the whistleblower reward.
        TreasuryInsufficientFunds,
    }

    // =========================================================================
    // Internal helpers
    // =========================================================================

    impl<T: Config> Pallet<T> {
        /// Verify that `caller` is authorised to manage tickets for a **non-Government** target.
        ///
        /// | Target            | Authority                         |
        /// |-------------------|-----------------------------------|
        /// | `Guild(id)`       | Master of guild `id` (via bridge) |
        /// | `Entity(account)` | The `account` itself              |
        ///
        /// Government targets must be handled separately via `ensure_root`.
        fn ensure_signed_target_authority(
            caller: &T::AccountId,
            target: &FeedbackTarget<T::AccountId>,
        ) -> DispatchResult {
            match target {
                FeedbackTarget::Guild(guild_id) => {
                    ensure!(
                        T::GuildsChecker::is_guild_master(*guild_id, caller),
                        Error::<T>::NotAuthorised
                    );
                }
                FeedbackTarget::Government(_) => {
                    // Government targets require Root — caller path should never reach here.
                    return Err(Error::<T>::NotAuthorised.into());
                }
                FeedbackTarget::Entity(account) => {
                    ensure!(caller == account, Error::<T>::NotAuthorised);
                }
            }
            Ok(())
        }
    }

    // =========================================================================
    // Extrinsics
    // =========================================================================

    #[pallet::call]
    impl<T: Config> Pallet<T> {
        // ─── submit_ticket ────────────────────────────────────────────────────

        /// Submit a civic feedback ticket to any registered target.
        ///
        /// Any signed account (citizen) may call this.
        ///
        /// ## Anti-Spam
        /// `TicketDeposit` (1 ALTAN) is reserved from the caller's free balance.
        /// Returned if the ticket resolves as "helpful"; burned if rejected.
        ///
        /// # Origin: Signed
        #[pallet::call_index(0)]
        #[pallet::weight(T::WeightInfo::submit_ticket())]
        pub fn submit_ticket(
            origin: OriginFor<T>,
            target: FeedbackTarget<T::AccountId>,
            feedback_type: FeedbackType,
            content_hash: H256,
        ) -> DispatchResult {
            let caller = ensure_signed(origin)?;

            let deposit = T::TicketDeposit::get();
            ensure!(deposit > BalanceOf::<T>::default(), Error::<T>::ZeroDeposit);

            // Reserve the anti-spam deposit.
            T::Currency::reserve(&caller, deposit).map_err(|_| Error::<T>::InsufficientBalance)?;

            let ticket_id = NextTicketId::<T>::get();

            let ticket = Ticket::<T> {
                author: caller.clone(),
                target: target.clone(),
                feedback_type: feedback_type.clone(),
                content_hash,
                status: TicketStatus::Open,
                deposit,
            };

            Tickets::<T>::insert(ticket_id, &ticket);
            TargetTickets::<T>::insert(&target, ticket_id, ());
            NextTicketId::<T>::put(ticket_id.saturating_add(1));

            Self::deposit_event(Event::TicketSubmitted {
                ticket_id,
                author: caller,
                target,
                feedback_type,
                content_hash,
                deposit,
            });
            Ok(())
        }

        // ─── resolve_ticket ───────────────────────────────────────────────────

        /// Officially resolve a feedback ticket.
        ///
        /// The caller must be the authorised responsible party for the ticket's target:
        /// - **Guild** target      → Guild Master of that specific Guild.
        /// - **Government** target → Root / Sudo (constitutional authority).
        /// - **Entity** target     → The exact account that is the Entity.
        ///
        /// ## [VECTOR 2] Escalation Guard
        /// Tickets in `Escalated` status can ONLY be resolved by Root.
        /// Non-root callers receive `EscalatedTicketRequiresRoot`.
        ///
        /// ## Deposit Logic
        /// - `is_helpful == true`:  Deposit **unreserved** (returned) to the author.
        /// - `is_helpful == false`: Deposit **slashed** (burned via imbalance drop).
        ///   EXCEPTION: Deposit is NEVER slashed while ticket is `Escalated` (Root must
        ///   explicitly choose `is_helpful=false` after the escalation review).
        ///
        /// # Origin: Signed (Guild/Entity) or Root (Government or Escalated)
        #[pallet::call_index(1)]
        #[pallet::weight(T::WeightInfo::resolve_ticket())]
        pub fn resolve_ticket(
            origin: OriginFor<T>,
            ticket_id: u32,
            resolution_hash: H256,
            is_helpful: bool,
        ) -> DispatchResult {
            let ticket = Tickets::<T>::get(ticket_id).ok_or(Error::<T>::TicketNotFound)?;

            // [VECTOR 2] Escalated tickets can ONLY be resolved by Root.
            if ticket.status == TicketStatus::Escalated {
                ensure_root(origin.clone()).map_err(|_| Error::<T>::EscalatedTicketRequiresRoot)?;
            } else {
                // Ticket must still be Open or InReview to be resolved.
                ensure!(
                    ticket.status == TicketStatus::Open || ticket.status == TicketStatus::InReview,
                    Error::<T>::InvalidTicketStatus
                );
            }

            // ── Authority check and resolver identity ─────────────────────────
            // For Government targets and Escalated tickets, we require Root.
            // For others, we require Signed.
            let resolver: Option<T::AccountId> = match &ticket.target {
                FeedbackTarget::Government(_branch) => {
                    // Root authority: no AccountId available.
                    ensure_root(origin).map_err(|_| Error::<T>::NotAuthorised)?;
                    None
                }
                _signed_target => {
                    // Escalated already consumed root above; for normal tickets check signed.
                    if ticket.status == TicketStatus::Escalated {
                        // Already verified Root above; skip signed auth.
                        None
                    } else {
                        let caller = ensure_signed(origin)?;
                        Self::ensure_signed_target_authority(&caller, &ticket.target)?;
                        Some(caller)
                    }
                }
            };

            // ── Deposit enforcement ───────────────────────────────────────────
            let author = ticket.author.clone();
            let deposit = ticket.deposit;

            if is_helpful {
                // Helpful ticket: return the deposit to the citizen.
                T::Currency::unreserve(&author, deposit);

                Self::deposit_event(Event::TicketResolved {
                    ticket_id,
                    resolver,
                    resolution_hash,
                    deposit_returned: deposit,
                });
            } else {
                // Spam / slander: slash the deposit (burn the imbalance).
                let (slashed, _remainder) = T::Currency::slash_reserved(&author, deposit);
                // Imbalance `slashed` is dropped here → tokens are burned.
                drop(slashed);

                Self::deposit_event(Event::TicketRejected {
                    ticket_id,
                    resolver,
                    resolution_hash,
                    deposit_slashed: deposit,
                });
            }

            // ── Update ticket status ──────────────────────────────────────────
            let new_status = if is_helpful {
                TicketStatus::Resolved
            } else {
                TicketStatus::Rejected
            };

            Tickets::<T>::try_mutate(ticket_id, |maybe| -> DispatchResult {
                let t = maybe.as_mut().ok_or(Error::<T>::TicketNotFound)?;
                t.status = new_status;
                Ok(())
            })?;

            Ok(())
        }

        // ─── mark_in_review ──────────────────────────────────────────────────

        /// Mark a ticket as InReview — signals to the citizen that action is being taken.
        ///
        /// Same authority rules as `resolve_ticket`. Does not touch the deposit.
        ///
        /// # Origin: Signed (Guild Master or Entity) / Root (Government)
        #[pallet::call_index(2)]
        #[pallet::weight(T::WeightInfo::mark_in_review())]
        pub fn mark_in_review(origin: OriginFor<T>, ticket_id: u32) -> DispatchResult {
            let ticket = Tickets::<T>::get(ticket_id).ok_or(Error::<T>::TicketNotFound)?;

            ensure!(
                ticket.status == TicketStatus::Open,
                Error::<T>::InvalidTicketStatus
            );

            let reviewer: Option<T::AccountId> = match &ticket.target {
                FeedbackTarget::Government(_branch) => {
                    ensure_root(origin).map_err(|_| Error::<T>::NotAuthorised)?;
                    None
                }
                _signed_target => {
                    let caller = ensure_signed(origin)?;
                    Self::ensure_signed_target_authority(&caller, &ticket.target)?;
                    Some(caller)
                }
            };

            Tickets::<T>::try_mutate(ticket_id, |maybe| -> DispatchResult {
                let t = maybe.as_mut().ok_or(Error::<T>::TicketNotFound)?;
                t.status = TicketStatus::InReview;
                Ok(())
            })?;

            Self::deposit_event(Event::TicketInReview {
                ticket_id,
                reviewer,
            });
            Ok(())
        }

        // ─── escalate_ticket ─────────────────────────────────────────────────

        /// [VECTOR 2] Escalate a ticket for constitutional review.
        ///
        /// Only the **ticket's author** can escalate.
        /// Once escalated, only Root can resolve the ticket (the accused cannot close it).
        /// The anti-spam deposit is **NOT** burned or touched on escalation.
        ///
        /// ## Use Case
        /// The accused manager/Guild Master attempted to close a complaint against
        /// themselves. The author escalates to prevent self-resolution.
        ///
        /// # Origin: Signed (must be the ticket author)
        #[pallet::call_index(3)]
        #[pallet::weight(T::WeightInfo::escalate_ticket())]
        pub fn escalate_ticket(origin: OriginFor<T>, ticket_id: u32) -> DispatchResult {
            let caller = ensure_signed(origin)?;

            let ticket = Tickets::<T>::get(ticket_id).ok_or(Error::<T>::TicketNotFound)?;

            // Only the author can escalate.
            ensure!(caller == ticket.author, Error::<T>::OnlyAuthorCanEscalate);

            // Ticket must be Open or InReview.
            ensure!(
                ticket.status == TicketStatus::Open || ticket.status == TicketStatus::InReview,
                if ticket.status == TicketStatus::Escalated {
                    Error::<T>::TicketAlreadyEscalated
                } else {
                    Error::<T>::InvalidTicketStatus
                }
            );

            // Escalate — deposit remains reserved, safe from burning.
            Tickets::<T>::try_mutate(ticket_id, |maybe| -> DispatchResult {
                let t = maybe.as_mut().ok_or(Error::<T>::TicketNotFound)?;
                t.status = TicketStatus::Escalated;
                Ok(())
            })?;

            Self::deposit_event(Event::TicketEscalated {
                ticket_id,
                author: caller,
            });

            Ok(())
        }

        // ─── request_sting_operation ──────────────────────────────────────────

        /// [VECTOR 0] Register intent to perform a controlled bribe operation.
        ///
        /// The citizen reserves `amount` from their balance as proof of funds.
        /// A `commit_hash` = blake2_256(amount_le_bytes || secret_salt) is stored
        /// on-chain — the salt (and thus the amount) remains hidden from the public
        /// until the Prosecutor springs the trap.
        ///
        /// ## Prerequisites
        /// 1. Citizen is being extorted / asked for a bribe by `target`.
        /// 2. Citizen computes `commit_hash` = blake2_256(amount_as_16_le_bytes ++ salt).
        /// 3. Citizen calls this extrinsic, reserving `amount` as collateral.
        ///
        /// After approval by the Prosecutor (Root), the citizen makes the real
        /// transfer outside the chain (or via the normal `transfer` extrinsic) so
        /// the target does not suspect entrapment.
        ///
        /// # Origin: Signed
        #[pallet::call_index(4)]
        #[pallet::weight(T::WeightInfo::request_sting_operation())]
        pub fn request_sting_operation(
            origin: OriginFor<T>,
            target: T::AccountId,
            amount: BalanceOf<T>,
            commit_hash: [u8; 32],
        ) -> DispatchResult {
            let citizen = ensure_signed(origin)?;

            // Reserve the bribe amount as collateral (proves the citizen has the funds).
            T::Currency::reserve(&citizen, amount).map_err(|_| Error::<T>::InsufficientBalance)?;

            let sting_id = NextStingId::<T>::get();

            StingOperations::<T>::insert(
                sting_id,
                StingOperation::<T> {
                    citizen: citizen.clone(),
                    target: target.clone(),
                    amount,
                    commit_hash,
                    status: StingStatus::Pending,
                },
            );

            NextStingId::<T>::put(sting_id.saturating_add(1));

            Self::deposit_event(Event::StingRequested {
                sting_id,
                citizen,
                target,
                commit_hash,
            });

            Ok(())
        }

        // ─── approve_sting ────────────────────────────────────────────────────

        /// [VECTOR 0] Prosecutor approves a pending sting operation.
        ///
        /// **Origin: Root only** (Constitutional Prosecutor / Sudo).
        ///
        /// This grants the citizen **prosecutorial immunity** — they are protected
        /// from any criminal liability for the controlled transfer to the target.
        /// The operation status changes from `Pending` → `Approved`.
        ///
        /// # Origin: Root
        #[pallet::call_index(5)]
        #[pallet::weight(T::WeightInfo::approve_sting())]
        pub fn approve_sting(origin: OriginFor<T>, sting_id: u64) -> DispatchResult {
            ensure_root(origin)?;

            let mut op = StingOperations::<T>::get(sting_id).ok_or(Error::<T>::StingNotFound)?;

            ensure!(op.status == StingStatus::Pending, {
                if op.status == StingStatus::Approved {
                    Error::<T>::StingAlreadyApproved
                } else {
                    Error::<T>::StingAlreadySprung
                }
            });

            op.status = StingStatus::Approved;
            StingOperations::<T>::insert(sting_id, &op);

            Self::deposit_event(Event::StingApproved {
                sting_id,
                citizen: op.citizen,
                target: op.target,
            });

            Ok(())
        }

        // ─── reveal_and_spring_trap ──────────────────────────────────────────

        /// [VECTOR 0] Spring the trap: verify the commit-reveal, exile the target,
        /// and pay the Whistleblower Reward.
        ///
        /// **Origin: Root only** (Prosecutor).
        ///
        /// ## Verification
        /// Computes blake2_256(`amount` as 16-byte little-endian || `secret_salt`) and
        /// checks it matches the `commit_hash` stored on-chain.
        ///
        /// ## Effects
        /// 1. Target is exiled via `T::BlackBookBridge::exile_citizen`.
        /// 2. Citizen's reserved `amount` is unreserved (returned).
        /// 3. Whistleblower Reward = `amount × WhistleblowerRewardPercent / 100` is
        ///    transferred from `T::StateTreasury` to the citizen.
        /// 4. Sting status set to `Sprung`.
        ///
        /// # Origin: Root
        #[pallet::call_index(6)]
        #[pallet::weight(T::WeightInfo::reveal_and_spring_trap())]
        pub fn reveal_and_spring_trap(
            origin: OriginFor<T>,
            sting_id: u64,
            secret_salt: [u8; 32],
        ) -> DispatchResult {
            ensure_root(origin)?;

            let mut op = StingOperations::<T>::get(sting_id).ok_or(Error::<T>::StingNotFound)?;

            // Must be Approved before springing.
            ensure!(op.status == StingStatus::Approved, {
                if op.status == StingStatus::Pending {
                    Error::<T>::StingNotApproved
                } else {
                    Error::<T>::StingAlreadySprung
                }
            });

            // ── Commit-Reveal Verification ────────────────────────────────────
            // Reconstruct the hash: blake2_256(amount_u128_le || secret_salt)
            let amount_bytes: u128 = op.amount.try_into().unwrap_or(u128::MAX);
            let mut preimage = [0u8; 48]; // 16 bytes amount + 32 bytes salt
            preimage[..16].copy_from_slice(&amount_bytes.to_le_bytes());
            preimage[16..].copy_from_slice(&secret_salt);
            let computed_hash = blake2_256(&preimage);

            ensure!(computed_hash == op.commit_hash, Error::<T>::InvalidReveal);

            // ── Step 1: Exile the target ──────────────────────────────────────
            T::BlackBookBridge::exile_citizen(&op.target)?;

            // ── Step 2: Return reserved amount to citizen ─────────────────────
            T::Currency::unreserve(&op.citizen, op.amount);

            // ── Step 3: Pay Whistleblower Reward from Treasury ────────────────
            let reward_percent = T::WhistleblowerRewardPercent::get() as u128;
            let amount_u128: u128 = op.amount.try_into().unwrap_or(0u128);
            let reward_u128 = amount_u128.saturating_mul(reward_percent) / 100;

            // Convert back to BalanceOf<T>
            let reward: BalanceOf<T> = reward_u128.try_into().unwrap_or(BalanceOf::<T>::default());

            let whistleblower_reward = if reward > BalanceOf::<T>::default() {
                T::Currency::transfer(
                    &T::StateTreasury::get(),
                    &op.citizen,
                    reward,
                    ExistenceRequirement::KeepAlive,
                )
                .map_err(|_| Error::<T>::TreasuryInsufficientFunds)?;
                reward
            } else {
                BalanceOf::<T>::default()
            };

            // ── Step 4: Mark as Sprung ────────────────────────────────────────
            op.status = StingStatus::Sprung;
            StingOperations::<T>::insert(sting_id, &op);

            Self::deposit_event(Event::TrapSprung {
                sting_id,
                target: op.target,
                whistleblower: op.citizen,
                amount_returned: op.amount,
                whistleblower_reward,
            });

            Ok(())
        }
    }
}
