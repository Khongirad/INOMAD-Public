//! # Pallet Chronicles (Летописи Государства)
//!
//! **Altan Network — Sovereign L1 Blockchain**
//! **Sprint L1-22: Decentralised Intellectual Property Registry**
//!
//! ## Overview
//!
//! `pallet-chronicles` implements an **immutable, decentralised registry** for
//! intellectual property produced by citizens of the Altan State.  Every
//! document — scientific paper, media work, historical record, or law — is
//! anchored on-chain by two identifiers:
//!
//! | Field          | Purpose                                                         |
//! |----------------|-----------------------------------------------------------------|
//! | `content_hash` | `H256` Blake2 / SHA-256 digest of the raw file.  Proof-of-Existence; ZKP-ready. |
//! | `ipfs_cid`     | IPFS Content Identifier (`Qm…` or `bafy…`) pointing to the off-chain file. |
//!
//! The **hybrid model** (on-chain hashes + off-chain IPFS) guarantees:
//! - Immutability and Proof-of-Existence without storing full files on L1.
//! - Censorship resistance via the IPFS network.
//! - Authorship attribution via `T::AccountId`.
//!
//! ## Anti-Plagiarism
//!
//! The same `content_hash` cannot be registered twice.  Any attempt returns
//! [`Error::DocumentAlreadyExists`].  This gives the first publisher provable
//! priority on that content digest.
//!
//! ## Web3 Tipping Economy (Altans)
//!
//! `donate_to_author` lets any citizen send ALTAN directly to an author
//! as a tip for their work.  The chain enforces the transfer atomically so
//! neither party can cheat.
//!
//! ## Categories
//!
//! | Category  | Scope                                      |
//! |-----------|--------------------------------------------|
//! | `Science` | Research, academic papers, patents          |
//! | `Media`   | Journalism, art, music, film               |
//! | `History` | Chronicles, oral traditions, archival docs  |
//! | `Law`     | Constitutional texts, regulations, decrees  |
//!
//! ## Dispatchables
//!
//! | Call | Origin | Description |
//! |---|---|---|
//! | `publish_document` | Signed (Author) | Publish an immutable document to the on-chain chronicle |
//! | `donate_to_author` | Signed | Send ALTAN tip to a document's author as recognition |

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
    use alloc::vec::Vec;
    use frame_support::{
        pallet_prelude::*,
        traits::{Currency, ExistenceRequirement},
    };
    use frame_system::pallet_prelude::*;
    use sp_core::H256;

    // =========================================================================
    // Type Aliases
    // =========================================================================

    /// Convenience alias for the currency balance type.
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
        /// Maximum byte-length of an IPFS CID stored on-chain.
        /// Typical CIDv0 = 46 bytes (`Qm…`), CIDv1 = up to 59 bytes (`bafy…`).
        /// Set to 64 to accommodate both formats with a small safety margin.
        #[pallet::constant]
        type MaxCidLength: Get<u32>;

        /// The currency used for citizen-to-author donations (ALTAN).
        type Currency: Currency<Self::AccountId>;
    }

    // =========================================================================
    // Enums
    // =========================================================================

    /// Category of an intellectual-property document published to the Chronicles.
    ///
    /// | Variant   | Description                                      |
    /// |-----------|--------------------------------------------------|
    /// | `Science` | Peer-reviewed research, patents, academic papers |
    /// | `Media`   | Journalism, art, film, music, photography        |
    /// | `History` | Chronicles, oral traditions, archival records    |
    /// | `Law`     | Constitutional texts, decrees, legal instruments |
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
    pub enum DocumentCategory {
        /// Scientific research, academic papers, patents.
        Science,
        /// Media productions: journalism, art, music, film.
        Media,
        /// Historical records, chronicles, oral traditions.
        History,
        /// Legal instruments: constitutional texts, regulations, decrees.
        Law,
    }

    // =========================================================================
    // Structs
    // =========================================================================

    /// On-chain record for a document published to the Altan Chronicles.
    ///
    /// Stored in [`Documents`] indexed by `content_hash`.
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
    pub struct DocumentRecord<T: Config> {
        /// The AccountId of the author — the sovereign owner of this work.
        pub owner: T::AccountId,
        /// Blake2 / SHA-256 hash of the raw file content.
        /// Acts as Proof-of-Existence and enables ZKP verification off-chain.
        /// **Uniqueness enforced**: no two documents may share the same hash.
        pub content_hash: H256,
        /// IPFS Content Identifier pointing to the off-chain file.
        /// Bounded to [`Config::MaxCidLength`] bytes (64 by default).
        pub ipfs_cid: BoundedVec<u8, T::MaxCidLength>,
        /// Thematic category of the document.
        pub category: DocumentCategory,
        /// Block number at which the document was published.
        /// Serves as an on-chain timestamp for priority disputes.
        pub block_number: BlockNumberFor<T>,
    }

    // =========================================================================
    // Storage
    // =========================================================================

    /// Main Chronicles registry: content_hash → DocumentRecord.
    ///
    /// Indexed by the file's `H256` content hash so that any holder of the
    /// original file can independently verify its on-chain registration.
    #[pallet::storage]
    pub type Documents<T: Config> =
        StorageMap<_, Blake2_128Concat, H256, DocumentRecord<T>, OptionQuery>;

    /// Author index: (AccountId, content_hash) → ().
    ///
    /// Allows efficient enumeration of all documents published by a given author
    /// without scanning the entire [`Documents`] map.
    #[pallet::storage]
    pub type AuthorDocuments<T: Config> =
        StorageDoubleMap<_, Twox64Concat, T::AccountId, Blake2_128Concat, H256, (), OptionQuery>;

    // =========================================================================
    // Events
    // =========================================================================

    #[pallet::event]
    #[pallet::generate_deposit(pub(super) fn deposit_event)]
    pub enum Event<T: Config> {
        /// A new document has been published to the Altan Chronicles.
        ///
        /// `[owner, content_hash, category]`
        DocumentPublished {
            /// The author / owner of the published work.
            owner: T::AccountId,
            /// Blake2 / SHA-256 hash of the document content.
            content_hash: H256,
            /// Thematic category of the published work.
            category: DocumentCategory,
        },

        /// A citizen donated ALTAN to an author as a tip for their work.
        ///
        /// `[donor, author, document_hash, amount]`
        DonatedToAuthor {
            /// The citizen who sent the donation.
            donor: T::AccountId,
            /// The author who received the donation.
            author: T::AccountId,
            /// Hash of the document that motivated the donation.
            document_hash: H256,
            /// Amount of ALTAN transferred.
            amount: BalanceOf<T>,
        },
    }

    // =========================================================================
    // Errors
    // =========================================================================

    #[pallet::error]
    pub enum Error<T> {
        /// A document with this `content_hash` already exists in the Chronicles.
        ///
        /// The first publisher holds provable priority on that content digest.
        /// Duplicate registrations constitute plagiarism within the network.
        DocumentAlreadyExists,

        /// No document with the given `document_hash` found in the Chronicles.
        DocumentNotFound,

        /// The supplied IPFS CID exceeds [`Config::MaxCidLength`] bytes.
        CidTooLong,
    }

    // =========================================================================
    // Extrinsics
    // =========================================================================

    #[pallet::call]
    impl<T: Config> Pallet<T> {
        /// Publish a new intellectual-property document to the Altan Chronicles.
        ///
        /// # Anti-plagiarism
        ///
        /// The `content_hash` must be globally unique within the Chronicles.
        /// Any duplicate returns [`Error::DocumentAlreadyExists`].
        ///
        /// # IPFS Anchoring
        ///
        /// The `ipfs_cid` stores only the Content Identifier; the actual file
        /// lives off-chain.  Together with `content_hash` this provides a
        /// tamper-evident, censorship-resistant provenance chain.
        ///
        /// # Emits
        ///
        /// [`Event::DocumentPublished`] on success.
        #[pallet::call_index(0)]
        #[pallet::weight(Weight::from_parts(50_000_000, 0)
            .saturating_add(T::DbWeight::get().reads(1))
            .saturating_add(T::DbWeight::get().writes(2)))]
        pub fn publish_document(
            origin: OriginFor<T>,
            content_hash: H256,
            ipfs_cid: Vec<u8>,
            category: DocumentCategory,
        ) -> DispatchResult {
            // ── 1. Authenticate ───────────────────────────────────────────────
            let owner = ensure_signed(origin)?;

            // ── 2. Anti-plagiarism guard ──────────────────────────────────────
            ensure!(
                !Documents::<T>::contains_key(content_hash),
                Error::<T>::DocumentAlreadyExists
            );

            // ── 3. Bound the IPFS CID ─────────────────────────────────────────
            let bounded_cid = BoundedVec::<u8, T::MaxCidLength>::try_from(ipfs_cid)
                .map_err(|_| Error::<T>::CidTooLong)?;

            // ── 4. Build the record ───────────────────────────────────────────
            let record = DocumentRecord::<T> {
                owner: owner.clone(),
                content_hash,
                ipfs_cid: bounded_cid,
                category: category.clone(),
                block_number: <frame_system::Pallet<T>>::block_number(),
            };

            // ── 5. Write to storage ───────────────────────────────────────────
            Documents::<T>::insert(content_hash, record);
            AuthorDocuments::<T>::insert(&owner, content_hash, ());

            // ── 6. Emit event ─────────────────────────────────────────────────
            Self::deposit_event(Event::DocumentPublished {
                owner,
                content_hash,
                category,
            });

            Ok(())
        }

        /// Donate ALTAN to the author of a published document.
        ///
        /// Any citizen of the Altan Network may tip an author for their
        /// intellectual contribution.  The transfer is atomic — either the
        /// full `amount` moves from donor to author, or the call fails.
        ///
        /// # Panics / Errors
        ///
        /// - [`Error::DocumentNotFound`] — no document with `document_hash` exists.
        /// - Standard currency errors (insufficient balance, below existential deposit).
        ///
        /// # Emits
        ///
        /// [`Event::DonatedToAuthor`] on success.
        #[pallet::call_index(1)]
        #[pallet::weight(Weight::from_parts(50_000_000, 0)
            .saturating_add(T::DbWeight::get().reads(1))
            .saturating_add(T::DbWeight::get().writes(2)))]
        pub fn donate_to_author(
            origin: OriginFor<T>,
            document_hash: H256,
            amount: BalanceOf<T>,
        ) -> DispatchResult {
            // ── 1. Authenticate ───────────────────────────────────────────────
            let donor = ensure_signed(origin)?;

            // ── 2. Resolve the author ─────────────────────────────────────────
            let record = Documents::<T>::get(document_hash).ok_or(Error::<T>::DocumentNotFound)?;
            let author = record.owner;

            // ── 3. Transfer ALTAN: donor → author ─────────────────────────────
            T::Currency::transfer(&donor, &author, amount, ExistenceRequirement::KeepAlive)?;

            // ── 4. Emit event ─────────────────────────────────────────────────
            Self::deposit_event(Event::DonatedToAuthor {
                donor,
                author,
                document_hash,
                amount,
            });

            Ok(())
        }
    }
}
