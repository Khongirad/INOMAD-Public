//! # pallet-recovery-nft
//!
//! **Altan Network — Arbad Social Recovery NFT**
//!
//! ## Механизм восстановления через NFT
//!
//! ### Концепция
//!
//! Владелец аккаунта программирует 5 Recovery NFT при создании.
//! В каждый NFT зашит:
//!   - `target_account` — аккаунт, средства которого восстанавливаются
//!   - `recovery_group_id` — уникальный ID группы (связывает 5 NFT)
//!   - `destination_kind` — куда перевести: PERSONAL_BANK_VAULT или FAMILY_VAULT
//!   - `vault_address` — конкретный адрес сейфа (зафиксирован при минте)
//!   - `threshold` — порог (обычно 3 из 5)
//!   - `total_issued` — общее число (обычно 5)
//!
//! ### Флоу
//!
//! ```text
//! 1. Владелец минтит 5 Recovery NFT → раздаёт 5 доверенным лицам
//!    (token_id=1 → Alice, token_id=2 → Bob, ...)
//!
//! 2. При утрате доступа:
//!    Alice вызывает initiate_recovery(token_id=1, group_id)
//!    → владелец немедленно уведомляется
//!    → начинается вето-окно (72ч)
//!
//! 3. Bob и Carol вызывают confirm_recovery(token_id, group_id)
//!    После 3-й подписи:
//!    → emit RecoveryExecuted → backend переводит средства на vault_address
//!
//! 4. NFT НЕ сгорают — они остаются как proof на L1
//! ```
//!
//! ### Два варианта назначения
//!
//! ```rust
//! pub enum RecoveryDestination {
//!     PersonalBankVault,  // Личный сейф в Банке Сибири (офицер банка управляет)
//!     FamilyVault,        // Семейный/Арбад сейф (совместный доступ Арбада)
//! }
//! ```
//!
//! ### Безопасность
//!
//! - NFT — on-chain credential. Подделать невозможно.
//! - Destination зафиксирован при минте. Доверенные лица не могут изменить адрес вывода.
//! - Вето-окно: владелец может заблокировать даже при 2+ подписях.
//! - Любая попытка → мгновенное уведомление владельцу.
//!
//! ## Dispatchables
//!
//! | Call | Origin | Description |
//! |---|---|---|
//! | `issue_recovery_nfts` | Signed (Officer) | Issue recovery NFTs to a citizen's designated guardians |
//! | `initiate_recovery` | Signed (Guardian) | Start a social recovery process for a locked account |
//! | `veto_recovery` | Signed (Guardian) | Veto an in-progress recovery (prevents abuse) |
//! | `confirm_recovery` | Signed (Officer/Threshold) | Confirm recovery after guardian threshold is reached |

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
    use frame_support::pallet_prelude::*;
    use frame_system::pallet_prelude::*;
    use frame_support::sp_runtime::traits::Saturating;


    // =========================================================================
    // Recovery Destination (pre-programmed at mint time)
    // =========================================================================

    /// Where funds go upon successful recovery.
    /// Pre-programmed at NFT mint — CANNOT be changed by holders.
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
    pub enum RecoveryDestination {
        /// Личный сейф в Банке Сибири.
        /// `vault_address` — это адрес банковского сейфа, открытого на имя владельца.
        /// Офицеры Банка Сибири управляют доступом.
        PersonalBankVault,

        /// Семейный / Арбад сейф.
        /// `vault_address` — общий сейф, доступный всем членам Арбада.
        /// Управляется коллективно.
        FamilyVault,
    }

    // =========================================================================
    // Recovery NFT Token
    // =========================================================================

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
    pub struct RecoveryNft<AccountId, BlockNumber> {
        /// Globally unique token ID (auto-counter).
        pub token_id: u64,
        /// The recovery group this NFT belongs to (links all 5 together).
        pub group_id: BoundedVec<u8, ConstU32<64>>,
        /// The account whose funds will be recovered.
        pub target_account: AccountId,
        /// The holder of this individual NFT.
        pub holder: AccountId,
        /// Where the recovered funds go (pre-programmed at mint).
        pub destination_kind: RecoveryDestination,
        /// The exact vault address to send funds to (pre-programmed at mint).
        pub vault_address: BoundedVec<u8, ConstU32<128>>,
        /// Threshold — how many NFTs needed to unlock (typically 3).
        pub threshold: u8,
        /// Total Recovery NFTs issued for this group (typically 5).
        pub total_issued: u8,
        /// Index of this NFT within the group (1..=total_issued).
        pub group_index: u8,
        /// Block when this NFT was minted.
        pub issued_at: BlockNumber,
        /// Whether this NFT has been used to initiate/confirm recovery.
        pub used_in_recovery: Option<BoundedVec<u8, ConstU32<64>>>, // recovery_request_id
    }

    // =========================================================================
    // Recovery Request (aggregates confirmations)
    // =========================================================================

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
    pub enum RecoveryRequestStatus {
        PendingVetoWindow,    // Owner notified, can veto
        CollectingSignatures, // Veto expired or owner approved
        OwnerVetoed,          // Owner blocked
        Executed,             // Threshold reached — funds moved
    }

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
    pub struct RecoveryRequest<AccountId, BlockNumber> {
        pub request_id: BoundedVec<u8, ConstU32<64>>,
        pub group_id: BoundedVec<u8, ConstU32<64>>,
        pub target_account: AccountId,
        pub destination_kind: RecoveryDestination,
        pub vault_address: BoundedVec<u8, ConstU32<128>>,
        pub initiated_by: AccountId,
        pub initiated_at: BlockNumber,
        pub veto_deadline_block: BlockNumber,
        pub status: RecoveryRequestStatus,
        pub confirmation_count: u8,
        pub threshold: u8,
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
        /// Weight information for extrinsics in this pallet.
        type WeightInfo: crate::weights::WeightInfo;
        /// Origin that may issue Recovery NFTs (Creator or Arbad administrator).
        type IssuerOrigin: EnsureOrigin<Self::RuntimeOrigin, Success = Self::AccountId>;

        /// Veto window in blocks (72h ≈ 43_200 blocks at 6s/block).
        #[pallet::constant]
        type VetoWindowBlocks: Get<u32>;

        /// Maximum number of Recovery NFTs per group.
        #[pallet::constant]
        type MaxGroupSize: Get<u8>;
    }

    // =========================================================================
    // Storage
    // =========================================================================

    /// Auto-incrementing Recovery NFT token ID.
    #[pallet::storage]
    pub type NextRecoveryTokenId<T: Config> = StorageValue<_, u64, ValueQuery>;

    /// All Recovery NFTs, indexed by token_id.
    #[pallet::storage]
    pub type RecoveryNfts<T: Config> = StorageMap<
        _,
        Blake2_128Concat,
        u64,
        RecoveryNft<T::AccountId, BlockNumberFor<T>>,
        OptionQuery,
    >;

    /// Index: holder → Vec<token_id> of Recovery NFTs they hold.
    #[pallet::storage]
    pub type HolderRecoveryNfts<T: Config> =
        StorageMap<_, Blake2_128Concat, T::AccountId, BoundedVec<u64, ConstU32<16>>, ValueQuery>;

    /// Index: group_id → Vec<token_id> in this recovery group.
    #[pallet::storage]
    pub type GroupNfts<T: Config> = StorageMap<
        _,
        Blake2_128Concat,
        BoundedVec<u8, ConstU32<64>>,
        BoundedVec<u64, ConstU32<16>>,
        ValueQuery,
    >;

    /// Active recovery requests, indexed by request_id.
    #[pallet::storage]
    pub type RecoveryRequests<T: Config> = StorageMap<
        _,
        Blake2_128Concat,
        BoundedVec<u8, ConstU32<64>>,
        RecoveryRequest<T::AccountId, BlockNumberFor<T>>,
        OptionQuery,
    >;

    /// Confirmations per recovery request: (request_id, token_id) → bool.
    #[pallet::storage]
    pub type RecoveryConfirmations<T: Config> = StorageDoubleMap<
        _,
        Blake2_128Concat,
        BoundedVec<u8, ConstU32<64>>,
        Blake2_128Concat,
        u64,
        bool,
        ValueQuery,
    >;

    // =========================================================================
    // Events
    // =========================================================================

    #[pallet::event]
    #[pallet::generate_deposit(pub(super) fn deposit_event)]
    pub enum Event<T: Config> {
        /// 5 Recovery NFTs were minted for a group.
        RecoveryNftsMinted {
            group_id: BoundedVec<u8, ConstU32<64>>,
            target_account: T::AccountId,
            destination_kind: RecoveryDestination,
            vault_address: BoundedVec<u8, ConstU32<128>>,
            threshold: u8,
            total_issued: u8,
        },
        /// A recovery was initiated by an NFT holder.
        /// Owner should receive notification immediately.
        RecoveryInitiated {
            request_id: BoundedVec<u8, ConstU32<64>>,
            group_id: BoundedVec<u8, ConstU32<64>>,
            target_account: T::AccountId,
            initiated_by: T::AccountId,
            veto_deadline: BlockNumberFor<T>,
        },
        /// Owner vetoed the recovery.
        RecoveryVetoed {
            request_id: BoundedVec<u8, ConstU32<64>>,
            target_account: T::AccountId,
        },
        /// An NFT holder confirmed a recovery request.
        RecoveryConfirmed {
            request_id: BoundedVec<u8, ConstU32<64>>,
            token_id: u64,
            confirmer: T::AccountId,
            total_confirmations: u8,
            threshold: u8,
        },
        /// Threshold reached — funds should be transferred to vault_address.
        /// Backend listens to this event and executes the fund transfer.
        RecoveryExecuted {
            request_id: BoundedVec<u8, ConstU32<64>>,
            target_account: T::AccountId,
            destination_kind: RecoveryDestination,
            vault_address: BoundedVec<u8, ConstU32<128>>,
        },
    }

    // =========================================================================
    // Errors
    // =========================================================================

    #[pallet::error]
    pub enum Error<T> {
        GroupIdTooLong,
        VaultAddressTooLong,
        TokenNotFound,
        NotHolder,
        AlreadyConfirmed,
        RequestNotFound,
        RecoveryVetoWindowActive,
        RecoveryAlreadyExecuted,
        RecoveryVetoedByOwner,
        NotTargetAccount,
        TooManyHolders,
        ThresholdExceedsGroupSize,
        InvalidThreshold,
    }

    // =========================================================================
    // Extrinsics
    // =========================================================================

    #[pallet::call]
    impl<T: Config> Pallet<T> {
        // ─── issue_recovery_nfts ──────────────────────────────────────────────

        /// Mint a set of Recovery NFTs for an Arbad recovery group.
        ///
        /// Called by the account owner (or Creator).
        /// Mints `holders.len()` NFTs, one per trusted person.
        /// The `destination_kind` and `vault_address` are embedded in each NFT
        /// and CANNOT be changed after minting.
        ///
        /// ## Parameters
        /// - `group_id`        — unique identifier for this recovery group (UUID bytes)
        /// - `target_account`  — the account to recover (usually the caller themselves)
        /// - `holders`         — exactly 5 trusted addresses
        /// - `destination_kind`— PERSONAL_BANK_VAULT or FAMILY_VAULT
        /// - `vault_address`   — destination vault address (pre-programmed)
        /// - `threshold`       — min NFTs needed (usually 3)
        #[pallet::call_index(0)]
        #[pallet::weight(T::WeightInfo::issue_recovery_nfts())]
        pub fn issue_recovery_nfts(
            origin: OriginFor<T>,
            group_id: alloc::vec::Vec<u8>,
            target_account: T::AccountId,
            holders: alloc::vec::Vec<T::AccountId>,
            destination_kind: RecoveryDestination,
            vault_address: alloc::vec::Vec<u8>,
            threshold: u8,
        ) -> DispatchResult {
            let _issuer = T::IssuerOrigin::ensure_origin(origin)?;

            ensure!(threshold >= 1, Error::<T>::InvalidThreshold);
            ensure!(
                (threshold as usize) <= holders.len(),
                Error::<T>::ThresholdExceedsGroupSize
            );
            ensure!(
                holders.len() <= T::MaxGroupSize::get() as usize,
                Error::<T>::TooManyHolders
            );

            let group_id_bounded: BoundedVec<u8, ConstU32<64>> = group_id
                .try_into()
                .map_err(|_| Error::<T>::GroupIdTooLong)?;

            let vault_addr_bounded: BoundedVec<u8, ConstU32<128>> = vault_address
                .try_into()
                .map_err(|_| Error::<T>::VaultAddressTooLong)?;

            let total = holders.len() as u8;
            let now = frame_system::Pallet::<T>::block_number();
            let mut group_token_ids: alloc::vec::Vec<u64> = alloc::vec::Vec::new();

            for (i, holder) in holders.iter().enumerate() {
                let token_id = NextRecoveryTokenId::<T>::get();
                NextRecoveryTokenId::<T>::put(token_id.saturating_add(1));

                let nft = RecoveryNft {
                    token_id,
                    group_id: group_id_bounded.clone(),
                    target_account: target_account.clone(),
                    holder: holder.clone(),
                    destination_kind: destination_kind.clone(),
                    vault_address: vault_addr_bounded.clone(),
                    threshold,
                    total_issued: total,
                    group_index: (i + 1) as u8,
                    issued_at: now,
                    used_in_recovery: None,
                };

                RecoveryNfts::<T>::insert(token_id, &nft);

                // Update holder index
                HolderRecoveryNfts::<T>::mutate(holder, |ids| {
                    let _ = ids.try_push(token_id);
                });

                group_token_ids.push(token_id);
            }

            // Update group index
            GroupNfts::<T>::mutate(&group_id_bounded, |ids| {
                for tid in &group_token_ids {
                    let _ = ids.try_push(*tid);
                }
            });

            Self::deposit_event(Event::RecoveryNftsMinted {
                group_id: group_id_bounded,
                target_account,
                destination_kind,
                vault_address: vault_addr_bounded,
                threshold,
                total_issued: total,
            });

            Ok(())
        }

        // ─── initiate_recovery ────────────────────────────────────────────────

        /// NFT holder initiates recovery for the target account.
        ///
        /// ## Security Model
        ///
        /// The NFT IS the credential. The holder proves ownership by being
        /// the on-chain holder of a Recovery NFT for this group_id.
        /// No passphrase needed — the NFT contains all necessary information.
        ///
        /// Owner is IMMEDIATELY notified via the `RecoveryInitiated` event.
        /// Owner has `VetoWindowBlocks` to call `veto_recovery` before
        /// signature collection begins.
        #[pallet::call_index(1)]
        #[pallet::weight(T::WeightInfo::initiate_recovery())]
        pub fn initiate_recovery(
            origin: OriginFor<T>,
            token_id: u64,
            request_id: alloc::vec::Vec<u8>,
        ) -> DispatchResult {
            let caller = ensure_signed(origin)?;

            let nft = RecoveryNfts::<T>::get(token_id).ok_or(Error::<T>::TokenNotFound)?;
            ensure!(nft.holder == caller, Error::<T>::NotHolder);

            let request_id_bounded: BoundedVec<u8, ConstU32<64>> = request_id
                .try_into()
                .map_err(|_| Error::<T>::GroupIdTooLong)?;

            let now = frame_system::Pallet::<T>::block_number();
            let veto_deadline = now.saturating_add(T::VetoWindowBlocks::get().into());

            let request = RecoveryRequest {
                request_id: request_id_bounded.clone(),
                group_id: nft.group_id.clone(),
                target_account: nft.target_account.clone(),
                destination_kind: nft.destination_kind.clone(),
                vault_address: nft.vault_address.clone(),
                initiated_by: caller,
                initiated_at: now,
                veto_deadline_block: veto_deadline,
                status: RecoveryRequestStatus::PendingVetoWindow,
                confirmation_count: 1,
                threshold: nft.threshold,
            };

            RecoveryRequests::<T>::insert(&request_id_bounded, &request);

            // First confirmation from initiator
            RecoveryConfirmations::<T>::insert(&request_id_bounded, token_id, true);

            Self::deposit_event(Event::RecoveryInitiated {
                request_id: request_id_bounded,
                group_id: nft.group_id,
                target_account: nft.target_account,
                initiated_by: request.initiated_by,
                veto_deadline,
            });

            Ok(())
        }

        // ─── veto_recovery ────────────────────────────────────────────────────

        /// OWNER blocks the recovery within the veto window.
        ///
        /// Only the `target_account` (the account being recovered) can call this.
        /// This is their constitutional right to stop unauthorized recovery.
        #[pallet::call_index(2)]
        #[pallet::weight(T::WeightInfo::veto_recovery())]
        pub fn veto_recovery(
            origin: OriginFor<T>,
            request_id: alloc::vec::Vec<u8>,
        ) -> DispatchResult {
            let caller = ensure_signed(origin)?;

            let request_id_bounded: BoundedVec<u8, ConstU32<64>> = request_id
                .try_into()
                .map_err(|_| Error::<T>::GroupIdTooLong)?;

            RecoveryRequests::<T>::try_mutate(
                &request_id_bounded,
                |maybe_req| -> DispatchResult {
                    let req = maybe_req.as_mut().ok_or(Error::<T>::RequestNotFound)?;
                    ensure!(req.target_account == caller, Error::<T>::NotTargetAccount);
                    ensure!(
                        !matches!(req.status, RecoveryRequestStatus::Executed),
                        Error::<T>::RecoveryAlreadyExecuted,
                    );
                    req.status = RecoveryRequestStatus::OwnerVetoed;
                    Ok(())
                },
            )?;

            Self::deposit_event(Event::RecoveryVetoed {
                request_id: request_id_bounded,
                target_account: caller,
            });

            Ok(())
        }

        // ─── confirm_recovery ─────────────────────────────────────────────────

        /// NFT holder adds their confirmation to an active recovery request.
        ///
        /// When `confirmation_count >= threshold`, the pallet emits `RecoveryExecuted`.
        /// The backend listener then transfers funds from `target_account` to `vault_address`.
        #[pallet::call_index(3)]
        #[pallet::weight(T::WeightInfo::confirm_recovery())]
        pub fn confirm_recovery(
            origin: OriginFor<T>,
            token_id: u64,
            request_id: alloc::vec::Vec<u8>,
        ) -> DispatchResult {
            let caller = ensure_signed(origin)?;

            let nft = RecoveryNfts::<T>::get(token_id).ok_or(Error::<T>::TokenNotFound)?;
            ensure!(nft.holder == caller, Error::<T>::NotHolder);

            let request_id_bounded: BoundedVec<u8, ConstU32<64>> = request_id
                .try_into()
                .map_err(|_| Error::<T>::GroupIdTooLong)?;

            ensure!(
                !RecoveryConfirmations::<T>::get(&request_id_bounded, token_id),
                Error::<T>::AlreadyConfirmed,
            );

            RecoveryRequests::<T>::try_mutate(
                &request_id_bounded.clone(),
                |maybe_req| -> DispatchResult {
                    let req = maybe_req.as_mut().ok_or(Error::<T>::RequestNotFound)?;

                    ensure!(
                        !matches!(req.status, RecoveryRequestStatus::OwnerVetoed),
                        Error::<T>::RecoveryVetoedByOwner,
                    );
                    ensure!(
                        !matches!(req.status, RecoveryRequestStatus::Executed),
                        Error::<T>::RecoveryAlreadyExecuted,
                    );

                    // Verify NFT belongs to same group
                    ensure!(nft.group_id == req.group_id, Error::<T>::TokenNotFound);

                    // Check veto window — if still active, move to CollectingSignatures
                    let now = frame_system::Pallet::<T>::block_number();
                    if matches!(req.status, RecoveryRequestStatus::PendingVetoWindow) {
                        if now <= req.veto_deadline_block {
                            // Veto window still active — cannot confirm yet
                            return Err(Error::<T>::RecoveryVetoWindowActive.into());
                        }
                        req.status = RecoveryRequestStatus::CollectingSignatures;
                    }

                    // Add confirmation
                    RecoveryConfirmations::<T>::insert(&request_id_bounded, token_id, true);
                    req.confirmation_count = req.confirmation_count.saturating_add(1);

                    Self::deposit_event(Event::RecoveryConfirmed {
                        request_id: request_id_bounded.clone(),
                        token_id,
                        confirmer: caller,
                        total_confirmations: req.confirmation_count,
                        threshold: req.threshold,
                    });

                    // Threshold reached!
                    if req.confirmation_count >= req.threshold {
                        req.status = RecoveryRequestStatus::Executed;

                        // Backend listens to this event and executes fund transfer
                        Self::deposit_event(Event::RecoveryExecuted {
                            request_id: request_id_bounded,
                            target_account: req.target_account.clone(),
                            destination_kind: req.destination_kind.clone(),
                            vault_address: req.vault_address.clone(),
                        });
                    }

                    Ok(())
                },
            )?;

            Ok(())
        }
    }
}
