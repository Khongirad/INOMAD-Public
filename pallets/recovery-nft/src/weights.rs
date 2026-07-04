//! WeightInfo trait and conservative placeholder weights for pallet_recovery_nft.
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
//!   --pallet=pallet_recovery_nft \
//!   --extrinsic="*" --steps=50 --repeat=20 \
//!   --output=pallets/recovery-nft/src/weights.rs
//! ```

use core::marker::PhantomData;
use frame_support::traits::Get;
use frame_support::weights::Weight;

/// Weight functions needed for pallet_recovery_nft.
pub trait WeightInfo {
    /// Weight of [`issue_recovery_nfts`] extrinsic.
    fn issue_recovery_nfts() -> Weight;
    /// Weight of [`initiate_recovery`] extrinsic.
    fn initiate_recovery() -> Weight;
    /// Weight of [`veto_recovery`] extrinsic.
    fn veto_recovery() -> Weight;
    /// Weight of [`confirm_recovery`] extrinsic.
    fn confirm_recovery() -> Weight;
}

/// Placeholder weights for pallet_recovery_nft.
///
/// Reference: conservative static analysis.
/// Replace with `cargo benchmark` output before mainnet.
pub struct SubstrateWeight<T>(PhantomData<T>);

impl<T: frame_system::Config> WeightInfo for SubstrateWeight<T> {
    /// Worst-case: 3 read(s), 3 write(s).
    fn issue_recovery_nfts() -> Weight {
        Weight::from_parts(80_000_000, 4_096)
            .saturating_add(T::DbWeight::get().reads(3))
            .saturating_add(T::DbWeight::get().writes(3))
    }
    /// Worst-case: 1 read(s), 1 write(s).
    fn initiate_recovery() -> Weight {
        Weight::from_parts(25_000_000, 4_096)
            .saturating_add(T::DbWeight::get().reads(1))
            .saturating_add(T::DbWeight::get().writes(1))
    }
    /// Worst-case: 1 read(s), 1 write(s).
    fn veto_recovery() -> Weight {
        Weight::from_parts(25_000_000, 4_096)
            .saturating_add(T::DbWeight::get().reads(1))
            .saturating_add(T::DbWeight::get().writes(1))
    }
    /// Worst-case: 1 read(s), 1 write(s).
    fn confirm_recovery() -> Weight {
        Weight::from_parts(25_000_000, 4_096)
            .saturating_add(T::DbWeight::get().reads(1))
            .saturating_add(T::DbWeight::get().writes(1))
    }
}

/// Unit weights for tests (zero cost).
impl WeightInfo for () {
    fn issue_recovery_nfts() -> Weight {
        Weight::zero()
    }
    fn initiate_recovery() -> Weight {
        Weight::zero()
    }
    fn veto_recovery() -> Weight {
        Weight::zero()
    }
    fn confirm_recovery() -> Weight {
        Weight::zero()
    }
}
