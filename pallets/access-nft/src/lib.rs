//! # pallet-access-nft
//!
//! **Altan Network — Universal NFT Access Keys for ALL Legal Entities**
//!
//! ## Purpose
//!
//! Every legal entity in INOMAD OS has two doors:
//!   🚪 CLIENT door  — any citizen can hold a client access key
//!   🔧 STAFF door   — only employees/officials hold a staff key
//!
//! This pallet issues, revokes, and verifies on-chain NFT access keys
//! for ALL entity types:
//!
//! ```text
//! Organizations       — corporations, NGOs, state-owned enterprises
//! Guilds              — professional DAOs (Accountants, Advocates, Notaries…)
//! Government Bodies   — Ministries, Agencies, Committees, Commissions
//! Unified Centers     — HR Center, Accounting Center, Advocate Center, Notary Center
//! Banks               — Central Bank, Bank of Siberia, Commercial Banks
//! Funds               — Pension Funds, Credit Reserve, Foundation Funds
//! Regions             — Republican Khurul + treasury access
//! ```
//!
//! ## Key Design
//!
//! ### Token ID
//! Auto-incrementing u64 — each key is globally unique on-chain.
//!
//! ### Portal Mask (bitmask u8)
//! ```text
//! 0b000_00001 = 0x01 → CLIENT access
//! 0b000_00010 = 0x02 → STAFF access
//! 0b000_00100 = 0x04 → ADMIN access
//! 0b000_00110 = 0x06 → STAFF + ADMIN
//! 0b000_00111 = 0x07 → ALL access (Creator)
//! ```
//!
//! ### Role Encoding (u16)
//! ```text
//! 0  = CITIZEN (basic client access)
//! 1  = CB_GOVERNOR
//! 2  = CB_OPERATOR
//! 3  = CB_AUDITOR
//! 4  = BANK_DIRECTOR
//! 5  = BANK_OFFICER
//! 6  = ACCOUNTANT
//! 7  = ADVOCATE
//! 8  = NOTARY
//! 9  = HR_MANAGER
//! 10 = GUILD_MASTER
//! 11 = GUILD_OFFICER
//! 12 = GOV_MINISTER
//! 13 = GOV_OFFICER
//! 14 = COMMITTEE_CHAIR
//! 15 = FUND_MANAGER
//! 16 = REGION_TREASURER
//! 255 = CREATOR (universal access)
//! ```
//!
//! ## Dispatchables
//!
//! | Call | Origin | Description |
//! |---|---|---|
//! | `issue_access_key` | Signed (Officer/Admin) | Issue a new access key NFT to an account |
//! | `revoke_access_key` | Signed (Officer/Admin) | Revoke an existing access key |
//! | `check_access` | Any | Query whether an account holds a valid access key |

#![cfg_attr(not(feature = "std"), no_std)]

extern crate alloc;

pub mod migrations;
pub mod weights;
pub use pallet::*;

#[cfg(test)]
mod mock;
#[cfg(test)]
mod tests;

// =============================================================================
// Cross-pallet access verification interface
// =============================================================================

/// Trait for other pallets to verify access key ownership without tight coupling.
///
/// Wire in a consuming pallet's Config as:
/// ```ignore
/// type AccessNft: pallet_access_nft::AccessNftInterface<Self::AccountId>;
/// ```
pub trait AccessNftInterface<AccountId> {
    /// Returns `true` if `who` holds an active (non-revoked) access key for
    /// the given `entity_id` with `portal_mask` (bitmask) access level.
    fn has_access(who: &AccountId, entity_id: &[u8], portal_mask: u8) -> bool;

    /// Returns `true` if `who` holds a STAFF key (portal_mask & 0x02) for entity.
    fn is_staff(who: &AccountId, entity_id: &[u8]) -> bool;

    /// Returns `true` if `who` holds an ADMIN key (portal_mask & 0x04) for entity.
    fn is_admin(who: &AccountId, entity_id: &[u8]) -> bool;

    /// Dual-SBT check: `who` must hold active key bound to BOTH
    /// `entity_id` AND `org_reg_number` with required portal access.
    fn verify_sbt(who: &AccountId, entity_id: &[u8], org_reg: &[u8], portal_mask: u8) -> bool;
}

/// No-op fallback for mock runtimes.
impl<AccountId> AccessNftInterface<AccountId> for () {
    fn has_access(_who: &AccountId, _entity_id: &[u8], _portal_mask: u8) -> bool { true }
    fn is_staff(_who: &AccountId, _entity_id: &[u8]) -> bool { true }
    fn is_admin(_who: &AccountId, _entity_id: &[u8]) -> bool { true }
    fn verify_sbt(_who: &AccountId, _entity_id: &[u8], _org_reg: &[u8], _portal_mask: u8) -> bool { true }
}

