//! pallet-chancery — Altan Network: Universal Digital Chancellery
//!
//! Implements the Универсальная Цифровая Канцелярия:
//! Multi-party, multi-signature Ricardian contracts anchored on-chain via
//! Blake2_256 document hashes (H256) pointing to off-chain IPFS storage.
//!
//! CONSTITUTIONAL INTEGRATION:
//! - Documents are stored off-chain (IPFS / Altan Gateway local storage).
//!   On-chain we record ONLY the Blake2_256 hash (= IPFS CID anchor).
//! - Parties sign on-chain via `sign_agreement` — each signature is an
//!   AccountId committing to the document hash.
//! - Professional validators (Lawyers, Notaries, Mediators) must be in
//!   `pallet-guilds` with at least `Professional` rank in their guild.
//! - `raise_dispute` triggers the judicial flow (pallet-judicial-courts).
//!
//! LIFECYCLE:
//!
//! ```text
//!   propose_agreement
//!        │
//!        ▼
//!   [PendingSignatures] ──all parties signed──▶ [PendingValidation]
//!                                               (only if validators set)
//!                  └──all signed, no validators──▶ [Active]
//!   [PendingValidation] ──all validators validated──▶ [Active]
//!   [Active] ──raise_dispute (by party)──▶ [Disputed]
//! ```
//!
//! ## Dispatchables
//!
//! | Call | Origin | Description |
//! |---|---|---|
//! | `propose_agreement` | Signed | Draft a new bilateral agreement and invite counterparty |
//! | `sign_agreement` | Signed (Counterparty) | Countersign and activate a proposed agreement |
//! | `validate_agreement` | Signed (Notary) | Validate agreement terms (notarial certification) |
//! | `raise_dispute` | Signed (Party) | Open a dispute on an active agreement |
//! | `complete_agreement` | Signed (Both Parties) | Mark an agreement as fulfilled by mutual consent |
//! | `annul_signatures` | Signed (Judicial Origin) | Void all signatures on an agreement by court order |

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
// ChanceryGuildsInterface — loose-coupling trait for cross-pallet usage
// =========================================================================

/// Trait that allows `pallet-chancery` to verify that a proposed validator
/// (Lawyer, Notary, or Mediator) holds a qualifying professional rank
/// in `pallet-guilds` without tight-coupling to that pallet's storage.
///
/// Wire it in the runtime's `configs/mod.rs` as:
/// ```ignore
/// pub struct ChanceryGuildsBridge;
/// impl pallet_chancery::ChanceryGuildsInterface<AccountId> for ChanceryGuildsBridge {
///     fn is_valid_validator(who: &AccountId) -> bool { /* read GuildMembers */ }
/// }
/// ```
pub trait ChanceryGuildsInterface<AccountId> {
    /// Returns `true` if `who` holds at least `Professional` rank in any guild.
    ///
    /// In the Altan constitutional framework, Lawyers (guild 1), Notaries (guild 0),
    /// and Mediators (guild 2) are all expected to reach at least Professional level
    /// before they can validate binding legal agreements.
    fn is_valid_validator(who: &AccountId) -> bool;
}

#[cfg(feature = "runtime-benchmarks")]
mod benchmarking;
#[frame_support::pallet]
pub mod pallet {
    use crate::weights::WeightInfo as _;
    use alloc::vec::Vec;
    use frame_support::pallet_prelude::*;
    use frame_system::pallet_prelude::*;
    use sp_core::H256;
    // Import the loose-coupling trait so `T::GuildsChecker::is_valid_validator()`
    // is callable within extrinsics.
    use crate::ChanceryGuildsInterface as _;

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
        /// Maximum number of parties allowed in a single agreement.
        ///
        /// Prevents unbounded `signed_by` vector growth. 10 is sufficient for
        /// most commercial and civil law contracts.
        #[pallet::constant]
        type MaxParties: Get<u32>;

        /// Maximum number of professional validators in a single agreement.
        ///
        /// 5 covers the typical case of: 1 lawyer per party + 1 notary.
        #[pallet::constant]
        type MaxValidators: Get<u32>;

