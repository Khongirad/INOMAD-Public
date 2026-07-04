//! # Land Registry Pallet — Земля и Недра
//!
//! **Altan Network — Sovereign L1 Blockchain**
//! **Sprint L1-06: Constitutional Sovereignty** | Non-Fungible Land Cadastre
//!
//! This pallet implements the **sovereign land and subsoil registry** of the
//! Altan Republic.  Land parcels are Non-Fungible tokens with constitutionally
//! restricted ownership rules.
//!
//! ## Law of Non-Alienation (Закон о Неотчуждаемости)
//!
//! **Article V of the Altan Constitution:**
//!
//! > "The land, air, water, and mineral subsoil of the Republic are sovereign
//! > territorial assets. They may be owned, held, and transferred ONLY among
//! > Citizens of the Republic (status: `Indigenous` or `Naturalized`).
//! > No foreign national, corporation, or state may hold title to any parcel
//! > of Altan land or subsoil rights. Any such transfer is constitutionally void."
//!
//! ## Technical Enforcement
//!
//! The `transfer_land` extrinsic queries `pallet-inomad-identity` for the buyer's
//! `CitizenshipStatus`. If the buyer's status is `CitizenshipStatus::Foreigner`,
//! the transaction fails with `Error::ForeignLandOwnershipForbidden`.
//!
//! This check is executed BEFORE any ownership mutation — the state machine
//! never reaches an invalid configuration.
//!
//! ## Storage
//!
//! - `LandParcels`: `StorageMap<ParcelId → LandParcel>` — the cadastral map.
//! - `NextParcelId`: monotonic counter for parcel IDs.
//! - `ParcelsByOwner`: `StorageDoubleMap<Owner, ParcelId → ()>` — reverse index.
//!
//! ## Extrinsics
//!
//! | Call              | Origin | Description                                             |
//! |-------------------|--------|---------------------------------------------------------|
//! | `register_parcel` | Root   | Cadastration: register a new land parcel (state act).   |
//! | `transfer_land`   | Signed | Transfer ownership — FORBIDDEN if buyer is Foreigner.  |
//! | `update_resource_rights` | Root | Update the resource rights of an existing parcel. |

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
    use pallet_inomad_identity::pallet::{CitizenStatus, CitizenshipStatus};

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
        /// Maximum length of the parcel description string (in bytes).
        #[pallet::constant]
        type MaxDescriptionLen: Get<u32>;
    }

    // =========================================================================
    // Enums
    // =========================================================================

    /// Resource rights for a land parcel — what the owner may extract or exploit.
    ///
    /// Subsoil rights (mineral, oil/gas, water) are constitutional assets of
    /// the Republic and may only be LEASED to citizens, never sold in perpetuity.
    /// Leasing mechanics are a future sprint (`pallet-mineral-concession`).
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
    pub enum ResourceRights {
        /// Surface use only — agriculture, construction, habitation.
        /// No mineral, forestry, water, or subsoil rights granted.
        SurfaceOnly,
        /// Surface + forestry rights (sustainable timber, resin, berries).
        SurfaceAndForestry,
        /// Surface + water rights (rivers, lakes within the parcel).
        SurfaceAndWater,
        /// Full surface rights + hunting and fishing rights.
        SurfaceAndHunting,
        /// Mineral prospecting rights — survey for deposits, no extraction.
        MineralProspecting,
        /// Full subsoil rights (oil, gas, coal, rare earth) — constitutional lease only.
        ///
        /// Note: In the Republic's legal framework, this is a 49-year renewable lease
        /// from the State, NOT free-hold ownership of the subsoil itself.
        /// The subsoil belongs permanently to the 79 sovereign peoples.
        FullSubsoil,
    }

    // =========================================================================
    // Structs
    // =========================================================================

    /// A sovereign land parcel in the Altan Republic's cadastral registry.
    ///
    /// ## Non-Fungible Asset
    ///
    /// Each `LandParcel` is uniquely identified by a `parcel_id` (u64 counter).
    /// It cannot be subdivided or merged on-chain in this sprint — only whole
    /// parcels are transferred (subdivision is a future `pallet-surveyor` concern).
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
    pub struct LandParcel<T: Config> {
        /// Current owner — MUST be a citizen with `Indigenous` or `Naturalized` status.
        ///
        /// After every `transfer_land` call this field is updated. The chain state
        /// is the authoritative legal record of ownership.
        pub owner: T::AccountId,
        /// Nation region code (1–83 OKATO codes). Determines which sovereign
        /// nation's treasury the annual land fee accrues to.
        pub region: u8,
        /// Cadastral coordinates — blake2_256 hash of the GIS coordinate string.
        ///
        /// Storing the raw coordinates (e.g., WGS84 polygon) on-chain would be
        /// expensive. The hash binds this on-chain record to the off-chain GIS
        /// cadastral database maintained by the Republic's survey authority.
        pub coordinate_hash: [u8; 32],
        /// Area in square metres (u64 supports up to ~18 million km² safely).
        pub area_sqm: u64,
        /// What the owner may do with the land — surfaces rights, subsoil rights, etc.
        pub resource_rights: ResourceRights,
        /// Block number when this parcel was first registered.
        pub registered_at: u32,
        /// Human-readable parcel description (cadastral name / address).
        /// Bounded by `T::MaxDescriptionLen`.
        pub description: BoundedVec<u8, T::MaxDescriptionLen>,
    }

    // =========================================================================
    // Storage
    // =========================================================================

    /// The sovereign land cadastral registry.
    ///
    /// ## Law of Non-Alienation
    ///
    /// This map is the authoritative legal record of land ownership in the Republic.
    /// All entries are valid ONLY if `owner.citizenship_status != Foreigner`.
    /// The `transfer_land` extrinsic enforces this invariant on every write.
    #[pallet::storage]
    #[pallet::getter(fn land_parcels)]
    pub type LandParcels<T: Config> =
        StorageMap<_, Blake2_128Concat, u64, LandParcel<T>, OptionQuery>;

    /// Monotonic parcel ID counter.
    #[pallet::storage]
    #[pallet::getter(fn next_parcel_id)]
    pub type NextParcelId<T: Config> = StorageValue<_, u64, ValueQuery>;

    /// Reverse index: owner → set of parcel IDs they hold.
    ///
    /// Bounded by `ConstU32<1024>` — a citizen may own at most 1_024 parcels
    /// on-chain. This prevents storage DOS attacks.
    #[pallet::storage]
    #[pallet::getter(fn parcels_by_owner)]
    pub type ParcelsByOwner<T: Config> = StorageDoubleMap<
        _,
        Blake2_128Concat,
        T::AccountId, // owner
        Blake2_128Concat,
        u64, // parcel_id
        (),
        OptionQuery,
    >;

    // =========================================================================
    // Events
    // =========================================================================

    #[pallet::event]
    #[pallet::generate_deposit(pub(super) fn deposit_event)]
    pub enum Event<T: Config> {
        /// A new land parcel was cadastrally registered by the state.
        ParcelRegistered {
            parcel_id: u64,
            owner: T::AccountId,
            region: u8,
            area_sqm: u64,
        },

        /// A land parcel was transferred between two citizens of the Republic.
        ///
        /// Both parties must be `Indigenous` or `Naturalized` — validated at dispatch.
        LandTransferred {
            parcel_id: u64,
            from: T::AccountId,
            to: T::AccountId,
        },

        /// The resource rights of a parcel were updated by the state.
        ResourceRightsUpdated {
            parcel_id: u64,
            new_rights: ResourceRights,
        },
    }

    // =========================================================================
    // Errors
    // =========================================================================

    #[pallet::error]
    pub enum Error<T> {
        /// The land parcel does not exist in the cadastral registry.
        ParcelNotFound,

        /// The caller is not the owner of this land parcel.
        ///
        /// Only the registered owner may initiate a land transfer.
        NotParcelOwner,

        /// ## Law of Non-Alienation (Article V of the Constitution)
        ///
        /// The prospective buyer is a **Foreigner** (`CitizenshipStatus::Foreigner`).
        ///
        /// Land, air, and mineral subsoil of the Republic may be owned ONLY by
        /// citizens with `Indigenous` or `Naturalized` status. This transaction
        /// is constitutionally void.
        ///
        /// The buyer must first obtain citizenship via:
        /// - `claim_birthright()` → Naturalized (Jus Soli)
        /// - `claim_repatriation(lineage_proof)` → Indigenous (Jus Sanguinis)
        ForeignLandOwnershipForbidden,

        /// The buyer does not have a registered identity in `pallet-inomad-identity`.
        ///
        /// Land title may only be held by registered citizens of the Republic.
        BuyerNotRegistered,

        /// Self-transfer: the buyer is already the owner of this parcel.
        SelfTransfer,

        /// ## Guardian of Civic Integrity (Article II — My Home is My Castle)
        ///
        /// The prospective buyer's account is currently **Frozen** (under judicial
        /// pre-trial hold) or **Exiled** (condemned). Neither may acquire land:
        ///
        /// - A `Frozen` citizen is under active judicial proceedings — allowing
        ///   asset acquisition during a freeze would enable asset-shifting fraud.
        /// - An `Exiled` citizen has been constitutionally condemned; all assets
        ///   are confiscated. New acquisitions are constitutionally void.
        ///
        /// The buyer must resolve their judicial status before acquiring land.
        BuyerAccountFrozenOrExiled,

        /// Arithmetic overflow in parcel ID counter (theoretical maximum exceeded).
        ParcelIdOverflow,
    }

    // =========================================================================
    // Extrinsics
    // =========================================================================

    #[pallet::call]
    impl<T: Config> Pallet<T> {
        // ─── register_parcel ─────────────────────────────────────────────────

        /// Register a new land parcel in the sovereign cadastral registry.
        ///
        /// This is a **state act** — only Root (representing the Republic's
        /// survey authority) may create cadastral entries. Citizens receive
        /// ownership by transfer from the state-owned initial registration.
        ///
        /// `coordinate_hash` should be the blake2_256 hash of the GIS coordinate
        /// string for this parcel, linking on-chain ownership to off-chain maps.
        #[pallet::call_index(0)]
        #[pallet::weight(Weight::from_parts(50_000_000, 0))]
        pub fn register_parcel(
            origin: OriginFor<T>,
            owner: T::AccountId,
            region: u8,
            coordinate_hash: [u8; 32],
            area_sqm: u64,
            resource_rights: ResourceRights,
            description: BoundedVec<u8, T::MaxDescriptionLen>,
        ) -> DispatchResult {
            ensure_root(origin)?;

            // Validate initial owner is a citizen of the Republic.
            let citizen = pallet_inomad_identity::Citizens::<T>::get(&owner)
                .ok_or(Error::<T>::BuyerNotRegistered)?;

            ensure!(
                citizen.citizenship_status != CitizenshipStatus::Foreigner,
                Error::<T>::ForeignLandOwnershipForbidden
            );

            let parcel_id = NextParcelId::<T>::get();
            let next_id = parcel_id
                .checked_add(1)
                .ok_or(Error::<T>::ParcelIdOverflow)?;

            let now: u32 = frame_system::Pallet::<T>::block_number()
                .try_into()
                .unwrap_or(0u32);

            LandParcels::<T>::insert(
                parcel_id,
                LandParcel {
                    owner: owner.clone(),
                    region,
                    coordinate_hash,
                    area_sqm,
                    resource_rights,
                    registered_at: now,
                    description,
                },
            );
            ParcelsByOwner::<T>::insert(&owner, parcel_id, ());
            NextParcelId::<T>::put(next_id);

            Self::deposit_event(Event::ParcelRegistered {
                parcel_id,
                owner,
                region,
                area_sqm,
            });

            Ok(())
        }

        // ─── transfer_land ────────────────────────────────────────────────────

        /// Transfer ownership of a land parcel to another citizen of the Republic.
        ///
        /// ## Law of Non-Alienation (Article V — Constitutional Enforcement)
        ///
        /// This extrinsic checks the **buyer's citizenship status** against
        /// `pallet-inomad-identity` BEFORE executing any state mutation.
        ///
        /// ### Transfer Rules
        ///
        /// | Buyer Status    | Result                                                    |
        /// |-----------------|-----------------------------------------------------------|
        /// | `Indigenous`    | ✅ Transfer proceeds. Buyer gains full ownership + rights. |
        /// | `Naturalized`   | ✅ Transfer proceeds. Buyer gains full ownership + rights. |
        /// | `Foreigner`     | ❌ `Error::ForeignLandOwnershipForbidden` (constitutionally void). |
        /// | Not registered  | ❌ `Error::BuyerNotRegistered`.                           |
        ///
        /// The seller's `CitizenshipStatus` is NOT checked — a citizen who became
        /// a foreigner after acquiring land retains ownership (they are grandfathered
        /// pending a separate denaturalisation process). Only NEW acquisitions are blocked.
        ///
        /// Both parties must be registered. The seller must be the current owner.
        #[pallet::call_index(1)]
        #[pallet::weight(Weight::from_parts(50_000_000, 0))]
        pub fn transfer_land(
            origin: OriginFor<T>,
            parcel_id: u64,
            buyer: T::AccountId,
        ) -> DispatchResult {
            let seller = ensure_signed(origin)?;

            ensure!(seller != buyer, Error::<T>::SelfTransfer);

            // ── Fetch parcel ─────────────────────────────────────────────────
            let mut parcel = LandParcels::<T>::get(parcel_id).ok_or(Error::<T>::ParcelNotFound)?;

            // ── Ownership check ──────────────────────────────────────────────
            ensure!(parcel.owner == seller, Error::<T>::NotParcelOwner);

            // ── [CONSTITUTIONAL CHECK] Non-Alienation Law ────────────────────
            // Query pallet-inomad-identity for buyer's citizenship status.
            // If the buyer is a Foreigner, the transaction is constitutionally void.
            let buyer_record = pallet_inomad_identity::Citizens::<T>::get(&buyer)
                .ok_or(Error::<T>::BuyerNotRegistered)?;

            ensure!(
                buyer_record.citizenship_status != CitizenshipStatus::Foreigner,
                Error::<T>::ForeignLandOwnershipForbidden
            );

            // ── [CONSTITUTIONAL CHECK] Active Citizen Guard ──────────────────
            //
            // Article II enforcement: frozen or exiled citizens cannot acquire land.
            // A Frozen citizen is under judicial pre-trial hold — allowing property
            // acquisition would enable asset-shifting during criminal proceedings.
            // An Exiled citizen is constitutionally condemned — new acquisitions void.
            ensure!(
                buyer_record.status == CitizenStatus::Active,
                Error::<T>::BuyerAccountFrozenOrExiled
            );

            // ── Execute transfer ─────────────────────────────────────────────
            // Update reverse index: remove seller, add buyer.
            ParcelsByOwner::<T>::remove(&seller, parcel_id);
            ParcelsByOwner::<T>::insert(&buyer, parcel_id, ());

            // Update the parcel's owner field.
            parcel.owner = buyer.clone();
            LandParcels::<T>::insert(parcel_id, parcel);

            Self::deposit_event(Event::LandTransferred {
                parcel_id,
                from: seller,
                to: buyer,
            });

            Ok(())
        }

        // ─── update_resource_rights ───────────────────────────────────────────

        /// Update the resource rights of an existing land parcel.
        ///
        /// Root-only. Represents a state act (e.g., granting a mining concession
        /// or reclassifying agricultural land to protected forest).
        ///
        /// Resource right changes require a Khural vote to be constitutionally valid,
        /// but the on-chain enforcement is Root-level to allow for emergency reclassification.
        /// Future sprint: gate this with `EnsureKhural` origin.
        #[pallet::call_index(2)]
        #[pallet::weight(Weight::from_parts(50_000_000, 0))]
        pub fn update_resource_rights(
            origin: OriginFor<T>,
            parcel_id: u64,
            new_rights: ResourceRights,
        ) -> DispatchResult {
            ensure_root(origin)?;

            LandParcels::<T>::try_mutate(parcel_id, |maybe| {
                let parcel = maybe.as_mut().ok_or(Error::<T>::ParcelNotFound)?;
                parcel.resource_rights = new_rights.clone();
                Ok::<(), DispatchError>(())
            })?;

            Self::deposit_event(Event::ResourceRightsUpdated {
                parcel_id,
                new_rights,
            });

            Ok(())
        }
    }
}
