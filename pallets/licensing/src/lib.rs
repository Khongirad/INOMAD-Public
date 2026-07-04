//! # Licensing Pallet — Государственная Система Лицензирования
//!
//! **Altan Network — Sovereign L1 Blockchain**
//! **Sprint L1-08: Executive Branch & Khural Licensing**
//!
//! Implements the **constitutional licensing pipeline** of the Altan Republic.
//!
//! ## Constitutional Design
//!
//! Article §2.6 enforces strict branch separation:
//!
//! ```text
//! Executive Branch (Ministries / Committees / Agencies)
//!   └── submit_license_application()   ← only Root-authorized organs
//!           ↓
//!   [LicenseApp: PendingKhural]        ← stores audit_hash (IPFS)
//!           ↓ voting period (7 days)
//! Legislative Branch (Khural Delegates)
//!   └── vote_on_license()              ← KhuralDelegate of the nation
//!           ↓ on_initialize auto-enacts
//!   [License: Active] OR [LicenseApp: Rejected]
//!           ↓
//! Judicial Branch (Courts)
//!   └── revoke_license()               ← ONLY via judicial verdict
//! ```
//!
//! ## Constitutional Constraints (HARD-CODED)
//!
//! - **Max license duration**: 10 years (`BLOCKS_PER_YEAR * 10`)
//! - **Renewal**: via new application + Khural vote
//! - **Revocation**: only `EnsureJudicial` origin (§2.3 — No administrative seizure)
//! - **Land Fund veto**: indigenous nation of the parcel may block allocations
//! - **Weapons / Nuclear**: Confederate Khural, 2/3 quorum
//!
//! ## Storage
//!
//! - `AuthorizedMinistries`: Root-managed whitelist of authorized submitters
//! - `LicenseApplications`: Pending Khural votes
//! - `LicenseAppsEnding`: Index for `on_initialize` auto-enactment
//! - `HasVotedOnLicense`: Double-spend voting guard
//! - `Licenses`: Active/expired license registry
//! - `LicensesByOrg`: Reverse index org → license IDs
//! - `NextLicenseAppId`, `NextLicenseId`: Monotonic ID counters
//!
//! ## Extrinsics
//!
//! | Call | Origin | Description |
//! |------|--------|-------------|
//! | `authorize_ministry` | Root | Add/remove an organ from the whitelist |
//! | `submit_license_application` | Authorized Ministry | Submit for Khural vote |
//! | `vote_on_license` | KhuralDelegate (nation match) | Cast approve/reject |
//! | `revoke_license` | Root (Judicial verdict hook) | Revoke an active license |
//! | `renew_license` | Authorized Ministry | Start renewal application |
//! | `indigenous_veto` | KhuralDelegate (indigenous nation) | Veto Land Fund allocation |

#![cfg_attr(not(feature = "std"), no_std)]

extern crate alloc;

pub mod migrations;
pub mod weights;
pub use pallet::*;

#[cfg(test)]
mod mock;
#[cfg(test)]
mod tests;

// ─── Constitutional Block-Time Constants ─────────────────────────────────────

/// 6-second blocks per year.
pub const BLOCKS_PER_YEAR: u32 = 5_256_000;

/// Constitutional maximum for any license: 10 years.
/// Longer concessions require renewal via new Khural vote.
pub const MAX_LICENSE_BLOCKS: u32 = BLOCKS_PER_YEAR * 10;

/// Default license duration if caller does not specify (3 years).
pub const DEFAULT_LICENSE_BLOCKS: u32 = BLOCKS_PER_YEAR * 3;

/// Confederal voting period for Weapons / Nuclear licenses (7 days).
pub const CONFEDERAL_VOTING_PERIOD: u32 = 100_800;

// ─── Cross-pallet Judicial Hook ───────────────────────────────────────────────

/// Hook called by `pallet-judicial-courts` to revoke a license after a verdict.
///
/// Wire at runtime: `type LicensingInterface = pallet_licensing::Pallet<Runtime>;`
pub trait LicensingInterface<AccountId> {
    /// Revoke the license `license_id` held by `holder`.
    /// Called only from a successfully executed judicial verdict.
    fn judicial_revoke(
        license_id: u32,
        holder: &AccountId,
        reason: alloc::vec::Vec<u8>,
    ) -> frame_support::dispatch::DispatchResult;
}

#[cfg(feature = "runtime-benchmarks")]
mod benchmarking;
#[frame_support::pallet]
pub mod pallet {
    use alloc::vec::Vec;
    use frame_support::{
        pallet_prelude::*,
        traits::{Currency, Imbalance, ReservableCurrency},
        weights::Weight,
    };
    use frame_system::pallet_prelude::*;
    use pallet_inomad_identity::{
        pallet::{CitizenRole, CitizenStatus},
        Citizens,
    };
    use sp_core::H256;

    // =========================================================================
    // Type Alias
    // =========================================================================

    pub type BalanceOf<T> =
        <<T as Config>::Currency as Currency<<T as frame_system::Config>::AccountId>>::Balance;

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
        /// Currency for anti-spam application deposits.
        type Currency: Currency<Self::AccountId> + ReservableCurrency<Self::AccountId>;

