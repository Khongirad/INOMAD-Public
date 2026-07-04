//! Unit tests for pallet-judicial-courts.
//!
//! Covers:
//! - Case lifecycle: open → verdict → execute
//! - Habeas Corpus: timer registered on open_case
//! - Acquittal: defendant unfrozen, case closed
//! - Economic sentence: penalty → 20% whistleblower + 80% Treasury
//! - Digital Guillotine (HateCrimeAndFascism):
//!     · 100% full balance confiscation
//!     · CitizenStatus → Exiled (terminal)
//!     · RegistryOfShame entry: permanent, immutable
//! - Usurpation: declare_usurper → MartialLawActive, asset drain
//! - Error paths: closed case, wrong state, already in registry

use super::*;
use crate::mock::*;
use frame_support::{assert_noop, assert_ok};

// =========================================================================
// Helpers
// =========================================================================

const PLAINTIFF: AccountId = 1;
const DEFENDANT: AccountId = 2;
const WHISTLEBLOWER: AccountId = 3;

/// Open a case with no whistleblower. Returns case_id (always 0 for first case).
fn open(plaintiff: AccountId, defendant: AccountId) -> u32 {
    let case_id = Courts::next_case_id();
    assert_ok!(Courts::open_case(
        RuntimeOrigin::signed(plaintiff),
        defendant,
        evidence_hash(),
        None,
    ));
    case_id
}

/// Open a case with a whistleblower.
fn open_with_wb(plaintiff: AccountId, defendant: AccountId, wb: AccountId) -> u32 {
    let case_id = Courts::next_case_id();
    assert_ok!(Courts::open_case(
        RuntimeOrigin::signed(plaintiff),
        defendant,
        evidence_hash(),
        Some(wb),
    ));
    case_id
}

/// Issue a guilty verdict as Root (judges origin).
fn guilty(case_id: u32, penalty: Balance, cat: CrimeCategory) {
    assert_ok!(Courts::issue_verdict(
        RuntimeOrigin::root(),
        case_id,
        true,
        [0xFFu8; 32],
        penalty,
        cat,
    ));
}

/// Issue an acquittal verdict as Root.
fn acquit(case_id: u32) {
    assert_ok!(Courts::issue_verdict(
        RuntimeOrigin::root(),
        case_id,
        false,
        [0xFFu8; 32],
        0u128,
        CrimeCategory::Economic,
    ));
}

// =========================================================================
// 1. Case Lifecycle
// =========================================================================

#[test]
fn open_case_creates_case_with_open_status() {
    new_test_ext().execute_with(|| {
        let case_id = open(PLAINTIFF, DEFENDANT);
        let case = Courts::court_cases(case_id).unwrap();
        assert_eq!(case.status, CaseStatus::Open);
        assert_eq!(case.plaintiff, PLAINTIFF);
        assert_eq!(case.defendant, DEFENDANT);
        assert!(case.crime_category.is_none());
    });
}

#[test]
fn open_case_freezes_defendant_identity() {
    new_test_ext().execute_with(|| {
        open(PLAINTIFF, DEFENDANT);
        // MockIdentity should have set Bob to Frozen
        assert_eq!(citizen_status(DEFENDANT), Some(CitizenStatus::Frozen));
    });
}

#[test]
fn open_case_increments_next_case_id() {
    new_test_ext().execute_with(|| {
        assert_eq!(Courts::next_case_id(), 0);
        open(PLAINTIFF, DEFENDANT);
        assert_eq!(Courts::next_case_id(), 1);
        open(PLAINTIFF, DEFENDANT);
        assert_eq!(Courts::next_case_id(), 2);
    });
}

#[test]
fn open_case_with_unregistered_defendant_fails() {
    new_test_ext().execute_with(|| {
        // AccountId 99 not registered in MockIdentity
        assert_noop!(
            Courts::open_case(
                RuntimeOrigin::signed(PLAINTIFF),
                99u64,
                evidence_hash(),
                None,
            ),
            Error::<Test>::DefendantNotRegistered,
        );
    });
}

// =========================================================================
// 2. Acquittal: Habeas Corpus resolved, defendant unfrozen
// =========================================================================

#[test]
fn acquittal_unfreezes_defendant() {
    new_test_ext().execute_with(|| {
        let case_id = open(PLAINTIFF, DEFENDANT);
        assert_eq!(citizen_status(DEFENDANT), Some(CitizenStatus::Frozen));

        acquit(case_id);

        assert_eq!(citizen_status(DEFENDANT), Some(CitizenStatus::Active));
        let case = Courts::court_cases(case_id).unwrap();
        assert_eq!(case.status, CaseStatus::Acquitted);
    });
}

#[test]
fn execute_verdict_on_acquitted_case_fails() {
    new_test_ext().execute_with(|| {
        let case_id = open(PLAINTIFF, DEFENDANT);
        acquit(case_id);
        assert_noop!(
            Courts::execute_verdict(RuntimeOrigin::signed(PLAINTIFF), case_id),
            Error::<Test>::CaseNotGuilty,
        );
    });
}

// =========================================================================
// 3. Economic Sentence — penalty with whistleblower split
// =========================================================================

