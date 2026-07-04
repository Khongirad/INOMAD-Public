//! WeightInfo trait and conservative placeholder weights for pallet_khural_governance.
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
//!   --pallet=pallet_khural_governance \
//!   --extrinsic="*" --steps=50 --repeat=20 \
//!   --output=pallets/khural-governance/src/weights.rs
//! ```

use core::marker::PhantomData;
use frame_support::traits::Get;
use frame_support::weights::Weight;

/// Weight functions needed for pallet_khural_governance.
pub trait WeightInfo {
    /// Weight of [`create_proposal`] extrinsic.
    fn create_proposal() -> Weight;
    /// Weight of [`vote`] extrinsic.
    fn vote() -> Weight;
    /// Weight of [`execute_proposal`] extrinsic.
    fn execute_proposal() -> Weight;
    /// Weight of [`propose_expert_bill`] extrinsic.
    fn propose_expert_bill() -> Weight;
    /// Weight of [`vote_on_expert_bill`] extrinsic.
    fn vote_on_expert_bill() -> Weight;
    /// Weight of [`execute_expert_bill`] extrinsic.
    fn execute_expert_bill() -> Weight;
}

/// Placeholder weights for pallet_khural_governance.
///
/// Reference: conservative static analysis.
/// Replace with `cargo benchmark` output before mainnet.
pub struct SubstrateWeight<T>(PhantomData<T>);

impl<T: frame_system::Config> WeightInfo for SubstrateWeight<T> {
    /// Worst-case: 3 read(s), 3 write(s).
    fn create_proposal() -> Weight {
        Weight::from_parts(80_000_000, 4_096)
            .saturating_add(T::DbWeight::get().reads(3))
            .saturating_add(T::DbWeight::get().writes(3))
    }
    /// Worst-case: 1 read(s), 1 write(s).
    fn vote() -> Weight {
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
    /// Worst-case: 1 read(s), 1 write(s).
    fn propose_expert_bill() -> Weight {
        Weight::from_parts(25_000_000, 4_096)
            .saturating_add(T::DbWeight::get().reads(1))
            .saturating_add(T::DbWeight::get().writes(1))
    }
    /// Worst-case: 1 read(s), 1 write(s).
    fn vote_on_expert_bill() -> Weight {
        Weight::from_parts(25_000_000, 4_096)
            .saturating_add(T::DbWeight::get().reads(1))
            .saturating_add(T::DbWeight::get().writes(1))
    }
    /// Worst-case: 3 read(s), 3 write(s).
    fn execute_expert_bill() -> Weight {
        Weight::from_parts(80_000_000, 4_096)
            .saturating_add(T::DbWeight::get().reads(3))
            .saturating_add(T::DbWeight::get().writes(3))
    }
}

/// Unit weights for tests (zero cost).
impl WeightInfo for () {
    fn create_proposal() -> Weight {
        Weight::zero()
    }
    fn vote() -> Weight {
        Weight::zero()
    }
    fn execute_proposal() -> Weight {
        Weight::zero()
    }
    fn propose_expert_bill() -> Weight {
        Weight::zero()
    }
    fn vote_on_expert_bill() -> Weight {
        Weight::zero()
    }
    fn execute_expert_bill() -> Weight {
        Weight::zero()
    }
}
