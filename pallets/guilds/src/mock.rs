//! Mock runtime for pallet-guilds unit tests.

use frame_support::{
    construct_runtime, derive_impl,
    traits::{ConstU128, ConstU32},
    PalletId,
};
use sp_runtime::BuildStorage;

pub type AccountId = u64;
pub type Balance = u128;
pub type Block = frame_system::mocking::MockBlock<Test>;

pub const UNIT: Balance = 1_000_000_000_000; // 1 ALTAN

construct_runtime!(
    pub enum Test {
        System: frame_system,
        Balances: pallet_balances,
        GuildsPallet: crate,
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
    pub const GuildsPalletId: PalletId = PalletId(*b"inm/glds");
}

impl crate::Config for Test {
    type Currency = Balances;
    type MaxNameLength = ConstU32<64>;
    type MaxIpfsCidLength = ConstU32<64>;
    type MaxUnionMembers = ConstU32<50>;
    type PalletId = GuildsPalletId;
    type MaxMembers = ConstU32<100>;
    type MaxCouncilMembers = ConstU32<100>;
    type WeightInfo = ();
}

pub fn new_test_ext() -> sp_io::TestExternalities {
    let mut t = frame_system::GenesisConfig::<Test>::default()
        .build_storage()
        .unwrap();

    pallet_balances::GenesisConfig::<Test> {
        balances: vec![
            (1, 1_000 * UNIT), // Alice (master)
            (2, 500 * UNIT),   // Bob (member)
            (3, 200 * UNIT),   // Charlie (also member)
        ],
        dev_accounts: None,
    }
    .assimilate_storage(&mut t)
    .unwrap();

    let mut ext = sp_io::TestExternalities::new(t);
    ext.execute_with(|| System::set_block_number(1));
    ext
}
