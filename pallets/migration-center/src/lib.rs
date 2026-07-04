//! # INOMAD Migration Center Pallet
//!
//! **Altan Network — Sovereign L1 Blockchain**
//!
//! ## Purpose
//!
//! This pallet provides **immutable on-chain anchoring** for migration
//! (citizenship) applications. Even if the off-chain database is lost or
//! wiped, the following facts remain verifiable forever on Altan L1:
//!
//! - Which wallet submitted a citizenship application (and the Blake2b-256
//!   hash of its data).
//! - When the application was submitted (block number + timestamp hash).
//! - Which officer (guarantor) claimed the application and their sr25519
//!   signature of the claim message.
//! - The final outcome hash (APPROVED or REJECTED with review notes hash).
//!
//! ## Data Model
//!
//! ```text
//! ApplicationAnchor {
//!     application_id_hash : H256,  // blake2_256(off-chain UUID)
//!     applicant           : AccountId,
//!     data_hash           : H256,  // blake2_256(fullName|birthPlace|region|regionCode)
//!     submitted_at_block  : BlockNumber,
//!     status              : AnchorStatus,
//!     officer             : Option<AccountId>,
//!     officer_claim_sig   : Option<[u8; 64]>,  // sr25519 sig of claim message
//!     outcome_hash        : Option<H256>,       // blake2_256(decision|notes)
//! }
//! ```
//!
//! ## Dispatchables
//!
//! | Call | Origin | Description |
//! |---|---|---|
//! | `submit_application` | Signed (Applicant) | Anchor new citizenship application on-chain |
//! | `claim_application` | Signed (Officer) | Officer claims app + records sr25519 signature |
//! | `finalize_application` | Signed (Officer) | Record final APPROVED/REJECTED outcome hash |
//! | `revoke_application` | Root | Emergency revocation (court order) |
//!
//! ## Security Invariants
//!
//! - One active application per wallet at any time.
//! - Only the claiming officer may finalize their own claimed application.
//! - Finalized applications are IMMUTABLE (status terminal).
//! - All signatures are verified on-chain via `sp_io::crypto::sr25519_verify`.

#![cfg_attr(not(feature = "std"), no_std)]

extern crate alloc;

pub use pallet::*;

#[cfg(test)]
mod mock;

#[cfg(test)]
mod tests;

/// Runtime migration hooks — required by the `Migrations` tuple in `runtime/src/lib.rs`.
///
/// `NoopMigration` is used when the pallet was just added (no storage to migrate).
/// Replace with a versioned `vN::Migration<T>` when you need to transform existing data.
pub mod migrations {
    use frame_support::weights::Weight;

    pub struct NoopMigration<T>(core::marker::PhantomData<T>);

    impl<T: super::Config> frame_support::traits::OnRuntimeUpgrade for NoopMigration<T> {
        fn on_runtime_upgrade() -> Weight {
            Weight::zero()
        }
    }
}

#[frame_support::pallet]
pub mod pallet {
    use alloc::vec::Vec;
    use frame_support::pallet_prelude::*;
    use frame_system::pallet_prelude::*;
    use sp_core::{sr25519, H256};
    use sp_io::crypto::sr25519_verify;

    // =========================================================================
    // Enums
    // =========================================================================

    /// Lifecycle status of an on-chain migration application anchor.
    ///
    /// State machine:
    /// ```text
    /// [Submitted] ──claim──▶ [UnderReview] ──approve──▶ [Approved]  (TERMINAL)
    ///                                       ──reject──▶  [Rejected]  (TERMINAL)
    ///                                       ──revoke──▶  [Revoked]   (TERMINAL, Root only)
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
    pub enum AnchorStatus {
        /// Application submitted by citizen — waiting for officer assignment.
        Submitted,
        /// An officer has claimed the application and is reviewing it.
        UnderReview,
        /// Officer approved — citizen identity verified.
        Approved,
        /// Officer rejected — with reason hash recorded.
        Rejected,
        /// Root-level emergency revocation (court order).
        Revoked,
    }

    // =========================================================================
    // Structs
    // =========================================================================

    /// Immutable on-chain anchor for a migration (citizenship) application.
    ///
    /// The anchor is keyed by `applicant: AccountId` — one active anchor per wallet.
    /// Sensitive personal data NEVER touches the chain; only hashes are stored.
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
    pub struct ApplicationAnchor<T: Config> {
        /// `blake2_256(off_chain_uuid)` — links anchor to the off-chain DB record.
        /// Even if DB is wiped, this hash can be used to re-identify the record.
        pub application_id_hash: H256,

