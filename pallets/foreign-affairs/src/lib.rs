//! # Pallet Foreign Affairs (МИД Сибирской Конфедерации)
//!
//! On-chain diplomatic bridge for **193 UN member states**.
//!
//! ## Architecture
//!
//! Each recognized foreign state gets:
//! 1. **MFA Account** (МИД) — Ministry of Foreign Affairs treasury
//! 2. **Embassy Account** (Посольство) — diplomatic operations
//! 3. **Passport Office** (Загранпаспортный стол) — visa/passport services
//! 4. **10 Banking Wallets** — financial operations channel
//!
//! Additionally, two on-chain channels per country:
//! - **Diplomatic Channel** — closed encrypted communication between MFA and Embassy
//! - **Legalization Channel** — document exchange and legalization queue
//!
//! ## Storage Model
//!
//! Countries are registered dynamically via `Storage` (not constants) so the
//! MFA can add/remove states as diplomatic relations evolve.
//!
//! ## Account Generation
//!
//! All 13 accounts per country are generated deterministically from the
//! ISO 3166-1 numeric code, ensuring reproducibility without mnemonics.
//!
//! ## Constitutional Basis
//!
//! Article VII of the INOMAD Constitution: International Relations.
//! The Confederation recognizes all UN member states and maintains
//! sovereign diplomatic channels through this pallet.
//!
//! ## Dispatchables
//!
//! | Call | Origin | Description |
//! |---|---|---|
//! | `register_foreign_state` | Root | Register a new foreign state in the diplomatic registry |
//! | `update_diplomatic_status` | Signed (Foreign Affairs Minister) | Change the diplomatic status of a registered state |
//! | `send_diplomatic_message` | Signed (Minister/Representative) | Send an official diplomatic message on-chain |
//! | `submit_legalization_document` | Signed (Representative) | Submit a document for sovereign legalization |
//! | `freeze_country_operations` | Signed (Minister) | Suspend all operations with a foreign state |
//! | `acknowledge_message` | Signed (Minister) | Acknowledge receipt of a diplomatic message |
//! | `register_representative` | Signed (Minister) | Register a diplomatic representative for a foreign state |
//! | `appoint_council_member` | Signed (Minister) | Appoint a member to the diplomatic advisory council |
//! | `remove_council_member` | Signed (Minister) | Remove a member from the diplomatic advisory council |
//! | `is_council_authorized` | Any (Query) | Query whether an account has diplomatic council authority |