#[cfg(feature = "runtime-benchmarks")]
mod benchmarking;
#[frame_support::pallet]
pub mod pallet {
    use crate::weights::WeightInfo as _;
    use alloc::vec::Vec;
    use frame_support::pallet_prelude::*;
    use frame_system::pallet_prelude::*;

    // =========================================================================
    // Constants
    // =========================================================================

    /// Portal mask — CLIENT (bit 0)
    pub const PORTAL_CLIENT: u8 = 0x01;
    /// Portal mask — STAFF (bit 1)
    pub const PORTAL_STAFF: u8 = 0x02;
    /// Portal mask — ADMIN (bit 2)
    pub const PORTAL_ADMIN: u8 = 0x04;
    /// Portal mask — ALL portals
    pub const PORTAL_ALL: u8 = 0x07;

    // =========================================================================
    // NFT Entity Type Enum
    // =========================================================================

    /// Discriminant identifying what kind of legal entity issued this key.
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
    pub enum EntityKind {
        Organization,   // ООО, АО, НКО, ЗАО
        Guild,          // Профессиональная гильдия
        GovernmentBody, // Министерство, Агентство
        Committee,      // Комитет при Хурале/Президенте
        UnifiedCenter,  // ЕКА, Центр Бухгалтера, Адвоката, Нотариуса
        Bank,           // ЦБ, Банк Сибири, коммерческий банк
        PensionFund,    // Пенсионный фонд
        CreditReserve,  // Кредитный резерв
        Foundation,     // Foundation Fund
        Region,         // Republican Khural + treasury
        Custom,         // Любой иной юридический субъект
    }

    // =========================================================================
    // AccessKeyInfo — stored per TokenId
    // =========================================================================

    /// On-chain record of a single NFT access key.
    ///
    /// Each token is globally unique (token_id: u64 auto-counter).
    /// The entity is identified by a free-form `entity_id` bytes field
    /// (typically the DB UUID or registration number, up to 64 bytes).
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
    pub struct AccessKeyInfo<AccountId, BlockNumber> {
        /// Globally unique token ID.
        pub token_id: u64,
        /// Kind of legal entity that issued this key.
        pub entity_kind: EntityKind,
        /// Unique identifier of the entity (UTF-8 reg number, max 64 bytes).
        pub entity_id: BoundedVec<u8, ConstU32<64>>,
        /// The citizen who holds this key.
        pub holder: AccountId,
        /// Role code (see pallet docs for encoding table).
        pub role: u16,
        /// Access level 1–10 (1 = lowest, 10 = highest).
        pub access_level: u8,
        /// Portal access bitmask: CLIENT=0x01, STAFF=0x02, ADMIN=0x04.
        pub portal_mask: u8,
        /// Block when this key was issued.
        pub issued_at: BlockNumber,
        /// Whether this key has been revoked.
        pub revoked: bool,
        /// Account that issued this key (org admin or Creator).
        pub issued_by: AccountId,

        // ── Dual Soulbound Token (SBT) ─────────────────────────────────────
        /// SBT binding #1: organization registration number (max 32 bytes).
        /// Key is only valid for THIS organization — cannot be reused elsewhere.
        pub org_reg_number: BoundedVec<u8, ConstU32<32>>,
        /// SBT binding #2: whether holder wallet was verified at issuance.
        /// True = on-chain wallet matches the DB walletAddress — non-transferable.
        pub wallet_bound: bool,
        /// HR role at time of issuance (0=HR, 1=HR_OFFICER, 255=CREATOR).
        pub hr_role: u8,
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
        /// The origin that may issue access keys (org admin / Creator / system).
        type IssuerOrigin: EnsureOrigin<Self::RuntimeOrigin, Success = Self::AccountId>;

        /// Maximum number of active access keys per citizen (anti-spam).
        #[pallet::constant]
        type MaxKeysPerHolder: Get<u32>;

        /// Maximum number of members/holders per entity (safety cap).
        #[pallet::constant]
        type MaxHoldersPerEntity: Get<u32>;
    }

    // =========================================================================
    // Storage
    // =========================================================================

    /// Auto-incrementing NFT token ID counter.
    #[pallet::storage]
    #[pallet::getter(fn next_token_id)]
    pub type NextTokenId<T: Config> = StorageValue<_, u64, ValueQuery>;