        /// `blake2_256(fullName | "|" | birthPlace | "|" | regionName | "|" | regionCode)`
        /// Proves the DATA submitted by the citizen without exposing PII on-chain.
        pub data_hash: H256,

        /// Block number when the citizen submitted the application.
        /// Provides an immutable timestamp anchor.
        pub submitted_at_block: BlockNumberFor<T>,

        /// Current lifecycle status.
        pub status: AnchorStatus,

        /// Officer (guarantor) who claimed this application.
        /// `None` until an officer calls `claim_application`.
        pub officer: Option<T::AccountId>,

        /// sr25519 signature by the officer over the claim message.
        ///
        /// Message format (UTF-8):
        /// `"INOMAD:CLAIM:{application_id_hash_hex}:{block_number}"`
        ///
        /// Encoded as 64-byte raw sr25519 signature.
        /// `None` until `claim_application` is called.
        pub officer_claim_sig: Option<BoundedVec<u8, ConstU32<64>>>,

        /// Block when the officer claimed the application.
        pub claimed_at_block: Option<BlockNumberFor<T>>,

        /// `blake2_256(outcome | "|" | review_notes)`
        /// Provides proof of the decision content without exposing notes on-chain.
        /// `None` until `finalize_application` is called.
        pub outcome_hash: Option<H256>,

