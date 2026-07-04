//! Mock runtime for pallet-inheritance unit tests.

use frame_support::{
    construct_runtime, derive_impl,
    traits::{ConstU128, ConstU32},
};
use sp_runtime::BuildStorage;
use std::cell::RefCell;

pub type AccountId = u64;
pub type Balance = u128;
pub type Block = frame_system::mocking::MockBlock<Test>;

pub const UNIT: Balance = 1_000_000_000_000; // 1 ALTAN

// ── Well-known accounts ───────────────────────────────────────────────────

pub const ALICE: AccountId = 1; // testator
pub const BOB: AccountId = 2; // heir 1
pub const CHARLIE: AccountId = 3; // heir 2
pub const NOTARY: AccountId = 4; // licensed notary
pub const TREASURY: AccountId = 900;

// ── Thread-local mocks ────────────────────────────────────────────────────

thread_local! {
    pub static DECEASED: RefCell<Vec<AccountId>> = RefCell::new(Vec::new());
    pub static NOTARIES: RefCell<Vec<AccountId>> = RefCell::new(Vec::new());
    pub static CDP_DEBT: RefCell<Vec<(AccountId, u128)>> = RefCell::new(Vec::new());
}

pub fn register_deceased(who: AccountId) {
    DECEASED.with(|d| d.borrow_mut().push(who));
}

pub fn register_notary(who: AccountId) {
    NOTARIES.with(|n| n.borrow_mut().push(who));
}

pub fn set_cdp_debt(who: AccountId, amount: u128) {
    CDP_DEBT.with(|d| {
        let mut dd = d.borrow_mut();
        dd.retain(|(a, _)| *a != who);
        dd.push((who, amount));
    });
}

pub fn clear_mocks() {
    DECEASED.with(|d| d.borrow_mut().clear());
    NOTARIES.with(|n| n.borrow_mut().clear());
    CDP_DEBT.with(|d| d.borrow_mut().clear());
}

// ── Mock implementations ──────────────────────────────────────────────────

pub struct MockIdentity;

impl crate::IdentityInheritanceInterface<AccountId> for MockIdentity {
    fn is_deceased(who: &AccountId) -> bool {
        DECEASED.with(|d| d.borrow().contains(who))
    }
}

pub struct MockNotaryGuild;

impl crate::GuildsNotaryInterface<AccountId> for MockNotaryGuild {
    fn is_valid_notary(who: &AccountId) -> bool {
        NOTARIES.with(|n| n.borrow().contains(who))
    }
}

pub struct MockBankDebt;

impl crate::BankDebtInterface<AccountId> for MockBankDebt {
    fn total_outstanding_debt(who: &AccountId) -> u128 {
        CDP_DEBT.with(|d| {
            d.borrow()
                .iter()
                .find(|(a, _)| a == who)
                .map(|(_, amt)| *amt)
                .unwrap_or(0)
        })
    }
}

// ── construct_runtime ──────────────────────────────────────────────────────

construct_runtime!(
    pub enum Test {
        System:      frame_system,
        Balances:    pallet_balances,
        Inheritance: crate,
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

frame_support::parameter_types! {
    pub const InheritanceTreasury: AccountId = TREASURY;
}

impl crate::Config for Test {
    type Currency = Balances;
    type GuildsChecker = MockNotaryGuild;
    type IdentityChecker = MockIdentity;
    type BankInterface = MockBankDebt;
    type StateTreasury = InheritanceTreasury;
    type MaxHeirs = ConstU32<20>;
    type WeightInfo = ();
}

// ── Genesis ────────────────────────────────────────────────────────────────

pub fn new_test_ext() -> sp_io::TestExternalities {
    let mut t = frame_system::GenesisConfig::<Test>::default()
        .build_storage()
        .unwrap();

    pallet_balances::GenesisConfig::<Test> {
        balances: vec![
            (ALICE, 10_000 * UNIT),
            (BOB, 1_000 * UNIT),
            (CHARLIE, 1_000 * UNIT),
            (NOTARY, 1_000 * UNIT),
            (TREASURY, 100_000 * UNIT),
        ],
        dev_accounts: None,
    }
    .assimilate_storage(&mut t)
    .unwrap();

    let mut ext = sp_io::TestExternalities::new(t);
    ext.execute_with(|| {
        System::set_block_number(1);
        register_notary(NOTARY);
    });
    ext
}
