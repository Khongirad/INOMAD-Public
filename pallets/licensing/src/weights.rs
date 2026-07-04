//! WeightInfo trait and conservative placeholder weights for pallet_licensing.
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
//!   --pallet=pallet_licensing \
//!   --extrinsic="*" --steps=50 --repeat=20 \
//!   --output=pallets/licensing/src/weights.rs
//! ```

use core::marker::PhantomData;
use frame_support::traits::Get;
use frame_support::weights::Weight;

/// Weight functions needed for pallet_licensing.
pub trait WeightInfo {
    /// Weight of [`authorize_ministry`] extrinsic.
    fn authorize_ministry() -> Weight;
    /// Weight of [`deauthorize_ministry`] extrinsic.
    fn deauthorize_ministry() -> Weight;
    /// Weight of [`submit_license_application`] extrinsic.
    fn submit_license_application() -> Weight;
    /// Weight of [`vote_on_license`] extrinsic.
    fn vote_on_license() -> Weight;
    /// Weight of [`revoke_license`] extrinsic.
    fn revoke_license() -> Weight;
    /// Weight of [`renew_license`] extrinsic.
    fn renew_license() -> Weight;
    /// Weight of [`is_licensed`] extrinsic.
    fn is_licensed() -> Weight;
}

/// Placeholder weights for pallet_licensing.
///
/// Reference: conservative static analysis.
/// Replace with `cargo benchmark` output before mainnet.
pub struct SubstrateWeight<T>(PhantomData<T>);

impl<T: frame_system::Config> WeightInfo for SubstrateWeight<T> {
    /// Worst-case: 1 read(s), 1 write(s).
    fn authorize_ministry() -> Weight {
        Weight::from_parts(25_000_000, 4_096)
            .saturating_add(T::DbWeight::get().reads(1))
            .saturating_add(T::DbWeight::get().writes(1))
    }
    /// Worst-case: 1 read(s), 1 write(s).
    fn deauthorize_ministry() -> Weight {
        Weight::from_parts(25_000_000, 4_096)
            .saturating_add(T::DbWeight::get().reads(1))
            .saturating_add(T::DbWeight::get().writes(1))
    }
    /// Worst-case: 1 read(s), 1 write(s).
    fn submit_license_application() -> Weight {
        Weight::from_parts(25_000_000, 4_096)
            .saturating_add(T::DbWeight::get().reads(1))
            .saturating_add(T::DbWeight::get().writes(1))
    }
    /// Worst-case: 1 read(s), 1 write(s).
    fn vote_on_license() -> Weight {
        Weight::from_parts(25_000_000, 4_096)
            .saturating_add(T::DbWeight::get().reads(1))
            .saturating_add(T::DbWeight::get().writes(1))
    }
    /// Worst-case: 1 read(s), 1 write(s).
    fn revoke_license() -> Weight {
        Weight::from_parts(25_000_000, 4_096)
            .saturating_add(T::DbWeight::get().reads(1))
            .saturating_add(T::DbWeight::get().writes(1))
    }
    /// Worst-case: 1 read(s), 1 write(s).
    fn renew_license() -> Weight {
        Weight::from_parts(25_000_000, 4_096)
            .saturating_add(T::DbWeight::get().reads(1))
            .saturating_add(T::DbWeight::get().writes(1))
    }
    /// Worst-case: 1 read(s), 0 write(s).
    fn is_licensed() -> Weight {
        Weight::from_parts(5_000_000, 4_096).saturating_add(T::DbWeight::get().reads(1))
    }
}

/// Unit weights for tests (zero cost).
impl WeightInfo for () {
    fn authorize_ministry() -> Weight {
        Weight::zero()
    }
    fn deauthorize_ministry() -> Weight {
        Weight::zero()
    }
    fn submit_license_application() -> Weight {
        Weight::zero()
    }
    fn vote_on_license() -> Weight {
        Weight::zero()
    }
    fn revoke_license() -> Weight {
        Weight::zero()
    }
    fn renew_license() -> Weight {
        Weight::zero()
    }
    fn is_licensed() -> Weight {
        Weight::zero()
    }
}