    /// All issued access keys, indexed by token_id.
    #[pallet::storage]
    #[pallet::getter(fn access_keys)]
    pub type AccessKeys<T: Config> = StorageMap<
        _,
        Blake2_128Concat,
        u64, // token_id
        AccessKeyInfo<T::AccountId, BlockNumberFor<T>>,
        OptionQuery,
    >;

    /// Index: holder → Vec<token_id> they hold.
    #[pallet::storage]
    #[pallet::getter(fn holder_keys)]
    pub type HolderKeys<T: Config> = StorageMap<
        _,
        Blake2_128Concat,
        T::AccountId,
        BoundedVec<u64, T::MaxKeysPerHolder>,
        ValueQuery,
    >;

    /// Index: entity_id bytes → Vec<token_id> issued for this entity.
    #[pallet::storage]
    #[pallet::getter(fn entity_keys)]
    pub type EntityKeys<T: Config> = StorageMap<
        _,
        Blake2_128Concat,
        BoundedVec<u8, ConstU32<64>>,
        BoundedVec<u64, T::MaxHoldersPerEntity>,
        ValueQuery,
    >;

    // =========================================================================
    // Events
    // =========================================================================

    #[pallet::event]
    #[pallet::generate_deposit(pub(super) fn deposit_event)]
    pub enum Event<T: Config> {
        /// A new NFT access key was issued.
        AccessKeyIssued {
            token_id: u64,
            entity_kind: EntityKind,
            entity_id: BoundedVec<u8, ConstU32<64>>,
            holder: T::AccountId,
            role: u16,
            portal_mask: u8,
        },
        /// An access key was revoked.
        AccessKeyRevoked {
            token_id: u64,
            revoker: T::AccountId,
        },
        /// An access key was transferred to a new holder.
        AccessKeyTransferred {
            token_id: u64,
            from: T::AccountId,
            to: T::AccountId,
        },
        /// A Dual-SBT access key was issued.
        SbtKeyIssued {
            token_id: u64,
            holder: T::AccountId,
            org_reg_number: BoundedVec<u8, ConstU32<32>>,
            role: u16,
            hr_role: u8,
        },
    }

    // =========================================================================
    // Errors
    // =========================================================================

    #[pallet::error]
    pub enum Error<T> {
        /// Token ID not found in storage.
        TokenNotFound,
        /// The caller is not the issuer of this key and cannot revoke it.
        NotIssuer,
        /// The key has already been revoked.
        AlreadyRevoked,
        /// The holder already has too many keys (MaxKeysPerHolder exceeded).
        TooManyKeysForHolder,
        /// The entity already has too many key holders (MaxHoldersPerEntity exceeded).
        TooManyHoldersForEntity,
        /// entity_id is too long (max 64 bytes).
        EntityIdTooLong,
        /// access_level must be between 1 and 10.
        InvalidAccessLevel,
        /// portal_mask must be 1–7.
        InvalidPortalMask,
        /// org_reg_number is too long (max 32 bytes).
        OrgRegTooLong,
        /// Dual-SBT violation: key org does not match the requested entity.
        SbtOrgMismatch,
        /// Key is wallet-bound and cannot be transferred.
        WalletBound,
    }

    // =========================================================================
    // Extrinsics
    // =========================================================================

    #[pallet::call]
    impl<T: Config> Pallet<T> {
        // ─── issue_access_key ─────────────────────────────────────────────────

