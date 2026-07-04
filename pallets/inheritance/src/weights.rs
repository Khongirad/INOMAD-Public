//! WeightInfo trait and conservative placeholder weights for pallet_inheritance.
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
//!   --pallet=pallet_inheritance \
//!   --extrinsic="*" --steps=50 --repeat=20 \
//!   --output=pallets/inheritance/src/weights.rs
//! ```

use core::marker::PhantomData;
use frame_support::traits::Get;
use frame_support::weights::Weight;

/// Weight functions needed for pallet_inheritance.
pub trait WeightInfo {
    /// Weight of [`draft_will`] extrinsic.
    fn draft_will() -> Weight;
    /// Weight of [`notarize_will`] extrinsic.
    fn notarize_will() -> Weight;
    /// Weight of [`execute_will`] extrinsic.
    fn execute_will() -> Weight;
    /// Weight of [`trigger_inheritance`] extrinsic.
    fn trigger_inheritance() -> Weight;
}

/// Placeholder weights for pallet_inheritance.
///
/// Reference: conservative static analysis.
/// Replace with `cargo benchmark` output before mainnet.
pub struct SubstrateWeight<T>(PhantomData<T>);

impl<T: frame_system::Config> WeightInfo for SubstrateWeight<T> {
    /// Worst-case: 1 read(s), 1 write(s).
    fn draft_will() -> Weight {
        Weight::from_parts(25_000_000, 4_096)
            .saturating_add(T::DbWeight::get().reads(1))
            .saturating_add(T::DbWeight::get().writes(1))
    }
    /// Worst-case: 1 read(s), 1 write(s).
    fn notarize_will() -> Weight {
        Weight::from_parts(25_000_000, 4_096)
            .saturating_add(T::DbWeight::get().reads(1))
            .saturating_add(T::DbWeight::get().writes(1))
    }
    /// Worst-case: 3 read(s), 3 write(s).
    fn execute_will() -> Weight {
        Weight::from_parts(80_000_000, 4_096)
            .saturating_add(T::DbWeight::get().reads(3))
            .saturating_add(T::DbWeight::get().writes(3))
    }
    /// Worst-case: 1 read(s), 1 write(s).
    fn trigger_inheritance() -> Weight {
        Weight::from_parts(25_000_000, 4_096)
            .saturating_add(T::DbWeight::get().reads(1))
            .saturating_add(T::DbWeight::get().writes(1))
    }
}

/// Unit weights for tests (zero cost).
impl WeightInfo for () {
    fn draft_will() -> Weight {
        Weight::zero()
    }
    fn notarize_will() -> Weight {
        Weight::zero()
    }
    fn execute_will() -> Weight {
        Weight::zero()
    }
    fn trigger_inheritance() -> Weight {
        Weight::zero()
    }
}
