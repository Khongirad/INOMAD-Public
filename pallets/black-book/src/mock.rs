//! Mock runtime for pallet-black-book unit tests.

use frame_support::{construct_runtime, derive_impl, traits::ConstU128};
use sp_runtime::BuildStorage;
use std::cell::RefCell;

pub type AccountId = u64;
pub type Balance = u128;
pub type Block = frame_system::mocking::MockBlock<Test>;

pub const UNIT: Balance = 1_000_000_000_000;

// ── Well-known accounts ───────────────────────────────────────────────────

pub const ROOT_TREASURY: AccountId = 900;
pub const ALICE: AccountId = 1; // regular criminal
pub const ACADEMICIAN: AccountId = 2; // protected by anti-tyranny guard
pub const DELEGATE: AccountId = 3; // khural delegate, also protected
pub const HUNTER: AccountId = 4; // bounty hunter
pub const DONOR: AccountId = 5; // citizen who donates to bounty

// ── Thread-local mocks ─────────────────────────────────────────────────────

thread_local! {
    static ACADEMICIANS:   RefCell<Vec<AccountId>> = RefCell::new(Vec::new());
    static DELEGATES:      RefCell<Vec<AccountId>> = RefCell::new(Vec::new());
    static EXILED:         RefCell<Vec<AccountId>> = RefCell::new(Vec::new());
}

pub fn register_academician(who: AccountId) {
    ACADEMICIANS.with(|a| a.borrow_mut().push(who));
}

pub fn register_delegate(who: AccountId) {
    DELEGATES.with(|d| d.borrow_mut().push(who));
}

pub fn is_exiled(who: AccountId) -> bool {
    EXILED.with(|e| e.borrow().contains(&who))
}

pub fn clear_mocks() {
    ACADEMICIANS.with(|a| a.borrow_mut().clear());
    DELEGATES.with(|d| d.borrow_mut().clear());
    EXILED.with(|e| e.borrow_mut().clear());
}

// ── Mock implementations ──────────────────────────────────────────────────

pub struct MockIdentity;

impl crate::BlackBookIdentityInterface<AccountId> for MockIdentity {
    fn exile_citizen(who: &AccountId) -> frame_support::dispatch::DispatchResult {
        EXILED.with(|e| e.borrow_mut().push(*who));
        Ok(())
    }
}

pub struct MockAcademy;

impl pallet_guilds::AcademyInterface<AccountId> for MockAcademy {
    fn is_academician(who: &AccountId) -> bool {
        ACADEMICIANS.with(|a| a.borrow().contains(who))
    }
}

pub struct MockKhural;

impl crate::KhuralDelegateInterface<AccountId> for MockKhural {
    fn is_active_khural_delegate(who: &AccountId) -> bool {
        DELEGATES.with(|d| d.borrow().contains(who))
    }
}

// ── construct_runtime ─────────────────────────────────────────────────────

construct_runtime!(
    pub enum Test {
        System:    frame_system,
        Balances:  pallet_balances,
        BlackBook: crate,
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
    type ExistentialDeposit = ConstU128<1_000_000_000>;
    type AccountStore = System;
}

frame_support::parameter_types! {
    pub const Treasury: AccountId = ROOT_TREASURY;
    pub const BountyLock: u32 = 10; // 10 blocks for tests
}

impl crate::Config for Test {
    type Currency = Balances;
    type IdentityBridge = MockIdentity;
    type AcademyChecker = MockAcademy;
    type KhuralChecker = MockKhural;
    type StateTreasury = Treasury;
    type BountyLockPeriod = BountyLock;
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
            (ACADEMICIAN, 5_000 * UNIT),
            (DELEGATE, 5_000 * UNIT),
            (HUNTER, 1_000 * UNIT),
            (DONOR, 1_000 * UNIT),
            (ROOT_TREASURY, 50_000 * UNIT),
        ],
        dev_accounts: None,
    }
    .assimilate_storage(&mut t)
    .unwrap();

    let mut ext = sp_io::TestExternalities::new(t);
    ext.execute_with(|| System::set_block_number(1));
    ext
}
