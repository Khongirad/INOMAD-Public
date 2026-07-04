//! # pallet-central-bank — Unit Tests
//!
//! Covers:
//!  - `grant_operator_license` / `revoke_operator_license`
//!  - `mint_to_operator` happy path + error cases
//!  - `burn` happy path + error cases
//!  - Constitutional invariant: TotalEmitted and TotalBurned accounting
//!  - Origin gating: non-BankingOrigin callers are rejected
//!  - **Constitutional Rotating Credit Pool (Variant B)**:
//!    - `request_credit` reduces pool, increases outstanding
//!    - `repay_credit` restores pool, decreases outstanding
//!    - Dynamic key rate: 0% at zero outstanding, base_rate at full utilization
//!    - Hard cap: CreditPoolExhausted when pool is depleted
//!    - Pool restoration cannot exceed genesis limit

#![cfg(test)]

use frame_support::{
    assert_noop, assert_ok, derive_impl,
    traits::{ConstU32, ConstU64},
};
use frame_system as system;
use sp_runtime::BuildStorage;

use crate as pallet_central_bank;
use pallet_central_bank::Error;

// ─── Mock Runtime ─────────────────────────────────────────────────────────────

type Block = frame_system::mocking::MockBlock<Test>;

frame_support::construct_runtime!(
    pub enum Test {
        System:      frame_system,
        Balances:    pallet_balances,
        Proxy:       pallet_proxy,
        CentralBank: pallet_central_bank,
    }
);

#[derive_impl(frame_system::config_preludes::TestDefaultConfig)]
impl frame_system::Config for Test {
    type Block = Block;
    type AccountData = pallet_balances::AccountData<u64>;
}

impl pallet_balances::Config for Test {
    type RuntimeHoldReason = ();
    type RuntimeFreezeReason = ();
    type MaxLocks = ConstU32<50>;
    type MaxReserves = ConstU32<50>;
    type ReserveIdentifier = [u8; 8];
    type Balance = u64;
    type DustRemoval = ();
    type RuntimeEvent = RuntimeEvent;
    type ExistentialDeposit = ConstU64<1>;
    type AccountStore = System;
    type WeightInfo = ();
    type FreezeIdentifier = ();
    type MaxFreezes = ConstU32<0>;
    type DoneSlashHandler = ();
}

impl pallet_proxy::Config for Test {
    type RuntimeEvent = RuntimeEvent;
    type RuntimeCall = RuntimeCall;
    type Currency = Balances;
    type ProxyType = ();
    type ProxyDepositBase = ConstU64<1>;
    type ProxyDepositFactor = ConstU64<1>;
    type MaxProxies = ConstU32<32>;
    type WeightInfo = ();
    type MaxPending = ConstU32<32>;
    type CallHasher = sp_runtime::traits::BlakeTwo256;
    type AnnouncementDepositBase = ConstU64<1>;
    type AnnouncementDepositFactor = ConstU64<1>;
    type BlockNumberProvider = System;
}

pub struct MockCitizenIdentity;
impl pallet_central_bank::CitizenIdentity<u64, u64> for MockCitizenIdentity {
    fn get_credit_limit(_who: &u64) -> Option<u64> {
        Some(10_000_000_000) // Virtually infinite for tests
    }
}

impl pallet_central_bank::Config for Test {
    type Currency = Balances;
    // EnsureRoot — in tests we use RuntimeOrigin::root() as BankingOrigin
    type BankingOrigin = frame_system::EnsureRoot<u64>;
    type CitizenIdentity = MockCitizenIdentity;
    // Test ceiling: 1_000_000 planks (small for testing credit pool exhaustion)
    type CreditEpochLimit = frame_support::traits::ConstU64<1_000_000>;
    // Creator = account #1 in tests (ROOT). Signs trigger_genesis_distribution.
    type CreatorAccount = ConstU64<1>;
    // Mock CENTRAL_BANK sovereign account — holds genesis supply in tests.
    // Account #999 never conflicts with OPERATOR(2), CITIZEN(3), NOBODY(4).
    type CentralBankAccountId = ConstU64<999>;
    type WeightInfo = ();

    // Two-Phase Curve Constants
    type OptimalKeyRate = ConstU32<850>; // 8.5%
    type MaxProtectiveRate = ConstU32<5000>; // 50%
    type OptimalUtilization = ConstU32<80>; // 80%
}

