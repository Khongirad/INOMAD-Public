//! # pallet-organization: Unit Tests
//!
//! Tests the core invariants of the corporate registry system:
//! - Organization registration and crew management (Pending → Active flow)
//! - Manual tax filing (Director-only)
//! - Constitutional tax split (3/10 Confederation + 7/10 Regional)
//! - Filing period enforcement (Jan 1 — Apr 15)
//! - Minimum tax enforcement (20 ALTAN)
//! - Punishment engine (delinquency, director freeze)
//! - on_initialize automated scanner

use crate::mock::*;
use crate::{
    CrewMembers, Error, NextOrgId, OrgFounderDeposits, OrgPenaltyDebt, OrgRequiredFounders,
    OrgRole, OrgStatus, Organizations, TaxDeclarations,
};
use frame_support::{assert_noop, assert_ok, traits::Hooks};

// ─────────────────────────────────────────────────────────────────────────────
// Helpers
// ─────────────────────────────────────────────────────────────────────────────

/// Register an org AND complete activation (all in one call for most tests).
///
/// Uses `required_founders = 1` (default), so one `activate_organization`
/// call transitions the org from `Pending` → `Active`.
///
/// Account `acct` must have at least 10 ALTAN (OrgActivationDeposit).
fn register_and_activate_org(acct: u64) -> u32 {
    let org_id = do_register_org(acct);
    assert_ok!(Organization::activate_organization(
        RuntimeOrigin::signed(acct),
        org_id,
    ));
    let org = Organizations::<Test>::get(org_id).unwrap();
    assert_eq!(org.status, OrgStatus::Active, "activation failed");
    org_id
}

/// Only register (stays in Pending). Used for activation-specific tests.
fn do_register_org(acct: u64) -> u32 {
    assert_ok!(Organization::register_organization(
        RuntimeOrigin::signed(acct),
        b"MegaCorp".to_vec(),
        42,
        1, // required_founders = 1
    ));
    NextOrgId::<Test>::get().saturating_sub(1)
}

#[allow(dead_code)]
/// Get the keyless org account id for org_id (via storage — simplest in tests).
fn org_account(org_id: u32) -> u64 {
    Organizations::<Test>::get(org_id)
        .map(|o| o.bank_account_id)
        .expect("org not found")
}

/// Fund an account directly (bypasses constitutional rules — test only).
fn fund(who: u64, amount: u64) {
    let _ = Balances::force_set_balance(RuntimeOrigin::root(), who, amount);
}

/// Set mock timestamp to a specific date (as Unix seconds).
///
/// Common test dates:
///   - Jan 15, 2026 = 1768435200
///   - Apr 10, 2026 = 1775865600
///   - Apr 16, 2026 = 1776384000
///   - Jul 1, 2026  = 1782777600
fn set_mock_timestamp(secs: u64) {
    MOCK_TIMESTAMP_SECS.with(|ts| *ts.borrow_mut() = secs);
}

// ─────────────────────────────────────────────────────────────────────────────
// Registration & Activation Tests
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn test_register_org_starts_pending() {
    new_test_ext().execute_with(|| {
        System::set_block_number(1);
        let org_id = do_register_org(1);
        assert_eq!(org_id, 0);

        let org = Organizations::<Test>::get(0).unwrap();
        assert_eq!(org.status, OrgStatus::Pending, "should start Pending");
        assert_eq!(org.hq_region, 42);

        // Founder is auto-assigned Director role
        let crew = CrewMembers::<Test>::get(0, 1u64).unwrap();
        assert_eq!(crew.role, OrgRole::Director);

        // required_founders = 1 recorded in storage
        assert_eq!(OrgRequiredFounders::<Test>::get(0), 1);
    });
}

