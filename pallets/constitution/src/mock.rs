//! Mock runtime for pallet-constitution unit tests.
//!
//! Uses a configurable `TestHabeasHook` so tests can control whether
//! `has_verdict` returns true or false for a given citizen.

use crate as pallet_constitution;
use frame_support::{derive_impl, parameter_types};
use sp_io::TestExternalities;
use sp_runtime::BuildStorage;
use std::cell::RefCell;
use std::collections::BTreeSet;

type Block = frame_system::mocking::MockBlock<Test>;

// ─────────────────────────────────────────────────────────────────────────────
// Configurable hook: citizens with a verdict are in VERDICT_SET
// ─────────────────────────────────────────────────────────────────────────────

thread_local! {
    /// Set of citizens for whom `has_verdict` returns `true`.
    pub static VERDICT_SET: RefCell<BTreeSet<u64>> = RefCell::new(BTreeSet::new());
    /// Track which citizens were released via `release_citizen`.
    pub static RELEASED: RefCell<Vec<u64>> = RefCell::new(Vec::new());
}

pub struct TestHabeasHook;

impl pallet_constitution::HabeasCorpusInterface<u64> for TestHabeasHook {
    fn has_verdict(who: &u64) -> bool {
        VERDICT_SET.with(|s| s.borrow().contains(who))
    }
    fn release_citizen(who: &u64) -> frame_support::dispatch::DispatchResult {
        RELEASED.with(|r| r.borrow_mut().push(*who));
        Ok(())
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Mock Runtime
// ─────────────────────────────────────────────────────────────────────────────

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
    pub type Constitution = pallet_constitution::Pallet<Test>;
}

#[derive_impl(frame_system::config_preludes::TestDefaultConfig)]
impl frame_system::Config for Test {
    type Block = Block;
}

parameter_types! {
    pub const MaxConcurrentTimers: u32 = 16;
}

impl pallet_constitution::Config for Test {
    type MaxConcurrentTimers = MaxConcurrentTimers;
    type HabeasCorpusHook = TestHabeasHook;
    type WeightInfo = ();
}

// ─────────────────────────────────────────────────────────────────────────────
// Named accounts
// ─────────────────────────────────────────────────────────────────────────────

pub const ALICE: u64 = 1;
pub const BOB: u64 = 2;

pub fn new_test_ext() -> TestExternalities {
    VERDICT_SET.with(|s| s.borrow_mut().clear());
    RELEASED.with(|r| r.borrow_mut().clear());

    let t = frame_system::GenesisConfig::<Test>::default()
        .build_storage()
        .unwrap();
    let mut ext = TestExternalities::new(t);
    ext.execute_with(|| System::set_block_number(1));
    ext
}
