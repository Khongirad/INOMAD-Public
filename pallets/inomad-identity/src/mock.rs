//! Mock runtime for pallet-inomad-identity unit tests.

use crate as pallet_inomad_identity;
use frame_support::{derive_impl, parameter_types};
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
}

#[derive_impl(frame_system::config_preludes::TestDefaultConfig)]
impl frame_system::Config for Test {
    type Block = Block;
    type AccountData = pallet_balances::AccountData<u64>;
}

#[derive_impl(pallet_balances::config_preludes::TestDefaultConfig)]
impl pallet_balances::Config for Test {
    type AccountStore = System;
}

parameter_types! {
    pub const UnlockLevel0: u128 = 100;
    pub const UnlockLevel1: u128 = 1_000;
    pub const MedicalAuthority: u64 = 999;
    pub const ArbadCooldownPeriod: u64 = 10;
}

pub struct NoopTerminalHook;
impl pallet_inomad_identity::OnTerminalStatus<u64> for NoopTerminalHook {
    fn on_deceased(_who: &u64) {}
    fn on_exiled(_who: &u64) {}
}

impl pallet_inomad_identity::Config for Test {
    type Currency = Balances;
    type UnlockLevel0 = UnlockLevel0;
    type UnlockLevel1 = UnlockLevel1;
    type MedicalAuthority = MedicalAuthority;
    type ArbadCooldownPeriod = ArbadCooldownPeriod;
    type MaxRegistrationsPerBlock = frame_support::traits::ConstU32<100>;
    type TerminalHook = NoopTerminalHook;
    // Chain of Legitimacy: disabled in tests (None = no hash check).
    type ConstitutionHashProvider = NoConstitutionHash;
    type MarriageFee = frame_support::traits::ConstU64<0>;
    type CivilFeeTreasury = frame_support::traits::ConstU64<9999>;
    type WeightInfo = ();
}

pub struct NoConstitutionHash;
impl frame_support::traits::Get<Option<[u8; 32]>> for NoConstitutionHash {
    fn get() -> Option<[u8; 32]> {
        None
    }
}

// ─── Named test accounts ──────────────────────────────────────────────────────

pub const ALICE: u64 = 1;
pub const BOB: u64 = 2;
#[allow(dead_code)]
pub const CHARLIE: u64 = 3;

pub fn new_test_ext() -> TestExternalities {
    let t = frame_system::GenesisConfig::<Test>::default()
        .build_storage()
        .unwrap();
    let mut ext = TestExternalities::new(t);
    ext.execute_with(|| System::set_block_number(1));
    ext
}

// ─── Storage insertion helper ─────────────────────────────────────────────────

use crate::pallet::{
    CitizenRecord, CitizenRole, CitizenStatus, CitizenshipStatus, PassportType, VerificationStatus,
};
use sp_core::H256;

/// Insert a `CitizenRecord` directly into storage, bypassing extrinsic validation.
///
/// Used in tests that need a pre-existing citizen without going through
/// the full KYC flow (`register_citizen` requires Medical Authority).
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
    crate::Citizens::<Test>::insert(who, record);
    crate::UsedDocumentHashes::<Test>::insert(doc_hash, true);
    crate::UsedEmailHashes::<Test>::insert(email_hash, true);
}
