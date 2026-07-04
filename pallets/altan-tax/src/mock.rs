//! Mock runtime for pallet-altan-tax unit tests.
//!
//! v2 Constitutional Reform: 54/36/10 split.
//! Creator compensation flows through INOMAD AG (Swiss GmbH) — no direct on-chain routing.

use frame_support::{construct_runtime, derive_impl, traits::ConstU128};
use sp_runtime::BuildStorage;

pub type AccountId = u64;
pub type Balance = u128;
pub type Block = frame_system::mocking::MockBlock<Test>;

/// 1 ALTAN = 10^12 planck
pub const UNIT: Balance = 1_000_000_000_000;

construct_runtime!(
    pub enum Test {
        System: frame_system,
        Balances: pallet_balances,
        AltanTax: crate,
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

impl crate::Config for Test {
    type Currency = Balances;
}

/// Treasury accounts (v2: 3-way split)
pub const AG_ACCOUNT: AccountId = 100;       // INOMAD AG Swiss (36% — includes Creator)
pub const KHURAL_ACCOUNT: AccountId = 200;   // Khural Foundation (54% + dust)
pub const VALIDATOR_ACCOUNT: AccountId = 300; // Validator Pool (10%)

pub fn new_test_ext() -> sp_io::TestExternalities {
    let mut t = frame_system::GenesisConfig::<Test>::default()
        .build_storage()
        .unwrap();

    pallet_balances::GenesisConfig::<Test> {
        balances: vec![
            (1, 10_000_000 * UNIT), // Alice — wealthy sender for fee cap tests
            (2, 500 * UNIT),        // Bob — recipient
            // Seed treasury accounts with ED so they can receive transfers
            (AG_ACCOUNT, 1_000_000_000u128),
            (KHURAL_ACCOUNT, 1_000_000_000u128),
            (VALIDATOR_ACCOUNT, 1_000_000_000u128),
        ],
        dev_accounts: None,
    }
    .assimilate_storage(&mut t)
    .unwrap();

    crate::GenesisConfig::<Test> {
        ag_treasury: Some(AG_ACCOUNT),
        khural_foundation: Some(KHURAL_ACCOUNT),
        validator_pool: Some(VALIDATOR_ACCOUNT),
        nation_treasuries: vec![],
    }
    .assimilate_storage(&mut t)
    .unwrap();

    sp_io::TestExternalities::new(t)
}
