//! Mock runtime for pallet-recovery-nft unit tests.

use frame_support::sp_runtime::BuildStorage;
use frame_support::{construct_runtime, derive_impl, parameter_types, traits::ConstU32};

pub type AccountId = u64;
pub type Block = frame_system::mocking::MockBlock<Test>;

// ── Well-known accounts ───────────────────────────────────────────────────

/// The issuer (IssuerOrigin — Creator / Arbad admin).
pub const ISSUER: AccountId = 1;
/// The target account whose funds would be recovered.
pub const TARGET: AccountId = 2;
/// Three trusted holders (Alice, Bob, Carol).
pub const ALICE: AccountId = 10;
pub const BOB: AccountId = 11;
pub const CAROL: AccountId = 12;
pub const DAVE: AccountId = 13;
pub const EVE: AccountId = 14;

// ── Runtime ───────────────────────────────────────────────────────────────

construct_runtime!(
    pub enum Test {
        System: frame_system,
        RecoveryNft: crate,
    }
);

#[derive_impl(frame_system::config_preludes::TestDefaultConfig)]
impl frame_system::Config for Test {
    type Block = Block;
}

parameter_types! {
    /// 72 h × 3600 s / 6 s per block = 43_200 blocks
    pub const VetoWindowBlocks: u32 = 43_200;
    pub const MaxGroupSize: u8 = 10;
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
    type IssuerOrigin = EnsureSigned;
    type VetoWindowBlocks = VetoWindowBlocks;
    type MaxGroupSize = MaxGroupSize;
    type WeightInfo = ();
}

// ── Externalities ─────────────────────────────────────────────────────────

pub fn new_test_ext() -> sp_io::TestExternalities {
    let t = frame_system::GenesisConfig::<Test>::default()
        .build_storage()
        .unwrap();
    sp_io::TestExternalities::new(t)
}
