//! WeightInfo trait and conservative placeholder weights for pallet_buryad_mongol_mixer.
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
//!   --pallet=pallet_buryad_mongol_mixer \
//!   --extrinsic="*" --steps=50 --repeat=20 \
//!   --output=pallets/buryad-mongol-mixer/src/weights.rs
//! ```

use core::marker::PhantomData;
use frame_support::traits::Get;
use frame_support::weights::Weight;

/// Weight functions needed for pallet_buryad_mongol_mixer.
pub trait WeightInfo {
    /// Weight of [`deposit`] extrinsic.
    fn deposit() -> Weight;
    /// Weight of [`withdraw`] extrinsic.
    fn withdraw() -> Weight;
    /// Weight of [`reveal_transaction`] extrinsic.
    fn reveal_transaction() -> Weight;
    /// Weight of [`submit_quarterly_audit`] extrinsic.
    fn submit_quarterly_audit() -> Weight;
}

/// Placeholder weights for pallet_buryad_mongol_mixer.
///
/// Reference: conservative static analysis.
/// Replace with `cargo benchmark` output before mainnet.
pub struct SubstrateWeight<T>(PhantomData<T>);

impl<T: frame_system::Config> WeightInfo for SubstrateWeight<T> {
    /// Worst-case: 1 read(s), 1 write(s).
    fn deposit() -> Weight {
        Weight::from_parts(25_000_000, 4_096)
            .saturating_add(T::DbWeight::get().reads(1))
            .saturating_add(T::DbWeight::get().writes(1))
    }
    /// Worst-case: 1 read(s), 1 write(s).
    fn withdraw() -> Weight {
        Weight::from_parts(25_000_000, 4_096)
            .saturating_add(T::DbWeight::get().reads(1))
            .saturating_add(T::DbWeight::get().writes(1))
    }
    /// Worst-case: 1 read(s), 1 write(s).
    fn reveal_transaction() -> Weight {
        Weight::from_parts(25_000_000, 4_096)
            .saturating_add(T::DbWeight::get().reads(1))
            .saturating_add(T::DbWeight::get().writes(1))
    }
    /// Worst-case: 1 read(s), 1 write(s).
    fn submit_quarterly_audit() -> Weight {
        Weight::from_parts(25_000_000, 4_096)
            .saturating_add(T::DbWeight::get().reads(1))
            .saturating_add(T::DbWeight::get().writes(1))
    }
}

/// Unit weights for tests (zero cost).
impl WeightInfo for () {
    fn deposit() -> Weight {
        Weight::zero()
    }
    fn withdraw() -> Weight {
        Weight::zero()
    }
    fn reveal_transaction() -> Weight {
        Weight::zero()
    }
    fn submit_quarterly_audit() -> Weight {
        Weight::zero()
    }
}
