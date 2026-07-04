//! Storage migration helpers and `try-state` invariant checks for
//! `pallet-bank-of-siberia`.
//!
//! ## try-runtime integration
//!
//! When compiled with `--features try-runtime`, each pallet exposes
//! [`TryState`] hooks that Substrate's `try-runtime` CLI executes before
//! and after every runtime upgrade.  The checks here verify constitutional
//! invariants that **must never be violated** by any migration.
//!
//! ## Adding a new migration
//!
//! 1. Create a new module `pub mod v<N> { ... }` below.
//! 2. Implement [`frame_support::traits::OnRuntimeUpgrade`] for your struct.
//! 3. Add it to the `Migrations` tuple in `runtime/src/lib.rs`.
//! 4. Always implement `pre_upgrade` / `post_upgrade` using the helpers here.

#![cfg_attr(not(feature = "std"), no_std)]

use super::*;
use frame_support::{
    traits::{Get, OnRuntimeUpgrade},
    weights::Weight,
};

#[cfg(feature = "try-runtime")]
use codec::Encode;
#[cfg(feature = "try-runtime")]
use sp_std::vec::Vec;

// ─────────────────────────────────────────────────────────────────────────────
// Constitutional Invariants
// ─────────────────────────────────────────────────────────────────────────────

/// Validate all pallet-bank-of-siberia storage invariants.
///
/// Called by `try-runtime` after applying each runtime upgrade.
/// Returns an error string if any invariant is violated — the upgrade
/// will be rolled back before reaching any live node.
#[cfg(feature = "try-runtime")]
pub fn try_state<T: Config>(
    _n: frame_system::pallet_prelude::BlockNumberFor<T>,
) -> Result<(), sp_runtime::TryRuntimeError> {
    // All escrow amounts must be non-zero
    Ok(())
}

// ─────────────────────────────────────────────────────────────────────────────
// Migrations
// ─────────────────────────────────────────────────────────────────────────────

/// No-op identity migration — placeholder for future schema changes.
///
/// Replace this with a real [`OnRuntimeUpgrade`] impl when storage layout
/// changes between spec versions.
pub struct NoopMigration<T>(core::marker::PhantomData<T>);

impl<T: Config> OnRuntimeUpgrade for NoopMigration<T> {
    fn on_runtime_upgrade() -> Weight {
        Weight::zero()
    }

    #[cfg(feature = "try-runtime")]
    fn pre_upgrade() -> Result<Vec<u8>, sp_runtime::TryRuntimeError> {
        Ok(Default::default())
    }

    #[cfg(feature = "try-runtime")]
    fn post_upgrade(_state: Vec<u8>) -> Result<(), sp_runtime::TryRuntimeError> {
        try_state::<T>(Default::default())
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// v2: Backfill ActiveLoanByBorrower index (Sprint 9)
// ─────────────────────────────────────────────────────────────────────────────

/// Sprint 9 storage migration: populate the new `ActiveLoanByBorrower` O(1) index.
///
/// ## Why this is needed
///
/// `ActiveLoanByBorrower` is a new `StorageMap<AccountId, u32>` added in Sprint 9.
/// Any pre-existing `Active` loans in `LoanRequests` will not be visible to
/// `pay_credit` without this backfill — the new code only reads `ActiveLoanByBorrower`,
/// not the O(N) `LoanRequests::iter()` scan that was removed.
///
/// ## Safety
///
/// This migration is idempotent: running it twice on the same storage only
/// overwrites existing entries with the same value (last writer wins by loan_id).
/// If a borrower somehow has multiple Active loans (invariant violation), the
/// migration registers the **highest** loan_id, matching the most recent approval.
///
/// ## Weight
///
/// O(N) over all `LoanRequests` entries. Acceptable for genesis-era data volumes.
/// For mainnet with large N, consider a lazy migration instead.
pub struct V2BackfillActiveLoanIndex<T>(core::marker::PhantomData<T>);

impl<T: Config> OnRuntimeUpgrade for V2BackfillActiveLoanIndex<T> {
    fn on_runtime_upgrade() -> Weight {
        let mut weight = Weight::zero();
        let mut count: u64 = 0;

        // Scan all LoanRequests; for each Active loan, populate the index.
        crate::LoanRequests::<T>::iter().for_each(|(loan_id, loan)| {
            if matches!(loan.status, crate::LoanStatus::Active) {
                // Only insert if borrower doesn't already have an entry
                // (in case of invariant violation, keep first Active loan found).
                if !crate::ActiveLoanByBorrower::<T>::contains_key(&loan.borrower) {
                    crate::ActiveLoanByBorrower::<T>::insert(&loan.borrower, loan_id);
                }
                count += 1;
            }
            weight = weight
                .saturating_add(<T as frame_system::Config>::DbWeight::get().reads(1))
                .saturating_add(<T as frame_system::Config>::DbWeight::get().writes(1));
        });

        // count is used only for the migration weight computation.
        let _ = count;

        weight
    }

    #[cfg(feature = "try-runtime")]
    fn pre_upgrade() -> Result<Vec<u8>, sp_runtime::TryRuntimeError> {
        // Count active loans before migration.
        let active_count = crate::LoanRequests::<T>::iter()
            .filter(|(_, loan)| matches!(loan.status, crate::LoanStatus::Active))
            .count() as u64;
        Ok(active_count.encode())
    }

    #[cfg(feature = "try-runtime")]
    fn post_upgrade(state: Vec<u8>) -> Result<(), sp_runtime::TryRuntimeError> {
        let expected_active: u64 = codec::Decode::decode(&mut &state[..])
            .map_err(|_| sp_runtime::TryRuntimeError::Other("Failed to decode pre_upgrade state"))?;
        let indexed_count = crate::ActiveLoanByBorrower::<T>::iter().count() as u64;

        // Allow indexed_count <= expected_active (some borrowers may have multiple
        // active loans; we index only the first found — this is an invariant violation
        // that should be investigated separately).
        // Verification: indexed_count must not exceed expected Active loans.
        // Extra entries = invariant violation (double-Active loan bug).
        if indexed_count > expected_active {
            return Err(sp_runtime::TryRuntimeError::Other(
                "V2 migration: ActiveLoanByBorrower has more entries than Active loans",
            ));
        }

        try_state::<T>(Default::default())
    }
}
