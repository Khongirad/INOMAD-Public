use frame_support::{
    derive_impl, parameter_types,
    traits::{ConstU32, ConstU64},
};
use sp_runtime::{traits::IdentityLookup, BuildStorage};

type Block = frame_system::mocking::MockBlock<Test>;

frame_support::construct_runtime!(
    pub enum Test {
        System:   frame_system,
        Balances: pallet_balances,
        Identity: pallet_inomad_identity,
        Forums:   crate,
    }
);

#[derive_impl(frame_system::config_preludes::TestDefaultConfig)]
impl frame_system::Config for Test {
    type Block = Block;
    type AccountId = u64;
    type Lookup = IdentityLookup<u64>;
    type AccountData = pallet_balances::AccountData<u128>;
    type BlockHashCount = ConstU64<250>;
}

#[derive_impl(pallet_balances::config_preludes::TestDefaultConfig)]
impl pallet_balances::Config for Test {
    type Balance = u128;
    type ExistentialDeposit = frame_support::traits::ConstU128<1>;
    type AccountStore = System;
}

// ── Identity config ──

parameter_types! {
    pub const ForumsMedicalAuthority: u64 = 999;
}

pub struct NoConstitutionHash;
impl frame_support::traits::Get<Option<[u8; 32]>> for NoConstitutionHash {
    fn get() -> Option<[u8; 32]> {
        None
    }
}

impl pallet_inomad_identity::Config for Test {
    type Currency = Balances;
    type ArbadCooldownPeriod = ConstU64<0>;
    type MaxRegistrationsPerBlock = ConstU32<100>;
    type UnlockLevel0 = frame_support::traits::ConstU128<0>;
    type UnlockLevel1 = frame_support::traits::ConstU128<0>;
    type MedicalAuthority = ForumsMedicalAuthority;
    type MarriageFee = frame_support::traits::ConstU128<0>;
    type CivilFeeTreasury = ForumsMedicalAuthority; // noop treasury for tests
    type TerminalHook = ();
    type ConstitutionHashProvider = NoConstitutionHash;
    type WeightInfo = ();
}

// ── Forums config ──

impl crate::Config for Test {
    type MaxForumIdLen = ConstU32<128>;
    type MaxRepliesPerMessage = ConstU32<1024>;
}

pub fn new_test_ext() -> sp_io::TestExternalities {
    let storage = frame_system::GenesisConfig::<Test>::default()
        .build_storage()
        .unwrap();
    let mut ext = sp_io::TestExternalities::new(storage);
    ext.execute_with(|| {
        frame_system::Pallet::<Test>::set_block_number(1);
    });
    ext
}