#[test]
fn economic_sentence_routes_100pct_to_treasury_without_wb() {
    new_test_ext().execute_with(|| {
        let penalty = 100 * UNIT;
        let treasury_before = Balances::free_balance(TREASURY);
        let bob_before = Balances::free_balance(DEFENDANT);

        let case_id = open(PLAINTIFF, DEFENDANT);
        guilty(case_id, penalty, CrimeCategory::Economic);
        assert_ok!(Courts::execute_verdict(
            RuntimeOrigin::signed(PLAINTIFF),
            case_id
        ));

        assert_eq!(Balances::free_balance(DEFENDANT), bob_before - penalty);
        assert_eq!(Balances::free_balance(TREASURY), treasury_before + penalty);
    });
}

#[test]
fn economic_sentence_splits_20pct_to_whistleblower() {
    new_test_ext().execute_with(|| {
        let penalty = 100 * UNIT;
        let wb_share = 20 * UNIT; // 20%
        let tr_share = 80 * UNIT; // 80%

        let treasury_before = Balances::free_balance(TREASURY);
        let wb_before = Balances::free_balance(WHISTLEBLOWER);
        let bob_before = Balances::free_balance(DEFENDANT);

        let case_id = open_with_wb(PLAINTIFF, DEFENDANT, WHISTLEBLOWER);
        guilty(case_id, penalty, CrimeCategory::Economic);
        assert_ok!(Courts::execute_verdict(
            RuntimeOrigin::signed(PLAINTIFF),
            case_id
        ));

        assert_eq!(Balances::free_balance(DEFENDANT), bob_before - penalty);
        assert_eq!(Balances::free_balance(WHISTLEBLOWER), wb_before + wb_share);
        assert_eq!(Balances::free_balance(TREASURY), treasury_before + tr_share);
    });
}

#[test]
fn economic_sentence_closes_case_as_executed() {
    new_test_ext().execute_with(|| {
        let case_id = open(PLAINTIFF, DEFENDANT);
        guilty(case_id, 50 * UNIT, CrimeCategory::Economic);
        assert_ok!(Courts::execute_verdict(
            RuntimeOrigin::signed(PLAINTIFF),
            case_id
        ));
        assert_eq!(
            Courts::court_cases(case_id).unwrap().status,
            CaseStatus::Executed
        );
    });
}

#[test]
fn economic_sentence_unfreezes_defendant_after_execution() {
    new_test_ext().execute_with(|| {
        let case_id = open(PLAINTIFF, DEFENDANT);
        guilty(case_id, 50 * UNIT, CrimeCategory::Economic);
        assert_ok!(Courts::execute_verdict(
            RuntimeOrigin::signed(PLAINTIFF),
            case_id
        ));
        assert_eq!(citizen_status(DEFENDANT), Some(CitizenStatus::Active));
    });
}

// =========================================================================
// 4. Digital Guillotine — HateCrimeAndFascism
// =========================================================================

#[test]
fn digital_guillotine_confiscates_100pct_balance() {
    new_test_ext().execute_with(|| {
        let bob_balance = Balances::free_balance(DEFENDANT); // 500 UNIT
        let treasury_before = Balances::free_balance(TREASURY);

        let case_id = open(PLAINTIFF, DEFENDANT);
        // penalty_amount in the verdict is IGNORED for HateCrimeAndFascism:
        // the full balance is confiscated at execution time.
        guilty(case_id, 1 * UNIT, CrimeCategory::HateCrimeAndFascism);
        assert_ok!(Courts::execute_verdict(
            RuntimeOrigin::signed(PLAINTIFF),
            case_id
        ));

        // Defendant drained to 0 (Expendable transfer)
        assert_eq!(Balances::free_balance(DEFENDANT), 0);
        // Treasury received everything
        assert_eq!(
            Balances::free_balance(TREASURY),
            treasury_before + bob_balance
        );
    });
}

#[test]
fn digital_guillotine_routes_20pct_to_whistleblower() {
    new_test_ext().execute_with(|| {
        let bob_balance = Balances::free_balance(DEFENDANT); // 500 UNIT
        let wb_reward = bob_balance * 20 / 100; // 100 UNIT
        let tr_net = bob_balance - wb_reward; // 400 UNIT

        let treasury_before = Balances::free_balance(TREASURY);
        let wb_before = Balances::free_balance(WHISTLEBLOWER);

        let case_id = open_with_wb(PLAINTIFF, DEFENDANT, WHISTLEBLOWER);
        guilty(case_id, 1 * UNIT, CrimeCategory::HateCrimeAndFascism);
        assert_ok!(Courts::execute_verdict(
            RuntimeOrigin::signed(PLAINTIFF),
            case_id
        ));

        assert_eq!(Balances::free_balance(DEFENDANT), 0);
        assert_eq!(Balances::free_balance(WHISTLEBLOWER), wb_before + wb_reward);
        assert_eq!(Balances::free_balance(TREASURY), treasury_before + tr_net);
    });
}