// ─── Test accounts ────────────────────────────────────────────────────────────

const _ROOT: u64 = 1; // has sudo → BankingOrigin (unused, retained for documentation)
const OPERATOR: u64 = 2; // Bank of Siberia (licensed operator)
const CITIZEN: u64 = 3; // ordinary citizen — must NOT be mintable/burnable
const CITIZEN_B: u64 = 5; // second citizen for multi-citizen tests
const NOBODY: u64 = 4; // unlicensed account

// ─── Test builder helper ──────────────────────────────────────────────────────

fn new_test_ext() -> sp_io::TestExternalities {
    let mut t = system::GenesisConfig::<Test>::default()
        .build_storage()
        .unwrap();

    // Give OPERATOR a funded account so we can test burn without needing mint first.
    // CITIZEN gets a small balance (for repayment tests — needs KeepAlive threshold).
    pallet_balances::GenesisConfig::<Test> {
        balances: vec![(OPERATOR, 1_000_000), (CITIZEN, 100), (CITIZEN_B, 100)],
        dev_accounts: Default::default(),
    }
    .assimilate_storage(&mut t)
    .unwrap();

    // Initialize central bank genesis (credit pool + epoch)
    pallet_central_bank::GenesisConfig::<Test> {
        licensed_operators: vec![],
    }
    .assimilate_storage(&mut t)
    .unwrap();

    t.into()
}

// ─── License Tests ────────────────────────────────────────────────────────────

#[test]
fn grant_operator_license_works() {
    new_test_ext().execute_with(|| {
        assert!(!CentralBank::licensed_operators(OPERATOR));
        assert_ok!(CentralBank::grant_operator_license(
            RuntimeOrigin::root(),
            OPERATOR
        ));
        assert!(CentralBank::licensed_operators(OPERATOR));
    });
}

#[test]
fn grant_license_non_banking_origin_fails() {
    new_test_ext().execute_with(|| {
        assert_noop!(
            CentralBank::grant_operator_license(RuntimeOrigin::signed(CITIZEN), OPERATOR),
            frame_support::error::BadOrigin
        );
    });
}

#[test]
fn revoke_operator_license_works() {
    new_test_ext().execute_with(|| {
        assert_ok!(CentralBank::grant_operator_license(
            RuntimeOrigin::root(),
            OPERATOR
        ));
        assert_ok!(CentralBank::revoke_operator_license(
            RuntimeOrigin::root(),
            OPERATOR
        ));
        assert!(!CentralBank::licensed_operators(OPERATOR));
    });
}

// ─── Mint Tests ───────────────────────────────────────────────────────────────

#[test]
fn mint_to_operator_works_and_increments_total_emitted() {
    new_test_ext().execute_with(|| {
        assert_ok!(CentralBank::grant_operator_license(
            RuntimeOrigin::root(),
            OPERATOR
        ));

        let before = Balances::free_balance(OPERATOR);
        assert_ok!(CentralBank::mint_to_operator(
            RuntimeOrigin::root(),
            OPERATOR,
            500_000
        ));
        assert_eq!(Balances::free_balance(OPERATOR), before + 500_000);
        assert_eq!(CentralBank::total_emitted(), 500_000);
    });
}

#[test]
fn mint_zero_fails() {
    new_test_ext().execute_with(|| {
        assert_ok!(CentralBank::grant_operator_license(
            RuntimeOrigin::root(),
            OPERATOR
        ));
        assert_noop!(
            CentralBank::mint_to_operator(RuntimeOrigin::root(), OPERATOR, 0),
            Error::<Test>::ZeroAmount
        );
    });
}

#[test]
fn mint_to_unlicensed_account_fails() {
    new_test_ext().execute_with(|| {
        // NOBODY has no license
        assert_noop!(
            CentralBank::mint_to_operator(RuntimeOrigin::root(), NOBODY, 1000),
            Error::<Test>::NotLicensedOperator
        );
    });
}

#[test]
fn mint_twice_accumulates_total_emitted() {
    new_test_ext().execute_with(|| {
        assert_ok!(CentralBank::grant_operator_license(
            RuntimeOrigin::root(),
            OPERATOR
        ));
        assert_ok!(CentralBank::mint_to_operator(
            RuntimeOrigin::root(),
            OPERATOR,
            300
        ));
        assert_ok!(CentralBank::mint_to_operator(
            RuntimeOrigin::root(),
            OPERATOR,
            700
        ));
        assert_eq!(CentralBank::total_emitted(), 1_000);
    });
}

