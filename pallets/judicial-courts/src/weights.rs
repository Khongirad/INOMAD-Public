//! WeightInfo trait and conservative placeholder weights for pallet_judicial_courts.
//!
//! **STATUS: PLACEHOLDER — Replace with `cargo benchmark` output before mainnet.**
//!
//! These weights are derived from static analysis of each extrinsic's
//! worst-case storage access pattern. They are safe upper bounds.
//!
//! ## Regeneration
//! ```bash
//! cargo benchmark \
//!   --chain=dev --execution=wasm \
//!   --pallet=pallet_judicial_courts \
//!   --extrinsic="*" --steps=50 --repeat=20 \
//!   --output=pallets/judicial-courts/src/weights.rs
//! ```

use core::marker::PhantomData;
use frame_support::traits::Get;
use frame_support::weights::Weight;

/// Weight functions needed for pallet_judicial_courts.
pub trait WeightInfo {
    /// Weight of [`open_case`] extrinsic.
    fn open_case() -> Weight;
    /// Weight of [`issue_verdict`] extrinsic.
    fn issue_verdict() -> Weight;
    /// Weight of [`execute_verdict`] extrinsic.
    fn execute_verdict() -> Weight;
    /// Weight of [`declare_usurper`] extrinsic.
    fn declare_usurper() -> Weight;
    /// Weight of [`defendant_has_verdict`] extrinsic.
    fn defendant_has_verdict() -> Weight;
    /// Weight of [`is_in_registry_of_shame`] extrinsic.
    fn is_in_registry_of_shame() -> Weight;
}

/// Placeholder weights for pallet_judicial_courts.
///
/// Reference: conservative static analysis.
/// Replace with `cargo benchmark` output before mainnet.
pub struct SubstrateWeight<T>(PhantomData<T>);

impl<T: frame_system::Config> WeightInfo for SubstrateWeight<T> {
    /// Worst-case: 3 read(s), 3 write(s).
    fn open_case() -> Weight {
        Weight::from_parts(80_000_000, 4_096)
            .saturating_add(T::DbWeight::get().reads(3))
            .saturating_add(T::DbWeight::get().writes(3))
    }
    /// Worst-case: 3 read(s), 3 write(s).
    fn issue_verdict() -> Weight {
        Weight::from_parts(80_000_000, 4_096)
            .saturating_add(T::DbWeight::get().reads(3))
            .saturating_add(T::DbWeight::get().writes(3))
    }
    /// Worst-case: 3 read(s), 3 write(s).
    fn execute_verdict() -> Weight {
        Weight::from_parts(80_000_000, 4_096)
            .saturating_add(T::DbWeight::get().reads(3))
            .saturating_add(T::DbWeight::get().writes(3))
    }
    /// Worst-case: 1 read(s), 1 write(s).
    fn declare_usurper() -> Weight {
        Weight::from_parts(25_000_000, 4_096)
            .saturating_add(T::DbWeight::get().reads(1))
            .saturating_add(T::DbWeight::get().writes(1))
    }
    /// Worst-case: 1 read(s), 1 write(s).
    fn defendant_has_verdict() -> Weight {
        Weight::from_parts(25_000_000, 4_096)
            .saturating_add(T::DbWeight::get().reads(1))
            .saturating_add(T::DbWeight::get().writes(1))
    }
    /// Worst-case: 1 read(s), 0 write(s).
    fn is_in_registry_of_shame() -> Weight {
        Weight::from_parts(5_000_000, 4_096).saturating_add(T::DbWeight::get().reads(1))
    }
}

/// Unit weights for tests (zero cost).
impl WeightInfo for () {
    fn open_case() -> Weight {
        Weight::zero()
    }
    fn issue_verdict() -> Weight {
        Weight::zero()
    }
    fn execute_verdict() -> Weight {
        Weight::zero()
    }
    fn declare_usurper() -> Weight {
        Weight::zero()
    }
    fn defendant_has_verdict() -> Weight {
        Weight::zero()
    }
    fn is_in_registry_of_shame() -> Weight {
        Weight::zero()
    }
}