        /// Cross-pallet bridge to `pallet-guilds` for professional rank checks.
        ///
        /// A validator must have at least `Professional` rank in any guild
        /// before they can call `validate_agreement`.
        type GuildsChecker: crate::ChanceryGuildsInterface<Self::AccountId>;
    }

    // =========================================================================
    // Enums
    // =========================================================================

    /// Lifecycle status of a Ricardian Agreement.
    ///
    /// ```text
    /// propose
    ///   │
    ///   ▼
    /// [PendingSignatures] ──all parties signed, no validators──▶ [Active]
    ///        │
    ///        └──all parties signed, validators set──▶ [PendingValidation]
    ///                                                      │
    ///                                        all validated─┘
    ///                                                      ▼
    ///                                                  [Active]
    ///                                                      │
    ///                                     raise_dispute────┘
    ///                                                      ▼
    ///                                                 [Disputed]
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
    pub enum AgreementStatus {
        /// Agreement created; waiting for all parties to sign.
        PendingSignatures,
        /// All parties signed; waiting for professional validators to validate.
        /// Only entered when `validators` is `Some(_)` with at least one entry.
        PendingValidation,
        /// Fully executed — all signatures and validations collected.
        /// This is the legally binding ончейн-activated state.
        Active,
        /// One or more parties raised a dispute. Triggers judicial review.
        /// (pallet-judicial-courts will handle resolution.)
        Disputed,
        /// Agreement has been formally completed/fulfilled by all parties.
        /// Terminal state — cannot be re-opened.
        Completed,
        /// [SECURITY: GHOST STATE] Agreement was annulled because a party transitioned
        /// to a terminal identity state (Deceased or Exiled). Terminal state — cannot
        /// be re-opened. Only `PendingSignatures` agreements are annulled; Active/
        /// Completed agreements remain intact as historical records.
        Cancelled,
    }

    // =========================================================================
    // Structs
    // =========================================================================

    /// A Ricardian Agreement record stored on-chain.
    ///
    /// The full legal document lives off-chain (IPFS / Altan Gateway).
    /// The `document_hash` (Blake2_256 H256) is the cryptographic anchor
    /// that links the immutable on-chain record to the off-chain file.
    ///
    /// Indexing by `document_hash` means:
    ///   - Any party can reconstruct the key from the original file.
    ///   - Tampering with the off-chain document breaks the H256 link.
    ///   - IPFS CID can be derived from the same hash (Blake2b-256 mode).
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
    pub struct Agreement<T: Config> {
        /// Blake2_256 hash of the off-chain document (PDF, Word, etc.).
        ///
        /// This is both the storage key AND an immutable content anchor.
        /// The Altan Gateway computes this hash from the uploaded file and
        /// returns it to the frontend for use as the `document_hash` argument.
        pub document_hash: H256,

        /// AccountId that called `propose_agreement` — the initiating party.
        pub creator: T::AccountId,

        /// All parties to the agreement (counterparties to the contract).
        ///
        /// Every account in this list must call `sign_agreement` before the
        /// status can advance past `PendingSignatures`.
        pub parties: BoundedVec<T::AccountId, T::MaxParties>,

        /// Optional list of invited professional validators (Lawyers, Notaries, Mediators).
        ///
        /// If `None` or empty, the agreement is "simple" — it becomes `Active`
        /// as soon as all parties have signed (no professional validation required).
        ///
        /// If `Some`, all listed validators must call `validate_agreement` AND
        /// possess at least `Professional` guild rank before status → `Active`.
        pub validators: Option<BoundedVec<T::AccountId, T::MaxValidators>>,

        /// Parties who have called `sign_agreement` on this document hash.
        pub signed_by: BoundedVec<T::AccountId, T::MaxParties>,

        /// Validators who have called `validate_agreement` on this document hash.
        pub validated_by: BoundedVec<T::AccountId, T::MaxValidators>,

        /// Current lifecycle status.
        pub status: AgreementStatus,
    }

    // =========================================================================
    // Storage
    // =========================================================================

    /// Agreement registry: DocumentHash → Agreement.
    ///
    /// The document hash (Blake2_256 H256) is used directly as the storage key.
    /// This means:
    ///   - Looking up an agreement by file hash is O(1).
    ///   - The same document cannot be registered twice (deterministic key).
    ///   - Off-chain IPFS retrieval and on-chain lookup use the same identifier.
    #[pallet::storage]
    #[pallet::getter(fn agreements)]
    pub type Agreements<T: Config> =
        StorageMap<_, Blake2_128Concat, H256, Agreement<T>, OptionQuery>;

    // =========================================================================
    // Events
    // =========================================================================

    #[pallet::event]
    #[pallet::generate_deposit(pub(super) fn deposit_event)]
    pub enum Event<T: Config> {
        /// A new Ricardian Agreement was proposed and is pending party signatures.
        AgreementProposed {
            document_hash: H256,
            creator: T::AccountId,
            party_count: u32,
            has_validators: bool,
        },
        /// A party signed the agreement.
        AgreementSigned {
            document_hash: H256,
            signer: T::AccountId,
            signatures_collected: u32,
            signatures_required: u32,
        },
        /// All parties signed; now awaiting professional validation.
        AgreementPendingValidation { document_hash: H256 },
        /// A professional validator validated the agreement.
        AgreementValidated {
            document_hash: H256,
            validator: T::AccountId,
            validations_collected: u32,
            validations_required: u32,
        },
        /// All required signatures and validations collected — agreement is now Active.
        ///
        /// The Altan Gateway listens for this event to issue a digital seal
        /// and notify all parties that the contract is legally binding ончейн.
        AgreementActivated { document_hash: H256 },
        /// A party raised a dispute — agreement enters judicial review.
        ///
        /// The Altan Gateway and pallet-judicial-courts consume this event
        /// to open a JudicialCase automatically (future integration).
        AgreementDisputed {
            document_hash: H256,
            raised_by: T::AccountId,
        },
        /// Agreement was formally completed by mutual consent.
        AgreementCompleted { document_hash: H256 },
    }

    // =========================================================================
    // Errors
    // =========================================================================

    #[pallet::error]
    pub enum Error<T> {
        /// An agreement with this document hash already exists.
        AgreementAlreadyExists,
        /// No agreement found for this document hash.
        AgreementNotFound,
        /// At least two parties are required to form a valid agreement.
        InsufficientParties,
        /// The caller is not listed as a party to this agreement.
        NotAParty,
        /// The caller is not listed as a validator for this agreement.
        NotAValidator,
        /// The caller has already signed this agreement.
        AlreadySigned,
        /// The caller has already validated this agreement.
        AlreadyValidated,
        /// The agreement is not in `PendingSignatures` status — cannot sign.
        NotPendingSignatures,
        /// The agreement is not in `PendingValidation` status — cannot validate.
        NotPendingValidation,
        /// The agreement must be `Active` to raise a dispute.
        NotActive,
        /// The agreement has already been disputed.
        AlreadyDisputed,
        /// The agreement has been completed and cannot be modified.
        AlreadyCompleted,
        /// The validator does not hold sufficient professional rank in any guild.
        /// Must be `Professional` or `Master` in a Lawyer, Notary, or Mediator guild.
        ValidatorNotProfessional,
        /// Parties list exceeds `MaxParties` bound.
        TooManyParties,
        /// Validators list exceeds `MaxValidators` bound.
        TooManyValidators,
        /// The creator must be included in the list of parties.
        CreatorNotInParties,
    }

    // =========================================================================
    // Internal helpers
    // =========================================================================

    impl<T: Config> Pallet<T> {
        /// Returns `true` if every account in `required` appears in `collected`.
        fn all_present(required: &[T::AccountId], collected: &[T::AccountId]) -> bool {
            required.iter().all(|acc| collected.contains(acc))
        }
    }

    // =========================================================================
    // Extrinsics
    // =========================================================================

    #[pallet::call]
    impl<T: Config> Pallet<T> {
        // ─── propose_agreement ───────────────────────────────────────────────

        /// Register a new Ricardian Agreement on-chain.
        ///
        /// The `document_hash` is the Blake2_256 hash of the off-chain document
        /// (PDF, Word, etc.) computed by the Altan Gateway. Any party can retrieve
        /// the document from IPFS using this hash as the CID anchor.
        ///
        /// ## Parameters
        /// - `document_hash`: H256 computed off-chain by the Altan Gateway.
        /// - `parties`: All counterparties (including the caller). Min 2 required.
        /// - `validators`: Optional list of invited Lawyers/Notaries/Mediators.
        ///   If `None`, agreement becomes `Active` as soon as all parties sign.
        ///
        /// ## Errors
        /// - `AgreementAlreadyExists` if this hash is already registered.
        /// - `InsufficientParties` if fewer than 2 parties supplied.
        /// - `CreatorNotInParties` if the caller is not in the parties list.
        /// - `TooManyParties` / `TooManyValidators` if bounds exceeded.
        ///
        /// # Origin: Signed
        #[pallet::call_index(0)]
        #[pallet::weight(T::WeightInfo::propose_agreement())]
        pub fn propose_agreement(
            origin: OriginFor<T>,
            document_hash: H256,
            parties: Vec<T::AccountId>,
            validators: Option<Vec<T::AccountId>>,
        ) -> DispatchResult {
            let creator = ensure_signed(origin)?;

            // Guard: no duplicate registration
            ensure!(
                !Agreements::<T>::contains_key(document_hash),
                Error::<T>::AgreementAlreadyExists
            );

            // Guard: minimum 2 parties
            ensure!(parties.len() >= 2, Error::<T>::InsufficientParties);

            // Guard: creator must be a party
            ensure!(parties.contains(&creator), Error::<T>::CreatorNotInParties);

            // Bound parties
            let bounded_parties: BoundedVec<T::AccountId, T::MaxParties> =
                parties.try_into().map_err(|_| Error::<T>::TooManyParties)?;

            // Bound validators (optional)
            let bounded_validators: Option<BoundedVec<T::AccountId, T::MaxValidators>> =
                match validators {
                    Some(v) => Some(v.try_into().map_err(|_| Error::<T>::TooManyValidators)?),
                    None => None,
                };

            let has_validators = bounded_validators
                .as_ref()
                .map(|v| !v.is_empty())
                .unwrap_or(false);

            let party_count = bounded_parties.len() as u32;

            Agreements::<T>::insert(
                document_hash,
                Agreement {
                    document_hash,
                    creator: creator.clone(),
                    parties: bounded_parties,
                    validators: bounded_validators,
                    signed_by: BoundedVec::default(),
                    validated_by: BoundedVec::default(),
                    status: AgreementStatus::PendingSignatures,
                },
            );

            Self::deposit_event(Event::AgreementProposed {
                document_hash,
                creator,
                party_count,
                has_validators,
            });
            Ok(())
        }

        // ─── sign_agreement ──────────────────────────────────────────────────

        /// Sign an existing agreement as a party.
        ///
        /// The caller must be listed in `parties`. Each party may sign only once.
        /// When all parties have signed:
        ///   - If no validators: status → `Active`
        ///   - If validators set: status → `PendingValidation`
        ///
        /// ## Parameters
        /// - `document_hash`: The H256 hash identifying the agreement.
        ///
        /// # Origin: Signed (must be a party to the agreement)
        #[pallet::call_index(1)]
        #[pallet::weight(T::WeightInfo::sign_agreement())]
        pub fn sign_agreement(origin: OriginFor<T>, document_hash: H256) -> DispatchResult {
            let signer = ensure_signed(origin)?;

            Agreements::<T>::try_mutate(document_hash, |maybe| -> DispatchResult {
                let agreement = maybe.as_mut().ok_or(Error::<T>::AgreementNotFound)?;

                // Must be in PendingSignatures
                ensure!(
                    agreement.status == AgreementStatus::PendingSignatures,
                    Error::<T>::NotPendingSignatures
                );

                // Must be a party
                ensure!(agreement.parties.contains(&signer), Error::<T>::NotAParty);

                // No duplicate signatures
                ensure!(
                    !agreement.signed_by.contains(&signer),
                    Error::<T>::AlreadySigned
                );

                // Record signature
                agreement
                    .signed_by
                    .try_push(signer.clone())
                    .map_err(|_| Error::<T>::TooManyParties)?;

                let signatures_collected = agreement.signed_by.len() as u32;
                let signatures_required = agreement.parties.len() as u32;

                Self::deposit_event(Event::AgreementSigned {
                    document_hash,
                    signer,
                    signatures_collected,
                    signatures_required,
                });

                // Check if all parties have signed
                if Self::all_present(&agreement.parties, &agreement.signed_by) {
                    let has_validators = agreement
                        .validators
                        .as_ref()
                        .map(|v| !v.is_empty())
                        .unwrap_or(false);

                    if has_validators {
                        agreement.status = AgreementStatus::PendingValidation;
                        Self::deposit_event(Event::AgreementPendingValidation { document_hash });
                    } else {
                        // Simple agreement: no validators required → Active immediately
                        agreement.status = AgreementStatus::Active;
                        Self::deposit_event(Event::AgreementActivated { document_hash });
                    }
                }

                Ok(())
            })
        }

        // ─── validate_agreement ──────────────────────────────────────────────

        /// Validate the agreement as an invited professional (Lawyer, Notary, Mediator).
        ///
        /// ## Checks
        /// 1. Agreement exists and is in `PendingValidation`.
        /// 2. Caller is listed in `validators`.
        /// 3. Caller has not already validated.
        /// 4. `GuildsChecker::is_valid_validator(caller)` → must hold at least
        ///    `Professional` rank in any registered guild.
        ///
        /// When all validators have validated: status → `Active`.
        ///
        /// # Origin: Signed (must be an invited validator with Professional guild rank)
        #[pallet::call_index(2)]
        #[pallet::weight(T::WeightInfo::validate_agreement())]
        pub fn validate_agreement(origin: OriginFor<T>, document_hash: H256) -> DispatchResult {
            let validator = ensure_signed(origin)?;

            // ROLE CHECK: must be a guild Professional or Master
            ensure!(
                T::GuildsChecker::is_valid_validator(&validator),
                Error::<T>::ValidatorNotProfessional
            );

            Agreements::<T>::try_mutate(document_hash, |maybe| -> DispatchResult {
                let agreement = maybe.as_mut().ok_or(Error::<T>::AgreementNotFound)?;

                // Must be in PendingValidation
                ensure!(
                    agreement.status == AgreementStatus::PendingValidation,
                    Error::<T>::NotPendingValidation
                );

                // Must be an invited validator
                let validators = agreement
                    .validators
                    .as_ref()
                    .ok_or(Error::<T>::NotAValidator)?;
                ensure!(validators.contains(&validator), Error::<T>::NotAValidator);

                // No duplicate validations
                ensure!(
                    !agreement.validated_by.contains(&validator),
                    Error::<T>::AlreadyValidated
                );

                // Record validation
                agreement
                    .validated_by
                    .try_push(validator.clone())
                    .map_err(|_| Error::<T>::TooManyValidators)?;

                let validations_collected = agreement.validated_by.len() as u32;
                let validations_required = validators.len() as u32;

                Self::deposit_event(Event::AgreementValidated {
                    document_hash,
                    validator,
                    validations_collected,
                    validations_required,
                });

                // Check if all validators have validated
                let all_validated = Self::all_present(
                    validators,
                    &agreement.validated_by,
                );

                if all_validated {
                    agreement.status = AgreementStatus::Active;
                    Self::deposit_event(Event::AgreementActivated { document_hash });
                }

                Ok(())
            })
        }

        // ─── raise_dispute ───────────────────────────────────────────────────

        /// Raise a dispute on an active agreement.
        ///
        /// Any party to the agreement may call this if they believe the other
        /// parties have violated the terms. Status transitions: `Active → Disputed`.
        ///
        /// The `AgreementDisputed` event triggers judicial review in
        /// `pallet-judicial-courts` (future integration via Gateway event listener).
        ///
        /// ## Parameters
        /// - `document_hash`: The H256 hash identifying the agreement.
        ///
        /// # Origin: Signed (must be a party to the agreement)
        #[pallet::call_index(3)]
        #[pallet::weight(T::WeightInfo::raise_dispute())]
        pub fn raise_dispute(origin: OriginFor<T>, document_hash: H256) -> DispatchResult {
            let caller = ensure_signed(origin)?;

            Agreements::<T>::try_mutate(document_hash, |maybe| -> DispatchResult {
                let agreement = maybe.as_mut().ok_or(Error::<T>::AgreementNotFound)?;

                // Must be Active to dispute
                ensure!(
                    agreement.status == AgreementStatus::Active,
                    Error::<T>::NotActive
                );

                // Only parties can raise disputes
                ensure!(agreement.parties.contains(&caller), Error::<T>::NotAParty);

                agreement.status = AgreementStatus::Disputed;

                Self::deposit_event(Event::AgreementDisputed {
                    document_hash,
                    raised_by: caller,
                });

                Ok(())
            })
        }

        // ─── complete_agreement ──────────────────────────────────────────────

        /// Mark an active agreement as fully completed.
        ///
        /// Called by any party to signal mutual fulfilment of the contract terms.
        /// Status: `Active → Completed` (terminal state).
        ///
        /// # Origin: Signed (must be a party to the agreement)
        #[pallet::call_index(4)]
        #[pallet::weight(T::WeightInfo::complete_agreement())]
        pub fn complete_agreement(origin: OriginFor<T>, document_hash: H256) -> DispatchResult {
            let caller = ensure_signed(origin)?;

            Agreements::<T>::try_mutate(document_hash, |maybe| -> DispatchResult {
                let agreement = maybe.as_mut().ok_or(Error::<T>::AgreementNotFound)?;

                // Must be Active to complete
                ensure!(
                    agreement.status == AgreementStatus::Active,
                    Error::<T>::NotActive
                );

                // Only parties can mark as completed
                ensure!(agreement.parties.contains(&caller), Error::<T>::NotAParty);

                agreement.status = AgreementStatus::Completed;

                Self::deposit_event(Event::AgreementCompleted { document_hash });

                Ok(())
            })
        }
    }
}