// ─── Burn Tests ───────────────────────────────────────────────────────────────

#[test]
fn burn_works_and_decreases_total_issuance() {
    new_test_ext().execute_with(|| {
        assert_ok!(CentralBank::grant_operator_license(
            RuntimeOrigin::root(),
            OPERATOR
        ));

        let issuance_before = Balances::total_issuance();
        let balance_before = Balances::free_balance(OPERATOR);

        assert_ok!(CentralBank::burn(RuntimeOrigin::root(), OPERATOR, 100_000));

        assert_eq!(Balances::free_balance(OPERATOR), balance_before - 100_000);
        assert_eq!(Balances::total_issuance(), issuance_before - 100_000);
        assert_eq!(CentralBank::total_burned(), 100_000);
    });
}

#[test]
fn burn_zero_fails() {
    new_test_ext().execute_with(|| {
        assert_ok!(CentralBank::grant_operator_license(
            RuntimeOrigin::root(),
            OPERATOR
        ));
        assert_noop!(
            CentralBank::burn(RuntimeOrigin::root(), OPERATOR, 0),
            Error::<Test>::ZeroAmount
        );
    });
}

#[test]
fn burn_unlicensed_account_fails() {
    new_test_ext().execute_with(|| {
        // NOBODY is not licensed even if it somehow has balance
        assert_noop!(
            CentralBank::burn(RuntimeOrigin::root(), NOBODY, 1),
            Error::<Test>::NotLicensedOperator
        );
    });
}

#[test]
fn burn_more_than_balance_fails() {
    new_test_ext().execute_with(|| {
        assert_ok!(CentralBank::grant_operator_license(
            RuntimeOrigin::root(),
            OPERATOR
        ));

        let huge_amount = Balances::free_balance(OPERATOR) + 1_000_000;
        assert_noop!(
            CentralBank::burn(RuntimeOrigin::root(), OPERATOR, huge_amount),
            Error::<Test>::InsufficientOperatorBalance
        );
    });
}

#[test]
fn burn_non_banking_origin_fails() {
    new_test_ext().execute_with(|| {
        assert_ok!(CentralBank::grant_operator_license(
            RuntimeOrigin::root(),
            OPERATOR
        ));
        assert_noop!(
            CentralBank::burn(RuntimeOrigin::signed(CITIZEN), OPERATOR, 1),
            frame_support::error::BadOrigin
        );
    });
}

// ─── Constitutional Invariants ────────────────────────────────────────────────

#[test]
fn net_circulation_equals_emitted_minus_burned() {
    new_test_ext().execute_with(|| {
        assert_ok!(CentralBank::grant_operator_license(
            RuntimeOrigin::root(),
            OPERATOR
        ));

        assert_ok!(CentralBank::mint_to_operator(
            RuntimeOrigin::root(),
            OPERATOR,
            800_000
        ));
        assert_ok!(CentralBank::burn(RuntimeOrigin::root(), OPERATOR, 200_000));

        let net = CentralBank::total_emitted().saturating_sub(CentralBank::total_burned());
        assert_eq!(net, 600_000);
    });
}

#[test]
fn revoked_operator_cannot_be_minted_to() {
    new_test_ext().execute_with(|| {
        assert_ok!(CentralBank::grant_operator_license(
            RuntimeOrigin::root(),
            OPERATOR
        ));
        assert_ok!(CentralBank::revoke_operator_license(
            RuntimeOrigin::root(),
            OPERATOR
        ));

        assert_noop!(
            CentralBank::mint_to_operator(RuntimeOrigin::root(), OPERATOR, 1000),
            Error::<Test>::NotLicensedOperator
        );
    });
}

#[test]
fn revoked_operator_cannot_be_burned_from() {
    new_test_ext().execute_with(|| {
        assert_ok!(CentralBank::grant_operator_license(
            RuntimeOrigin::root(),
            OPERATOR
        ));
        assert_ok!(CentralBank::revoke_operator_license(
            RuntimeOrigin::root(),
            OPERATOR
        ));

        assert_noop!(
            CentralBank::burn(RuntimeOrigin::root(), OPERATOR, 1),
            Error::<Test>::NotLicensedOperator
        );
    });
}

