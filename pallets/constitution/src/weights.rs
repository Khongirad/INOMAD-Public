//! WeightInfo trait and conservative placeholder weights for pallet_constitution.
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
//!   --pallet=pallet_constitution \
//!   --extrinsic="*" --steps=50 --repeat=20 \
//!   --output=pallets/constitution/src/weights.rs
//! ```

use core::marker::PhantomData;
use frame_support::traits::Get;
use frame_support::weights::Weight;

/// Weight functions needed for pallet_constitution.
pub trait WeightInfo {
    /// Weight of [`set_constitution_hash`] extrinsic.
    fn set_constitution_hash() -> Weight;
    /// Weight of [`register_lockup`] extrinsic.
    fn register_lockup() -> Weight;
    /// Weight of [`resolve_habeas_corpus`] extrinsic.
    fn resolve_habeas_corpus() -> Weight;
}

/// Placeholder weights for pallet_constitution.
///
/// Reference: conservative static analysis.
/// Replace with `cargo benchmark` output before mainnet.
pub struct SubstrateWeight<T>(PhantomData<T>);

impl<T: frame_system::Config> WeightInfo for SubstrateWeight<T> {
    /// Worst-case: 1 read(s), 1 write(s).
    fn set_constitution_hash() -> Weight {
        Weight::from_parts(25_000_000, 4_096)
            .saturating_add(T::DbWeight::get().reads(1))
            .saturating_add(T::DbWeight::get().writes(1))
    }
    /// Worst-case: 3 read(s), 3 write(s).
    fn register_lockup() -> Weight {
        Weight::from_parts(80_000_000, 4_096)
            .saturating_add(T::DbWeight::get().reads(3))
            .saturating_add(T::DbWeight::get().writes(3))
    }
    /// Worst-case: 1 read(s), 1 write(s).
    fn resolve_habeas_corpus() -> Weight {
        Weight::from_parts(25_000_000, 4_096)
            .saturating_add(T::DbWeight::get().reads(1))
            .saturating_add(T::DbWeight::get().writes(1))
    }
}

/// Unit weights for tests (zero cost).
impl WeightInfo for () {
    fn set_constitution_hash() -> Weight {
        Weight::zero()
    }
    fn register_lockup() -> Weight {
        Weight::zero()
    }
    fn resolve_habeas_corpus() -> Weight {
        Weight::zero()
    }
}
