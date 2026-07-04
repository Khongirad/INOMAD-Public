//! Storage migration helpers and `try-state` invariant checks for
//! `pallet-judicial-courts`.
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
use frame_support::{traits::OnRuntimeUpgrade, weights::Weight};

#[cfg(feature = "try-runtime")]
use sp_std::vec::Vec;

// ─────────────────────────────────────────────────────────────────────────────
// Constitutional Invariants
// ─────────────────────────────────────────────────────────────────────────────

/// Validate all pallet-judicial-courts storage invariants.
///
/// Called by `try-runtime` after applying each runtime upgrade.
/// Returns an error string if any invariant is violated — the upgrade
/// will be rolled back before reaching any live node.
#[cfg(feature = "try-runtime")]
pub fn try_state<T: Config>(
    _n: frame_system::pallet_prelude::BlockNumberFor<T>,
) -> Result<(), sp_runtime::TryRuntimeError> {
    // No specific invariants for this pallet — add them as the pallet matures.
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