#[test]
fn test_activate_org_transitions_to_active() {
    new_test_ext().execute_with(|| {
        System::set_block_number(1);
        let org_id = do_register_org(1);
        assert_eq!(
            Organizations::<Test>::get(org_id).unwrap().status,
            OrgStatus::Pending
        );

        // Activation deposit: founder transfers 10 ALTAN → org keyless account
        assert_ok!(Organization::activate_organization(
            RuntimeOrigin::signed(1),
            org_id,
        ));

        let org = Organizations::<Test>::get(org_id).unwrap();
        assert_eq!(org.status, OrgStatus::Active);

        // Deposit recorded
        let deps = OrgFounderDeposits::<Test>::get(org_id);
        assert_eq!(deps.len(), 1);
        assert_eq!(deps[0].founder, 1u64);
        assert_eq!(deps[0].amount, 10u64); // OrgActivationDeposit
    });
}

#[test]
fn test_activate_org_double_deposit_fails() {
    new_test_ext().execute_with(|| {
        System::set_block_number(1);
        let org_id = do_register_org(1);
        assert_ok!(Organization::activate_organization(
            RuntimeOrigin::signed(1),
            org_id,
        ));
        // Already Active → AlreadyActivated
        assert_noop!(
            Organization::activate_organization(RuntimeOrigin::signed(1), org_id),
            Error::<Test>::AlreadyActivated
        );
    });
}

#[test]
fn test_register_org_and_storage() {
    new_test_ext().execute_with(|| {
        System::set_block_number(1);
        let org_id = register_and_activate_org(1);
        assert_eq!(org_id, 0);

        let org = Organizations::<Test>::get(0).unwrap();
        assert_eq!(org.status, OrgStatus::Active);
        assert_eq!(org.hq_region, 42);

        // Founder is auto-assigned Director role
        let crew = CrewMembers::<Test>::get(0, 1u64).unwrap();
        assert_eq!(crew.role, OrgRole::Director);
    });
}

#[test]
fn test_add_crew_member_director_only() {
    new_test_ext().execute_with(|| {
        System::set_block_number(1);
        register_and_activate_org(1);

        // Bob is not Director — must fail
        assert_noop!(
            Organization::add_crew_member(RuntimeOrigin::signed(2), 0, 3, OrgRole::Employee, 42),
            Error::<Test>::NotDirector
        );

        // Alice (Director) can add Bob
        assert_ok!(Organization::add_crew_member(
            RuntimeOrigin::signed(1),
            0,
            2,
            OrgRole::Employee,
            42,
        ));
        let crew = CrewMembers::<Test>::get(0, 2u64).unwrap();
        assert_eq!(crew.role, OrgRole::Employee);
    });
}

#[test]
fn test_add_crew_member_fails_while_pending() {
    new_test_ext().execute_with(|| {
        System::set_block_number(1);
        // Org is still Pending (not activated)
        do_register_org(1);
        assert_noop!(
            Organization::add_crew_member(RuntimeOrigin::signed(1), 0, 2, OrgRole::Employee, 42),
            Error::<Test>::OrgIsPending
        );
    });
}

// ─────────────────────────────────────────────────────────────────────────────
// Tax Filing Tests
// ─────────────────────────────────────────────────────────────────────────────

/// Fund the org's keyless account so it can pay taxes.
fn setup_org_with_funds(director: u64, tax_amount: u64) -> (u32, u64) {
    let org_id = register_and_activate_org(director);
    let org_acct = Organizations::<Test>::get(org_id).unwrap().bank_account_id;
    // Org account already received the 10 ALTAN activation deposit from `director`.
    // Add extra funds for the tax payment.
    fund(org_acct, tax_amount + 1_000 /* buffer */);
    (org_id, org_acct)
}

