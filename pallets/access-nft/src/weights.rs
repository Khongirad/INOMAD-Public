//! WeightInfo trait and conservative placeholder weights for pallet_access_nft.
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
//!   --pallet=pallet_access_nft \
//!   --extrinsic="*" --steps=50 --repeat=20 \
//!   --output=pallets/access-nft/src/weights.rs
//! ```

use core::marker::PhantomData;
use frame_support::traits::Get;
use frame_support::weights::Weight;

/// Weight functions needed for pallet_access_nft.
pub trait WeightInfo {
    /// Weight of [`issue_access_key`] extrinsic.
    fn issue_access_key() -> Weight;
    /// Weight of [`revoke_access_key`] extrinsic.
    fn revoke_access_key() -> Weight;
    /// Weight of [`check_access`] extrinsic.
    fn check_access() -> Weight;
}

/// Placeholder weights for pallet_access_nft.
///
/// Reference: conservative static analysis.
/// Replace with `cargo benchmark` output before mainnet.
pub struct SubstrateWeight<T>(PhantomData<T>);

impl<T: frame_system::Config> WeightInfo for SubstrateWeight<T> {
    /// Worst-case: 3 read(s), 3 write(s).
    fn issue_access_key() -> Weight {
        Weight::from_parts(80_000_000, 4_096)
            .saturating_add(T::DbWeight::get().reads(3))
            .saturating_add(T::DbWeight::get().writes(3))
    }
    /// Worst-case: 1 read(s), 1 write(s).
    fn revoke_access_key() -> Weight {
        Weight::from_parts(25_000_000, 4_096)
            .saturating_add(T::DbWeight::get().reads(1))
            .saturating_add(T::DbWeight::get().writes(1))
    }
    /// Worst-case: 1 read(s), 1 write(s).
    fn check_access() -> Weight {
        Weight::from_parts(25_000_000, 4_096)
            .saturating_add(T::DbWeight::get().reads(1))
            .saturating_add(T::DbWeight::get().writes(1))
    }
}

/// Unit weights for tests (zero cost).
impl WeightInfo for () {
    fn issue_access_key() -> Weight {
        Weight::zero()
    }
    fn revoke_access_key() -> Weight {
        Weight::zero()
    }
    fn check_access() -> Weight {
        Weight::zero()
    }
}
