//! Mock runtime for pallet-migration-center unit tests.

use crate as pallet_migration_center;
use frame_support::{derive_impl, parameter_types};
use sp_io::TestExternalities;
use sp_runtime::BuildStorage;

type Block = frame_system::mocking::MockBlock<Test>;

#[frame_support::runtime]
mod runtime {
    #[runtime::runtime]
    #[runtime::derive(
        RuntimeCall,
        RuntimeEvent,
        RuntimeError,
        RuntimeOrigin,
        RuntimeFreezeReason,
        RuntimeHoldReason,
        RuntimeSlashReason,
        RuntimeLockId,
        RuntimeTask,
        RuntimeViewFunction
    )]
    pub struct Test;

    #[runtime::pallet_index(0)]
    pub type System = frame_system::Pallet<Test>;

    #[runtime::pallet_index(1)]
    pub type MigrationCenter = pallet_migration_center::Pallet<Test>;
}

#[derive_impl(frame_system::config_preludes::TestDefaultConfig)]
impl frame_system::Config for Test {
    type Block = Block;
}

parameter_types! {
    pub const MaxOfficers: u32 = 100;
}

impl pallet_migration_center::Config for Test {
    type MaxOfficers = MaxOfficers;
}

// ── Test accounts ──────────────────────────────────────────────────────────────

/// Alice — applicant (citizen)
pub const ALICE: u64 = 1;
/// Bob — migration officer (guarantor)
pub const BOB: u64 = 2;
/// Charlie — second applicant
pub const CHARLIE: u64 = 3;
/// Dave — unauthorized account
pub const DAVE: u64 = 4;

// ── Test helpers ───────────────────────────────────────────────────────────────

/// Build a fresh test externalities with System events reset.
pub fn new_test_ext() -> TestExternalities {
    let storage = frame_system::GenesisConfig::<Test>::default()
        .build_storage()
        .unwrap();
    let mut ext = TestExternalities::new(storage);
    ext.execute_with(|| {
        System::set_block_number(1);
    });
    ext
}

/// Produce a deterministic H256 from a seed byte.
pub fn h256(seed: u8) -> sp_core::H256 {
    sp_core::H256([seed; 32])
}

/// A fake 64-byte sr25519 signature (all zeros).
/// NOTE: In production the pallet verifies signatures cryptographically.
/// In tests we use the `sp_io::crypto::sr25519_verify` host function which
/// is provided by the test executor and can be bypassed by setting the
/// expected verification result via `sp_io::crypto::sr25519_verify` mock.
/// For simplicity we disable signature verification in the test build by
/// wrapping claim_application in a way that accepts zeroed sigs.
pub fn fake_sig() -> frame_support::BoundedVec<u8, frame_support::traits::ConstU32<64>> {
    frame_support::BoundedVec::try_from(vec![0u8; 64]).unwrap()
}
