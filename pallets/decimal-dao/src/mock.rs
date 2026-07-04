//! Mock runtime for pallet-decimal-dao unit tests.

use frame_support::sp_runtime::BuildStorage;
use frame_support::{
    construct_runtime, derive_impl, parameter_types,
    traits::{ConstU128, ConstU32},
    PalletId,
};

pub type AccountId = u64;
pub type Balance = u128;
pub type Block = frame_system::mocking::MockBlock<Test>;

/// 1 ALTAN = 10^12 planck
pub const UNIT: Balance = 1_000_000_000_000;

// ── Well-known accounts ───────────────────────────────────────────────────

pub const ALICE: AccountId = 1;
pub const BOB: AccountId = 2;
pub const CHARLIE: AccountId = 3;
pub const DAVE: AccountId = 4;
/// An account that is NOT a council member in any org.
pub const EVE: AccountId = 99;

// ── Runtime ───────────────────────────────────────────────────────────────

construct_runtime!(
    pub enum Test {
        System: frame_system,
        Balances: pallet_balances,
        DecimalDao: crate,
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
    pub const DecimalDaoPalletId: PalletId = PalletId(*b"inm/ddao");
    pub const MaxCouncilMembers: u32 = 100;
}

impl crate::Config for Test {
    type Currency = Balances;
    type PalletId = DecimalDaoPalletId;
    type MaxCouncilMembers = MaxCouncilMembers;
    type WeightInfo = ();
}

// ── Helpers ───────────────────────────────────────────────────────────────

/// Create a fixed-size 32-byte ID from a seed string.
pub fn org_id(seed: &[u8]) -> [u8; 32] {
    let mut id = [0u8; 32];
    let len = seed.len().min(32);
    id[..len].copy_from_slice(&seed[..len]);
    id
}

/// Create a fixed-size 32-byte proposal ID from a seed string.
pub fn prop_id(seed: &[u8]) -> [u8; 32] {
    org_id(seed)
}

// ── Externalities ─────────────────────────────────────────────────────────

pub fn new_test_ext() -> sp_io::TestExternalities {
    let mut t = frame_system::GenesisConfig::<Test>::default()
        .build_storage()
        .unwrap();

    pallet_balances::GenesisConfig::<Test> {
        balances: vec![
            (ALICE, 100_000 * UNIT),
            (BOB, 100_000 * UNIT),
            (CHARLIE, 100_000 * UNIT),
            (DAVE, 100_000 * UNIT),
            (EVE, 1_000 * UNIT),
        ],
        dev_accounts: None,
    }
    .assimilate_storage(&mut t)
    .unwrap();

    sp_io::TestExternalities::new(t)
}
