//! Mock runtime for pallet-foreign-affairs unit tests.

use frame_support::sp_runtime::BuildStorage;
use frame_support::{
    construct_runtime, derive_impl, parameter_types,
    traits::{ConstU128, ConstU32},
};

pub type AccountId = u64;
pub type Balance = u128;
pub type Block = frame_system::mocking::MockBlock<Test>;

pub const UNIT: Balance = 1_000_000_000_000;

// ── Well-known accounts ───────────────────────────────────────────────────

/// The Confederation MFA admin (DiplomaticCouncil / Foreign Minister).
pub const MFA_ADMIN: AccountId = 1;
/// A foreign diplomat.
pub const DIPLOMAT: AccountId = 2;
/// An unprivileged citizen.
pub const CITIZEN: AccountId = 3;

// ── Runtime ───────────────────────────────────────────────────────────────

construct_runtime!(
    pub enum Test {
        System: frame_system,
        Balances: pallet_balances,
        ForeignAffairs: crate,
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

parameter_types! {
    pub const MaxChannelMessages: u32 = 50;
    pub const MaxDiplomaticCouncil: u32 = 20;
}

impl crate::Config for Test {
    type Currency = Balances;
    type MaxChannelMessages = MaxChannelMessages;
    type MaxDiplomaticCouncil = MaxDiplomaticCouncil;
    type WeightInfo = ();
}

// ── Externalities ─────────────────────────────────────────────────────────

pub fn new_test_ext() -> sp_io::TestExternalities {
    let mut t = frame_system::GenesisConfig::<Test>::default()
        .build_storage()
        .unwrap();

    pallet_balances::GenesisConfig::<Test> {
        balances: vec![
            (MFA_ADMIN, 100_000 * UNIT),
            (DIPLOMAT, 10_000 * UNIT),
            (CITIZEN, 1_000 * UNIT),
        ],
        dev_accounts: None,
    }
    .assimilate_storage(&mut t)
    .unwrap();

    sp_io::TestExternalities::new(t)
}
