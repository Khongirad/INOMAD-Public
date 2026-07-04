//! Mock runtime for pallet-organization unit tests.

use crate as pallet_organization;
use frame_support::{derive_impl, parameter_types};
use sp_runtime::{BuildStorage, Perbill};

type Block = frame_system::mocking::MockBlock<Test>;

// ---------------------------------------------------------------------------
// Runtime definition
// ---------------------------------------------------------------------------

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
    pub type Balances = pallet_balances::Pallet<Test>;

    #[runtime::pallet_index(2)]
    pub type Organization = pallet_organization::Pallet<Test>;
}

// ---------------------------------------------------------------------------
// frame-system
// ---------------------------------------------------------------------------

#[derive_impl(frame_system::config_preludes::TestDefaultConfig)]
impl frame_system::Config for Test {
    type Block = Block;
    type AccountData = pallet_balances::AccountData<u64>;
}

// ---------------------------------------------------------------------------
// pallet-balances
// ---------------------------------------------------------------------------

#[derive_impl(pallet_balances::config_preludes::TestDefaultConfig)]
impl pallet_balances::Config for Test {
    type AccountStore = System;
}

// ---------------------------------------------------------------------------
// Mock IdentityBridge
// ---------------------------------------------------------------------------

use std::cell::RefCell;
thread_local! {
    pub static FROZEN_ACCOUNTS: RefCell<Vec<u64>> = RefCell::new(Vec::new());
    /// Mock Unix timestamp in seconds. Set this in tests to control filing period.
    pub static MOCK_TIMESTAMP_SECS: RefCell<u64> = RefCell::new(0);
}

pub struct MockIdentityBridge;
impl pallet_organization::OrgIdentityInterface<u64> for MockIdentityBridge {
    fn freeze_citizen(who: &u64) -> frame_support::dispatch::DispatchResult {
        FROZEN_ACCOUNTS.with(|fa| fa.borrow_mut().push(*who));
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Mock TimeProvider (UnixTime)
// ---------------------------------------------------------------------------

/// Mock UnixTime provider for testing filing period enforcement.
///
/// Set `MOCK_TIMESTAMP_SECS` in tests to simulate specific calendar dates.
/// Default: 0 (genesis bootstrap -- filing always allowed).
pub struct MockTimeProvider;
impl frame_support::traits::UnixTime for MockTimeProvider {
    fn now() -> core::time::Duration {
        let secs = MOCK_TIMESTAMP_SECS.with(|ts| *ts.borrow());
        core::time::Duration::from_secs(secs)
    }
}

// ---------------------------------------------------------------------------
// Mock ShieldedVaultsBridge (no-op)
// ---------------------------------------------------------------------------

pub struct MockShieldedVaultsBridge;
impl pallet_shielded_vaults::ShieldedVaultsInterface<u64> for MockShieldedVaultsBridge {
    fn unshield_for_tax(
        _nullifier: &[u8; 32],
        _commitment: &[u8; 32],
        _amount: u128,
        _target: &u64,
    ) -> frame_support::dispatch::DispatchResult {
        Ok(())
    }

    fn is_nullifier_spent(_nullifier: &[u8; 32]) -> bool {
        false
    }
}

// ---------------------------------------------------------------------------
// pallet-organization Config
// ---------------------------------------------------------------------------

parameter_types! {
    pub const TaxPeriodBlocks: u32 = 100;
    pub const PenaltyAccrualPeriodBlocks: u32 = 50;
    pub const BasePenaltyAmount: u64 = 1_000;
    pub const AuditorRewardAmount: u64 = 100;
    pub const RegionTreasury: u64 = 999;
    pub const ConfederationTreasury: u64 = 998;
    pub const MaxOrgsPerBlock: u32 = 10;
    pub const MaxNameLength: u32 = 64;
    pub const MaxDirectorsPerOrg: u32 = 20;
    pub const MinTaxAmount: u64 = 20;
    pub const StateTaxRate: Perbill = Perbill::from_percent(10);
    pub const OrgActivationDeposit: u64 = 10;
    pub const MaxFounders: u32 = 20;
}

impl pallet_organization::Config for Test {
    type Currency = Balances;
    type IdentityBridge = MockIdentityBridge;
    type TaxPeriodBlocks = TaxPeriodBlocks;
    type PenaltyAccrualPeriodBlocks = PenaltyAccrualPeriodBlocks;
    type BasePenaltyAmount = BasePenaltyAmount;
    type AuditorRewardAmount = AuditorRewardAmount;
    type RegionTreasuryAccount = RegionTreasury;
    type ConfederationTreasuryAccount = ConfederationTreasury;
    type StateTaxRate = StateTaxRate;
    type MinTaxAmount = MinTaxAmount;
    type TimeProvider = MockTimeProvider;
    type MaxOrgsPerBlock = MaxOrgsPerBlock;
    type MaxNameLength = MaxNameLength;
    type MaxDirectorsPerOrg = MaxDirectorsPerOrg;
    type ShieldedVaultsBridge = MockShieldedVaultsBridge;
    type OrgActivationDeposit = OrgActivationDeposit;
    type MaxFounders = MaxFounders;
    type WeightInfo = ();
}

// ---------------------------------------------------------------------------
// Test externalities
// ---------------------------------------------------------------------------

pub fn new_test_ext() -> sp_io::TestExternalities {
    FROZEN_ACCOUNTS.with(|fa| fa.borrow_mut().clear());
    MOCK_TIMESTAMP_SECS.with(|ts| *ts.borrow_mut() = 0);
    let mut t = frame_system::GenesisConfig::<Test>::default()
        .build_storage()
        .unwrap();

    pallet_balances::GenesisConfig::<Test> {
        balances: vec![
            (1u64, 100_000),  // Alice (Director)
            (2u64, 50_000),   // Bob (Employee)
            (3u64, 50_000),   // Carol (Auditor)
            (10u64, 500_000), // Org bank account
            (998u64, 10_000), // Confederation Treasury
            (999u64, 10_000), // Regional Treasury
        ],
        dev_accounts: None,
    }
    .assimilate_storage(&mut t)
    .unwrap();

    t.into()
}
