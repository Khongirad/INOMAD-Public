//! Mock runtime for pallet-access-nft unit tests.

use frame_support::sp_runtime::BuildStorage;
use frame_support::{construct_runtime, derive_impl, parameter_types, traits::ConstU32};

pub type AccountId = u64;
pub type Block = frame_system::mocking::MockBlock<Test>;

// ── Well-known accounts ───────────────────────────────────────────────────

/// The issuer — has IssuerOrigin authority.
pub const ISSUER: AccountId = 1;
/// Alice — a citizen who receives access keys.
pub const ALICE: AccountId = 2;
/// Bob — another citizen.
pub const BOB: AccountId = 3;
/// An unregistered account (no special privilege).
pub const EVE: AccountId = 99;

// ── Runtime construction ──────────────────────────────────────────────────

construct_runtime!(
    pub enum Test {
        System: frame_system,
        AccessNft: crate,
    }
);

#[derive_impl(frame_system::config_preludes::TestDefaultConfig)]
impl frame_system::Config for Test {
    type Block = Block;
}

parameter_types! {
    pub const MaxKeysPerHolder: u32 = 10;
    pub const MaxHoldersPerEntity: u32 = 20;
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
    type MaxKeysPerHolder = MaxKeysPerHolder;
    type MaxHoldersPerEntity = MaxHoldersPerEntity;
    type WeightInfo = ();
}

// ── Externalities ─────────────────────────────────────────────────────────

pub fn new_test_ext() -> sp_io::TestExternalities {
    let t = frame_system::GenesisConfig::<Test>::default()
        .build_storage()
        .unwrap();
    sp_io::TestExternalities::new(t)
}