// =========================================================================
// Ghost-State Cleanup (Task 1: Cross-Pallet Cascading Cleanup)
// =========================================================================

impl<T: Config> Pallet<T> {
    /// **SECURITY (GHOST STATE)** — Scan all `PendingSignatures` agreements where
    /// `who` is a named party and cancel them.
    ///
    /// Called when a citizen's status becomes terminal (`Deceased` or `Exiled`).
    ///
    /// Only `PendingSignatures` agreements are cancelled — agreements in
    /// `Active`, `Completed`, or `Disputed` states remain intact as permanent
    /// legal records. An agreement that was already fully Active is valid even
    /// if one party subsequently dies; the remaining parties and the court
    /// handle enforcement.
    pub fn annul_signatures(who: &T::AccountId) {
        use sp_core::H256;
        let ids_to_cancel: alloc::vec::Vec<H256> = Agreements::<T>::iter()
            .filter(|(_, a)| {
                a.status == AgreementStatus::PendingSignatures && a.parties.contains(who)
            })
            .map(|(hash, _)| hash)
            .collect();

        for doc_hash in ids_to_cancel {
            Agreements::<T>::mutate(doc_hash, |maybe| {
                if let Some(a) = maybe {
                    a.status = AgreementStatus::Cancelled;
                }
            });
        }
    }
}

/// Wire `pallet_inomad_identity::OnTerminalStatus` so that when a citizen
/// dies or is exiled, their pending Chancery agreements are cancelled.
impl<T: Config> pallet_inomad_identity::OnTerminalStatus<T::AccountId> for Pallet<T> {
    fn on_deceased(who: &T::AccountId) {
        Self::annul_signatures(who);
    }
    fn on_exiled(who: &T::AccountId) {
        Self::annul_signatures(who);
    }
}