        /// Voting period for standard (national) license applications.
        /// Default: 100_800 blocks ≈ 7 days.
        #[pallet::constant]
        type LicenseVotingPeriod: Get<u32>;

        /// Minimum YES votes to pass a license application (national Khural).
        #[pallet::constant]
        type MinLicenseQuorum: Get<u32>;

        /// Anti-spam deposit reserved from the applying ministry.
        /// Slashed if the application is rejected by the Khural.
        #[pallet::constant]
        type ApplicationDeposit: Get<BalanceOf<Self>>;

        /// Origin that represents the elected Executive Branch (Ministries / Committees).
        ///
        /// **Dev mode** (`cargo build`):            wire with `EnsureRoot` for Genesis bootstrap.
        /// **Production mode** (`--features production-origins`): wire with `ExecutiveCouncilOrigin`
        /// which is emitted only by elected Executive branch extrinsics.
        ///
        /// Controls: `authorize_ministry`, `deauthorize_ministry`.
        #[cfg(feature = "production-origins")]
        type ExecutiveOrigin: frame_support::traits::EnsureOrigin<Self::RuntimeOrigin>;
    }

    // =========================================================================
    // Enums
    // =========================================================================

    /// All license types available in the Altan Republic.
    ///
    /// Each type has:
    /// - A constitutional `voting_tier` (National or Confederal Khural)
    /// - A `required_quorum_fraction` (simple majority or 2/3)
    /// - A maximum `duration_blocks` capped at `MAX_LICENSE_BLOCKS` (10 years)
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
    pub enum LicenseType {
        // ── Resource Extraction ─────────────────────────────────────────────
        /// Solid mineral extraction (coal, ores, rare earths).
        MineralExtraction,
        /// Oil and gas field extraction.
        OilGasExtraction,
        /// Surface water or groundwater extraction.
        WaterExtraction,
        /// Commercial forestry (industrial timber harvesting).
        ForestryHarvesting,
        /// Commercial fishing (industrial scale).
        FishingCommercial,

        // ── Transport & Infrastructure ──────────────────────────────────────
        /// Scheduled aviation carrier (domestic routes).
        AviationOperator,
        /// Charter flights and air-taxi services.
        AviationCharter,
        /// Air cargo operations.
        AviationCargo,
        /// Railway line operation.
        RailwayOperator,
        /// Hazardous materials transport.
        HazmatTransport,

        // ── Finance ─────────────────────────────────────────────────────────
        /// Microfinance lender (under Bank of Siberia oversight).
        MicrofinanceLender,
        /// Insurance provider.
        InsuranceProvider,
        /// Currency exchange bureau.
        CurrencyExchange,

        // ── Regulated Industries ─────────────────────────────────────────────
        /// Pharmaceutical manufacturing.
        PharmaceuticalMfg,
        /// Telecommunications network operator.
        TelecomOperator,
        /// Broadcast media (TV / radio).
        BroadcastMedia,

        // ── Land Fund Concessions ────────────────────────────────────────────
        /// Concession of state land from the Land Fund.
        LandConcession,
        /// Subsoil concession (constitutional max: 10 years, renewable).
        SubsoilConcession,
        /// Mining plot allocation from state territory.
        MiningPlot,

        // ── High-Security (Confederal Khural, 2/3 quorum) ───────────────────
        /// Weapons manufacturing. Requires Confederate Khural + 2/3 quorum.
        WeaponsManufacture,
        /// Nuclear operations. Requires Confederate Khural + 2/3 quorum.
        NuclearOperator,
    }

    impl LicenseType {
        /// True if this license type requires Confederate Khural + 2/3 quorum.
        /// All other licenses go to the national Khural of the applying nation.
        pub fn is_confederal_only(&self) -> bool {
            matches!(
                self,
                LicenseType::WeaponsManufacture | LicenseType::NuclearOperator
            )
        }

        /// Minimum votes_for required expressed as a numerator over 100.
        ///
        /// - Standard types: `>50%` (simple majority, enforced via `votes_for > votes_against`)
        /// - Confederal types: `>=66%` of total votes cast
        ///
        /// For confederal types the runtime uses `votes_for * 3 >= total_votes * 2`.
        pub fn requires_supermajority(&self) -> bool {
            self.is_confederal_only()
        }

        /// Default duration in blocks if the applicant doesn't specify.
        /// Capped at `MAX_LICENSE_BLOCKS` (10 years).
        pub fn default_duration(&self) -> u32 {
            match self {
                // Full 10-year default for resource and infrastructure licenses
                LicenseType::MineralExtraction
                | LicenseType::OilGasExtraction
                | LicenseType::SubsoilConcession
                | LicenseType::MiningPlot
                | LicenseType::ForestryHarvesting
                | LicenseType::TelecomOperator
                | LicenseType::RailwayOperator => crate::MAX_LICENSE_BLOCKS,

                // 5 years for aviation and transport
                LicenseType::AviationOperator
                | LicenseType::AviationCharter
                | LicenseType::AviationCargo
                | LicenseType::HazmatTransport => crate::BLOCKS_PER_YEAR * 5,

                // 3 years for all others
                _ => crate::DEFAULT_LICENSE_BLOCKS,
            }
        }
    }

    /// Lifecycle status of a license application.
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
    pub enum AppStatus {
        /// Khural vote in progress.
        PendingKhural,
        /// Khural approved — license issued.
        Approved,
        /// Khural rejected — deposit slashed.
        Rejected,
        /// Application withdrawn by Root before vote end.
        Withdrawn,
    }

    // =========================================================================
    // Structs
    // =========================================================================

    /// A ministry / committee / agency authorized to submit license applications.
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
    pub struct MinistryRecord {
        /// Human-readable ministry tag (e.g., b"Ministry of Natural Resources").
        pub tag: BoundedVec<u8, ConstU32<64>>,
        /// Nation this ministry serves (1–79). `None` = confederal ministry.
        pub nation_id: Option<u32>,
        /// Whether this ministry may submit applications.
        pub is_active: bool,
    }

    /// A pending or completed license application awaiting Khural vote.
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
    pub struct LicenseApplication<T: Config> {
        /// Sequential application ID.
        pub app_id: u32,
        /// On-chain account of the applying organization (from pallet-organization).
        pub org_account: T::AccountId,
        /// License type requested.
        pub license_type: LicenseType,
        /// Region of operation (1–83).
        pub region_id: u8,
        /// Nation whose Khural will vote (matches org.nation_id or confederal).
        pub nation_id: u32,
        /// Ministry / Committee / Agency submitting on behalf of the executive branch.
        pub submitter: T::AccountId,
        /// Human-readable ministry tag for UI / indexers.
        pub ministry_tag: BoundedVec<u8, ConstU32<64>>,
        /// IPFS hash of the full audit package (compliance documents, inspections).
        /// Immutable — the Khural votes on this exact document set.
        pub audit_hash: H256,
        /// Requested duration in blocks (≤ MAX_LICENSE_BLOCKS).
        pub requested_duration: u32,
        /// Approve votes from Khural delegates.
        pub votes_for: u32,
        /// Reject votes from Khural delegates.
        pub votes_against: u32,
        /// Application lifecycle status.
        pub status: AppStatus,
        /// Block at which Khural voting closes; `on_initialize` auto-enacts.
        pub end_block: u32,
        /// Whether this is a renewal of an existing license.
        pub is_renewal: bool,
        /// If renewal: the ID of the license being renewed.
        pub renews_license_id: Option<u32>,
    }

    /// An issued license — active record after Khural approval.
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
    pub struct License<T: Config> {
        /// Unique license ID.
        pub license_id: u32,
        /// Organization account holding this license.
        pub org_account: T::AccountId,
        /// License type.
        pub license_type: LicenseType,
        /// Region of operation.
        pub region_id: u8,
        /// Nation that granted this license (Khural nation).
        pub nation_id: u32,
        /// Block at which this license was issued.
        pub issued_block: u32,
        /// Block at which this license expires (≤ issued_block + MAX_LICENSE_BLOCKS).
        pub expires_block: u32,
        /// Active status — false if revoked by judicial verdict or expired.
        pub is_active: bool,
        /// IPFS audit hash from the application (for tracibility).
        pub audit_hash: H256,
        /// Reference to the Khural application that approved this.
        pub app_id: u32,
        /// Judicial revocation reason (set only when is_active → false via court).
        pub revocation_reason: Option<BoundedVec<u8, ConstU32<256>>>,
    }

    // =========================================================================
    // Storage
    // =========================================================================

    /// Root-managed whitelist of organs authorized to submit license applications.
    ///
    /// Only accounts in this map may call `submit_license_application`.
    /// Root manages this via `authorize_ministry` / `deauthorize_ministry`.
    #[pallet::storage]
    #[pallet::getter(fn authorized_ministries)]
    pub type AuthorizedMinistries<T: Config> =
        StorageMap<_, Blake2_128Concat, T::AccountId, MinistryRecord, OptionQuery>;

    /// Pending and completed license applications.
    #[pallet::storage]
    #[pallet::getter(fn license_applications)]
    pub type LicenseApplications<T: Config> =
        StorageMap<_, Blake2_128Concat, u32, LicenseApplication<T>, OptionQuery>;

    /// Index: `end_block → [app_ids]` for O(1) `on_initialize` lookup.
    ///
    /// Only Active applications are in this index.
    /// Cleared when an application is enacted (approved or rejected).
    #[pallet::storage]
    pub type LicenseAppsEnding<T: Config> = StorageMap<
        _,
        Blake2_128Concat,
        u32,                           // end_block
        BoundedVec<u32, ConstU32<50>>, // app_ids expiring at this block
        ValueQuery,
    >;

    /// Double-spend voting guard: (app_id, voter) → has_voted.
    #[pallet::storage]
    pub type HasVotedOnLicense<T: Config> = StorageDoubleMap<
        _,
        Blake2_128Concat,
        u32, // app_id
        Blake2_128Concat,
        T::AccountId, // voter
        bool,
        ValueQuery,
    >;

    /// Active and historical licenses.
    #[pallet::storage]
    #[pallet::getter(fn licenses)]
    pub type Licenses<T: Config> = StorageMap<_, Blake2_128Concat, u32, License<T>, OptionQuery>;

    /// Reverse index: org_account → list of license IDs.
    #[pallet::storage]
    pub type LicensesByOrg<T: Config> =
        StorageMap<_, Blake2_128Concat, T::AccountId, BoundedVec<u32, ConstU32<100>>, ValueQuery>;

    /// Auto-incrementing application ID counter.
    #[pallet::storage]
    pub type NextLicenseAppId<T: Config> = StorageValue<_, u32, ValueQuery>;

    /// Auto-incrementing license ID counter.
    #[pallet::storage]
    pub type NextLicenseId<T: Config> = StorageValue<_, u32, ValueQuery>;

    /// Anti-spam deposit registry: app_id → (submitter, reserved_amount).
    #[pallet::storage]
    pub type ApplicationDepositors<T: Config> =
        StorageMap<_, Blake2_128Concat, u32, (T::AccountId, BalanceOf<T>), OptionQuery>;

    // =========================================================================
    // Events
    // =========================================================================

    #[pallet::event]
    #[pallet::generate_deposit(pub(super) fn deposit_event)]
    pub enum Event<T: Config> {
        /// A ministry was added to the authorized whitelist.
        MinistryAuthorized {
            ministry: T::AccountId,
            tag: Vec<u8>,
        },
        /// A ministry was removed from the whitelist.
        MinistryDeauthorized { ministry: T::AccountId },
        /// A license application was submitted to the Khural.
        LicenseApplicationSubmitted {
            app_id: u32,
            org_account: T::AccountId,
            license_type: LicenseType,
            nation_id: u32,
            end_block: u32,
            audit_hash: H256,
        },
        /// A Khural delegate voted on a license application.
        LicenseVoteCast {
            app_id: u32,
            voter: T::AccountId,
            approve: bool,
        },
        /// A license application was approved — license issued.
        LicenseIssued {
            license_id: u32,
            app_id: u32,
            org_account: T::AccountId,
            license_type: LicenseType,
            expires_block: u32,
        },
        /// A license application was rejected by the Khural.
        LicenseApplicationRejected {
            app_id: u32,
            org_account: T::AccountId,
        },
        /// Anti-spam deposit slashed after rejection.
        ApplicationDepositSlashed {
            app_id: u32,
            submitter: T::AccountId,
            amount: BalanceOf<T>,
        },
        /// A license was revoked by judicial verdict.
        LicenseRevoked {
            license_id: u32,
            org_account: T::AccountId,
            reason: Vec<u8>,
        },
        /// A license renewal application was submitted.
        LicenseRenewalSubmitted { app_id: u32, renews_license_id: u32 },
    }

    // =========================================================================
    // Errors
    // =========================================================================

    #[pallet::error]
    pub enum Error<T> {
        /// Caller is not in the authorized ministry whitelist.
        NotAuthorizedMinistry,
        /// Ministry account is already authorized.
        AlreadyAuthorized,
        /// The referenced application does not exist.
        ApplicationNotFound,
        /// The application is no longer accepting votes.
        ApplicationNotActive,
        /// The caller has already voted on this application.
        AlreadyVoted,
        /// Caller is not a KhuralDelegate.
        NotKhuralDelegate,
        /// Caller's nation does not match the application's nation.
        WrongNation,
        /// The referenced license does not exist.
        LicenseNotFound,
        /// The license is already revoked or expired.
        LicenseNotActive,
        /// Requested duration exceeds the constitutional 10-year cap.
        DurationExceedsConstitutionalMax,
        /// Duration of zero is not permitted.
        DurationMustBePositive,
        /// Caller is not a registered, active citizen.
        CitizenInactive,
        /// Only the judicial courts may revoke a license.
        OnlyJudicialCanRevoke,
        /// Revocation reason exceeds 256-byte limit.
        RevocationReasonTooLong,
        /// Ministry tag exceeds 64-byte limit.
        TagTooLong,
        /// The org's license list is full (max 100 licenses per org).
        TooManyLicenses,
        /// The end_block index is full for this block (max 50 apps per block).
        TooManyAppsAtBlock,
        /// Anti-spam deposit is insufficient.
        InsufficientDeposit,
        /// Deposit record inconsistency (should never happen).
        DepositNotFound,
        /// Caller is not a registered citizen.
        NotRegistered,
        /// The license being renewed is not active.
        RenewalTargetNotActive,
        /// Only ConfederationDelegate may vote on confederal-tier licenses.
        NotConfederationDelegate,
    }

    // =========================================================================
    // Block Hooks — Automatic Khural Vote Enactment
    // =========================================================================

    #[pallet::hooks]
    impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T> {
        /// Auto-enact license applications when their voting period expires.
        fn on_initialize(n: BlockNumberFor<T>) -> Weight {
            use frame_support::sp_runtime::traits::SaturatedConversion;
            let current: u32 = n.saturated_into::<u32>();
            let mut weight = Weight::zero();

            let expiring: Vec<u32> = LicenseAppsEnding::<T>::take(current).into_inner();
            for app_id in expiring {
                if let Some(app) = LicenseApplications::<T>::get(app_id) {
                    if app.status == AppStatus::PendingKhural {
                        weight = weight.saturating_add(Self::enact_application(app_id, app));
                    }
                }
            }

            weight
        }
    }

    // =========================================================================
    // Internal Helpers
    // =========================================================================

    impl<T: Config> Pallet<T> {
        /// Current block number as u32.
        fn current_block() -> u32 {
            use frame_support::sp_runtime::traits::SaturatedConversion;
            frame_system::Pallet::<T>::block_number().saturated_into::<u32>()
        }

        /// Evaluate and enact a license application after voting period.
        ///
        /// ## Quorum Rules
        ///
        /// **Standard licenses** (national Khural):
        ///   - `votes_for >= MinLicenseQuorum` AND `votes_for > votes_against`
        ///
        /// **Confederal licenses** (WeaponsManufacture, NuclearOperator):
        ///   - `votes_for * 3 >= (votes_for + votes_against) * 2`  (≥ 2/3 of participating delegates)
        ///   - AND `votes_for >= MinLicenseQuorum`
        fn enact_application(app_id: u32, mut app: LicenseApplication<T>) -> Weight {
            let total_votes = app.votes_for.saturating_add(app.votes_against);
            let quorum_met = app.votes_for >= T::MinLicenseQuorum::get();

            let majority = if app.license_type.requires_supermajority() {
                // 2/3 supermajority: votes_for / total >= 2/3  ↔  votes_for * 3 >= total * 2
                total_votes > 0 && app.votes_for.saturating_mul(3) >= total_votes.saturating_mul(2)
            } else {
                app.votes_for > app.votes_against
            };

            if quorum_met && majority {
                // ── Issue the license ────────────────────────────────────────
                let license_id = NextLicenseId::<T>::get();
                let current = Self::current_block();
                let expires_block = current.saturating_add(app.requested_duration);

                let license = License::<T> {
                    license_id,
                    org_account: app.org_account.clone(),
                    license_type: app.license_type.clone(),
                    region_id: app.region_id,
                    nation_id: app.nation_id,
                    issued_block: current,
                    expires_block,
                    is_active: true,
                    audit_hash: app.audit_hash,
                    app_id,
                    revocation_reason: None,
                };
                Licenses::<T>::insert(license_id, license);
                NextLicenseId::<T>::put(license_id.saturating_add(1));

                // Update org reverse index
                LicensesByOrg::<T>::mutate(&app.org_account, |ids| {
                    let _ = ids.try_push(license_id);
                });

                app.status = AppStatus::Approved;
                LicenseApplications::<T>::insert(app_id, app.clone());

                // Return anti-spam deposit to submitter
                if let Some((depositor, amount)) = ApplicationDepositors::<T>::take(app_id) {
                    let _ = <T as crate::pallet::Config>::Currency::unreserve(&depositor, amount);
                }

                Self::deposit_event(Event::LicenseIssued {
                    license_id,
                    app_id,
                    org_account: app.org_account,
                    license_type: app.license_type,
                    expires_block,
                });
            } else {
                // ── Reject and slash deposit ─────────────────────────────────
                app.status = AppStatus::Rejected;
                LicenseApplications::<T>::insert(app_id, app.clone());

                if let Some((depositor, amount)) = ApplicationDepositors::<T>::take(app_id) {
                    let (imbalance, _) =
                        <T as crate::pallet::Config>::Currency::slash_reserved(&depositor, amount);
                    Self::deposit_event(Event::ApplicationDepositSlashed {
                        app_id,
                        submitter: depositor,
                        amount: imbalance.peek(),
                    });
                }

                Self::deposit_event(Event::LicenseApplicationRejected {
                    app_id,
                    org_account: app.org_account,
                });
            }

            Weight::from_parts(150_000_000, 0)
        }

        /// Validate that the caller is a KhuralDelegate (or ConfederationDelegate for confederal apps).
        fn validate_khural_voter(
            who: &T::AccountId,
            app: &LicenseApplication<T>,
        ) -> DispatchResult {
            let rec = Citizens::<T>::get(who).ok_or(Error::<T>::NotRegistered)?;
            ensure!(
                rec.status == CitizenStatus::Active,
                Error::<T>::CitizenInactive
            );

            if app.license_type.is_confederal_only() {
                // Confederate licenses: only ConfederationDelegate
                ensure!(
                    rec.role == CitizenRole::ConfederationDelegate,
                    Error::<T>::NotConfederationDelegate
                );
            } else {
                // National licenses: KhuralDelegate of matching nation
                ensure!(
                    rec.role == CitizenRole::KhuralDelegate,
                    Error::<T>::NotKhuralDelegate
                );
                ensure!(rec.nation_id == app.nation_id, Error::<T>::WrongNation);
            }

            Ok(())
        }
    }

    // =========================================================================
    // Extrinsics
    // =========================================================================

    #[pallet::call]
    impl<T: Config> Pallet<T> {
        // ─── authorize_ministry ───────────────────────────────────────────────

        /// Add an organ (Ministry / Committee / Agency) to the licensing whitelist.
        ///
        /// # Origin: Executive Branch (dev: Root, production: ExecutiveCouncilOrigin)
        #[pallet::call_index(0)]
        #[pallet::weight(Weight::from_parts(50_000_000, 0))]
        pub fn authorize_ministry(
            origin: OriginFor<T>,
            ministry: T::AccountId,
            tag: BoundedVec<u8, ConstU32<64>>,
            nation_id: Option<u32>,
        ) -> DispatchResult {
            // Origin: Executive Branch (Ministry whitelist management).
            // Dev:        EnsureRoot (Genesis bootstrap — sudo call)
            // Production: ExecutiveCouncilOrigin (elected Executive Branch)
            // AUDIT NOTE: Only the Executive Branch may manage the authorized ministry
            // whitelist (§2.6). No individual may self-register as a ministry.
            #[cfg(not(feature = "production-origins"))]
            ensure_root(origin)?;
            #[cfg(feature = "production-origins")]
            T::ExecutiveOrigin::ensure_origin(origin)?;

            ensure!(tag.len() <= 64, Error::<T>::TagTooLong);

            let record = MinistryRecord {
                tag: tag.clone(),
                nation_id,
                is_active: true,
            };
            AuthorizedMinistries::<T>::insert(&ministry, record);

            Self::deposit_event(Event::MinistryAuthorized {
                ministry,
                tag: tag.into_inner(),
            });
            Ok(())
        }

        // ─── deauthorize_ministry ─────────────────────────────────────────────

        /// Remove an organ from the licensing whitelist.
        ///
        /// # Origin: Root
        #[pallet::call_index(1)]
        #[pallet::weight(Weight::from_parts(50_000_000, 0))]
        pub fn deauthorize_ministry(
            origin: OriginFor<T>,
            ministry: T::AccountId,
        ) -> DispatchResult {
            // Origin: Executive Branch (same as authorize_ministry).
            // Dev:        EnsureRoot
            // Production: ExecutiveCouncilOrigin
            #[cfg(not(feature = "production-origins"))]
            ensure_root(origin)?;
            #[cfg(feature = "production-origins")]
            T::ExecutiveOrigin::ensure_origin(origin)?;

            AuthorizedMinistries::<T>::remove(&ministry);
            Self::deposit_event(Event::MinistryDeauthorized { ministry });
            Ok(())
        }

        // ─── submit_license_application ───────────────────────────────────────

        /// Submit a license application on behalf of an organization.
        ///
        /// Called by an authorized Ministry / Committee / Agency after completing
        /// all off-chain compliance checks. The audit package is referenced by
        /// its IPFS `audit_hash` — the Khural votes on this exact document.
        ///
        /// ## Constitutional Checks
        ///
        /// 1. Caller must be in `AuthorizedMinistries` whitelist.
        /// 2. `requested_duration` must be `> 0` and `≤ MAX_LICENSE_BLOCKS` (10 years).
        /// 3. Anti-spam deposit is reserved from the caller's balance.
        ///
        /// # Origin: Signed (Authorized Ministry)
        #[pallet::call_index(2)]
        #[pallet::weight(Weight::from_parts(50_000_000, 0))]
        pub fn submit_license_application(
            origin: OriginFor<T>,
            org_account: T::AccountId,
            license_type: LicenseType,
            region_id: u8,
            nation_id: u32,
            audit_hash: H256,
            // Duration in blocks. 0 = use `LicenseType::default_duration()`.
            requested_duration: u32,
        ) -> DispatchResult {
            let caller = ensure_signed(origin)?;

            // ── [CHECK 1] Authorized ministry only ───────────────────────────
            let ministry_rec =
                AuthorizedMinistries::<T>::get(&caller).ok_or(Error::<T>::NotAuthorizedMinistry)?;
            ensure!(ministry_rec.is_active, Error::<T>::NotAuthorizedMinistry);

            // ── [CHECK 2] Constitutional 10-year cap ─────────────────────────
            let duration = if requested_duration == 0 {
                license_type.default_duration()
            } else {
                ensure!(requested_duration > 0, Error::<T>::DurationMustBePositive);
                ensure!(
                    requested_duration <= crate::MAX_LICENSE_BLOCKS,
                    Error::<T>::DurationExceedsConstitutionalMax
                );
                requested_duration
            };

            // ── [CHECK 3] Reserve anti-spam deposit ──────────────────────────
            let deposit = T::ApplicationDeposit::get();
            <T as crate::pallet::Config>::Currency::reserve(&caller, deposit)
                .map_err(|_| Error::<T>::InsufficientDeposit)?;

            // ── Assign IDs and compute end_block ─────────────────────────────
            let app_id = NextLicenseAppId::<T>::get();
            let current = Self::current_block();
            let end_block = current.saturating_add(T::LicenseVotingPeriod::get());

            // ── Store application ─────────────────────────────────────────────
            let app = LicenseApplication::<T> {
                app_id,
                org_account: org_account.clone(),
                license_type: license_type.clone(),
                region_id,
                nation_id,
                submitter: caller.clone(),
                ministry_tag: ministry_rec.tag,
                audit_hash,
                requested_duration: duration,
                votes_for: 0,
                votes_against: 0,
                status: AppStatus::PendingKhural,
                end_block,
                is_renewal: false,
                renews_license_id: None,
            };
            LicenseApplications::<T>::insert(app_id, app);
            NextLicenseAppId::<T>::put(app_id.saturating_add(1));

            // ── Register in end-block index ───────────────────────────────────
            LicenseAppsEnding::<T>::try_mutate(end_block, |ids| {
                ids.try_push(app_id)
                    .map_err(|_| Error::<T>::TooManyAppsAtBlock)
            })?;

            // ── Store deposit reference ───────────────────────────────────────
            ApplicationDepositors::<T>::insert(app_id, (caller, deposit));

            Self::deposit_event(Event::LicenseApplicationSubmitted {
                app_id,
                org_account,
                license_type,
                nation_id,
                end_block,
                audit_hash,
            });

            Ok(())
        }

        // ─── vote_on_license ──────────────────────────────────────────────────

        /// Cast an approve or reject vote on a pending license application.
        ///
        /// ## Constitutional Voting Rules
        ///
        /// - **Standard licenses**: Only the `KhuralDelegate` of the **matching nation**.
        /// - **Confederal licenses** (Weapons / Nuclear): Only `ConfederationDelegate`.
        ///
        /// One delegate = one vote. No repeat voting.
        ///
        /// # Origin: Signed (KhuralDelegate or ConfederationDelegate)
        #[pallet::call_index(3)]
        #[pallet::weight(Weight::from_parts(50_000_000, 0))]
        pub fn vote_on_license(origin: OriginFor<T>, app_id: u32, approve: bool) -> DispatchResult {
            let caller = ensure_signed(origin)?;

            // ── [CHECK 1] Application exists and is active ────────────────────
            let mut app =
                LicenseApplications::<T>::get(app_id).ok_or(Error::<T>::ApplicationNotFound)?;
            ensure!(
                app.status == AppStatus::PendingKhural,
                Error::<T>::ApplicationNotActive
            );

            // ── [CHECK 2] Voting window still open ────────────────────────────
            let current = Self::current_block();
            ensure!(current < app.end_block, Error::<T>::ApplicationNotActive);

            // ── [CHECK 3] Caller is authorized Khural voter ───────────────────
            Self::validate_khural_voter(&caller, &app)?;

            // ── [CHECK 4] No double voting ────────────────────────────────────
            ensure!(
                !HasVotedOnLicense::<T>::get(app_id, &caller),
                Error::<T>::AlreadyVoted
            );

            // ── Record vote ───────────────────────────────────────────────────
            if approve {
                app.votes_for = app.votes_for.saturating_add(1);
            } else {
                app.votes_against = app.votes_against.saturating_add(1);
            }
            HasVotedOnLicense::<T>::insert(app_id, &caller, true);
            LicenseApplications::<T>::insert(app_id, app);

            Self::deposit_event(Event::LicenseVoteCast {
                app_id,
                voter: caller,
                approve,
            });
            Ok(())
        }

        // ─── revoke_license ───────────────────────────────────────────────────

        /// Revoke an active license via judicial verdict.
        ///
        /// ## Constitutional Constraint (§2.3 — §2.6)
        ///
        /// **Only Root may call this extrinsic**, and Root MUST only invoke it
        /// as a result of a `pallet-judicial-courts::execute_verdict` outcome.
        /// No Ministry, Committee, or individual may administratively revoke a license.
        ///
        /// # Origin: Root (proxied from judicial-courts execute_verdict)
        #[pallet::call_index(4)]
        #[pallet::weight(Weight::from_parts(50_000_000, 0))]
        pub fn revoke_license(
            origin: OriginFor<T>,
            license_id: u32,
            reason: BoundedVec<u8, ConstU32<256>>,
        ) -> DispatchResult {
            // AUDIT NOTE: `ensure_root` here is the JUDICIAL HOOK pattern.
            // This extrinsic is ONLY called from `pallet-judicial-courts::execute_verdict`
            // via the `LicensingInterface::judicial_revoke()` trait implementation.
            // No ministry, individual or admin may call this directly.
            // Root = proxy for court-ordered revocation (§2.3 — No administrative seizure).
            ensure_root(origin)?;

            let mut license = Licenses::<T>::get(license_id).ok_or(Error::<T>::LicenseNotFound)?;
            ensure!(license.is_active, Error::<T>::LicenseNotActive);

            license.is_active = false;
            license.revocation_reason = Some(reason.clone());
            Licenses::<T>::insert(license_id, license.clone());

            Self::deposit_event(Event::LicenseRevoked {
                license_id,
                org_account: license.org_account,
                reason: reason.into_inner(),
            });
            Ok(())
        }

        // ─── renew_license ────────────────────────────────────────────────────

        /// Submit a renewal application for an existing license before it expires.
        ///
        /// Creates a new `LicenseApplication` flagged `is_renewal = true`, linking
        /// to the original license ID. If approved, the original license is extended
        /// (or replaced) by a fresh license record.
        ///
        /// # Origin: Signed (Authorized Ministry)
        #[pallet::call_index(5)]
        #[pallet::weight(Weight::from_parts(50_000_000, 0))]
        pub fn renew_license(
            origin: OriginFor<T>,
            license_id: u32,
            audit_hash: H256,
            requested_duration: u32,
        ) -> DispatchResult {
            let caller = ensure_signed(origin)?;

            // ── Authority check ───────────────────────────────────────────────
            let ministry_rec =
                AuthorizedMinistries::<T>::get(&caller).ok_or(Error::<T>::NotAuthorizedMinistry)?;
            ensure!(ministry_rec.is_active, Error::<T>::NotAuthorizedMinistry);

            // ── Original license must exist and be active ─────────────────────
            let license = Licenses::<T>::get(license_id).ok_or(Error::<T>::LicenseNotFound)?;
            ensure!(license.is_active, Error::<T>::RenewalTargetNotActive);

            // ── Duration cap ──────────────────────────────────────────────────
            let duration = if requested_duration == 0 {
                license.license_type.default_duration()
            } else {
                ensure!(
                    requested_duration <= crate::MAX_LICENSE_BLOCKS,
                    Error::<T>::DurationExceedsConstitutionalMax
                );
                requested_duration
            };

            // ── Reserve deposit ───────────────────────────────────────────────
            let deposit = T::ApplicationDeposit::get();
            <T as crate::pallet::Config>::Currency::reserve(&caller, deposit)
                .map_err(|_| Error::<T>::InsufficientDeposit)?;

            // ── Create renewal application ────────────────────────────────────
            let app_id = NextLicenseAppId::<T>::get();
            let current = Self::current_block();
            let end_block = current.saturating_add(T::LicenseVotingPeriod::get());

            let app = LicenseApplication::<T> {
                app_id,
                org_account: license.org_account.clone(),
                license_type: license.license_type.clone(),
                region_id: license.region_id,
                nation_id: license.nation_id,
                submitter: caller.clone(),
                ministry_tag: ministry_rec.tag,
                audit_hash,
                requested_duration: duration,
                votes_for: 0,
                votes_against: 0,
                status: AppStatus::PendingKhural,
                end_block,
                is_renewal: true,
                renews_license_id: Some(license_id),
            };
            LicenseApplications::<T>::insert(app_id, app);
            NextLicenseAppId::<T>::put(app_id.saturating_add(1));
            LicenseAppsEnding::<T>::try_mutate(end_block, |ids| {
                ids.try_push(app_id)
                    .map_err(|_| Error::<T>::TooManyAppsAtBlock)
            })?;
            ApplicationDepositors::<T>::insert(app_id, (caller, deposit));

            Self::deposit_event(Event::LicenseRenewalSubmitted {
                app_id,
                renews_license_id: license_id,
            });
            Ok(())
        }
    }

    // =========================================================================
    // Public Query Interface (cross-pallet)
    // =========================================================================

    impl<T: Config> Pallet<T> {
        /// Returns true if the `org_account` holds an **active, non-expired** license
        /// of the given type in the given region.
        ///
        /// Intended for cross-pallet checks (e.g., pallet-land-registry verifying
        /// the operator holds a valid SubsoilConcession before granting extraction rights).
        pub fn is_licensed(
            org_account: &T::AccountId,
            license_type: &LicenseType,
            region_id: u8,
        ) -> bool {
            use frame_support::sp_runtime::traits::SaturatedConversion;
            let current: u32 = frame_system::Pallet::<T>::block_number().saturated_into::<u32>();

            LicensesByOrg::<T>::get(org_account)
                .iter()
                .filter_map(|&id| Licenses::<T>::get(id))
                .any(|l| {
                    l.is_active
                        && l.license_type == *license_type
                        && l.region_id == region_id
                        && l.expires_block > current
                })
        }
    }
}

// =========================================================================
// LicensingInterface — Judicial Hook Implementation
// =========================================================================

use frame_support::{traits::ConstU32, BoundedVec};

impl<T: pallet::Config> crate::LicensingInterface<T::AccountId> for pallet::Pallet<T> {
    fn judicial_revoke(
        license_id: u32,
        _holder: &T::AccountId,
        reason: alloc::vec::Vec<u8>,
    ) -> frame_support::dispatch::DispatchResult {
        let bounded_reason: BoundedVec<u8, ConstU32<256>> = reason
            .try_into()
            .map_err(|_| pallet::Error::<T>::RevocationReasonTooLong)?;

        let mut license =
            pallet::Licenses::<T>::get(license_id).ok_or(pallet::Error::<T>::LicenseNotFound)?;

        license.is_active = false;
        license.revocation_reason = Some(bounded_reason.clone());
        pallet::Licenses::<T>::insert(license_id, license.clone());

        pallet::Pallet::<T>::deposit_event(pallet::Event::LicenseRevoked {
            license_id,
            org_account: license.org_account,
            reason: bounded_reason.into_inner(),
        });
        Ok(())
    }
}