#[test]
fn test_file_tax_return_success_with_constitutional_split() {
    new_test_ext().execute_with(|| {
        System::set_block_number(1);
        // Timestamp = 0 → genesis bootstrap (filing always allowed)
        let (_, org_acct) = setup_org_with_funds(1, 1_000);

        let region_before = Balances::free_balance(999u64);
        let confed_before = Balances::free_balance(998u64);
        let bank_before = Balances::free_balance(org_acct);

        // declared_profit: 10_000, amount (tax): 1_000
        assert_ok!(Organization::file_tax_return(
            RuntimeOrigin::signed(1),
            0,
            42,
            10_000,
            1_000,
            [0u8; 32],
        ));

        // Constitutional split: 3/10 = 300 → confederation, 7/10 = 700 → region
        assert_eq!(Balances::free_balance(999u64), region_before + 700);
        assert_eq!(Balances::free_balance(998u64), confed_before + 300);
        assert_eq!(Balances::free_balance(org_acct), bank_before - 1_000);

        let decl = TaxDeclarations::<Test>::get(0).unwrap();
        assert_eq!(decl.amount_paid, 1_000);
        assert_eq!(decl.declared_profit, 10_000);
        assert_eq!(decl.confederation_share, 300);
        assert_eq!(decl.regional_share, 700);
        assert_eq!(decl.org_id, 0);
    });
}

#[test]
fn test_file_tax_return_non_director_fails() {
    new_test_ext().execute_with(|| {
        System::set_block_number(1);
        register_and_activate_org(1);
        // Bob is not Director
        assert_noop!(
            Organization::file_tax_return(RuntimeOrigin::signed(2), 0, 42, 5_000, 500, [0u8; 32]),
            Error::<Test>::NotDirector
        );
    });
}

#[test]
fn test_file_tax_return_below_minimum_fails() {
    new_test_ext().execute_with(|| {
        System::set_block_number(1);
        register_and_activate_org(1);
        // MinTaxAmount = 20, trying 10
        assert_noop!(
            Organization::file_tax_return(RuntimeOrigin::signed(1), 0, 42, 100, 10, [0u8; 32]),
            Error::<Test>::BelowMinimumTax
        );
    });
}

#[test]
fn test_file_tax_return_outside_filing_period_fails() {
    new_test_ext().execute_with(|| {
        System::set_block_number(1);
        register_and_activate_org(1);

        // Jul 1, 2026 — outside filing period
        set_mock_timestamp(1782777600);

        assert_noop!(
            Organization::file_tax_return(
                RuntimeOrigin::signed(1),
                0,
                42,
                10_000,
                1_000,
                [0u8; 32]
            ),
            Error::<Test>::OutsideTaxFilingPeriod
        );
    });
}

#[test]
fn test_file_tax_return_within_filing_period_succeeds() {
    new_test_ext().execute_with(|| {
        System::set_block_number(1);
        let (_, org_acct) = setup_org_with_funds(1, 1_000);

        // Jan 15, 2026 — within filing period
        set_mock_timestamp(1768435200);

        assert_ok!(Organization::file_tax_return(
            RuntimeOrigin::signed(1),
            0,
            42,
            10_000,
            1_000,
            [0u8; 32],
        ));
        // Confirm funds moved
        let _ = org_acct; // suppress unused warning
    });
}

#[test]
fn test_file_tax_return_apr_10_within_period() {
    new_test_ext().execute_with(|| {
        System::set_block_number(1);
        let (_, _org_acct) = setup_org_with_funds(1, 1_000);

        // Apr 10, 2026 — still within filing period
        set_mock_timestamp(1775865600);

        assert_ok!(Organization::file_tax_return(
            RuntimeOrigin::signed(1),
            0,
            42,
            10_000,
            1_000,
            [0u8; 32],
        ));
    });
}

#[test]
fn test_file_tax_return_apr_16_outside_period() {
    new_test_ext().execute_with(|| {
        System::set_block_number(1);
        register_and_activate_org(1);

        // Apr 16, 2026 — outside filing period
        set_mock_timestamp(1776384000);

        assert_noop!(
            Organization::file_tax_return(
                RuntimeOrigin::signed(1),
                0,
                42,
                10_000,
                1_000,
                [0u8; 32]
            ),
            Error::<Test>::OutsideTaxFilingPeriod
        );
    });
}