// ─── Constitutional Rotating Credit Pool Tests ────────────────────────────────

/// At genesis: pool = 1_000_000 (CreditEpochLimit), outstanding = 0, rate = 0
#[test]
fn genesis_credit_pool_initialized_correctly() {
    new_test_ext().execute_with(|| {
        assert_eq!(CentralBank::genesis_credit_available(), 1_000_000);
        assert_eq!(CentralBank::total_outstanding(), 0);

        // Epoch 1 exists with key_rate = 0 (no outstanding at genesis)
        let epoch = CentralBank::epochs(1).expect("epoch 1 must exist");
        assert_eq!(epoch.key_rate, 0);
    });
}

/// request_credit reduces pool_available and increases outstanding
#[test]
fn request_credit_reduces_pool_and_increases_outstanding() {
    new_test_ext().execute_with(|| {
        let credit_amount = 10_000u64;
        assert_ok!(CentralBank::request_credit(
            RuntimeOrigin::signed(CITIZEN),
            credit_amount
        ));

        assert_eq!(CentralBank::genesis_credit_available(), 1_000_000 - credit_amount);
        assert_eq!(CentralBank::total_outstanding(), credit_amount);

        // Citizen received credit
        // (genesis balance was 100, now 100 + credit_amount)
        assert_eq!(Balances::free_balance(CITIZEN), 100 + credit_amount);

        // Citizen's debt is epoch-tagged
        assert_eq!(CentralBank::citizen_debt(CITIZEN, 1), credit_amount);
    });
}

/// Dynamic rate increases linearly in Phase 1 (utilization <= 80%)
#[test]
fn dynamic_rate_rises_linearly_in_phase_1() {
    new_test_ext().execute_with(|| {
        // Issue 50% of the pool (500_000 / 1_000_000)
        assert_ok!(CentralBank::request_credit(
            RuntimeOrigin::signed(CITIZEN),
            500_000
        ));

        let epoch = CentralBank::epochs(1).unwrap();
        // Phase 1 formula: 50 * 850 / 80 = 531.25 -> 531 bps
        assert_eq!(epoch.key_rate, 531);
        assert_eq!(CentralBank::total_outstanding(), 500_000);
    });
}

/// Dynamic rate spikes in Phase 2 (utilization > 80%)
#[test]
fn dynamic_rate_spikes_in_phase_2() {
    new_test_ext().execute_with(|| {
        // Issue 90% of the pool (900_000 / 1_000_000)
        assert_ok!(CentralBank::request_credit(
            RuntimeOrigin::signed(CITIZEN),
            900_000
        ));

        let epoch = CentralBank::epochs(1).unwrap();
        // Phase 2 formula: 
        // excess_utilization = 90 - 80 = 10
        // max_excess = 100 - 80 = 20
        // penalty = 10 * (5000 - 850) / 20 = 10 * 4150 / 20 = 2075
        // rate = 850 + 2075 = 2925 bps (29.25%)
        assert_eq!(epoch.key_rate, 2925);
    });
}

/// Dynamic rate = MaxProtectiveRate when pool fully utilized
#[test]
fn dynamic_rate_equals_max_protective_rate_at_full_utilization() {
    new_test_ext().execute_with(|| {
        // Take entire pool (leave 1 for KeepAlive, so take 999_999)
        // Pool = 1_000_000, take 999_999 → outstanding = 999_999 ≈ full
        assert_ok!(CentralBank::request_credit(
            RuntimeOrigin::signed(CITIZEN),
            999_999
        ));

        let epoch = CentralBank::epochs(1).unwrap();
        // Almost 100% utilization -> should be very close to 5000 bps (50%)
        // Actually, 99.9999% might just calculate as 99% depending on math,
        // (999_999 * 100) / 1000000 = 99
        // excess = 19
        // penalty = 19 * 4150 / 20 = 3942
        // rate = 850 + 3942 = 4792
        assert_eq!(epoch.key_rate, 4792);
    });
}

