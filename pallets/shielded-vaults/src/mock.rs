//! Mock runtime for pallet-shielded-vaults unit tests.
//!
//! Key design decisions:
//! - `MockStateGuard` uses a thread-local blocklist — state accounts blocked from shielding.
//! - `MockOrgRegion` maps org_id → a fixed regional treasury account.
//! - Commitment hashes are deterministic test fixtures, not real ZK proofs.

use frame_support::{construct_runtime, derive_impl, parameter_types, traits::ConstU128};
use sp_runtime::BuildStorage;
use std::cell::RefCell;
use std::collections::BTreeSet;

// ── Types ─────────────────────────────────────────────────────────────────

pub type AccountId = u64;
pub type Balance = u128;
pub type Block = frame_system::mocking::MockBlock<Test>;

pub const UNIT: Balance = 1_000_000_000_000; // 1 ALTAN

// ── Well-known accounts ───────────────────────────────────────────────────

pub const ALICE: AccountId = 1; // citizen — may shield
pub const BOB: AccountId = 2; // citizen — may shield
pub const CENTRAL_BANK: AccountId = 100; // state account — BLOCKED
pub const REGIONAL_TREASURY: AccountId = 200;
pub const CONFEDERATION_TREASURY: AccountId = 201;
pub const ORG_ID: u32 = 42;

// ── Thread-local state-account blocklist ──────────────────────────────────

thread_local! {
    pub static STATE_ACCOUNTS: RefCell<BTreeSet<AccountId>> =
        RefCell::new(BTreeSet::from([CENTRAL_BANK]));
}

pub fn block_state_account(who: AccountId) {
    STATE_ACCOUNTS.with(|s| {
        s.borrow_mut().insert(who);
    });
}

pub fn unblock_state_account(who: AccountId) {
    STATE_ACCOUNTS.with(|s| {
        s.borrow_mut().remove(&who);
    });
}

// ── Mock StateAccountChecker ──────────────────────────────────────────────

pub struct MockStateGuard;

impl crate::StateAccountChecker<AccountId> for MockStateGuard {
    fn is_state_account(who: &AccountId) -> bool {
        STATE_ACCOUNTS.with(|s| s.borrow().contains(who))
    }
}

// ── Mock OrgRegionResolver ────────────────────────────────────────────────

pub struct MockOrgRegion;

impl crate::OrgRegionResolverTrait<AccountId> for MockOrgRegion {
    fn regional_treasury_for(_org_id: u32) -> Option<AccountId> {
        Some(REGIONAL_TREASURY)
    }
}

// ── construct_runtime ─────────────────────────────────────────────────────

construct_runtime!(
    pub enum Test {
        System:   frame_system,
        Balances: pallet_balances,
        Vaults:   crate,
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
    pub const ConfTreasury: AccountId = CONFEDERATION_TREASURY;
}

impl crate::Config for Test {
    type Currency = Balances;
    type TransparentStateGuard = MockStateGuard;
    type ConfederationTreasury = ConfTreasury;
    type OrgRegionResolver = MockOrgRegion;
    type WeightInfo = ();
}

// ── Commitment / Nullifier fixtures ───────────────────────────────────────

/// A deterministic commitment hash (simulates blake2_256 output).
pub fn commitment(seed: u8) -> [u8; 32] {
    [seed; 32]
}

/// A deterministic nullifier hash (simulates blake2_256(commitment || secret)).
pub fn nullifier(seed: u8) -> [u8; 32] {
    [seed + 0x80u8; 32]
}

// ── Genesis ───────────────────────────────────────────────────────────────

pub fn new_test_ext() -> sp_io::TestExternalities {
    let mut t = frame_system::GenesisConfig::<Test>::default()
        .build_storage()
        .unwrap();

    pallet_balances::GenesisConfig::<Test> {
        balances: vec![
            (ALICE, 1_000 * UNIT),
            (BOB, 500 * UNIT),
            (CENTRAL_BANK, 100 * UNIT),
            (REGIONAL_TREASURY, UNIT),      // seeded with ED
            (CONFEDERATION_TREASURY, UNIT), // seeded with ED
        ],
        dev_accounts: None,
    }
    .assimilate_storage(&mut t)
    .unwrap();

    let mut ext = sp_io::TestExternalities::new(t);
    ext.execute_with(|| System::set_block_number(1));
    ext
}
