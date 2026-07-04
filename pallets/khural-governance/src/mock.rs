//! Mock runtime for pallet-khural-governance unit tests.
//!
//! Includes `pallet-inomad-identity` in the runtime (tight coupling) so that
//! `Citizens<T>` storage can be seeded directly — enabling realistic tests
//! of the proposal lifecycle, voting gating, and expert bill flow.

use frame_support::{
    construct_runtime, derive_impl, parameter_types,
    traits::{ConstU128, ConstU32, ConstU64},
    BoundedVec,
};
use sp_core::H256;
use sp_runtime::BuildStorage;
use std::cell::RefCell;

pub use pallet_inomad_identity::{
    CitizenRecord, CitizenRole, CitizenStatus, CitizenshipStatus, PassportType, VerificationStatus,
};

// ── Types ──────────────────────────────────────────────────────────────────

pub type AccountId = u64;
pub type Balance = u128;
pub type Block = frame_system::mocking::MockBlock<Test>;

pub const UNIT: Balance = 1_000_000_000_000; // 1 ALTAN

// ── Well-known accounts ────────────────────────────────────────────────────

pub const ALICE: AccountId = 1; // active citizen, nation 1
pub const BOB: AccountId = 2; // active citizen, nation 1
pub const CHARLIE: AccountId = 3; // active citizen, nation 1
pub const DAVE: AccountId = 4; // active citizen, nation 1 — ArbadLeader
pub const EVE: AccountId = 5; // citizen of nation 2 (cross-nation tests)
pub const NATION1_TREASURY: AccountId = 100;
pub const NATION2_TREASURY: AccountId = 101;

// ── Thread-local Academician tracker ─────────────────────────────────────

thread_local! {
    pub static ACADEMICIANS: RefCell<Vec<AccountId>> = RefCell::new(Vec::new());
}

pub fn add_academician(who: AccountId) {
    ACADEMICIANS.with(|a| a.borrow_mut().push(who));
}

pub fn clear_academicians() {
    ACADEMICIANS.with(|a| a.borrow_mut().clear());
}

// ── MockAcademy ────────────────────────────────────────────────────────────

pub struct MockAcademy;

impl pallet_guilds::AcademyInterface<AccountId> for MockAcademy {
    fn is_academician(who: &AccountId) -> bool {
        ACADEMICIANS.with(|a| a.borrow().contains(who))
    }
}

// ── NoopTerminalHook ───────────────────────────────────────────────────────

pub struct NoopHook;

impl pallet_inomad_identity::OnTerminalStatus<AccountId> for NoopHook {
    fn on_exiled(_who: &AccountId) {}
    fn on_deceased(_who: &AccountId) {}
}

// ── construct_runtime ──────────────────────────────────────────────────────

construct_runtime!(
    pub enum Test {
        System:   frame_system,
        Balances: pallet_balances,
        Identity: pallet_inomad_identity,
        Khural:   crate,
    }
);

#[derive_impl(frame_system::config_preludes::TestDefaultConfig)]
impl frame_system::Config for Test {
    type Block = Block;
    type AccountData = pallet_balances::AccountData<Balance>;
}

#[derive_impl(pallet_balances::config_preludes::TestDefaultConfig)]
impl pallet_balances::Config for Test {
    type Balance = Balance;
    type ExistentialDeposit = ConstU128<1_000_000_000>; // 0.001 ALTAN
    type AccountStore = System;
}

parameter_types! {
    pub const MedicalAuthorityAccount: AccountId = 999;
    pub const CivilTreasury: AccountId = 998;
    pub const NoConstitutionHash: Option<[u8; 32]> = None;
}

// ── pallet-inomad-identity Config ─────────────────────────────────────────

impl pallet_inomad_identity::Config for Test {
    type Currency = Balances;
    type UnlockLevel0 = ConstU128<0>;
    type UnlockLevel1 = ConstU128<0>;
    type MedicalAuthority = MedicalAuthorityAccount;
    type MarriageFee = ConstU128<0>;
    type CivilFeeTreasury = CivilTreasury;
    type ArbadCooldownPeriod = ConstU64<0>;
    type MaxRegistrationsPerBlock = ConstU32<100>;
    type TerminalHook = NoopHook;
    type ConstitutionHashProvider = NoConstitutionHash;
    type WeightInfo = ();
}

// ── NationTreasuryProvider: 2 nations ─────────────────────────────────────

parameter_types! {
    pub NationTreasuryList: BoundedVec<AccountId, ConstU32<150>> = {
        let mut v = BoundedVec::new();
        let _ = v.try_push(NATION1_TREASURY); // nation 1
        let _ = v.try_push(NATION2_TREASURY); // nation 2
        v
    };
}

// ── Khural Config ──────────────────────────────────────────────────────────

impl crate::Config for Test {
    type Currency = Balances;
    type NationTreasuryProvider = NationTreasuryList;
    type AcademyInterface = MockAcademy;
    type ExpertBillDeposit = ConstU128<{ 10 * UNIT }>;
    type VotingPeriod = ConstU32<100>; // 100 blocks for tests
    type MinQuorum = ConstU32<3>;
}

// ── Citizen seeding helper ─────────────────────────────────────────────────

/// Insert a CitizenRecord directly into the Identity pallet's Citizens storage.
/// Used to bypass the full registration flow in unit tests.
pub fn seed_citizen(who: AccountId, nation: u32, role: CitizenRole) {
    pallet_inomad_identity::pallet::Citizens::<Test>::insert(
        who,
        CitizenRecord {
            citizen_id: who,
            nation_id: nation,
            naturalized_people_id: None,
            role,
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
            document_hash: H256::zero(),
            birth_page_hash: H256::zero(),
            email_hash: H256::zero(),
        },
    );
}

// ── Genesis ────────────────────────────────────────────────────────────────

pub fn new_test_ext() -> sp_io::TestExternalities {
    let mut t = frame_system::GenesisConfig::<Test>::default()
        .build_storage()
        .unwrap();

    pallet_balances::GenesisConfig::<Test> {
        balances: vec![
            (ALICE, 1_000 * UNIT),
            (BOB, 1_000 * UNIT),
            (CHARLIE, 1_000 * UNIT),
            (DAVE, 1_000 * UNIT),
            (EVE, 1_000 * UNIT),
            (NATION1_TREASURY, 100_000 * UNIT),
            (NATION2_TREASURY, 100_000 * UNIT),
        ],
        dev_accounts: None,
    }
    .assimilate_storage(&mut t)
    .unwrap();

    let mut ext = sp_io::TestExternalities::new(t);
    ext.execute_with(|| {
        System::set_block_number(1);
        // Seed citizens — nation 1
        seed_citizen(ALICE, 1, CitizenRole::Regular);
        seed_citizen(BOB, 1, CitizenRole::Regular);
        seed_citizen(CHARLIE, 1, CitizenRole::Regular);
        seed_citizen(DAVE, 1, CitizenRole::ArbadLeader); // Expert bill voter
                                                         // Nation 2 citizen for cross-nation tests
        seed_citizen(EVE, 2, CitizenRole::Regular);
    });
    ext
}