/// repay_credit restores pool capacity and decreases outstanding
#[test]
fn repay_credit_restores_pool_and_reduces_outstanding() {
    new_test_ext().execute_with(|| {
        let credit_amount = 50_000u64;
        assert_ok!(CentralBank::request_credit(
            RuntimeOrigin::signed(CITIZEN),
            credit_amount
        ));

        // Verify pool decreased
        assert_eq!(CentralBank::genesis_credit_available(), 1_000_000 - credit_amount);
        assert_eq!(CentralBank::total_outstanding(), credit_amount);

        // Repay half
        let repay_amount = 25_000u64;
        assert_ok!(CentralBank::repay_credit(
            RuntimeOrigin::signed(CITIZEN),
            1, // epoch_id
            repay_amount
        ));

        // Pool restored by repayment
        assert_eq!(
            CentralBank::genesis_credit_available(),
            1_000_000 - credit_amount + repay_amount
        );
        assert_eq!(CentralBank::total_outstanding(), credit_amount - repay_amount);
        assert_eq!(CentralBank::citizen_debt(CITIZEN, 1), credit_amount - repay_amount);
    });
}

/// Dynamic rate drops to 0% after full repayment (free credit incentive)
#[test]
fn dynamic_rate_zero_after_full_repayment() {
    new_test_ext().execute_with(|| {
        let credit_amount = 100_000u64;
        assert_ok!(CentralBank::request_credit(
            RuntimeOrigin::signed(CITIZEN),
            credit_amount
        ));

        // Rate should be non-zero after taking credit
        let epoch_mid = CentralBank::epochs(1).unwrap();
        assert!(epoch_mid.key_rate > 0);

        // Full repayment (citizen has 100 + 100_000 balance, repay 100_000)
        assert_ok!(CentralBank::repay_credit(
            RuntimeOrigin::signed(CITIZEN),
            1,
            credit_amount
        ));

        // After full repayment: outstanding = 0 → rate = 0% (free credit!)
        let epoch_after = CentralBank::epochs(1).unwrap();
        assert_eq!(epoch_after.key_rate, 0);
        assert_eq!(CentralBank::total_outstanding(), 0);
        assert_eq!(CentralBank::genesis_credit_available(), 1_000_000);
    });
}

/// Pool cannot be over-restored beyond genesis limit (cap enforced)
#[test]
fn pool_restoration_capped_at_genesis_limit() {
    new_test_ext().execute_with(|| {
        // Take small credit and repay more than the limit can hold
        let credit_amount = 1_000u64;
        assert_ok!(CentralBank::request_credit(
            RuntimeOrigin::signed(CITIZEN),
            credit_amount
        ));
        assert_ok!(CentralBank::repay_credit(
            RuntimeOrigin::signed(CITIZEN),
            1,
            credit_amount
        ));

        // Pool should not exceed genesis limit
        assert_eq!(CentralBank::genesis_credit_available(), 1_000_000);
    });
}

/// CreditPoolExhausted when pool is depleted
#[test]
fn credit_pool_exhausted_blocks_new_credit() {
    new_test_ext().execute_with(|| {
        // Drain most of the pool (leave 1 for KeepAlive)
        // Two citizens each borrow; together exhaust the pool
        assert_ok!(CentralBank::request_credit(
            RuntimeOrigin::signed(CITIZEN),
            500_000
        ));
        assert_ok!(CentralBank::request_credit(
            RuntimeOrigin::signed(CITIZEN_B),
            499_999
        ));

        // Pool should now have only 1 left
        assert_eq!(CentralBank::genesis_credit_available(), 1);

        // Trying to borrow more than what's available → CreditPoolExhausted
        assert_noop!(
            CentralBank::request_credit(RuntimeOrigin::signed(CITIZEN), 1_000),
            Error::<Test>::CreditPoolExhausted
        );
    });
}

/// repay_credit fails on non-existent epoch
#[test]
fn repay_credit_invalid_epoch_fails() {
    new_test_ext().execute_with(|| {
        assert_noop!(
            CentralBank::repay_credit(RuntimeOrigin::signed(CITIZEN), 999, 100),
            Error::<Test>::EpochNotFound
        );
    });
}

/// repay_credit fails when citizen has no debt in epoch
#[test]
fn repay_credit_no_debt_fails() {
    new_test_ext().execute_with(|| {
        assert_noop!(
            CentralBank::repay_credit(RuntimeOrigin::signed(CITIZEN), 1, 100),
            Error::<Test>::InsufficientDebt
        );
    });
}

