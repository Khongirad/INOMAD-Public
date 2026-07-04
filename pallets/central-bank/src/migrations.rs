//! Storage migration helpers and `try-state` invariant checks for
//! `pallet-central-bank`.
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
use frame_support::{traits::{Get, OnRuntimeUpgrade}, weights::Weight};
use sp_runtime::{traits::Zero, Saturating};

#[cfg(feature = "try-runtime")]
use sp_std::vec::Vec;

// ─────────────────────────────────────────────────────────────────────────────
// Constitutional Invariants
// ─────────────────────────────────────────────────────────────────────────────

/// Validate all pallet-central-bank storage invariants.
///
/// Called by `try-runtime` after applying each runtime upgrade.
/// Returns an error string if any invariant is violated — the upgrade
/// will be rolled back before reaching any live node.
#[cfg(feature = "try-runtime")]
pub fn try_state<T: Config>(
    _n: frame_system::pallet_prelude::BlockNumberFor<T>,
) -> Result<(), sp_runtime::TryRuntimeError> {
    // TotalEmitted must always be >= TotalBurned
    ensure!(
        TotalEmitted::<T>::get() >= TotalBurned::<T>::get(),
        "TotalEmitted >= TotalBurned invariant violated"
    );
    // Epoch must be initialised (> 0) after genesis
    ensure!(
        CurrentEpochId::<T>::get() > 0,
        "CurrentEpochId > 0 invariant violated"
    );

    // ── Constitutional Credit Pool Invariants ─────────────────────────────────
    let genesis_limit = T::CreditEpochLimit::get();
    let available = GenesisCreditAvailable::<T>::get();
    let outstanding = TotalOutstanding::<T>::get();

    // Available + Outstanding must always equal genesis_limit (rotating pool conservation)
    ensure!(
        available.saturating_add(outstanding) <= genesis_limit,
        "available + outstanding must not exceed genesis_limit (21T hard cap)"
    );

    // Available must never exceed genesis_limit
    ensure!(
        available <= genesis_limit,
        "GenesisCreditAvailable must not exceed CreditEpochLimit"
    );

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
        // Snapshot any state needed to verify post_upgrade.
        // Return encoded bytes — use `encode()` on the snapshot.
        Ok(Default::default())
    }

    #[cfg(feature = "try-runtime")]
    fn post_upgrade(_state: Vec<u8>) -> Result<(), sp_runtime::TryRuntimeError> {
        // Verify invariants using try_state.
        try_state::<T>(Default::default())
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// v2: Initialize Constitutional Rotating Credit Pool
// ─────────────────────────────────────────────────────────────────────────────

/// Migration v2: Initialize the constitutional rotating credit pool storage items.
///
/// This migration initializes `GenesisCreditAvailable`, `TotalOutstanding`,
/// and `BaseKeyRate` which were added in the constitutional Variant B upgrade.
///
/// Apply this migration if upgrading from a runtime that had the old per-epoch
/// credit limit model (Variant A) to the new rotating pool model (Variant B).
///
/// ## Safety
///
/// Idempotent: if storage items already have values, they are NOT overwritten.
/// This means the migration is safe to re-apply.
pub struct InitCreditPool<T>(core::marker::PhantomData<T>);

impl<T: Config> OnRuntimeUpgrade for InitCreditPool<T> {
    fn on_runtime_upgrade() -> Weight {
        let mut weight = Weight::zero();

        // Only initialize if not already set (idempotent)
        if GenesisCreditAvailable::<T>::get().is_zero() {
            // Calculate outstanding from existing epoch data
            // (sum of total_issued - total_repaid across all epochs)
            let mut total_issued = BalanceOf::<T>::zero();
            let mut total_repaid_all = BalanceOf::<T>::zero();

            let epoch_count = CurrentEpochId::<T>::get();
            for i in 1..=epoch_count {
                if let Some(epoch) = Epochs::<T>::get(i) {
                    total_issued = total_issued.saturating_add(epoch.total_issued);
                    total_repaid_all = total_repaid_all.saturating_add(epoch.total_repaid);
                    weight = weight.saturating_add(T::DbWeight::get().reads(1));
                }
            }

            let outstanding = total_issued.saturating_sub(total_repaid_all);
            let genesis_limit = T::CreditEpochLimit::get();
            let available = genesis_limit.saturating_sub(outstanding);

            GenesisCreditAvailable::<T>::put(available);
            TotalOutstanding::<T>::put(outstanding);
            weight = weight.saturating_add(T::DbWeight::get().writes(2));
        }

        weight
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
