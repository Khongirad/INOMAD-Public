//! WeightInfo trait and conservative placeholder weights for pallet_decimal_dao.
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
//!   --pallet=pallet_decimal_dao \
//!   --extrinsic="*" --steps=50 --repeat=20 \
//!   --output=pallets/decimal-dao/src/weights.rs
//! ```

use core::marker::PhantomData;
use frame_support::traits::Get;
use frame_support::weights::Weight;

/// Weight functions needed for pallet_decimal_dao.
pub trait WeightInfo {
    /// Weight of [`instantiate_org`] extrinsic.
    fn instantiate_org() -> Weight;
    /// Weight of [`sync_council`] extrinsic.
    fn sync_council() -> Weight;
    /// Weight of [`create_proposal`] extrinsic.
    fn create_proposal() -> Weight;
    /// Weight of [`vote_proposal`] extrinsic.
    fn vote_proposal() -> Weight;
    /// Weight of [`execute_proposal`] extrinsic.
    fn execute_proposal() -> Weight;
    /// Weight of [`get_council`] extrinsic.
    fn get_council() -> Weight;
    /// Weight of [`is_council_member`] extrinsic.
    fn is_council_member() -> Weight;
}

/// Placeholder weights for pallet_decimal_dao.
///
/// Reference: conservative static analysis.
/// Replace with `cargo benchmark` output before mainnet.
pub struct SubstrateWeight<T>(PhantomData<T>);

impl<T: frame_system::Config> WeightInfo for SubstrateWeight<T> {
    /// Worst-case: 1 read(s), 1 write(s).
    fn instantiate_org() -> Weight {
        Weight::from_parts(25_000_000, 4_096)
            .saturating_add(T::DbWeight::get().reads(1))
            .saturating_add(T::DbWeight::get().writes(1))
    }
    /// Worst-case: 1 read(s), 1 write(s).
    fn sync_council() -> Weight {
        Weight::from_parts(25_000_000, 4_096)
            .saturating_add(T::DbWeight::get().reads(1))
            .saturating_add(T::DbWeight::get().writes(1))
    }
    /// Worst-case: 3 read(s), 3 write(s).
    fn create_proposal() -> Weight {
        Weight::from_parts(80_000_000, 4_096)
            .saturating_add(T::DbWeight::get().reads(3))
            .saturating_add(T::DbWeight::get().writes(3))
    }
    /// Worst-case: 1 read(s), 1 write(s).
    fn vote_proposal() -> Weight {
        Weight::from_parts(25_000_000, 4_096)
            .saturating_add(T::DbWeight::get().reads(1))
            .saturating_add(T::DbWeight::get().writes(1))
    }
    /// Worst-case: 3 read(s), 3 write(s).
    fn execute_proposal() -> Weight {
        Weight::from_parts(80_000_000, 4_096)
            .saturating_add(T::DbWeight::get().reads(3))
            .saturating_add(T::DbWeight::get().writes(3))
    }
    /// Worst-case: 1 read(s), 0 write(s).
    fn get_council() -> Weight {
        Weight::from_parts(5_000_000, 4_096).saturating_add(T::DbWeight::get().reads(1))
    }
    /// Worst-case: 1 read(s), 0 write(s).
    fn is_council_member() -> Weight {
        Weight::from_parts(5_000_000, 4_096).saturating_add(T::DbWeight::get().reads(1))
    }
}

/// Unit weights for tests (zero cost).
impl WeightInfo for () {
    fn instantiate_org() -> Weight {
        Weight::zero()
    }
    fn sync_council() -> Weight {
        Weight::zero()
    }
    fn create_proposal() -> Weight {
        Weight::zero()
    }
    fn vote_proposal() -> Weight {
        Weight::zero()
    }
    fn execute_proposal() -> Weight {
        Weight::zero()
    }
    fn get_council() -> Weight {
        Weight::zero()
    }
    fn is_council_member() -> Weight {
        Weight::zero()
    }
}
