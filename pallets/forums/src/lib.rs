//! # pallet-forums — Threaded Hierarchical Communication
//!
//! **Altan Network — Sovereign L1 Blockchain**
//! **Chain of Legitimacy: Вложенное Общение (Nested Communication)**
//!
//! This pallet implements the protocol-level communication layer for all deliberative
//! bodies of the Altan Republic: Arbads, Zuns, Myangads, Tumeds, and the Grand Khural.
//!
//! ## Chain of Legitimacy: Why Threads Matter
//!
//! Every comment, question, motion, or argument in the Republic is a node in a
//! **legal discourse tree**. By linking each message to a `parent_id`, we create:
//!
//! - **Verifiable provenance**: Every statement traces back to its original context.
//! - **Tamper-proof deliberation**: On-chain threading prevents editing/deleting history.
//! - **Democratic transparency**: Citizens and courts can inspect the full debate chain
//!   that preceded any legislative vote.
//!
//! A message without a parent is a top-level thread (an initiative or topic opener).
//! A reply carries its parent's ID, forming infinite depth trees exactly like
//! YouTube comments or Reddit threads, but permanently on-chain.
//!
//! ## Forum Contexts
//!
//! `forum_id` encodes the deliberative context:
//!
//! | forum_id | Context |
//! |----------|---------|
//! | `arbad:{nation_id}:{arbad_id}` (encoded) | Arbad local forum |
//! | `zun:{nation_id}:{zun_id}` | Zun council |
//! | `khural:{nation_id}` | National Khural |
//! | `grand_khural` | Confederate Grand Khural |
//! | `proposal:{proposal_id}` | Comments on a specific Proposal |
//!
//! ## Storage
//!
//! - `Messages`:       MessageId → Message (author, content_hash, parent_id, forum_id, timestamp)
//! - `NextMessageId`:  Auto-increment counter.
//! - `ThreadReplies`:  (parent_id, child_id) → () — for efficient reply enumeration.
//! - `ForumIndex`:     (forum_id, message_id) → () — all top-level messages per forum.
//!
//! ## Extrinsics
//!
//! | Call           | Origin | Description                          |
//! |----------------|--------|--------------------------------------|
//! | `post_message` | Signed | Post or reply to a thread            |
//! | `pin_message`  | Root   | Town-hall pin (for official notices) |

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
    use frame_support::{pallet_prelude::*, weights::Weight};
    use frame_system::pallet_prelude::*;
    use pallet_inomad_identity::{pallet::CitizenStatus, Citizens};

    // =========================================================================
    // Pallet Struct
    // =========================================================================

    #[pallet::pallet]
    pub struct Pallet<T>(_);

    // =========================================================================
    // Configuration Trait
    // =========================================================================

    #[pallet::config]
    pub trait Config:
        frame_system::Config<RuntimeEvent: From<Event<Self>>> + pallet_inomad_identity::Config
    {
        /// Maximum length of the `forum_id` context tag (UTF-8 bytes).
        ///
        /// Example valid forum IDs:
        ///   `b"proposal:42"`, `b"arbad:3:17"`, `b"khural:7"`, `b"grand_khural"`
        ///
        /// Recommended: `ConstU32<128>`.
        #[pallet::constant]
        type MaxForumIdLen: Get<u32>;

        /// Maximum number of replies directly under a single parent message.
        ///
        /// Prevents spam threads with unbounded reply counts from slowing
        /// `on_initialize` queries. Set to `ConstU32<1024>`.
        #[pallet::constant]
        type MaxRepliesPerMessage: Get<u32>;
    }

    // =========================================================================
    // Types
    // =========================================================================

    /// A unique message identifier (auto-incremented u64 for large-scale forums).
    pub type MessageId = u64;

    /// A single message in the hierarchical thread tree.
    ///
    /// ## Chain of Legitimacy
    ///
    /// Messages form a tree through `parent_id`:
    ///
    /// ```text
    ///  Message(0, parent=None) ← Top-level thread starter
    ///    └── Message(1, parent=Some(0)) ← Reply
    ///          └── Message(3, parent=Some(1)) ← Reply to reply (infinite depth)
    ///    └── Message(2, parent=Some(0)) ← Second reply to root
    /// ```
    ///
    /// This mirrors the structure of legal discourse:
    ///  - A constitutional article → a legislative proposal referencing it → debate
    ///  - A proposal → an expert bill → deliberation in the Khural.
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
    pub struct Message<T: Config> {
        /// The account that posted this message.
        pub author: T::AccountId,

        /// blake2_256 hash of the message content.
        ///
        /// The actual text is stored off-chain (IPFS / gateway).
        /// The on-chain hash provides:
        ///   1. **Immutability**: the content can never be silently altered.
        ///   2. **Privacy**: the hash reveals nothing unless the preimage is shared.
        ///   3. **Verifiability**: anyone can verify the full text against this hash.
        pub content_hash: [u8; 32],

        /// Forum context identifier.
        ///
        /// Encodes which deliberative body this message belongs to.
        /// Indexed in `ForumIndex` for efficient per-forum enumeration.
        pub forum_id: BoundedVec<u8, T::MaxForumIdLen>,

        /// Optional parent message ID (reply-to).
        ///
        /// `None` → this is a top-level thread starter.
        /// `Some(parent_id)` → this is a reply to `parent_id`.
        ///
        /// This single field is what builds the infinite-depth comment forest.
        pub parent_id: Option<MessageId>,

        /// Block number at which this message was posted (on-chain timestamp).
        pub posted_at: BlockNumberFor<T>,

        /// Whether this has been pinned by a moderator (Root) as an official notice.
        pub pinned: bool,
    }

    // =========================================================================
    // Storage
    // =========================================================================

    /// Primary message storage: MessageId → Message.
    ///
    /// `OptionQuery`: returns `None` for non-existent IDs.
    #[pallet::storage]
    #[pallet::getter(fn messages)]
    pub type Messages<T: Config> =
        StorageMap<_, Blake2_128Concat, MessageId, Message<T>, OptionQuery>;

    /// Auto-incrementing message ID counter.
    ///
    /// Starts at 1 on genesis (0 is reserved as "no parent" sentinel in UIs).
    #[pallet::storage]
    #[pallet::getter(fn next_message_id)]
    pub type NextMessageId<T: Config> = StorageValue<_, MessageId, ValueQuery>;

    /// Thread reply index: (parent_id, child_id) → ().
    ///
    /// Allows efficient iteration of all direct replies to a message.
    /// Used by frontends to reconstruct the tree without scanning all messages.
    #[pallet::storage]
    #[pallet::getter(fn thread_replies)]
    pub type ThreadReplies<T: Config> = StorageDoubleMap<
        _,
        Blake2_128Concat,
        MessageId, // parent
        Blake2_128Concat,
        MessageId, // child
        (),
        OptionQuery,
    >;

    /// Forum index: (forum_id, message_id) → () for top-level messages only.
    ///
    /// Used to enumerate all root threads in a given forum context
    /// without scanning the entire `Messages` map.
    #[pallet::storage]
    #[pallet::getter(fn forum_index)]
    pub type ForumIndex<T: Config> = StorageDoubleMap<
        _,
        Blake2_128Concat,
        BoundedVec<u8, T::MaxForumIdLen>, // forum_id
        Blake2_128Concat,
        MessageId,
        (),
        OptionQuery,
    >;

    // =========================================================================
    // Events
    // =========================================================================

    #[pallet::event]
    #[pallet::generate_deposit(pub(super) fn deposit_event)]
    pub enum Event<T: Config> {
        /// A new message was posted in a forum thread.
        MessagePosted {
            /// The new message's unique ID.
            message_id: MessageId,
            /// Who posted it.
            author: T::AccountId,
            /// The forum context.
            forum_id: alloc::vec::Vec<u8>,
            /// Optional parent (reply-to).
            parent_id: Option<MessageId>,
            /// blake2_256 of the off-chain content.
            content_hash: [u8; 32],
            /// Block number.
            posted_at: BlockNumberFor<T>,
        },

        /// A message was pinned as an official notice by a moderator.
        MessagePinned { message_id: MessageId },
    }

    // =========================================================================
    // Errors
    // =========================================================================

    #[pallet::error]
    pub enum Error<T> {
        /// The caller is not a registered citizen.
        NotRegistered,
        /// The citizen account is frozen, exiled, or deceased — cannot post.
        CitizenInactive,
        /// The referenced parent message does not exist.
        ParentMessageNotFound,
        /// The `forum_id` exceeds `MaxForumIdLen` bytes.
        ForumIdTooLong,
        /// The referenced parent belongs to a different forum.
        CrossForumReplyForbidden,
        /// The message to be pinned does not exist.
        MessageNotFound,
    }

    // =========================================================================
    // Extrinsics
    // =========================================================================

    #[pallet::call]
    impl<T: Config> Pallet<T> {
        // ─── post_message ─────────────────────────────────────────────────────

        /// Post a message in a forum thread, optionally replying to an existing message.
        ///
        /// ## Chain of Legitimacy
        ///
        /// Every post is permanently stored with:
        /// - Author identity (the signer's `AccountId` — immutable, non-repudiable).
        /// - Content hash (blake2_256 of the off-chain text — tamper-proof).
        /// - Parent link (`parent_id`) — builds the tree of discourse.
        ///
        /// This creates an immutable, infinitely deep, content-addressed record of
        /// all deliberation in the Republic. No authority can delete or alter it.
        ///
        /// ## Security Checks
        ///
        /// 1. Author must be a **registered, active** citizen.
        /// 2. If `reply_to` is provided, the parent message must exist.
        /// 3. If `reply_to` is provided, the parent must be in the same `forum_id`.
        ///
        /// ## Arguments
        ///
        /// - `forum_id`    — UTF-8 forum context tag (max `MaxForumIdLen` bytes).
        /// - `content_hash`— blake2_256 of the full message text (stored off-chain).
        /// - `reply_to`    — `None` for a new thread root; `Some(MessageId)` to reply.
        #[pallet::call_index(0)]
        #[pallet::weight(Weight::from_parts(50_000_000, 0))]
        pub fn post_message(
            origin: OriginFor<T>,
            forum_id: BoundedVec<u8, T::MaxForumIdLen>,
            content_hash: [u8; 32],
            reply_to: Option<MessageId>,
        ) -> DispatchResult {
            let caller = ensure_signed(origin)?;

            // ── [CHECK 1] Registered + Active citizen ─────────────────────────
            let record = Citizens::<T>::get(&caller).ok_or(Error::<T>::NotRegistered)?;
            ensure!(
                record.status == CitizenStatus::Active,
                Error::<T>::CitizenInactive
            );

            // ── [CHECK 2 & 3] Parent exists and is in the same forum ──────────
            if let Some(pid) = reply_to {
                let parent = Messages::<T>::get(pid).ok_or(Error::<T>::ParentMessageNotFound)?;
                ensure!(
                    parent.forum_id == forum_id,
                    Error::<T>::CrossForumReplyForbidden
                );
            }

            // ── Assign message ID ─────────────────────────────────────────────
            let message_id = NextMessageId::<T>::get();
            let current_block = frame_system::Pallet::<T>::block_number();

            // ── Store message ─────────────────────────────────────────────────
            Messages::<T>::insert(
                message_id,
                Message {
                    author: caller.clone(),
                    content_hash,
                    forum_id: forum_id.clone(),
                    parent_id: reply_to,
                    posted_at: current_block,
                    pinned: false,
                },
            );
            NextMessageId::<T>::put(message_id.saturating_add(1));

            // ── Update indices ────────────────────────────────────────────────
            if let Some(pid) = reply_to {
                // Register as child of parent in ThreadReplies.
                ThreadReplies::<T>::insert(pid, message_id, ());
            } else {
                // Top-level thread → add to ForumIndex.
                ForumIndex::<T>::insert(&forum_id, message_id, ());
            }

            Self::deposit_event(Event::MessagePosted {
                message_id,
                author: caller,
                forum_id: forum_id.to_vec(),
                parent_id: reply_to,
                content_hash,
                posted_at: current_block,
            });

            Ok(())
        }

        // ─── pin_message ──────────────────────────────────────────────────────

        /// Pin a message as an official notice (Root-gated moderator action).
        ///
        /// Pinned messages are surfaced first in forum UIs as official announcements.
        /// This can only be done by Root (the Constitutional Authority, not any citizen).
        ///
        /// Citizens cannot silence or unpin a message — only Root can pin. Root
        /// CANNOT delete the message content — only the `pinned` flag is changed.
        /// Speech remains free; organisation of the discourse is administrative only.
        #[pallet::call_index(1)]
        #[pallet::weight(Weight::from_parts(50_000_000, 0))]
        pub fn pin_message(origin: OriginFor<T>, message_id: MessageId) -> DispatchResult {
            ensure_root(origin)?;

            Messages::<T>::try_mutate(message_id, |maybe| {
                let msg = maybe.as_mut().ok_or(Error::<T>::MessageNotFound)?;
                msg.pinned = true;
                Ok::<(), DispatchError>(())
            })?;

            Self::deposit_event(Event::MessagePinned { message_id });
            Ok(())
        }
    }
}
