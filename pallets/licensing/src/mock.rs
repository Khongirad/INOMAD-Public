//! Mock runtime for pallet-licensing unit tests.

use crate as pallet_licensing;
use frame_support::{derive_impl, parameter_types, traits::ConstU32};
use pallet_inomad_identity::{
    pallet::{
        CitizenRecord, CitizenRole, CitizenStatus, CitizenshipStatus, PassportType,
        VerificationStatus,
    },
    Citizens,
};
use sp_core::H256;
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
    pub type Licensing = pallet_licensing::Pallet<Test>;
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

// ── Identity pallet stubs ────────────────────────────────────────────────────

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

pub struct NoConstitutionHash;
impl frame_support::traits::Get<Option<[u8; 32]>> for NoConstitutionHash {
    fn get() -> Option<[u8; 32]> {
        None
    }
}

parameter_types! {
    pub const CivilTreasury: u64 = 9999;
}

impl pallet_inomad_identity::Config for Test {
    type Currency = Balances;
    type UnlockLevel0 = UnlockLevel0;
    type UnlockLevel1 = UnlockLevel1;
    type MedicalAuthority = MedicalAuthority;
    type ArbadCooldownPeriod = ArbadCooldownPeriod;
    type MaxRegistrationsPerBlock = ConstU32<100>;
    type TerminalHook = NoopTerminalHook;
    type ConstitutionHashProvider = NoConstitutionHash;
    type MarriageFee = frame_support::traits::ConstU64<0>;
    type CivilFeeTreasury = CivilTreasury;
    type WeightInfo = ();
}

// ── Licensing pallet config ───────────────────────────────────────────────────

parameter_types! {
    pub const LicenseVotingPeriod: u32 = 100_800; // 7 days
    pub const MinLicenseQuorum: u32 = 1;           // lowered for unit tests
    pub const ApplicationDeposit: u64 = 10;        // 10 planck in test units
}

impl pallet_licensing::Config for Test {
    type Currency = Balances;
    type LicenseVotingPeriod = LicenseVotingPeriod;
    type MinLicenseQuorum = MinLicenseQuorum;
    type ApplicationDeposit = ApplicationDeposit;
}

// ── Named test accounts ────────────────────────────────────────────────────────

pub const ALICE: u64 = 1; // Ministry submitter / executive
pub const BOB: u64 = 2; // KhuralDelegate (national)
pub const CHARLIE: u64 = 3; // Unregistered citizen
pub const ORG_ACCT: u64 = 42;

pub fn new_test_ext() -> TestExternalities {
    let t = frame_system::GenesisConfig::<Test>::default()
        .build_storage()
        .unwrap();
    let mut ext = TestExternalities::new(t);
    ext.execute_with(|| {
        System::set_block_number(1);
        // Fund Alice (ministry) enough for the deposit.
        let _ = pallet_balances::Pallet::<Test>::force_set_balance(
            frame_system::RawOrigin::Root.into(),
            ALICE,
            1_000_000,
        );
    });
    ext
}

// ── Storage helpers ────────────────────────────────────────────────────────────

/// Insert a KhuralDelegate citizen record directly into storage.
pub fn insert_khural_delegate(who: u64, nation_id: u32) {
    let doc_hash = H256::from([who as u8; 32]);
    let email_hash = H256::from([(who + 20) as u8; 32]);
    let record = CitizenRecord {
        citizen_id: who,
        nation_id,
        naturalized_people_id: None,
        role: CitizenRole::KhuralDelegate,
        status: CitizenStatus::Active,
        verification: VerificationStatus::Verified,
        vesting_level: None,
        branch: None,
        term_end: None,
        khural_terms_served: 0,
        is_indigenous: true,
        citizenship_status: CitizenshipStatus::Indigenous,
        region_id: None,
        birth_region_id: None,
        passport_type: PassportType::Internal,
        document_hash: doc_hash,
        birth_page_hash: H256::from([(who + 10) as u8; 32]),
        email_hash,
    };
    Citizens::<Test>::insert(who, record);
}

/// Insert a base Active citizen (Regular role).
pub fn insert_active_citizen(who: u64, nation_id: u32) {
    let doc_hash = H256::from([who as u8; 32]);
    let email_hash = H256::from([(who + 20) as u8; 32]);
    let record = CitizenRecord {
        citizen_id: who,
        nation_id,
        naturalized_people_id: None,
        role: CitizenRole::Regular,
        status: CitizenStatus::Active,
        verification: VerificationStatus::Verified,
        vesting_level: None,
        branch: None,
        term_end: None,
        khural_terms_served: 0,
        is_indigenous: false,
        citizenship_status: CitizenshipStatus::Naturalized,
        region_id: None,
        birth_region_id: None,
        passport_type: PassportType::Internal,
        document_hash: doc_hash,
        birth_page_hash: H256::from([(who + 10) as u8; 32]),
        email_hash,
    };
    Citizens::<Test>::insert(who, record);
}
