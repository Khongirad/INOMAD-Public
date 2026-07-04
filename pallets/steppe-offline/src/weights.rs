//! WeightInfo trait and conservative placeholder weights for pallet_steppe_offline.
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
//!   --pallet=pallet_steppe_offline \
//!   --extrinsic="*" --steps=50 --repeat=20 \
//!   --output=pallets/steppe-offline/src/weights.rs
//! ```

use core::marker::PhantomData;
use frame_support::traits::Get;
use frame_support::weights::Weight;

/// Weight functions needed for pallet_steppe_offline.
pub trait WeightInfo {
    /// Weight of [`lock_funds`] extrinsic.
    fn lock_funds() -> Weight;
    /// Weight of [`settle_iou`] extrinsic.
    fn settle_iou() -> Weight;
}

/// Placeholder weights for pallet_steppe_offline.
///
/// Reference: conservative static analysis.
/// Replace with `cargo benchmark` output before mainnet.
pub struct SubstrateWeight<T>(PhantomData<T>);

impl<T: frame_system::Config> WeightInfo for SubstrateWeight<T> {
    /// Worst-case: 1 read(s), 1 write(s).
    fn lock_funds() -> Weight {
        Weight::from_parts(25_000_000, 4_096)
            .saturating_add(T::DbWeight::get().reads(1))
            .saturating_add(T::DbWeight::get().writes(1))
    }
    /// Worst-case: 1 read(s), 1 write(s).
    fn settle_iou() -> Weight {
        Weight::from_parts(25_000_000, 4_096)
            .saturating_add(T::DbWeight::get().reads(1))
            .saturating_add(T::DbWeight::get().writes(1))
    }
}

/// Unit weights for tests (zero cost).
impl WeightInfo for () {
    fn lock_funds() -> Weight {
        Weight::zero()
    }
    fn settle_iou() -> Weight {
        Weight::zero()
    }
}