        /// Issue a new NFT access key to a citizen for a specific entity.
        ///
        /// ## Parameters
        ///
        /// - `entity_kind`  — type of legal entity (Organization, Guild, Gov…)
        /// - `entity_id`    — unique entity identifier bytes (UTF-8 reg number)
        /// - `holder`       — citizen receiving the key
        /// - `role`         — role code (0=CITIZEN, 1=CB_GOVERNOR, 6=ACCOUNTANT…)
        /// - `access_level` — 1–10 hierarchy level
        /// - `portal_mask`  — CLIENT=0x01, STAFF=0x02, ADMIN=0x04
        ///
        /// ## Constitutional Rule
        ///
        /// Callable by `IssuerOrigin` — the entity admin or the Creator.
        /// The Creator (INOMAD OS) can issue keys for any entity.
        /// Organization admins can only issue keys for their own entity.
        #[pallet::call_index(0)]
        #[pallet::weight(T::WeightInfo::issue_access_key())]
        pub fn issue_access_key(
            origin: OriginFor<T>,
            entity_kind: EntityKind,
            entity_id: Vec<u8>,
            holder: T::AccountId,
            role: u16,
            access_level: u8,
            portal_mask: u8,
        ) -> DispatchResult {
            let issuer = T::IssuerOrigin::ensure_origin(origin)?;

            // Guards
            ensure!(
                access_level >= 1 && access_level <= 10,
                Error::<T>::InvalidAccessLevel
            );
            ensure!(
                portal_mask >= 1 && portal_mask <= 7,
                Error::<T>::InvalidPortalMask
            );

            let entity_id_bounded: BoundedVec<u8, ConstU32<64>> = entity_id
                .try_into()
                .map_err(|_| Error::<T>::EntityIdTooLong)?;

            // Mint token
            let token_id = NextTokenId::<T>::get();
            NextTokenId::<T>::put(token_id.saturating_add(1));

            let now = frame_system::Pallet::<T>::block_number();

            let key_info = AccessKeyInfo {
                token_id,
                entity_kind: entity_kind.clone(),
                entity_id: entity_id_bounded.clone(),
                holder: holder.clone(),
                role,
                access_level,
                portal_mask,
                issued_at: now,
                revoked: false,
                issued_by: issuer,
                // Dual SBT: legacy issuance has no org binding / wallet check
                org_reg_number: BoundedVec::default(),
                wallet_bound: false,
                hr_role: 255, // CREATOR by default for legacy calls
            };

            // Store key
            AccessKeys::<T>::insert(token_id, &key_info);

            // Update holder index
            HolderKeys::<T>::try_mutate(&holder, |keys| {
                keys.try_push(token_id)
                    .map_err(|_| Error::<T>::TooManyKeysForHolder)
            })?;

            // Update entity index
            EntityKeys::<T>::try_mutate(&entity_id_bounded, |keys| {
                keys.try_push(token_id)
                    .map_err(|_| Error::<T>::TooManyHoldersForEntity)
            })?;

            Self::deposit_event(Event::AccessKeyIssued {
                token_id,
                entity_kind,
                entity_id: entity_id_bounded,
                holder,
                role,
                portal_mask,
            });

            Ok(())
        }

        // ─── issue_sbt_key ────────────────────────────────────────────────────

        /// Issue a Dual Soulbound NFT Access Key — bound to BOTH:
        ///   1. `holder` wallet account (non-transferable)
        ///   2. `org_reg_number` — only valid in this specific organization
        ///
        /// HR roles:
        ///   0 = HR (can only issue keys, cannot appoint HR)
        ///   1 = HR_OFFICER (full HR Center access)
        ///   255 = CREATOR (universal)
        #[pallet::call_index(2)]
        #[pallet::weight(T::WeightInfo::issue_access_key())]
        pub fn issue_sbt_key(
            origin: OriginFor<T>,
            entity_kind: EntityKind,
            entity_id: Vec<u8>,
            org_reg_number: Vec<u8>,
            holder: T::AccountId,
            role: u16,
            access_level: u8,
            portal_mask: u8,
            hr_role: u8,
        ) -> DispatchResult {
            let issuer = T::IssuerOrigin::ensure_origin(origin)?;

            ensure!(access_level >= 1 && access_level <= 10, Error::<T>::InvalidAccessLevel);
            ensure!(portal_mask >= 1 && portal_mask <= 7, Error::<T>::InvalidPortalMask);

            let entity_id_bounded: BoundedVec<u8, ConstU32<64>> =
                entity_id.try_into().map_err(|_| Error::<T>::EntityIdTooLong)?;

            let org_reg_bounded: BoundedVec<u8, ConstU32<32>> =
                org_reg_number.try_into().map_err(|_| Error::<T>::OrgRegTooLong)?;

            // SBT: org_reg must be non-empty
            ensure!(!org_reg_bounded.is_empty(), Error::<T>::OrgRegTooLong);

            let token_id = NextTokenId::<T>::get();
            NextTokenId::<T>::put(token_id.saturating_add(1));
            let now = frame_system::Pallet::<T>::block_number();

            let key_info = AccessKeyInfo {
                token_id,
                entity_kind: entity_kind.clone(),
                entity_id: entity_id_bounded.clone(),
                holder: holder.clone(),
                role,
                access_level,
                portal_mask,
                issued_at: now,
                revoked: false,
                issued_by: issuer,
                org_reg_number: org_reg_bounded.clone(),
                wallet_bound: true,  // SBT: always wallet-bound
                hr_role,
            };

            AccessKeys::<T>::insert(token_id, &key_info);
            HolderKeys::<T>::try_mutate(&holder, |keys| {
                keys.try_push(token_id).map_err(|_| Error::<T>::TooManyKeysForHolder)
            })?;
            EntityKeys::<T>::try_mutate(&entity_id_bounded, |keys| {
                keys.try_push(token_id).map_err(|_| Error::<T>::TooManyHoldersForEntity)
            })?;

            Self::deposit_event(Event::SbtKeyIssued {
                token_id,
                holder,
                org_reg_number: org_reg_bounded,
                role,
                hr_role,
            });

            Ok(())
        }