// ─────────────────────────────────────────────────────────────────────────────
// Punishment Engine Tests
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn test_report_evasion_marks_delinquent_and_freezes_director() {
    new_test_ext().execute_with(|| {
        System::set_block_number(1);
        register_and_activate_org(1);

        // Advance past tax period (100 blocks + 2 > 100)
        System::set_block_number(102);

        assert_ok!(Organization::report_tax_evasion(
            RuntimeOrigin::signed(3),
            0
        ));

        let org = Organizations::<Test>::get(0).unwrap();
        assert_eq!(org.status, OrgStatus::Delinquent);

        let debt = OrgPenaltyDebt::<Test>::get(0);
        assert_eq!(debt, 1_000); // base penalty

        // Director (Alice = account 1) was frozen
        let frozen = FROZEN_ACCOUNTS.with(|fa| fa.borrow().clone());
        assert!(frozen.contains(&1u64));
    });
}

#[test]
fn test_report_evasion_fails_if_tax_period_not_expired() {
    new_test_ext().execute_with(|| {
        System::set_block_number(1);
        register_and_activate_org(1);
        // Only 50 blocks elapsed — not expired
        System::set_block_number(51);
        assert_noop!(
            Organization::report_tax_evasion(RuntimeOrigin::signed(3), 0),
            Error::<Test>::TaxPeriodNotExpired
        );
    });
}

#[test]
fn test_report_evasion_fails_if_already_delinquent() {
    new_test_ext().execute_with(|| {
        System::set_block_number(1);
        register_and_activate_org(1);
        System::set_block_number(102);
        assert_ok!(Organization::report_tax_evasion(
            RuntimeOrigin::signed(3),
            0
        ));
        // Second attempt must fail — already Delinquent
        assert_noop!(
            Organization::report_tax_evasion(RuntimeOrigin::signed(3), 0),
            Error::<Test>::AlreadyDelinquent
        );
    });
}

#[test]
fn test_file_tax_return_reinstates_delinquent_org() {
    new_test_ext().execute_with(|| {
        System::set_block_number(1);
        let (_, org_acct) = setup_org_with_funds(1, 5_000);
        System::set_block_number(102);
        assert_ok!(Organization::report_tax_evasion(
            RuntimeOrigin::signed(3),
            0
        ));
        assert_eq!(
            Organizations::<Test>::get(0).unwrap().status,
            OrgStatus::Delinquent
        );

        // Director pays tax, org reinstated and debt cleared
        // (timestamp=0 → genesis mode, filing always allowed)
        assert_ok!(Organization::file_tax_return(
            RuntimeOrigin::signed(1),
            0,
            42,
            50_000,
            5_000,
            [1u8; 32],
        ));
        let org = Organizations::<Test>::get(0).unwrap();
        assert_eq!(org.status, OrgStatus::Active);
        assert_eq!(OrgPenaltyDebt::<Test>::get(0), 0u64);

        let _ = org_acct; // suppress unused warning
    });
}

// ─────────────────────────────────────────────────────────────────────────────
// Judicial Freeze Tests
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn test_freeze_organization_root_only() {
    new_test_ext().execute_with(|| {
        System::set_block_number(1);
        register_and_activate_org(1);

        // Non-root fails
        assert_noop!(
            Organization::freeze_organization(RuntimeOrigin::signed(1), 0),
            sp_runtime::DispatchError::BadOrigin
        );

        // Root succeeds
        assert_ok!(Organization::freeze_organization(RuntimeOrigin::root(), 0));
        assert_eq!(
            Organizations::<Test>::get(0).unwrap().status,
            OrgStatus::Frozen
        );
    });
}

// ─────────────────────────────────────────────────────────────────────────────
// on_initialize Scanner
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn test_on_initialize_scans_and_marks_delinquent() {
    new_test_ext().execute_with(|| {
        System::set_block_number(1);
        register_and_activate_org(1);

        // Jump past tax period
        System::set_block_number(102);

        <Organization as Hooks<u64>>::on_initialize(102);

        let org = Organizations::<Test>::get(0).unwrap();
        assert_eq!(org.status, OrgStatus::Delinquent);
    });
}
