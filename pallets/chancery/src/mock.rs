//! Mock runtime for pallet-chancery unit tests.

use frame_support::{construct_runtime, derive_impl, traits::ConstU32};
use sp_runtime::BuildStorage;
use std::cell::RefCell;

pub type AccountId = u64;
pub type Block = frame_system::mocking::MockBlock<Test>;

// ── Well-known accounts ───────────────────────────────────────────────────

pub const ALICE: AccountId = 1; // party / creator
pub const BOB: AccountId = 2; // party
pub const CHARLIE: AccountId = 3; // party
pub const VALIDATOR: AccountId = 10; // professional guild validator

// ── Thread-local mock for GuildsChecker ──────────────────────────────────

thread_local! {
    static VALIDATORS: RefCell<Vec<AccountId>> = RefCell::new(Vec::new());
}

pub fn register_validator(who: AccountId) {
    VALIDATORS.with(|v| v.borrow_mut().push(who));
}

#[allow(dead_code)]
pub fn clear_mocks() {
    VALIDATORS.with(|v| v.borrow_mut().clear());
}

pub struct MockGuilds;

impl crate::ChanceryGuildsInterface<AccountId> for MockGuilds {
    fn is_valid_validator(who: &AccountId) -> bool {
        VALIDATORS.with(|v| v.borrow().contains(who))
    }
}

// ── construct_runtime ─────────────────────────────────────────────────────

construct_runtime!(
    pub enum Test {
        System:   frame_system,
        Chancery: crate,
    }
);

#[derive_impl(frame_system::config_preludes::TestDefaultConfig)]
impl frame_system::Config for Test {
    type Block = Block;
}

impl crate::Config for Test {
    type MaxParties = ConstU32<10>;
    type MaxValidators = ConstU32<5>;
    type GuildsChecker = MockGuilds;
    type WeightInfo = ();
}

// ── Genesis ────────────────────────────────────────────────────────────────

pub fn new_test_ext() -> sp_io::TestExternalities {
    let t = frame_system::GenesisConfig::<Test>::default()
        .build_storage()
        .unwrap();
    let mut ext = sp_io::TestExternalities::new(t);
    ext.execute_with(|| {
        System::set_block_number(1);
        register_validator(VALIDATOR);
    });
    ext
}