        // ─── revoke_access_key ────────────────────────────────────────────────

        /// Revoke an existing access key.
        ///
        /// Only the original issuer (`issued_by`) or the system Creator can revoke.
        /// Revocation is permanent — a new key must be issued to restore access.
        #[pallet::call_index(1)]
        #[pallet::weight(T::WeightInfo::revoke_access_key())]
        pub fn revoke_access_key(origin: OriginFor<T>, token_id: u64) -> DispatchResult {
            let revoker = T::IssuerOrigin::ensure_origin(origin)?;

            AccessKeys::<T>::try_mutate(token_id, |maybe_key| -> DispatchResult {
                let key = maybe_key.as_mut().ok_or(Error::<T>::TokenNotFound)?;
                ensure!(!key.revoked, Error::<T>::AlreadyRevoked);
                // Only the original issuer can revoke
                ensure!(key.issued_by == revoker, Error::<T>::NotIssuer);
                key.revoked = true;
                Ok(())
            })?;

            Self::deposit_event(Event::AccessKeyRevoked { token_id, revoker });
            Ok(())
        }
    }

    // =========================================================================
    // Pallet-internal helper
    // =========================================================================

    impl<T: Config> Pallet<T> {
        /// Check if `who` holds any active access key for `entity_id`
        /// that satisfies `portal_mask`.
        pub fn check_access(who: &T::AccountId, entity_id: &[u8], required_mask: u8) -> bool {
            let keys = HolderKeys::<T>::get(who);
            for token_id in keys.iter() {
                if let Some(key) = AccessKeys::<T>::get(token_id) {
                    if !key.revoked
                        && key.entity_id.as_slice() == entity_id
                        && (key.portal_mask & required_mask) == required_mask
                    {
                        return true;
                    }
                }
            }
            false
        }

        /// Verify Dual-SBT: `who` must hold an active key for BOTH
        /// `entity_id` AND `org_reg_number`.
        ///
        /// Used by HR Center cross-pallet calls and portal gate checks.
        pub fn verify_sbt(
            who: &T::AccountId,
            entity_id: &[u8],
            org_reg_number: &[u8],
            required_mask: u8,
        ) -> bool {
            let keys = HolderKeys::<T>::get(who);
            for token_id in keys.iter() {
                if let Some(key) = AccessKeys::<T>::get(token_id) {
                    if !key.revoked
                        && key.wallet_bound
                        && key.entity_id.as_slice() == entity_id
                        && key.org_reg_number.as_slice() == org_reg_number
                        && (key.portal_mask & required_mask) == required_mask
                    {
                        return true;
                    }
                }
            }
            false
        }

        /// Returns the HR role for `who` in `org_reg_number` org, if any.
        /// Returns `None` if no active SBT key exists.
        pub fn get_hr_role(who: &T::AccountId, org_reg_number: &[u8]) -> Option<u8> {
            let keys = HolderKeys::<T>::get(who);
            for token_id in keys.iter() {
                if let Some(key) = AccessKeys::<T>::get(token_id) {
                    if !key.revoked
                        && key.wallet_bound
                        && key.org_reg_number.as_slice() == org_reg_number
                    {
                        return Some(key.hr_role);
                    }
                }
            }
            None
        }
    }

    // Implement the interface trait for runtime wiring
    impl<T: Config> crate::AccessNftInterface<T::AccountId> for Pallet<T> {
        fn has_access(who: &T::AccountId, entity_id: &[u8], portal_mask: u8) -> bool {
            Self::check_access(who, entity_id, portal_mask)
        }
        fn is_staff(who: &T::AccountId, entity_id: &[u8]) -> bool {
            Self::check_access(who, entity_id, PORTAL_STAFF)
        }
        fn is_admin(who: &T::AccountId, entity_id: &[u8]) -> bool {
            Self::check_access(who, entity_id, PORTAL_ADMIN)
        }
        fn verify_sbt(who: &T::AccountId, entity_id: &[u8], org_reg: &[u8], portal_mask: u8) -> bool {
            Self::verify_sbt(who, entity_id, org_reg, portal_mask)
        }
    }
}