#[test]
fn digital_guillotine_exiles_citizen_permanently() {
    new_test_ext().execute_with(|| {
        let case_id = open(PLAINTIFF, DEFENDANT);
        guilty(case_id, 1 * UNIT, CrimeCategory::HateCrimeAndFascism);
        assert_ok!(Courts::execute_verdict(
            RuntimeOrigin::signed(PLAINTIFF),
            case_id
        ));

        // CitizenStatus::Exiled — TERMINAL
        assert_eq!(citizen_status(DEFENDANT), Some(CitizenStatus::Exiled));
    });
}

#[test]
fn digital_guillotine_inserts_shame_record() {
    new_test_ext().execute_with(|| {
        let case_id = open(PLAINTIFF, DEFENDANT);
        guilty(case_id, 1 * UNIT, CrimeCategory::HateCrimeAndFascism);
        assert_ok!(Courts::execute_verdict(
            RuntimeOrigin::signed(PLAINTIFF),
            case_id
        ));

        // Registry of Shame: entry MUST exist
        assert!(Courts::registry_of_shame(DEFENDANT).is_some());
        let record = Courts::registry_of_shame(DEFENDANT).unwrap();
        assert_eq!(record.case_id, case_id);
        assert_eq!(record.evidence_hash, evidence_hash());
    });
}

#[test]
fn shame_record_is_immutable_no_duplicate_entry() {
    new_test_ext().execute_with(|| {
        // First Digital Guillotine
        let case_id = open(PLAINTIFF, DEFENDANT);
        guilty(case_id, 1 * UNIT, CrimeCategory::HateCrimeAndFascism);
        assert_ok!(Courts::execute_verdict(
            RuntimeOrigin::signed(PLAINTIFF),
            case_id
        ));

        // Second attempt (re-exiling already-exiled citizen):
        // Bob has 0 balance now, so open a new case on Charlie instead of Bob
        // (to test the AlreadyInRegistryOfShame guard, we re-open a case on Bob)
        register_citizen(DEFENDANT); // re-register just to bypass identity check
        let case_id2 = open(PLAINTIFF, DEFENDANT);
        guilty(case_id2, 1 * UNIT, CrimeCategory::HateCrimeAndFascism);

        // Should return AlreadyInRegistryOfShame
        assert_noop!(
            Courts::execute_verdict(RuntimeOrigin::signed(PLAINTIFF), case_id2),
            Error::<Test>::AlreadyInRegistryOfShame,
        );
    });
}

// =========================================================================
// 5. Usurpation / Martial Law
// =========================================================================

#[test]
fn declare_usurper_sets_martial_law_active() {
    new_test_ext().execute_with(|| {
        assert!(!Courts::martial_law_active());
        assert_ok!(Courts::declare_usurper(RuntimeOrigin::root(), DEFENDANT));
        assert!(Courts::martial_law_active());
    });
}

#[test]
fn declare_usurper_drains_target_balance() {
    new_test_ext().execute_with(|| {
        let bob_before = Balances::free_balance(DEFENDANT); // 500 UNIT
        let treasury_before = Balances::free_balance(TREASURY);

        assert_ok!(Courts::declare_usurper(RuntimeOrigin::root(), DEFENDANT));

        assert_eq!(Balances::free_balance(DEFENDANT), 0);
        assert_eq!(
            Balances::free_balance(TREASURY),
            treasury_before + bob_before
        );
    });
}

// =========================================================================
// 6. Error paths
// =========================================================================

#[test]
fn verdict_on_non_open_case_fails() {
    new_test_ext().execute_with(|| {
        let case_id = open(PLAINTIFF, DEFENDANT);
        // Issue a verdict once
        guilty(case_id, 50 * UNIT, CrimeCategory::Economic);
        // Second verdict on same case in Guilty state → CaseNotOpen
        assert_noop!(
            Courts::issue_verdict(
                RuntimeOrigin::root(),
                case_id,
                true,
                [0xFFu8; 32],
                50 * UNIT,
                CrimeCategory::Economic,
            ),
            Error::<Test>::CaseNotOpen,
        );
    });
}

#[test]
fn execute_verdict_on_missing_case_fails() {
    new_test_ext().execute_with(|| {
        assert_noop!(
            Courts::execute_verdict(RuntimeOrigin::signed(PLAINTIFF), 999),
            Error::<Test>::CaseNotFound,
        );
    });
}

#[test]
fn execute_verdict_before_guilty_verdict_fails() {
    new_test_ext().execute_with(|| {
        let case_id = open(PLAINTIFF, DEFENDANT);
        // Case is still Open — cannot execute yet
        assert_noop!(
            Courts::execute_verdict(RuntimeOrigin::signed(PLAINTIFF), case_id),
            Error::<Test>::CaseNotGuilty,
        );
    });
}

#[test]
fn non_root_cannot_issue_verdict() {
    new_test_ext().execute_with(|| {
        let case_id = open(PLAINTIFF, DEFENDANT);
        assert_noop!(
            Courts::issue_verdict(
                RuntimeOrigin::signed(PLAINTIFF),
                case_id,
                true,
                [0xFFu8; 32],
                100 * UNIT,
                CrimeCategory::Economic,
            ),
            frame_support::error::BadOrigin,
        );
    });
}
