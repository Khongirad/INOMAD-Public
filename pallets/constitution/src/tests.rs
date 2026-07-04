//! # pallet-constitution: Unit Tests
//!
//! Tests the `on_initialize` Habeas Corpus enforcement engine:
//! - Timer registration
//! - Auto-release when deadline passes (no verdict)
//! - No auto-release when verdict is already entered

use crate::mock::*;
use crate::pallet::{Error, Event};
use frame_support::{assert_noop, assert_ok, traits::OnInitialize};

// ─────────────────────────────────────────────────────────────────────────────
// register_lockup
// ─────────────────────────────────────────────────────────────────────────────

/// Root can register a Habeas Corpus timer.
#[test]
fn register_lockup_creates_timer() {
    new_test_ext().execute_with(|| {
        let case_hash = [0xCC; 32];
        assert_ok!(Constitution::register_lockup(
            RuntimeOrigin::root(),
            ALICE,
            10u64, // max_lockup_block
            case_hash,
        ));

        let timer = Constitution::habeas_corpus_timers(ALICE).unwrap();
        assert_eq!(timer.max_lockup_block, 10);
        assert_eq!(timer.case_hash, case_hash);
        assert!(!timer.resolved);

        System::assert_last_event(
            Event::HabeasCorpusTimerRegistered {
                citizen: ALICE,
                max_lockup_block: 10,
                case_hash,
            }
            .into(),
        );
    });
}

/// Non-root cannot register a lockup.
#[test]
fn register_lockup_non_root_fails() {
    new_test_ext().execute_with(|| {
        assert_noop!(
            Constitution::register_lockup(RuntimeOrigin::signed(ALICE), ALICE, 10u64, [0u8; 32]),
            sp_runtime::DispatchError::BadOrigin
        );
    });
}

// ─────────────────────────────────────────────────────────────────────────────
// on_initialize — auto-release (no verdict entered)
// ─────────────────────────────────────────────────────────────────────────────

/// When the deadline passes and the hook reports no verdict,
/// `on_initialize` removes the timer and calls `release_citizen`.
#[test]
fn on_initialize_auto_releases_when_deadline_passes_without_verdict() {
    new_test_ext().execute_with(|| {
        let case_hash = [0xAB; 32];
        // Register a timer expiring at block 5.
        assert_ok!(Constitution::register_lockup(
            RuntimeOrigin::root(),
            ALICE,
            5u64,
            case_hash,
        ));

        // Simulate block 4 — timer not yet expired, no action.
        System::set_block_number(4);
        Constitution::on_initialize(4);
        assert!(Constitution::habeas_corpus_timers(ALICE).is_some());
        RELEASED.with(|r| assert!(r.borrow().is_empty()));

        // Simulate block 5 — deadline reached, no verdict → auto-release.
        System::set_block_number(5);
        Constitution::on_initialize(5);

        // Timer must be removed.
        assert!(Constitution::habeas_corpus_timers(ALICE).is_none());

        // release_citizen was called exactly once for ALICE.
        RELEASED.with(|r| {
            let released = r.borrow();
            assert_eq!(released.len(), 1);
            assert_eq!(released[0], ALICE);
        });

        // Auto-release event emitted.
        System::assert_last_event(
            Event::HabeasCorpusAutoReleased {
                citizen: ALICE,
                released_at: 5,
            }
            .into(),
        );
    });
}

/// Multiple citizens can be held simultaneously; all are released when due.
#[test]
fn on_initialize_releases_multiple_expired_timers() {
    new_test_ext().execute_with(|| {
        assert_ok!(Constitution::register_lockup(
            RuntimeOrigin::root(),
            ALICE,
            3u64,
            [0x01; 32]
        ));
        assert_ok!(Constitution::register_lockup(
            RuntimeOrigin::root(),
            BOB,
            3u64,
            [0x02; 32]
        ));

        System::set_block_number(3);
        Constitution::on_initialize(3);

        assert!(Constitution::habeas_corpus_timers(ALICE).is_none());
        assert!(Constitution::habeas_corpus_timers(BOB).is_none());

        RELEASED.with(|r| {
            assert_eq!(r.borrow().len(), 2);
        });
    });
}

// ─────────────────────────────────────────────────────────────────────────────
// on_initialize — no auto-release when verdict already entered
// ─────────────────────────────────────────────────────────────────────────────

/// If `has_verdict` returns true, `on_initialize` does NOT auto-release.
#[test]
fn on_initialize_skips_citizens_with_verdict() {
    new_test_ext().execute_with(|| {
        assert_ok!(Constitution::register_lockup(
            RuntimeOrigin::root(),
            ALICE,
            5u64,
            [0xDD; 32]
        ));

        // Signal that a verdict exists for ALICE.
        VERDICT_SET.with(|s| s.borrow_mut().insert(ALICE));

        System::set_block_number(5);
        Constitution::on_initialize(5);

        // Timer still present — no auto-release.
        assert!(Constitution::habeas_corpus_timers(ALICE).is_some());
        RELEASED.with(|r| assert!(r.borrow().is_empty()));
    });
}

// ─────────────────────────────────────────────────────────────────────────────
// resolve_habeas_corpus
// ─────────────────────────────────────────────────────────────────────────────

/// resolve_habeas_corpus marks the timer resolved and removes it.
#[test]
fn resolve_habeas_corpus_prevents_auto_release() {
    new_test_ext().execute_with(|| {
        let case_hash = [0xEE; 32];
        assert_ok!(Constitution::register_lockup(
            RuntimeOrigin::root(),
            ALICE,
            10u64,
            case_hash
        ));

        // Court enters verdict before deadline.
        assert_ok!(Constitution::resolve_habeas_corpus(
            RuntimeOrigin::root(),
            ALICE
        ));

        // Timer removed.
        assert!(Constitution::habeas_corpus_timers(ALICE).is_none());

        // on_initialize at deadline does nothing.
        System::set_block_number(10);
        Constitution::on_initialize(10);

        RELEASED.with(|r| assert!(r.borrow().is_empty()));

        System::assert_last_event(Event::HabeasCorpusResolved { citizen: ALICE }.into());
    });
}

/// Resolving a non-existent timer fails.
#[test]
fn resolve_nonexistent_timer_fails() {
    new_test_ext().execute_with(|| {
        assert_noop!(
            Constitution::resolve_habeas_corpus(RuntimeOrigin::root(), ALICE),
            Error::<Test>::NoActiveTimer
        );
    });
}
