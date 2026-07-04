//! WeightInfo trait and conservative placeholder weights for pallet_altan_vault.
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
//!   --pallet=pallet_altan_vault \
//!   --extrinsic="*" --steps=50 --repeat=20 \
//!   --output=pallets/altan-vault/src/weights.rs
//! ```

use core::marker::PhantomData;
use frame_support::traits::Get;
use frame_support::weights::Weight;

/// Weight functions needed for pallet_altan_vault.
pub trait WeightInfo {
    /// Weight of [`create_vault`] extrinsic.
    fn create_vault() -> Weight;
    /// Weight of [`withdraw_to_owner`] extrinsic.
    fn withdraw_to_owner() -> Weight;
    /// Weight of [`record_inbound`] extrinsic.
    fn record_inbound() -> Weight;
}

/// Placeholder weights for pallet_altan_vault.
///
/// Reference: conservative static analysis.
/// Replace with `cargo benchmark` output before mainnet.
pub struct SubstrateWeight<T>(PhantomData<T>);

impl<T: frame_system::Config> WeightInfo for SubstrateWeight<T> {
    /// Worst-case: 3 read(s), 3 write(s).
    fn create_vault() -> Weight {
        Weight::from_parts(80_000_000, 4_096)
            .saturating_add(T::DbWeight::get().reads(3))
            .saturating_add(T::DbWeight::get().writes(3))
    }
    /// Worst-case: 1 read(s), 1 write(s).
    fn withdraw_to_owner() -> Weight {
        Weight::from_parts(25_000_000, 4_096)
            .saturating_add(T::DbWeight::get().reads(1))
            .saturating_add(T::DbWeight::get().writes(1))
    }
    /// Worst-case: 1 read(s), 1 write(s).
    fn record_inbound() -> Weight {
        Weight::from_parts(25_000_000, 4_096)
            .saturating_add(T::DbWeight::get().reads(1))
            .saturating_add(T::DbWeight::get().writes(1))
    }
}

/// Unit weights for tests (zero cost).
impl WeightInfo for () {
    fn create_vault() -> Weight {
        Weight::zero()
    }
    fn withdraw_to_owner() -> Weight {
        Weight::zero()
    }
    fn record_inbound() -> Weight {
        Weight::zero()
    }
}
