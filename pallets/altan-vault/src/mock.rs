//! Mock runtime for pallet-altan-vault unit tests.

use frame_support::sp_runtime::BuildStorage;
use frame_support::{construct_runtime, derive_impl, parameter_types, traits::ConstU128, PalletId};

pub type AccountId = u64;
pub type Balance = u128;
pub type Block = frame_system::mocking::MockBlock<Test>;

/// 1 ALTAN = 10^12 planck
pub const UNIT: Balance = 1_000_000_000_000;

// ── Well-known accounts ───────────────────────────────────────────────────

pub const ALICE: AccountId = 1;
pub const BOB: AccountId = 2;
/// An account with no vault (should trigger errors).
pub const EVE: AccountId = 99;

// ── Runtime ───────────────────────────────────────────────────────────────

construct_runtime!(
    pub enum Test {
        System: frame_system,
        Balances: pallet_balances,
        AltanVault: crate,
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
    pub const AltanVaultPalletId: PalletId = PalletId(*b"inm/vlt0");
    pub const MaxVaultsPerOwner: u16 = 16;
}

impl crate::Config for Test {
    type Currency = Balances;
    type PalletId = AltanVaultPalletId;
    type MaxVaultsPerOwner = MaxVaultsPerOwner;
    type WeightInfo = ();
}

// ── Externalities ─────────────────────────────────────────────────────────

pub fn new_test_ext() -> sp_io::TestExternalities {
    let mut t = frame_system::GenesisConfig::<Test>::default()
        .build_storage()
        .unwrap();

    pallet_balances::GenesisConfig::<Test> {
        balances: vec![
            (ALICE, 1_000_000 * UNIT),
            (BOB, 500_000 * UNIT),
            (EVE, 100 * UNIT),
        ],
        dev_accounts: None,
    }
    .assimilate_storage(&mut t)
    .unwrap();

    sp_io::TestExternalities::new(t)
}