        /// Block when the final decision was recorded.
        pub finalized_at_block: Option<BlockNumberFor<T>>,
    }

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
        /// The maximum number of officer accounts that can be registered.
        /// Prevents unbounded storage growth if we store officer lists.
        #[pallet::constant]
        type MaxOfficers: Get<u32>;
    }

    // =========================================================================
    // Storage
    // =========================================================================

    /// Primary storage: ApplicationAnchor keyed by applicant AccountId.
    ///
    /// One active anchor per wallet. Attempting to submit when an anchor
    /// already exists returns `Error::ApplicationAlreadyExists`.
    #[pallet::storage]
    #[pallet::getter(fn application_anchor)]
    pub type ApplicationAnchors<T: Config> = StorageMap<
        _,
        Blake2_128Concat,
        T::AccountId,
        ApplicationAnchor<T>,
        OptionQuery,
    >;

    /// Secondary index: application_id_hash → applicant AccountId.
    ///
    /// Allows lookup by off-chain UUID hash without scanning all anchors.
    /// Used by the backend L1 verifier service.
    #[pallet::storage]
    #[pallet::getter(fn hash_to_applicant)]
    pub type HashToApplicant<T: Config> = StorageMap<
        _,
        Blake2_128Concat,
        H256,
        T::AccountId,
        OptionQuery,
    >;

    /// Counter: total applications ever submitted (immutable audit counter).
    #[pallet::storage]
    #[pallet::getter(fn total_applications)]
    pub type TotalApplications<T: Config> = StorageValue<_, u64, ValueQuery>;

    /// Counter: total applications approved (sovereign citizenship grants).
    #[pallet::storage]
    #[pallet::getter(fn total_approved)]
    pub type TotalApproved<T: Config> = StorageValue<_, u64, ValueQuery>;

    // =========================================================================
    // Events
    // =========================================================================

    #[pallet::event]
    #[pallet::generate_deposit(pub(super) fn deposit_event)]
    pub enum Event<T: Config> {
        /// A citizen submitted a migration application anchor on-chain.
        ///
        /// Fields: [applicant, application_id_hash, data_hash, block]
        ApplicationSubmitted {
            applicant: T::AccountId,
            application_id_hash: H256,
            data_hash: H256,
            block: BlockNumberFor<T>,
        },

        /// An officer claimed an application — their signature is recorded.
        ///
        /// Fields: [officer, applicant, application_id_hash, block]
        ApplicationClaimed {
            officer: T::AccountId,
            applicant: T::AccountId,
            application_id_hash: H256,
            block: BlockNumberFor<T>,
        },

        /// An officer finalized an application (APPROVED or REJECTED).
        ///
        /// Fields: [officer, applicant, application_id_hash, approved, outcome_hash]
        ApplicationFinalized {
            officer: T::AccountId,
            applicant: T::AccountId,
            application_id_hash: H256,
            approved: bool,
            outcome_hash: H256,
        },

        /// Root emergency revocation.
        ApplicationRevoked {
            applicant: T::AccountId,
            application_id_hash: H256,
        },
    }

    // =========================================================================
    // Errors
    // =========================================================================

    #[pallet::error]
    pub enum Error<T> {
        /// Applicant already has an active (non-terminal) application anchor.
        ApplicationAlreadyExists,
        /// No application anchor found for this wallet.
        ApplicationNotFound,
        /// Application is not in `Submitted` status — cannot be claimed.
        NotSubmitted,
        /// Application is not in `UnderReview` status — cannot be finalized.
        NotUnderReview,
        /// Only the officer who claimed the application may finalize it.
        NotTheClaimingOfficer,
        /// Application is already in a terminal state (Approved/Rejected/Revoked).
        AlreadyFinalized,
        /// The provided officer signature failed sr25519 verification.
        InvalidOfficerSignature,
        /// The application_id_hash is already registered (duplicate UUID hash).
        DuplicateApplicationHash,
        /// Signature bytes have invalid length (must be exactly 64 bytes).
        InvalidSignatureLength,
    }

    // =========================================================================
    // Calls (Extrinsics)
    // =========================================================================

    #[pallet::call]
    impl<T: Config> Pallet<T> {
        /// Submit a migration application anchor on Altan L1.
        ///
        /// Called by the INOMAD backend (`migration-service`) after the citizen
        /// fills the form and signs the submission with SubWallet. The backend
        /// computes both hashes off-chain and sends this extrinsic signed by
        /// the citizen's wallet.
        ///
        /// ## Hash Construction
        ///
        /// ```text
        /// application_id_hash = blake2_256(utf8(off_chain_uuid))
        /// data_hash           = blake2_256(utf8(
        ///   fullName + "|" + birthPlace + "|" + regionName + "|" + regionCode
        /// ))
        /// ```
        ///
        /// ## Invariants
        ///
        /// - One active anchor per wallet (returns `ApplicationAlreadyExists` if
        ///   a non-terminal anchor already exists).
        /// - `application_id_hash` must be globally unique.
        #[pallet::call_index(0)]
        #[pallet::weight(Weight::from_parts(10_000, 0) + T::DbWeight::get().writes(3))]
        pub fn submit_application(
            origin: OriginFor<T>,
            application_id_hash: H256,
            data_hash: H256,
        ) -> DispatchResult {
            let applicant = ensure_signed(origin)?;
            let block = frame_system::Pallet::<T>::block_number();

            // Guard: no duplicate application_id_hash
            ensure!(
                !HashToApplicant::<T>::contains_key(&application_id_hash),
                Error::<T>::DuplicateApplicationHash
            );

            // Guard: allow re-submission only if previous anchor is terminal
            if let Some(existing) = ApplicationAnchors::<T>::get(&applicant) {
                let is_terminal = matches!(
                    existing.status,
                    AnchorStatus::Approved | AnchorStatus::Rejected | AnchorStatus::Revoked
                );
                ensure!(is_terminal, Error::<T>::ApplicationAlreadyExists);
            }

            let anchor = ApplicationAnchor::<T> {
                application_id_hash,
                data_hash,
                submitted_at_block: block,
                status: AnchorStatus::Submitted,
                officer: None,
                officer_claim_sig: None,
                claimed_at_block: None,
                outcome_hash: None,
                finalized_at_block: None,
            };

            ApplicationAnchors::<T>::insert(&applicant, anchor);
            HashToApplicant::<T>::insert(&application_id_hash, &applicant);
            TotalApplications::<T>::mutate(|n| *n = n.saturating_add(1));

            Self::deposit_event(Event::ApplicationSubmitted {
                applicant,
                application_id_hash,
                data_hash,
                block,
            });

            Ok(())
        }

        /// Officer claims a migration application.
        ///
        /// The officer signs the message `"INOMAD:CLAIM:{app_id_hash_hex}:{block}"` with
        /// their SubWallet (sr25519) and submits the raw 64-byte signature on-chain.
        ///
        /// The pallet verifies the signature cryptographically. This creates an
        /// **immutable proof** that a specific officer took responsibility for
        /// the application at a specific block.
        ///
        /// ## Signature Message Format
        ///
        /// ```text
        /// message = b"INOMAD:CLAIM:" + hex(application_id_hash) + b":" + decimal(block_number)
        /// ```
        ///
        /// The backend wraps this in `<Bytes>...</Bytes>` per SubWallet convention
        /// before passing to `signRaw`.
        #[pallet::call_index(1)]
        #[pallet::weight(Weight::from_parts(25_000, 0) + T::DbWeight::get().reads_writes(2, 1))]
        pub fn claim_application(
            origin: OriginFor<T>,
            applicant: T::AccountId,
            // Raw sr25519 signature (64 bytes) over the claim message.
            officer_sig: BoundedVec<u8, ConstU32<64>>,
        ) -> DispatchResult {
            let officer = ensure_signed(origin)?;
            let block = frame_system::Pallet::<T>::block_number();

            let mut anchor = ApplicationAnchors::<T>::get(&applicant)
                .ok_or(Error::<T>::ApplicationNotFound)?;

            ensure!(anchor.status == AnchorStatus::Submitted, Error::<T>::NotSubmitted);

            // Verify officer's sr25519 signature on-chain
            // Message: "INOMAD:CLAIM:{app_id_hash_hex}:{block}"
            ensure!(officer_sig.len() == 64, Error::<T>::InvalidSignatureLength);

            let sig_bytes: [u8; 64] = officer_sig
                .as_slice()
                .try_into()
                .map_err(|_| Error::<T>::InvalidSignatureLength)?;

            let app_hash_hex = Self::h256_to_hex(&anchor.application_id_hash);
            let block_num: u64 = frame_system::Pallet::<T>::block_number()
                .try_into()
                .unwrap_or(0u64);
            let message = Self::build_claim_message(&app_hash_hex, block_num);

            // Wrap in <Bytes> as SubWallet does
            let wrapped = Self::wrap_bytes(&message);

            let pub_key_bytes: [u8; 32] = officer
                .encode()
                .try_into()
                .map_err(|_| Error::<T>::InvalidOfficerSignature)?;

            let sr25519_sig = sr25519::Signature::from_raw(sig_bytes);
            let sr25519_pub = sr25519::Public::from_raw(pub_key_bytes);

            let valid = sr25519_verify(&sr25519_sig, &wrapped, &sr25519_pub);
            ensure!(valid, Error::<T>::InvalidOfficerSignature);

            anchor.status = AnchorStatus::UnderReview;
            anchor.officer = Some(officer.clone());
            anchor.officer_claim_sig = Some(officer_sig);
            anchor.claimed_at_block = Some(block);

            ApplicationAnchors::<T>::insert(&applicant, &anchor);

            Self::deposit_event(Event::ApplicationClaimed {
                officer,
                applicant,
                application_id_hash: anchor.application_id_hash,
                block,
            });

            Ok(())
        }

        /// Officer finalizes the application — records APPROVED or REJECTED outcome.
        ///
        /// The `outcome_hash` is:
        /// ```text
        /// blake2_256(outcome_str + "|" + review_notes)
        /// ```
        /// where `outcome_str` is `"APPROVED"` or `"REJECTED"`.
        ///
        /// This permanently records the decision on L1 without exposing
        /// sensitive review notes on-chain.
        ///
        /// Only the officer who originally claimed the application may call this.
        #[pallet::call_index(2)]
        #[pallet::weight(Weight::from_parts(15_000, 0) + T::DbWeight::get().reads_writes(1, 1))]
        pub fn finalize_application(
            origin: OriginFor<T>,
            applicant: T::AccountId,
            approved: bool,
            outcome_hash: H256,
        ) -> DispatchResult {
            let officer = ensure_signed(origin)?;
            let block = frame_system::Pallet::<T>::block_number();

            let mut anchor = ApplicationAnchors::<T>::get(&applicant)
                .ok_or(Error::<T>::ApplicationNotFound)?;

            ensure!(anchor.status == AnchorStatus::UnderReview, Error::<T>::NotUnderReview);

            // Only the claiming officer may finalize
            let claiming_officer = anchor.officer.as_ref().ok_or(Error::<T>::NotUnderReview)?;
            ensure!(*claiming_officer == officer, Error::<T>::NotTheClaimingOfficer);

            anchor.status = if approved {
                AnchorStatus::Approved
            } else {
                AnchorStatus::Rejected
            };
            anchor.outcome_hash = Some(outcome_hash);
            anchor.finalized_at_block = Some(block);

            if approved {
                TotalApproved::<T>::mutate(|n| *n = n.saturating_add(1));
            }

            ApplicationAnchors::<T>::insert(&applicant, &anchor);

            Self::deposit_event(Event::ApplicationFinalized {
                officer,
                applicant,
                application_id_hash: anchor.application_id_hash,
                approved,
                outcome_hash,
            });

            Ok(())
        }

        /// Root-only: Emergency revocation of an application (court order).
        ///
        /// Sets status to `Revoked` — a terminal state. Cannot be undone.
        /// Used when a judicial court issues an order blocking a specific application.
        #[pallet::call_index(3)]
        #[pallet::weight(Weight::from_parts(10_000, 0) + T::DbWeight::get().reads_writes(1, 1))]
        pub fn revoke_application(
            origin: OriginFor<T>,
            applicant: T::AccountId,
        ) -> DispatchResult {
            ensure_root(origin)?;

            let mut anchor = ApplicationAnchors::<T>::get(&applicant)
                .ok_or(Error::<T>::ApplicationNotFound)?;

            let is_terminal = matches!(
                anchor.status,
                AnchorStatus::Approved | AnchorStatus::Rejected | AnchorStatus::Revoked
            );
            ensure!(!is_terminal, Error::<T>::AlreadyFinalized);

            let application_id_hash = anchor.application_id_hash;
            anchor.status = AnchorStatus::Revoked;
            ApplicationAnchors::<T>::insert(&applicant, &anchor);

            Self::deposit_event(Event::ApplicationRevoked {
                applicant,
                application_id_hash,
            });

            Ok(())
        }
    }

    // =========================================================================
    // Internal Helpers
    // =========================================================================

    impl<T: Config> Pallet<T> {
        /// Encode H256 as lowercase hex string (no 0x prefix).
        fn h256_to_hex(h: &H256) -> Vec<u8> {
            let bytes = h.as_bytes();
            let mut hex = alloc::vec::Vec::with_capacity(64);
            for &b in bytes {
                hex.push(b"0123456789abcdef"[(b >> 4) as usize]);
                hex.push(b"0123456789abcdef"[(b & 0xf) as usize]);
            }
            hex
        }

        /// Build the claim message bytes.
        /// Format: `b"INOMAD:CLAIM:" + hex(app_id_hash) + b":" + decimal(block)`
        fn build_claim_message(app_hash_hex: &[u8], block_num: u64) -> Vec<u8> {
            let mut msg = alloc::vec::Vec::new();
            msg.extend_from_slice(b"INOMAD:CLAIM:");
            msg.extend_from_slice(app_hash_hex);
            msg.push(b':');
            // Append decimal block number
            let block_str = alloc::format!("{}", block_num);
            msg.extend_from_slice(block_str.as_bytes());
            msg
        }

        /// Wrap message in `<Bytes>...</Bytes>` as SubWallet does before signRaw.
        fn wrap_bytes(msg: &[u8]) -> Vec<u8> {
            let mut wrapped = alloc::vec::Vec::new();
            wrapped.extend_from_slice(b"<Bytes>");
            wrapped.extend_from_slice(msg);
            wrapped.extend_from_slice(b"</Bytes>");
            wrapped
        }
    }

    // =========================================================================
    // Public Query Interface (for cross-pallet use)
    // =========================================================================

    impl<T: Config> Pallet<T> {
        /// Returns true if the applicant has an active (non-terminal) anchor.
        pub fn has_active_application(applicant: &T::AccountId) -> bool {
            ApplicationAnchors::<T>::get(applicant)
                .map(|a| {
                    !matches!(
                        a.status,
                        AnchorStatus::Approved | AnchorStatus::Rejected | AnchorStatus::Revoked
                    )
                })
                .unwrap_or(false)
        }

        /// Returns true if the applicant's application is APPROVED.
        pub fn is_application_approved(applicant: &T::AccountId) -> bool {
            ApplicationAnchors::<T>::get(applicant)
                .map(|a| a.status == AnchorStatus::Approved)
                .unwrap_or(false)
        }

        /// Lookup applicant by application_id_hash (off-chain UUID hash).
        pub fn applicant_by_hash(hash: &H256) -> Option<T::AccountId> {
            HashToApplicant::<T>::get(hash)
        }
    }
}
