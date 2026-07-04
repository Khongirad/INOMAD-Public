//! Mock runtime for pallet-citizen-voice unit tests.

use frame_support::{
    construct_runtime, derive_impl,
    traits::{ConstU128, ConstU8},
};
use sp_runtime::BuildStorage;
use std::cell::RefCell;

pub type AccountId = u64;
pub type Balance = u128;
pub type Block = frame_system::mocking::MockBlock<Test>;

pub const UNIT: Balance = 1_000_000_000_000; // 1 ALTAN

// ── Well-known accounts ───────────────────────────────────────────────────

pub const ALICE: AccountId = 1; // whistleblower / citizen
pub const TARGET: AccountId = 2; // bribery target
pub const TREASURY: AccountId = 900;

// ── Thread-local Guild Master tracker ────────────────────────────────────

thread_local! {
    pub static GUILD_MASTERS: RefCell<Vec<AccountId>> = RefCell::new(Vec::new());
    pub static EXILED: RefCell<Vec<AccountId>> = RefCell::new(Vec::new());
}

#[allow(dead_code)]
pub fn add_guild_master(who: AccountId) {
    GUILD_MASTERS.with(|g| g.borrow_mut().push(who));
}

pub fn is_exiled(who: AccountId) -> bool {
    EXILED.with(|e| e.borrow().contains(&who))
}

pub fn clear_mocks() {
    GUILD_MASTERS.with(|g| g.borrow_mut().clear());
    EXILED.with(|e| e.borrow_mut().clear());
}

// ── MockGuildsChecker ─────────────────────────────────────────────────────

pub struct MockGuilds;

impl crate::GuildsInterface<AccountId> for MockGuilds {
    fn is_guild_master(_guild_id: u32, who: &AccountId) -> bool {
        GUILD_MASTERS.with(|g| g.borrow().contains(who))
    }
}

// ── MockBlackBook ─────────────────────────────────────────────────────────

pub struct MockBlackBook;

impl crate::BlackBookBridgeInterface<AccountId> for MockBlackBook {
    fn exile_citizen(who: &AccountId) -> frame_support::dispatch::DispatchResult {
        EXILED.with(|e| e.borrow_mut().push(*who));
        Ok(())
    }
}

// ── construct_runtime ──────────────────────────────────────────────────────

construct_runtime!(
    pub enum Test {
        System:   frame_system,
        Balances: pallet_balances,
        Voice:    crate,
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
    pub const TreasuryAccount: AccountId = TREASURY;
    pub const TicketDeposit: Balance = 1 * UNIT;
}

impl crate::Config for Test {
    type Currency = Balances;
    type GuildsChecker = MockGuilds;
    type BlackBookBridge = MockBlackBook;
    type StateTreasury = TreasuryAccount;
    type TicketDeposit = TicketDeposit;
    type WhistleblowerRewardPercent = ConstU8<20>; // 20%
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
            (TARGET, 10_000 * UNIT),
            (TREASURY, 50_000 * UNIT), // seeded so rewards can be paid
        ],
        dev_accounts: None,
    }
    .assimilate_storage(&mut t)
    .unwrap();

    let mut ext = sp_io::TestExternalities::new(t);
    ext.execute_with(|| System::set_block_number(1));
    ext
}
