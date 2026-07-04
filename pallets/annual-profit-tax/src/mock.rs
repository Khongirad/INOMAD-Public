//! Mock runtime for pallet-annual-profit-tax unit tests.

use frame_support::{construct_runtime, derive_impl, parameter_types, traits::ConstU128};
use sp_runtime::BuildStorage;
use std::cell::RefCell;

pub type AccountId = u64;
pub type Balance = u128;
pub type Block = frame_system::mocking::MockBlock<Test>;

/// 1 ALTAN = 10^12 planck
pub const UNIT: Balance = 1_000_000_000_000;

// ── Controllable mock clock ───────────────────────────────────────────────────

thread_local! {
    /// Mocked Unix time (seconds). Default: January 15, 2026 (on-time window).
    static MOCK_TIME: RefCell<u64> = RefCell::new(JAN1_2026 + 14 * 86_400);
}

pub struct MockUnixTime;
impl frame_support::traits::UnixTime for MockUnixTime {
    fn now() -> core::time::Duration {
        MOCK_TIME.with(|t| core::time::Duration::from_secs(*t.borrow()))
    }
}

/// Set mock clock to a specific Unix timestamp (seconds).
pub fn set_time(secs: u64) {
    MOCK_TIME.with(|t| *t.borrow_mut() = secs);
}

// ── Key timestamps for 2026 ───────────────────────────────────────────────────

/// Jan 1, 2026 00:00:00 UTC
pub const JAN1_2026: u64 = 1_735_689_600;
/// Apr 15, 2026 23:59:59 UTC = Jan 1 + 105 days - 1 second
pub const APR15_2026_END: u64 = JAN1_2026 + 105 * 86_400 - 1;
/// Apr 16, 2026 00:00:00 UTC — day 1 of late period
pub const APR16_2026: u64 = JAN1_2026 + 106 * 86_400;
/// Jul 1, 2026 00:00:00 UTC = 76 days late (Apr16 + 75 full days => day 76)
/// Calculation: Apr has 14 remaining days + May 31 + June 30 = 75 days after Apr16 → Jul 1 = day 76
pub const JUL1_2026: u64 = APR16_2026 + 76 * 86_400;
/// Dec 31, 2026 23:59:59 UTC = 259 days late (last day)
pub const DEC31_2026_END: u64 = JAN1_2026 + 365 * 86_400 - 1;
/// Jan 1, 2027 — after deadline, filing blocked
pub const JAN1_2027: u64 = JAN1_2026 + 365 * 86_400;

// ── Runtime ───────────────────────────────────────────────────────────────────

construct_runtime!(
    pub enum Test {
        System: frame_system,
        Balances: pallet_balances,
        AnnualProfitTax: crate,
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

parameter_types! {
    /// Standard tax: 10% = 100 per-mille
    pub const StandardTaxRatePermill: u32 = 100;
    /// Large-family tax: 5% = 50 per-mille
    pub const LargeFamilyTaxRatePermill: u32 = 50;
}

impl crate::Config for Test {
    type Currency = Balances;
    type UnixTime = MockUnixTime;
    type StandardTaxRatePermill = StandardTaxRatePermill;
    type LargeFamilyTaxRatePermill = LargeFamilyTaxRatePermill;
}

/// Treasury and test accounts
pub const REGIONAL_TREASURY: AccountId = 700;      // 70%
pub const CONFEDERATION_TREASURY: AccountId = 800; // 30%
pub const ORG: AccountId = 1;                      // Organization filing tax
pub const FAMILY: AccountId = 2;                   // Large-family citizen

pub fn new_test_ext() -> sp_io::TestExternalities {
    let mut t = frame_system::GenesisConfig::<Test>::default()
        .build_storage()
        .unwrap();

    pallet_balances::GenesisConfig::<Test> {
        balances: vec![
            (ORG, 100_000_000 * UNIT),
            (FAMILY, 100_000_000 * UNIT),
            (REGIONAL_TREASURY, 1_000_000_000),
            (CONFEDERATION_TREASURY, 1_000_000_000),
        ],
        dev_accounts: None,
    }
    .assimilate_storage(&mut t)
    .unwrap();

    crate::GenesisConfig::<Test> {
        regional_treasury: Some(REGIONAL_TREASURY),
        confederation_treasury: Some(CONFEDERATION_TREASURY),
    }
    .assimilate_storage(&mut t)
    .unwrap();

    // Default: Jan 15, 2026 (on-time)
    set_time(JAN1_2026 + 14 * 86_400);

    sp_io::TestExternalities::new(t)
}
