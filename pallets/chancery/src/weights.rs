//! WeightInfo trait and conservative placeholder weights for pallet_chancery.
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
//!   --pallet=pallet_chancery \
//!   --extrinsic="*" --steps=50 --repeat=20 \
//!   --output=pallets/chancery/src/weights.rs
//! ```

use core::marker::PhantomData;
use frame_support::traits::Get;
use frame_support::weights::Weight;

/// Weight functions needed for pallet_chancery.
pub trait WeightInfo {
    /// Weight of [`propose_agreement`] extrinsic.
    fn propose_agreement() -> Weight;
    /// Weight of [`sign_agreement`] extrinsic.
    fn sign_agreement() -> Weight;
    /// Weight of [`validate_agreement`] extrinsic.
    fn validate_agreement() -> Weight;
    /// Weight of [`raise_dispute`] extrinsic.
    fn raise_dispute() -> Weight;
    /// Weight of [`complete_agreement`] extrinsic.
    fn complete_agreement() -> Weight;
    /// Weight of [`annul_signatures`] extrinsic.
    fn annul_signatures() -> Weight;
}

/// Placeholder weights for pallet_chancery.
///
/// Reference: conservative static analysis.
/// Replace with `cargo benchmark` output before mainnet.
pub struct SubstrateWeight<T>(PhantomData<T>);

impl<T: frame_system::Config> WeightInfo for SubstrateWeight<T> {
    /// Worst-case: 1 read(s), 1 write(s).
    fn propose_agreement() -> Weight {
        Weight::from_parts(25_000_000, 4_096)
            .saturating_add(T::DbWeight::get().reads(1))
            .saturating_add(T::DbWeight::get().writes(1))
    }
    /// Worst-case: 1 read(s), 1 write(s).
    fn sign_agreement() -> Weight {
        Weight::from_parts(25_000_000, 4_096)
            .saturating_add(T::DbWeight::get().reads(1))
            .saturating_add(T::DbWeight::get().writes(1))
    }
    /// Worst-case: 1 read(s), 0 write(s).
    fn validate_agreement() -> Weight {
        Weight::from_parts(5_000_000, 4_096).saturating_add(T::DbWeight::get().reads(1))
    }
    /// Worst-case: 1 read(s), 1 write(s).
    fn raise_dispute() -> Weight {
        Weight::from_parts(25_000_000, 4_096)
            .saturating_add(T::DbWeight::get().reads(1))
            .saturating_add(T::DbWeight::get().writes(1))
    }
    /// Worst-case: 1 read(s), 1 write(s).
    fn complete_agreement() -> Weight {
        Weight::from_parts(25_000_000, 4_096)
            .saturating_add(T::DbWeight::get().reads(1))
            .saturating_add(T::DbWeight::get().writes(1))
    }
    /// Worst-case: 1 read(s), 1 write(s).
    fn annul_signatures() -> Weight {
        Weight::from_parts(25_000_000, 4_096)
            .saturating_add(T::DbWeight::get().reads(1))
            .saturating_add(T::DbWeight::get().writes(1))
    }
}

/// Unit weights for tests (zero cost).
impl WeightInfo for () {
    fn propose_agreement() -> Weight {
        Weight::zero()
    }
    fn sign_agreement() -> Weight {
        Weight::zero()
    }
    fn validate_agreement() -> Weight {
        Weight::zero()
    }
    fn raise_dispute() -> Weight {
        Weight::zero()
    }
    fn complete_agreement() -> Weight {
        Weight::zero()
    }
    fn annul_signatures() -> Weight {
        Weight::zero()
    }
}
