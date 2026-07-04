//! Mock runtime for pallet-achievement-nft unit tests.

use frame_support::sp_runtime::BuildStorage;
use frame_support::{
    construct_runtime, derive_impl, parameter_types,
    traits::{ConstU128, ConstU32},
};

pub type AccountId = u64;
pub type Balance = u128;
pub type Block = frame_system::mocking::MockBlock<Test>;

/// 1 ALTAN = 10^12 planck
pub const UNIT: Balance = 1_000_000_000_000;

// ── Well-known accounts ───────────────────────────────────────────────────

/// NFT Issuer — the NFT Akademik collective (multi-sig). Has NftIssuerOrigin.
pub const ISSUER: AccountId = 1;
/// Alice — receives achievement NFTs.
pub const ALICE: AccountId = 2;
/// Bob — another citizen.
pub const BOB: AccountId = 3;
/// Confederation treasury (source of Reward NFT ALTAN).
pub const CONFEDERATION_TREASURY: AccountId = 100;

// ── Runtime ───────────────────────────────────────────────────────────────

construct_runtime!(
    pub enum Test {
        System: frame_system,
        Balances: pallet_balances,
        AchievementNft: crate,
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
    pub const ConfederationTreasury: AccountId = CONFEDERATION_TREASURY;
    pub const MaxAchievementsPerHolder: u32 = 50;
}

/// Signed-account origin that succeeds for any signed account.
pub struct EnsureSigned;
impl frame_support::traits::EnsureOrigin<RuntimeOrigin> for EnsureSigned {
    type Success = AccountId;
    fn try_origin(o: RuntimeOrigin) -> Result<Self::Success, RuntimeOrigin> {
        match o.clone().into() {
            Ok(frame_system::RawOrigin::Signed(who)) => Ok(who),
            _ => Err(o),
        }
    }

    #[cfg(feature = "runtime-benchmarks")]
    fn try_successful_origin() -> Result<RuntimeOrigin, ()> {
        Ok(RuntimeOrigin::signed(ISSUER))
    }
}

impl crate::Config for Test {
    type Currency = Balances;
    type NftIssuerOrigin = EnsureSigned;
    type ConfederationTreasury = ConfederationTreasury;
    type MaxAchievementsPerHolder = MaxAchievementsPerHolder;
    type WeightInfo = ();
}

// ── Externalities ─────────────────────────────────────────────────────────

pub fn new_test_ext() -> sp_io::TestExternalities {
    let mut t = frame_system::GenesisConfig::<Test>::default()
        .build_storage()
        .unwrap();

    pallet_balances::GenesisConfig::<Test> {
        balances: vec![
            (ISSUER, 100_000 * UNIT),
            (ALICE, 10_000 * UNIT),
            (BOB, 10_000 * UNIT),
            (CONFEDERATION_TREASURY, 10_000_000 * UNIT),
        ],
        dev_accounts: None,
    }
    .assimilate_storage(&mut t)
    .unwrap();

    sp_io::TestExternalities::new(t)
}
