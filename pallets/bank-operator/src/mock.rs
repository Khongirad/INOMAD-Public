//! Mock runtime for pallet-bank-operator unit tests.

use frame_support::{
    construct_runtime, derive_impl,
    traits::{ConstU128, ConstU32},
};
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
        BankOperator: crate,
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
    type ReserveMultiplier = ConstU32<9>;
    /// 1 block = 1 interest period (for test speed).
    type InterestPeriodBlocks = ConstU32<1>;
    type WeightInfo = ();
}

/// Bank special account ID
pub const BANK: AccountId = 999;

pub fn new_test_ext() -> sp_io::TestExternalities {
    let mut t = frame_system::GenesisConfig::<Test>::default()
        .build_storage()
        .unwrap();

    pallet_balances::GenesisConfig::<Test> {
        balances: vec![
            (1, 1_000 * UNIT),     // Alice
            (2, 500 * UNIT),       // Bob
            (BANK, 10_000 * UNIT), // Bank special account
        ],
        dev_accounts: None,
    }
    .assimilate_storage(&mut t)
    .unwrap();

    let mut ext = sp_io::TestExternalities::new(t);
    ext.execute_with(|| {
        System::set_block_number(1);
        assert!(
            BankOperator::set_bank_account(frame_system::RawOrigin::Root.into(), BANK,).is_ok()
        );
    });
    ext
}