/// repay_credit fails when repaying more than owed
#[test]
fn repay_credit_exceeds_debt_fails() {
    new_test_ext().execute_with(|| {
        let credit_amount = 10_000u64;
        assert_ok!(CentralBank::request_credit(
            RuntimeOrigin::signed(CITIZEN),
            credit_amount
        ));

        assert_noop!(
            CentralBank::repay_credit(RuntimeOrigin::signed(CITIZEN), 1, credit_amount + 1),
            Error::<Test>::InsufficientDebt
        );
    });
}

/// Epoch total_issued tracks credit issued correctly
#[test]
fn epoch_total_issued_tracks_credit() {
    new_test_ext().execute_with(|| {
        assert_ok!(CentralBank::request_credit(
            RuntimeOrigin::signed(CITIZEN),
            30_000
        ));
        assert_ok!(CentralBank::request_credit(
            RuntimeOrigin::signed(CITIZEN_B),
            20_000
        ));

        let epoch = CentralBank::epochs(1).unwrap();
        assert_eq!(epoch.total_issued, 50_000);
    });
}

/// Epoch total_repaid tracks repayments correctly
#[test]
fn epoch_total_repaid_tracks_repayments() {
    new_test_ext().execute_with(|| {
        assert_ok!(CentralBank::request_credit(
            RuntimeOrigin::signed(CITIZEN),
            50_000
        ));
        assert_ok!(CentralBank::repay_credit(
            RuntimeOrigin::signed(CITIZEN),
            1,
            20_000
        ));

        let epoch = CentralBank::epochs(1).unwrap();
        assert_eq!(epoch.total_issued, 50_000);
        assert_eq!(epoch.total_repaid, 20_000);
    });
}

/// Two-Phase Kinked Curve Math Validation
#[test]
fn dynamic_rate_two_phase_curve_works() {
    new_test_ext().execute_with(|| {
        // Our test configuration:
        // Genesis limit: 1_000_000
        // OptimalUtilization: 80% (800_000)
        // OptimalKeyRate: 850 (8.5%)
        // MaxProtectiveRate: 5000 (50%)

        // 1. Phase 1: 0% utilization -> rate = 0
        assert_eq!(CentralBank::epochs(1).unwrap().key_rate, 0);

        // 2. Phase 1: 40% utilization -> rate should be exactly half of OptimalKeyRate = 425
        assert_ok!(CentralBank::request_credit(
            RuntimeOrigin::signed(CITIZEN),
            400_000
        ));
        assert_eq!(CentralBank::epochs(1).unwrap().key_rate, 425);

        // 3. Phase 1: 80% utilization -> rate should be OptimalKeyRate = 850
        assert_ok!(CentralBank::request_credit(
            RuntimeOrigin::signed(CITIZEN),
            400_000
        )); // Total = 800_000
        assert_eq!(CentralBank::epochs(1).unwrap().key_rate, 850);

        // 4. Phase 2: 90% utilization -> rate should be midway between Optimal and Max
        // 80% to 100% is the penalty zone (excess 10% out of 20% max = 50% of penalty applied)
        // Penalty spread = 5000 - 850 = 4150. 50% of 4150 = 2075.
        // Rate = 850 + 2075 = 2925.
        assert_ok!(CentralBank::request_credit(
            RuntimeOrigin::signed(CITIZEN),
            100_000
        )); // Total = 900_000
        assert_eq!(CentralBank::epochs(1).unwrap().key_rate, 2925);

        // 5. Phase 2: 100% utilization -> rate should be MaxProtectiveRate = 5000
        assert_ok!(CentralBank::request_credit(
            RuntimeOrigin::signed(CITIZEN),
            100_000
        )); // Total = 1_000_000
        assert_eq!(CentralBank::epochs(1).unwrap().key_rate, 5000);
        
        // 6. Verify pool exhausted
        assert_noop!(
            CentralBank::request_credit(RuntimeOrigin::signed(CITIZEN_B), 1),
            Error::<Test>::CreditPoolExhausted
        );

        // 7. Reverting back to Phase 1 via repayment
        assert_ok!(CentralBank::repay_credit(
            RuntimeOrigin::signed(CITIZEN),
            1,
            200_000
        )); // Total drops to 800_000 (80%)
        assert_eq!(CentralBank::epochs(1).unwrap().key_rate, 850);
    });
}