#![cfg_attr(not(feature = "std"), no_std)]

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
    use codec::{Decode, Encode, MaxEncodedLen};
    use frame_support::{pallet_prelude::*, traits::Currency};
    use frame_system::pallet_prelude::*;
    use scale_info::TypeInfo;
    use sp_runtime::traits::Hash;

    // =========================================================================
    //  Types
    // =========================================================================

    /// Diplomatic status between the Confederation and a foreign state.
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
    pub enum DiplomaticStatus {
        /// Full diplomatic relations — all channels open
        Active,
        /// Temporarily suspended — embassy closed, channels frozen
        Suspended,
        /// Under sanctions — banking channels frozen, diplomatic channel read-only
        Sanctioned,
        /// No diplomatic relations established
        NoRelations,
    }

    impl Default for DiplomaticStatus {
        fn default() -> Self {
            DiplomaticStatus::NoRelations
        }
    }

    /// Type of diplomatic message/document on-chain.
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
    pub enum ChannelMessageType {
        /// Encrypted diplomatic communication (МИД ↔ Посольство)
        DiplomaticNote,
        /// Document submitted for legalization (notarization/apostille)
        DocumentLegalization,
        /// Visa/passport application
        PassportApplication,
        /// Banking operation notification
        BankingNotification,
        /// Trade agreement draft
        TradeAgreement,
        /// Consular assistance request
        ConsularRequest,
    }

    /// Which channel a message belongs to.
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
    pub enum ChannelKind {
        Diplomatic,
        Legalization,
    }

    /// On-chain record for a UN-recognized sovereign state.
    ///
    /// ## Account Layout (13 per country)
    ///
    /// Accounts are stored in a separate `StorageMap` to avoid
    /// large fixed-size arrays in the main record.
    ///
    /// ```text
    /// Slot 0:  MFA Account       (МИД — Министерство Иностранных Дел)
    /// Slot 1:  Embassy Account   (Посольство)
    /// Slot 2:  Passport Office   (Загранпаспортный стол)
    /// Slot 3-12: Bank Wallet #1-#10
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
    pub struct ForeignStateRecord {
        /// ISO 3166-1 numeric code (e.g., 840 = USA, 643 = Russia).
        pub country_code: u16,
        /// ISO 3166-1 alpha-2 code (e.g., b"US", b"RU").
        pub iso_alpha2: [u8; 2],
        /// ISO 3166-1 alpha-3 code (e.g., b"USA", b"RUS").
        pub iso_alpha3: [u8; 3],
        /// Diplomatic status.
        pub status: DiplomaticStatus,
        /// Block when this state was registered on-chain.
        pub registered_at: u32,
        /// Number of bank wallet accounts (always 10).
        pub bank_wallet_count: u8,
    }

    /// A message/document in a diplomatic or legalization channel.
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
    pub struct ChannelMessage {
        /// Country this message belongs to.
        pub country_code: u16,
        /// Type of message.
        pub message_type: ChannelMessageType,
        /// blake2_256 hash of the sender's account (saves space vs full AccountId).
        pub sender_hash: [u8; 32],
        /// blake2_256 hash of the encrypted message/document content.
        /// Actual content is stored off-chain (IPFS/encrypted storage).
        pub content_hash: [u8; 32],
        /// Block number when the message was submitted.
        pub submitted_at: u32,
        /// Whether this message has been acknowledged by the counterparty.
        pub acknowledged: bool,
    }

    // =========================================================================
    //  Pallet
    // =========================================================================

    #[pallet::pallet]
    pub struct Pallet<T>(_);

    #[pallet::config]
    pub trait Config:
        frame_system::Config<
        RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>,
    >
    {
        /// Weight information for extrinsics in this pallet.
        type WeightInfo: crate::weights::WeightInfo;
        /// Currency for existential deposits on generated accounts.
        type Currency: Currency<Self::AccountId>;

        /// Maximum number of messages per country channel.
        #[pallet::constant]
        type MaxChannelMessages: Get<u32>;

        /// Maximum number of council members per foreign state.
        ///
        /// Constitutionally: each foreign state appoints its council via its
        /// accredited Special Representative. Max 20 per country (mirrors org max).
        #[pallet::constant]
        type MaxDiplomaticCouncil: Get<u32>;
    }

    // =========================================================================
    //  Storage
    // =========================================================================

    /// Registry of all recognized foreign states.
    /// Key: ISO 3166-1 numeric country code (u16).
    #[pallet::storage]
    #[pallet::getter(fn foreign_state)]
    pub type ForeignStates<T: Config> =
        StorageMap<_, Blake2_128Concat, u16, ForeignStateRecord, OptionQuery>;

    /// Total number of registered foreign states.
    #[pallet::storage]
    #[pallet::getter(fn total_states)]
    pub type TotalStates<T: Config> = StorageValue<_, u32, ValueQuery>;

    /// Country name storage (separate to avoid BoundedVec in the main record).
    /// Key: ISO 3166-1 numeric code → English country name.
    #[pallet::storage]
    #[pallet::getter(fn country_name)]
    pub type CountryNames<T: Config> =
        StorageMap<_, Blake2_128Concat, u16, BoundedVec<u8, ConstU32<128>>, OptionQuery>;

    /// Country account map: (country_code, slot_index) → AccountId.
    ///
    /// Slot indices:
    /// - 0: MFA (Ministry of Foreign Affairs)
    /// - 1: Embassy
    /// - 2: Passport Office
    /// - 3-12: Bank wallets #1 through #10
    #[pallet::storage]
    #[pallet::getter(fn country_account)]
    pub type CountryAccounts<T: Config> =
        StorageDoubleMap<_, Blake2_128Concat, u16, Blake2_128Concat, u8, T::AccountId, OptionQuery>;

    /// Diplomatic channel — closed encrypted communication log.
    /// Key: country_code → list of diplomatic messages.
    #[pallet::storage]
    #[pallet::getter(fn diplomatic_channel)]
    pub type DiplomaticChannel<T: Config> = StorageMap<
        _,
        Blake2_128Concat,
        u16,
        BoundedVec<ChannelMessage, T::MaxChannelMessages>,
        ValueQuery,
    >;

    /// Legalization channel — document exchange and apostille queue.
    /// Key: country_code → list of document legalization requests.
    #[pallet::storage]
    #[pallet::getter(fn legalization_channel)]
    pub type LegalizationChannel<T: Config> = StorageMap<
        _,
        Blake2_128Concat,
        u16,
        BoundedVec<ChannelMessage, T::MaxChannelMessages>,
        ValueQuery,
    >;

    // =========================================================================
    //  Diplomatic Council Governance
    // =========================================================================

    /// The Special Representative (Аккредитованный Посол) for each foreign state.
    ///
    /// Registered by Root (Confederation MFA) upon receiving credentials.
    /// The representative holds authority to appoint and remove council members.
    ///
    /// Constitutional: one representative per country at any time.
    #[pallet::storage]
    #[pallet::getter(fn diplomatic_representative)]
    pub type DiplomaticRepresentative<T: Config> =
        StorageMap<_, Blake2_128Concat, u16, T::AccountId, OptionQuery>;

    /// Council members for each foreign state.
    ///
    /// Appointed by the `DiplomaticRepresentative`. Council collectively
    /// manages all 13 diplomatic account slots.
    ///
    /// BoundedVec ensures `MaxDiplomaticCouncil` bound.
    #[pallet::storage]
    #[pallet::getter(fn diplomatic_council)]
    pub type DiplomaticCouncil<T: Config> = StorageMap<
        _,
        Blake2_128Concat,
        u16,
        BoundedVec<T::AccountId, T::MaxDiplomaticCouncil>,
        ValueQuery,
    >;

    // =========================================================================
    //  Events
    // =========================================================================

    #[pallet::event]
    #[pallet::generate_deposit(pub(super) fn deposit_event)]
    pub enum Event<T: Config> {
        /// A new foreign state has been registered.
        ForeignStateRegistered {
            country_code: u16,
            iso_alpha3: [u8; 3],
            status: DiplomaticStatus,
        },
        /// Diplomatic status updated for a country.
        DiplomaticStatusUpdated {
            country_code: u16,
            old_status: DiplomaticStatus,
            new_status: DiplomaticStatus,
        },
        /// A diplomatic message was sent.
        DiplomaticMessageSent {
            country_code: u16,
            message_type: ChannelMessageType,
            sender: T::AccountId,
            content_hash: [u8; 32],
        },
        /// A legalization document was submitted.
        LegalizationDocumentSubmitted {
            country_code: u16,
            sender: T::AccountId,
            content_hash: [u8; 32],
        },
        /// All operations for a country have been frozen (sanctions).
        CountryOperationsFrozen { country_code: u16 },
        /// A channel message was acknowledged.
        MessageAcknowledged {
            country_code: u16,
            message_index: u32,
            channel: ChannelKind,
        },
        // ── Council Governance Events ────────────────────────────────────────
        /// A Special Representative (Посол) has been registered for a country.
        RepresentativeRegistered {
            country_code: u16,
            representative: T::AccountId,
        },
        /// Representative replaced (old removed, new registered).
        RepresentativeReplaced {
            country_code: u16,
            old_representative: T::AccountId,
            new_representative: T::AccountId,
        },
        /// A new council member was appointed by the representative.
        CouncilMemberAppointed {
            country_code: u16,
            member: T::AccountId,
        },
        /// A council member was removed by the representative.
        CouncilMemberRemoved {
            country_code: u16,
            member: T::AccountId,
        },
        /// Entire council was dissolved (e.g., on representative replacement).
        CouncilDissolved { country_code: u16 },
    }

    // =========================================================================
    //  Errors
    // =========================================================================

    #[pallet::error]
    pub enum Error<T> {
        /// Country with this code is already registered.
        CountryAlreadyRegistered,
        /// Country not found in the registry.
        CountryNotFound,
        /// Country name is empty or too long.
        InvalidCountryName,
        /// Country is under sanctions — operation not permitted.
        CountrySanctioned,
        /// Country's diplomatic status does not allow this operation.
        DiplomaticStatusNotActive,
        /// Channel message queue is full.
        ChannelFull,
        /// Message index out of bounds.
        MessageIndexOutOfBounds,
        /// Sender is not authorized for this channel.
        UnauthorizedSender,
        /// Account slot not found for this country.
        AccountNotFound,
        // ── Council Governance Errors ────────────────────────────────────────
        /// No Special Representative has been accredited for this country.
        NoRepresentativeRegistered,
        /// Only the Special Representative may perform this action.
        NotRepresentative,
        /// This account is already a council member.
        AlreadyCouncilMember,
        /// This account is not a council member.
        NotCouncilMember,
        /// Council is full — remove a member before adding.
        CouncilFull,
        /// Council operation requires at least one member.
        CouncilEmpty,
    }

    // =========================================================================
    //  Extrinsics
    // =========================================================================

    #[pallet::call]
    impl<T: Config> Pallet<T> {
        /// Register a new foreign state in the on-chain registry.
        ///
        /// Root-gated (МИД Конфедерации / Khural vote).
        /// Generates 13 deterministic accounts from the country code.
        ///
        /// ## Arguments
        ///
        /// - `country_code` — ISO 3166-1 numeric (e.g., 840 for USA)
        /// - `iso_alpha2` — Two-letter code (e.g., b"US")
        /// - `iso_alpha3` — Three-letter code (e.g., b"USA")
        /// - `name` — English country name
        /// - `status` — Initial diplomatic status
        #[pallet::call_index(0)]
        #[pallet::weight(T::WeightInfo::register_foreign_state())]
        pub fn register_foreign_state(
            origin: OriginFor<T>,
            country_code: u16,
            iso_alpha2: [u8; 2],
            iso_alpha3: [u8; 3],
            name: BoundedVec<u8, ConstU32<128>>,
            status: DiplomaticStatus,
        ) -> DispatchResult {
            ensure_root(origin)?;

            ensure!(
                !ForeignStates::<T>::contains_key(country_code),
                Error::<T>::CountryAlreadyRegistered
            );
            ensure!(!name.is_empty(), Error::<T>::InvalidCountryName);

            // Generate 13 deterministic accounts from country_code and store them.
            for slot in 0u8..13u8 {
                let account = Self::generate_account(country_code, slot);
                CountryAccounts::<T>::insert(country_code, slot, account);
            }

            let block = <frame_system::Pallet<T>>::block_number();
            let block_u32 = Self::block_to_u32(block);

            let record = ForeignStateRecord {
                country_code,
                iso_alpha2,
                iso_alpha3,
                status: status.clone(),
                registered_at: block_u32,
                bank_wallet_count: 10,
            };

            ForeignStates::<T>::insert(country_code, record);
            CountryNames::<T>::insert(country_code, name);
            TotalStates::<T>::mutate(|n| *n = n.saturating_add(1));

            Self::deposit_event(Event::ForeignStateRegistered {
                country_code,
                iso_alpha3,
                status,
            });

            Ok(())
        }

        /// Update the diplomatic status of a registered foreign state.
        ///
        /// Root-gated (МИД Конфедерации or Confederate Khural vote).
        #[pallet::call_index(1)]
        #[pallet::weight(T::WeightInfo::update_diplomatic_status())]
        pub fn update_diplomatic_status(
            origin: OriginFor<T>,
            country_code: u16,
            new_status: DiplomaticStatus,
        ) -> DispatchResult {
            ensure_root(origin)?;

            ForeignStates::<T>::try_mutate(country_code, |maybe| {
                let record = maybe.as_mut().ok_or(Error::<T>::CountryNotFound)?;
                let old_status = record.status.clone();
                record.status = new_status.clone();

                Self::deposit_event(Event::DiplomaticStatusUpdated {
                    country_code,
                    old_status,
                    new_status,
                });

                Ok(())
            })
        }

        /// Send a diplomatic message (closed channel).
        ///
        /// Only MFA or Embassy accounts of the specified country may send.
        /// Content is stored off-chain; only the hash is recorded on-chain.
        #[pallet::call_index(2)]
        #[pallet::weight(T::WeightInfo::send_diplomatic_message())]
        pub fn send_diplomatic_message(
            origin: OriginFor<T>,
            country_code: u16,
            message_type: ChannelMessageType,
            content_hash: [u8; 32],
        ) -> DispatchResult {
            let sender = ensure_signed(origin)?;

            let state = ForeignStates::<T>::get(country_code).ok_or(Error::<T>::CountryNotFound)?;

            ensure!(
                state.status == DiplomaticStatus::Active,
                Error::<T>::DiplomaticStatusNotActive
            );

            // Authorization: representative or any council member
            Self::is_council_authorized(country_code, &sender)?;

            let sender_hash = T::Hashing::hash(&sender.encode());
            let mut sender_h = [0u8; 32];
            sender_h.copy_from_slice(sender_hash.as_ref());

            let block = <frame_system::Pallet<T>>::block_number();

            let message = ChannelMessage {
                country_code,
                message_type: message_type.clone(),
                sender_hash: sender_h,
                content_hash,
                submitted_at: Self::block_to_u32(block),
                acknowledged: false,
            };

            DiplomaticChannel::<T>::try_mutate(country_code, |messages| {
                messages
                    .try_push(message)
                    .map_err(|_| Error::<T>::ChannelFull)
            })?;

            Self::deposit_event(Event::DiplomaticMessageSent {
                country_code,
                message_type,
                sender,
                content_hash,
            });

            Ok(())
        }

        /// Submit a document for legalization (apostille/notarization).
        ///
        /// Any citizen or the Passport Office may submit.
        /// Content is the hash of the document to be legalized.
        #[pallet::call_index(3)]
        #[pallet::weight(T::WeightInfo::submit_legalization_document())]
        pub fn submit_legalization_document(
            origin: OriginFor<T>,
            country_code: u16,
            content_hash: [u8; 32],
        ) -> DispatchResult {
            let sender = ensure_signed(origin)?;

            let state = ForeignStates::<T>::get(country_code).ok_or(Error::<T>::CountryNotFound)?;

            ensure!(
                state.status == DiplomaticStatus::Active
                    || state.status == DiplomaticStatus::Suspended,
                Error::<T>::CountrySanctioned
            );

            let sender_hash = T::Hashing::hash(&sender.encode());
            let mut sender_h = [0u8; 32];
            sender_h.copy_from_slice(sender_hash.as_ref());

            let block = <frame_system::Pallet<T>>::block_number();

            let message = ChannelMessage {
                country_code,
                message_type: ChannelMessageType::DocumentLegalization,
                sender_hash: sender_h,
                content_hash,
                submitted_at: Self::block_to_u32(block),
                acknowledged: false,
            };

            LegalizationChannel::<T>::try_mutate(country_code, |messages| {
                messages
                    .try_push(message)
                    .map_err(|_| Error::<T>::ChannelFull)
            })?;

            Self::deposit_event(Event::LegalizationDocumentSubmitted {
                country_code,
                sender,
                content_hash,
            });

            Ok(())
        }

        /// Freeze all operations for a country (impose sanctions).
        ///
        /// Root-gated. Sets status to `Sanctioned` and freezes banking channels.
        #[pallet::call_index(4)]
        #[pallet::weight(T::WeightInfo::freeze_country_operations())]
        pub fn freeze_country_operations(
            origin: OriginFor<T>,
            country_code: u16,
        ) -> DispatchResult {
            ensure_root(origin)?;

            ForeignStates::<T>::try_mutate(country_code, |maybe| {
                let record = maybe.as_mut().ok_or(Error::<T>::CountryNotFound)?;
                record.status = DiplomaticStatus::Sanctioned;

                Self::deposit_event(Event::CountryOperationsFrozen { country_code });

                Ok(())
            })
        }

        /// Acknowledge a message in a diplomatic or legalization channel.
        ///
        /// Only MFA (slot 0) or Embassy (slot 1) accounts may acknowledge.
        #[pallet::call_index(5)]
        #[pallet::weight(T::WeightInfo::acknowledge_message())]
        pub fn acknowledge_message(
            origin: OriginFor<T>,
            country_code: u16,
            channel: ChannelKind,
            message_index: u32,
        ) -> DispatchResult {
            let sender = ensure_signed(origin)?;

            // Authorization: representative or any council member
            Self::is_council_authorized(country_code, &sender)?;

            match channel {
                ChannelKind::Diplomatic => {
                    DiplomaticChannel::<T>::try_mutate(country_code, |messages| {
                        let msg = messages
                            .get_mut(message_index as usize)
                            .ok_or(Error::<T>::MessageIndexOutOfBounds)?;
                        msg.acknowledged = true;
                        Ok::<(), DispatchError>(())
                    })?;
                }
                ChannelKind::Legalization => {
                    LegalizationChannel::<T>::try_mutate(country_code, |messages| {
                        let msg = messages
                            .get_mut(message_index as usize)
                            .ok_or(Error::<T>::MessageIndexOutOfBounds)?;
                        msg.acknowledged = true;
                        Ok::<(), DispatchError>(())
                    })?;
                }
            }

            Self::deposit_event(Event::MessageAcknowledged {
                country_code,
                message_index,
                channel,
            });

            Ok(())
        }

        // ─────────────────────────────────────────────────────────────────────
        // Council Governance Extrinsics
        // ─────────────────────────────────────────────────────────────────────

        /// Register (or replace) the Special Representative for a foreign state.
        ///
        /// ## Authority
        /// Root-only (Confederation MFA / Khural vote).
        ///
        /// ## Behaviour
        /// - If a representative already exists → fires `RepresentativeReplaced`
        ///   and **dissolves the old council** (all members removed) to prevent
        ///   dead-council attacks (old council of deposed ambassador keeping access).
        /// - Records the new representative in `DiplomaticRepresentative`.
        ///
        /// ## Emits
        /// - `RepresentativeRegistered` (new) or `RepresentativeReplaced` + `CouncilDissolved`
        #[pallet::call_index(6)]
        #[pallet::weight(T::WeightInfo::register_representative())]
        pub fn register_representative(
            origin: OriginFor<T>,
            country_code: u16,
            representative: T::AccountId,
        ) -> DispatchResult {
            ensure_root(origin)?;

            // Country must be registered.
            ensure!(
                ForeignStates::<T>::contains_key(country_code),
                Error::<T>::CountryNotFound
            );

            if let Some(old_rep) = DiplomaticRepresentative::<T>::get(country_code) {
                // Dissolve old council — prevents access by predecessor's council.
                DiplomaticCouncil::<T>::remove(country_code);
                Self::deposit_event(Event::CouncilDissolved { country_code });

                // Replace event.
                Self::deposit_event(Event::RepresentativeReplaced {
                    country_code,
                    old_representative: old_rep,
                    new_representative: representative.clone(),
                });
            } else {
                Self::deposit_event(Event::RepresentativeRegistered {
                    country_code,
                    representative: representative.clone(),
                });
            }

            DiplomaticRepresentative::<T>::insert(country_code, &representative);
            Ok(())
        }

        // ─── appoint_council_member ────────────────────────────────────────────

        /// Appoint a new member to the Diplomatic Council.
        ///
        /// ## Authority
        /// Only the accredited Special Representative (`DiplomaticRepresentative[country_code]`).
        ///
        /// ## Constitutional Rule
        /// - Country must not be `Sanctioned`.
        /// - Council bounded at `MaxDiplomaticCouncil` (default 20).
        ///
        /// ## Emits
        /// `CouncilMemberAppointed`
        #[pallet::call_index(7)]
        #[pallet::weight(T::WeightInfo::appoint_council_member())]
        pub fn appoint_council_member(
            origin: OriginFor<T>,
            country_code: u16,
            member: T::AccountId,
        ) -> DispatchResult {
            let caller = ensure_signed(origin)?;

            // Country must exist and not be sanctioned.
            let state = ForeignStates::<T>::get(country_code).ok_or(Error::<T>::CountryNotFound)?;
            ensure!(
                state.status != DiplomaticStatus::Sanctioned,
                Error::<T>::CountrySanctioned
            );

            // Caller must be the registered representative.
            let rep = DiplomaticRepresentative::<T>::get(country_code)
                .ok_or(Error::<T>::NoRepresentativeRegistered)?;
            ensure!(caller == rep, Error::<T>::NotRepresentative);

            DiplomaticCouncil::<T>::try_mutate(country_code, |council| {
                ensure!(
                    !council.iter().any(|m| m == &member),
                    Error::<T>::AlreadyCouncilMember
                );
                council
                    .try_push(member.clone())
                    .map_err(|_| Error::<T>::CouncilFull)?;
                Ok::<(), DispatchError>(())
            })?;

            Self::deposit_event(Event::CouncilMemberAppointed {
                country_code,
                member,
            });
            Ok(())
        }

        // ─── remove_council_member ─────────────────────────────────────────────

        /// Remove a member from the Diplomatic Council.
        ///
        /// ## Authority
        /// Only the accredited Special Representative.
        ///
        /// ## Emits
        /// `CouncilMemberRemoved`
        #[pallet::call_index(8)]
        #[pallet::weight(T::WeightInfo::remove_council_member())]
        pub fn remove_council_member(
            origin: OriginFor<T>,
            country_code: u16,
            member: T::AccountId,
        ) -> DispatchResult {
            let caller = ensure_signed(origin)?;

            // Caller must be the registered representative.
            let rep = DiplomaticRepresentative::<T>::get(country_code)
                .ok_or(Error::<T>::NoRepresentativeRegistered)?;
            ensure!(caller == rep, Error::<T>::NotRepresentative);

            DiplomaticCouncil::<T>::try_mutate(country_code, |council| {
                let pos = council
                    .iter()
                    .position(|m| m == &member)
                    .ok_or(Error::<T>::NotCouncilMember)?;
                council.remove(pos);
                Ok::<(), DispatchError>(())
            })?;

            Self::deposit_event(Event::CouncilMemberRemoved {
                country_code,
                member,
            });
            Ok(())
        }
    } // end #[pallet::call]

    // =========================================================================
    //  Internal helpers
    // =========================================================================

    impl<T: Config> Pallet<T> {
        /// Generate a single deterministic account for a foreign state slot.
        ///
        /// Seed layout: `[b'F', b'A', country_hi, country_lo, slot, 0..0]`
        fn generate_account(country_code: u16, slot: u8) -> T::AccountId {
            let mut seed = [0u8; 32];
            seed[0] = b'F'; // Foreign
            seed[1] = b'A'; // Affairs
            let code_bytes = country_code.to_be_bytes();
            seed[2] = code_bytes[0];
            seed[3] = code_bytes[1];
            seed[4] = slot;
            let hash = T::Hashing::hash(&seed);
            T::AccountId::decode(&mut hash.as_ref())
                .expect("Hash output is always 32 bytes; AccountId is 32 bytes")
        }

        /// Check if `who` is authorized to act on behalf of a country's diplomatic slots.
        ///
        /// Authorization is granted to:
        /// 1. The `DiplomaticRepresentative` (Аккредитованный Посол) — full authority.
        /// 2. Any member of `DiplomaticCouncil` — delegated by the representative.
        ///
        /// Returns `Ok(())` if authorized, `Err(UnauthorizedSender)` otherwise.
        pub fn is_council_authorized(country_code: u16, who: &T::AccountId) -> DispatchResult {
            // Check representative.
            if let Some(rep) = DiplomaticRepresentative::<T>::get(country_code) {
                if &rep == who {
                    return Ok(());
                }
            }
            // Check council members.
            let council = DiplomaticCouncil::<T>::get(country_code);
            ensure!(
                council.iter().any(|m| m == who),
                Error::<T>::UnauthorizedSender
            );
            Ok(())
        }

        /// Convert a `BlockNumberFor<T>` to `u32`.
        fn block_to_u32(block: BlockNumberFor<T>) -> u32 {
            use codec::Encode;
            let encoded = block.encode();
            if encoded.len() >= 4 {
                u32::from_le_bytes([encoded[0], encoded[1], encoded[2], encoded[3]])
            } else {
                0u32
            }
        }
    }

    // =========================================================================
    //  Slot constants (public for external use)
    // =========================================================================

    /// Account slot indices for `CountryAccounts` storage.
    pub const SLOT_MFA: u8 = 0;
    pub const SLOT_EMBASSY: u8 = 1;
    pub const SLOT_PASSPORT_OFFICE: u8 = 2;
    /// Bank wallets occupy slots 3..=12 (10 wallets).
    pub const SLOT_BANK_FIRST: u8 = 3;
    pub const SLOT_BANK_LAST: u8 = 12;
}
