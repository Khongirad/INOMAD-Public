//! Mock runtime for pallet-land-registry unit tests.
//!
//! Wires System + Balances + InomadIdentity + LandRegistry together.

use crate as pallet_land_registry;
use frame_support::{derive_impl, parameter_types};
use pallet_inomad_identity as identity;
use sp_io::TestExternalities;
use sp_runtime::BuildStorage;

type Block = frame_system::mocking::MockBlock<Test>;

#[frame_support::runtime]
mod runtime {
    #[runtime::runtime]
    #[runtime::derive(
        RuntimeCall,
        RuntimeEvent,
        RuntimeError,
        RuntimeOrigin,
        RuntimeFreezeReason,
        RuntimeHoldReason,
        RuntimeSlashReason,
        RuntimeLockId,
        RuntimeTask,
        RuntimeViewFunction
    )]
    pub struct Test;

    #[runtime::pallet_index(0)]
    pub type System = frame_system::Pallet<Test>;

    #[runtime::pallet_index(1)]
    pub type Balances = pallet_balances::Pallet<Test>;

    #[runtime::pallet_index(2)]
    pub type InomadIdentity = pallet_inomad_identity::Pallet<Test>;

    #[runtime::pallet_index(3)]
    pub type LandRegistry = pallet_land_registry::Pallet<Test>;
}

// ─────────────────────────────────────────────────────────────────────────────
// frame-system
// ─────────────────────────────────────────────────────────────────────────────

#[derive_impl(frame_system::config_preludes::TestDefaultConfig)]
impl frame_system::Config for Test {
    type Block = Block;
    type AccountData = pallet_balances::AccountData<u64>;
}

// ─────────────────────────────────────────────────────────────────────────────
// pallet-balances
// ─────────────────────────────────────────────────────────────────────────────

#[derive_impl(pallet_balances::config_preludes::TestDefaultConfig)]
impl pallet_balances::Config for Test {
    type AccountStore = System;
}

// ─────────────────────────────────────────────────────────────────────────────
// pallet-inomad-identity
// ─────────────────────────────────────────────────────────────────────────────

parameter_types! {
    pub const UnlockLevel0: u128 = 100;
    pub const UnlockLevel1: u128 = 1_000;
    pub const MedicalAuthority: u64 = 999;
    pub const ArbadCooldownPeriod: u64 = 10;
}

pub struct NoopTerminalHook;
impl identity::OnTerminalStatus<u64> for NoopTerminalHook {
    fn on_deceased(_who: &u64) {}
    fn on_exiled(_who: &u64) {}
}

pub struct NoConstitutionHash;
impl frame_support::traits::Get<Option<[u8; 32]>> for NoConstitutionHash {
    fn get() -> Option<[u8; 32]> {
        None
    }
}

impl identity::Config for Test {
    type Currency = Balances;
    type UnlockLevel0 = UnlockLevel0;
    type UnlockLevel1 = UnlockLevel1;
    type MedicalAuthority = MedicalAuthority;
    type ArbadCooldownPeriod = ArbadCooldownPeriod;
    type MaxRegistrationsPerBlock = frame_support::traits::ConstU32<100>;
    type MarriageFee = frame_support::traits::ConstU64<0>;
    type CivilFeeTreasury = MedicalAuthority; // noop treasury for tests
    type TerminalHook = NoopTerminalHook;
    type ConstitutionHashProvider = NoConstitutionHash;
    type WeightInfo = ();
}

// ─────────────────────────────────────────────────────────────────────────────
// pallet-land-registry
// ─────────────────────────────────────────────────────────────────────────────

parameter_types! {
    pub const MaxParcelDescriptionLen: u32 = 128;
}

impl pallet_land_registry::Config for Test {
    type MaxDescriptionLen = MaxParcelDescriptionLen;
}

// ─────────────────────────────────────────────────────────────────────────────
// Named test accounts
// ─────────────────────────────────────────────────────────────────────────────

pub const SELLER: u64 = 1;
pub const CITIZEN_BUYER: u64 = 2;
pub const FOREIGNER_BUYER: u64 = 3;
pub const FROZEN_BUYER: u64 = 4;

pub fn new_test_ext() -> TestExternalities {
    let t = frame_system::GenesisConfig::<Test>::default()
        .build_storage()
        .unwrap();
    let mut ext = TestExternalities::new(t);
    ext.execute_with(|| System::set_block_number(1));
    ext
}

// ─────────────────────────────────────────────────────────────────────────────
// Helpers
// ─────────────────────────────────────────────────────────────────────────────

// Use the public re-exports from pallet-inomad-identity (via `pub use pallet::*`)
use frame_support::BoundedVec;
use pallet_inomad_identity::{
    CitizenRecord, CitizenRole, CitizenStatus, Citizens, CitizenshipStatus, PassportType,
    UsedDocumentHashes, UsedEmailHashes, VerificationStatus,
};
use pallet_land_registry::pallet::{LandParcel, ResourceRights};
use sp_core::H256;

/// Insert a citizen directly into `pallet-inomad-identity`'s Citizens map.
pub fn insert_citizen(who: u64, citizenship: CitizenshipStatus, status: CitizenStatus) {
    let doc_hash = H256::from([who as u8; 32]);
    let email_hash = H256::from([(who + 20) as u8; 32]);
    let record = CitizenRecord {
        citizen_id: who,
        nation_id: 1,
        naturalized_people_id: None,
        role: CitizenRole::Regular,
        status,
        verification: VerificationStatus::Verified,
        vesting_level: None,
        branch: None,
        term_end: None,
        khural_terms_served: 0,
        is_indigenous: false,
        citizenship_status: citizenship,
        region_id: None,
        birth_region_id: None,
        passport_type: PassportType::Internal,
        document_hash: doc_hash,
        birth_page_hash: H256::from([(who + 10) as u8; 32]),
        email_hash,
    };
    Citizens::<Test>::insert(who, record);
    UsedDocumentHashes::<Test>::insert(doc_hash, true);
    UsedEmailHashes::<Test>::insert(email_hash, true);
}

/// Insert a land parcel owned by `owner`.
pub fn insert_parcel(parcel_id: u64, owner: u64) {
    let parcel = LandParcel::<Test> {
        owner,
        region: 1,
        coordinate_hash: [parcel_id as u8; 32],
        area_sqm: 10_000,
        resource_rights: ResourceRights::SurfaceOnly,
        registered_at: 1,
        description: BoundedVec::default(),
    };
    pallet_land_registry::LandParcels::<Test>::insert(parcel_id, parcel);
    pallet_land_registry::ParcelsByOwner::<Test>::insert(owner, parcel_id, ());
    // Advance the counter past this ID.
    pallet_land_registry::NextParcelId::<Test>::put(parcel_id + 1);
}