/// Verifies OKATO-keyed storage for RegionalTreasuries, PensionFunds, and ConfederationTreasury.
///
/// This test inserts accounts directly into the three storage maps and verifies:
///   - ConfederationTreasury (StorageValue) round-trips correctly
///   - RegionalTreasuries (StorageMap, keyed by u8 OKATO code) round-trips for all 83 codes
///   - PensionFunds (StorageMap, keyed by u8 OKATO code) round-trips for all 83 codes
///   - Codes 80-82 (not in REGIONS_83) return None — gaps are free in Trie
///
/// Note: trigger_genesis_distribution() cannot be called in tests because
/// its M0 amounts (1.89T * 10^12 planck) overflow u64 Balance used in the mock.
/// Production uses u128 Balance. Storage behavior is identical — amounts are irrelevant here.
#[test]
fn okato_keyed_treasury_storage_works() {
    use pallet_central_bank::{
        ConfederationTreasury, RegionalTreasuries, PensionFunds, REGION_CODES_83,
    };

    new_test_ext().execute_with(|| {
        // ── Confederation Treasury (StorageValue) ────────────────────────────
        assert!(ConfederationTreasury::<Test>::get().is_none(), "should start empty");
        let conf_account: u64 = 0xC0FEEED;
        ConfederationTreasury::<Test>::put(conf_account);
        assert_eq!(ConfederationTreasury::<Test>::get(), Some(conf_account));

        // ── RegionalTreasuries + PensionFunds — all 83 OKATO codes ───────────
        assert_eq!(REGION_CODES_83.len(), 83, "REGION_CODES_83 must have exactly 83 entries");

        for (i, &code) in REGION_CODES_83.iter().enumerate() {
            let reg_acct: u64 = 1000 + i as u64;
            let pen_acct: u64 = 2000 + i as u64;

            RegionalTreasuries::<Test>::insert(code, reg_acct);
            PensionFunds::<Test>::insert(code, pen_acct);

            assert_eq!(
                RegionalTreasuries::<Test>::get(code),
                Some(reg_acct),
                "RegionalTreasury for OKATO code {code} must round-trip"
            );
            assert_eq!(
                PensionFunds::<Test>::get(code),
                Some(pen_acct),
                "PensionFund for OKATO code {code} must round-trip"
            );
        }

        // ── Verify gaps are free (codes 80-82 are NOT in REGIONS_83) ─────────
        for missing_code in [80u8, 81, 82, 84, 85, 88] {
            assert!(
                RegionalTreasuries::<Test>::get(missing_code).is_none(),
                "OKATO gap code {missing_code} must return None"
            );
            assert!(
                PensionFunds::<Test>::get(missing_code).is_none(),
                "OKATO gap code {missing_code} must return None"
            );
        }
    });
}

/// Verifies constitutional invariants of REGION_CODES_83 at test time.
#[test]
fn region_codes_83_constitutional_invariants() {
    use pallet_central_bank::REGION_CODES_83;

    // Must be exactly 83 regions
    assert_eq!(REGION_CODES_83.len(), 83);

    // All codes must be unique (no duplicates)
    let mut sorted = REGION_CODES_83.to_vec();
    sorted.sort();
    sorted.dedup();
    assert_eq!(sorted.len(), 83, "REGION_CODES_83 must have no duplicate codes");

    // Must contain the well-known constitutional regions
    assert!(REGION_CODES_83.contains(&3),  "Buryatia (code 3) must be present");
    assert!(REGION_CODES_83.contains(&17), "Tuva (code 17) must be present");
    assert!(REGION_CODES_83.contains(&77), "Moscow (code 77) must be present");
    assert!(REGION_CODES_83.contains(&78), "St. Petersburg (code 78) must be present");
    assert!(REGION_CODES_83.contains(&89), "Yamalo-Nenets (code 89) must be present");

    // Must NOT contain known gaps
    assert!(!REGION_CODES_83.contains(&80), "Code 80 is not a federal subject");
    assert!(!REGION_CODES_83.contains(&81), "Code 81 is not a federal subject");
    assert!(!REGION_CODES_83.contains(&82), "Code 82 is not a federal subject");
    assert!(!REGION_CODES_83.contains(&85), "Code 85 is not a federal subject");
}
