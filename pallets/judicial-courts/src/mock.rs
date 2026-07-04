//! Mock runtime for pallet-judicial-courts unit tests.
//!
//! Uses a minimal in-memory `MockIdentity` to avoid depending on the full
//! pallet-inomad-identity runtime, while still correctly exercising the
//! citizen status transitions triggered by the judicial engine.

use frame_support::{
    construct_runtime, derive_impl, parameter_types,
    traits::{ConstU128, EnsureOrigin},
};
use sp_core::H256;
use sp_runtime::BuildStorage;
use std::cell::RefCell;
use std::collections::BTreeMap;

// ── Re-export types used by the pallet ────────────────────────────────────
pub use pallet_inomad_identity::{
    CitizenRecord, CitizenRole, CitizenStatus, CitizenshipStatus, PassportType, VerificationStatus,
};

pub type AccountId = u64;
pub type Balance = u128;
pub type Block = frame_system::mocking::MockBlock<Test>;

pub const UNIT: Balance = 1_000_000_000_000; // 1 ALTAN

// ── Thread-local identity store ───────────────────────────────────────────

thread_local! {
    /// Simulated citizen registry: AccountId → CitizenStatus.
    pub static CITIZEN_STATUS: RefCell<BTreeMap<AccountId, CitizenStatus>> =
        RefCell::new(BTreeMap::new());
}

/// Register a citizen in the mock registry.
pub fn register_citizen(who: AccountId) {
    CITIZEN_STATUS.with(|m| {
        m.borrow_mut().insert(who, CitizenStatus::Active);
    });
}

/// Get a citizen's status.
pub fn citizen_status(who: AccountId) -> Option<CitizenStatus> {
    CITIZEN_STATUS.with(|m| m.borrow().get(&who).cloned())
}

// ── construct_runtime ─────────────────────────────────────────────────────

construct_runtime!(
    pub enum Test {
        System: frame_system,
        Balances: pallet_balances,
        Courts: crate,
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

// ── Treasury constant ─────────────────────────────────────────────────────

pub const TREASURY: AccountId = 999;

parameter_types! {
    pub TreasuryAccount: AccountId = TREASURY;
}

// ── Mock IdentityInterface ────────────────────────────────────────────────

pub struct MockIdentity;

impl pallet_inomad_identity::IdentityInterface<AccountId> for MockIdentity {
    fn citizen_record_of(who: &AccountId) -> Option<CitizenRecord> {
        let status = CITIZEN_STATUS.with(|m| m.borrow().get(who).cloned())?;
        Some(CitizenRecord {
            citizen_id: *who as u64,
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
            citizenship_status: CitizenshipStatus::Naturalized,
            region_id: None,
            birth_region_id: None,
            passport_type: PassportType::Internal,
            document_hash: H256::zero(),
            birth_page_hash: H256::zero(),
            email_hash: H256::zero(),
        })
    }

    fn freeze_citizen(who: &AccountId) -> frame_support::dispatch::DispatchResult {
        CITIZEN_STATUS.with(|m| {
            let mut map = m.borrow_mut();
            if let Some(s) = map.get_mut(who) {
                *s = CitizenStatus::Frozen;
            }
        });
        Ok(())
    }

    fn unfreeze_citizen(who: &AccountId) -> frame_support::dispatch::DispatchResult {
        CITIZEN_STATUS.with(|m| {
            let mut map = m.borrow_mut();
            if let Some(s) = map.get_mut(who) {
                *s = CitizenStatus::Active;
            }
        });
        Ok(())
    }

    fn demote_to_regular(_who: &AccountId) -> frame_support::dispatch::DispatchResult {
        Ok(())
    }

    fn exile_citizen(who: &AccountId) -> frame_support::dispatch::DispatchResult {
        CITIZEN_STATUS.with(|m| {
            let mut map = m.borrow_mut();
            if let Some(s) = map.get_mut(who) {
                *s = CitizenStatus::Exiled;
            }
        });
        Ok(())
    }
}

// ── Root-gated origins for judges / usurpation ───────────────────────────

pub struct RootOrigin;

impl EnsureOrigin<RuntimeOrigin> for RootOrigin {
    type Success = ();
    fn try_origin(o: RuntimeOrigin) -> Result<Self::Success, RuntimeOrigin> {
        frame_system::ensure_root(o).map_err(|_| {
            // Return a signed origin as the error variant
            RuntimeOrigin::signed(0)
        })
    }
    #[cfg(feature = "runtime-benchmarks")]
    fn try_successful_origin() -> Result<RuntimeOrigin, ()> {
        Ok(RuntimeOrigin::root())
    }
}

// ── JudicialCourts Config ─────────────────────────────────────────────────

impl crate::Config for Test {
    type Currency = Balances;
    type Identity = MockIdentity;
    type ConstitutionBridge = (); // no-op
    type JudgesCollectiveOrigin = RootOrigin;
    type UsurpationOrigin = RootOrigin;
    type Treasury = TreasuryAccount;
    type WeightInfo = ();
}

// ── Evidence hash helper ──────────────────────────────────────────────────

pub fn evidence_hash() -> [u8; 32] {
    [0xABu8; 32]
}

// ── Genesis ───────────────────────────────────────────────────────────────

pub fn new_test_ext() -> sp_io::TestExternalities {
    let mut t = frame_system::GenesisConfig::<Test>::default()
        .build_storage()
        .unwrap();

    pallet_balances::GenesisConfig::<Test> {
        balances: vec![
            (1, 1_000 * UNIT), // Alice (plaintiff)
            (2, 500 * UNIT),   // Bob (defendant)
            (3, 100 * UNIT),   // Charlie (whistleblower)
            (TREASURY, UNIT),  // Treasury seeded with ED
        ],
        dev_accounts: None,
    }
    .assimilate_storage(&mut t)
    .unwrap();

    let mut ext = sp_io::TestExternalities::new(t);
    ext.execute_with(|| {
        System::set_block_number(1);
        // Register citizens in mock identity system
        register_citizen(1); // Alice
        register_citizen(2); // Bob
        register_citizen(3); // Charlie
    });
    ext
}
