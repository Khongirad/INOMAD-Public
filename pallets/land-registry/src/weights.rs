//! WeightInfo trait and conservative placeholder weights for pallet_land_registry.
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
//!   --pallet=pallet_land_registry \
//!   --extrinsic="*" --steps=50 --repeat=20 \
//!   --output=pallets/land-registry/src/weights.rs
//! ```

use core::marker::PhantomData;
use frame_support::traits::Get;
use frame_support::weights::Weight;

/// Weight functions needed for pallet_land_registry.
pub trait WeightInfo {
    /// Weight of [`register_parcel`] extrinsic.
    fn register_parcel() -> Weight;
    /// Weight of [`transfer_land`] extrinsic.
    fn transfer_land() -> Weight;
    /// Weight of [`update_resource_rights`] extrinsic.
    fn update_resource_rights() -> Weight;
}

/// Placeholder weights for pallet_land_registry.
///
/// Reference: conservative static analysis.
/// Replace with `cargo benchmark` output before mainnet.
pub struct SubstrateWeight<T>(PhantomData<T>);

impl<T: frame_system::Config> WeightInfo for SubstrateWeight<T> {
    /// Worst-case: 3 read(s), 3 write(s).
    fn register_parcel() -> Weight {
        Weight::from_parts(80_000_000, 4_096)
            .saturating_add(T::DbWeight::get().reads(3))
            .saturating_add(T::DbWeight::get().writes(3))
    }
    /// Worst-case: 1 read(s), 1 write(s).
    fn transfer_land() -> Weight {
        Weight::from_parts(25_000_000, 4_096)
            .saturating_add(T::DbWeight::get().reads(1))
            .saturating_add(T::DbWeight::get().writes(1))
    }
    /// Worst-case: 1 read(s), 1 write(s).
    fn update_resource_rights() -> Weight {
        Weight::from_parts(25_000_000, 4_096)
            .saturating_add(T::DbWeight::get().reads(1))
            .saturating_add(T::DbWeight::get().writes(1))
    }
}

/// Unit weights for tests (zero cost).
impl WeightInfo for () {
    fn register_parcel() -> Weight {
        Weight::zero()
    }
    fn transfer_land() -> Weight {
        Weight::zero()
    }
    fn update_resource_rights() -> Weight {
        Weight::zero()
    }
}
