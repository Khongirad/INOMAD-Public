//! WeightInfo trait and conservative placeholder weights for pallet_inomad_elections.
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
//!   --pallet=pallet_inomad_elections \
//!   --extrinsic="*" --steps=50 --repeat=20 \
//!   --output=pallets/inomad-elections/src/weights.rs
//! ```

use core::marker::PhantomData;
use frame_support::traits::Get;
use frame_support::weights::Weight;

/// Weight functions needed for pallet_inomad_elections.
pub trait WeightInfo {
    /// Weight of [`cast_vote`] extrinsic.
    fn cast_vote() -> Weight;
    /// Weight of [`add_citizen_to_arbad`] extrinsic.
    fn add_citizen_to_arbad() -> Weight;
    /// Weight of [`promote_leader`] extrinsic.
    fn promote_leader() -> Weight;
    /// Weight of [`create_election`] extrinsic.
    fn create_election() -> Weight;
    /// Weight of [`elect_branch_council`] extrinsic.
    fn elect_branch_council() -> Weight;
    /// Weight of [`confirm_branch_council`] extrinsic.
    fn confirm_branch_council() -> Weight;
    /// Weight of [`elect_supreme_leader`] extrinsic.
    fn elect_supreme_leader() -> Weight;
    /// Weight of [`confirm_supreme_leader`] extrinsic.
    fn confirm_supreme_leader() -> Weight;
    /// Weight of [`elect_khural_chairman`] extrinsic.
    fn elect_khural_chairman() -> Weight;
    /// Weight of [`confirm_khural_chairman`] extrinsic.
    fn confirm_khural_chairman() -> Weight;
    /// Weight of [`reset_ballot`] extrinsic.
    fn reset_ballot() -> Weight;
}

/// Placeholder weights for pallet_inomad_elections.
///
/// Reference: conservative static analysis.
/// Replace with `cargo benchmark` output before mainnet.
pub struct SubstrateWeight<T>(PhantomData<T>);

impl<T: frame_system::Config> WeightInfo for SubstrateWeight<T> {
    /// Worst-case: 1 read(s), 1 write(s).
    fn cast_vote() -> Weight {
        Weight::from_parts(25_000_000, 4_096)
            .saturating_add(T::DbWeight::get().reads(1))
            .saturating_add(T::DbWeight::get().writes(1))
    }
    /// Worst-case: 1 read(s), 1 write(s).
    fn add_citizen_to_arbad() -> Weight {
        Weight::from_parts(25_000_000, 4_096)
            .saturating_add(T::DbWeight::get().reads(1))
            .saturating_add(T::DbWeight::get().writes(1))
    }
    /// Worst-case: 1 read(s), 1 write(s).
    fn promote_leader() -> Weight {
        Weight::from_parts(25_000_000, 4_096)
            .saturating_add(T::DbWeight::get().reads(1))
            .saturating_add(T::DbWeight::get().writes(1))
    }
    /// Worst-case: 3 read(s), 3 write(s).
    fn create_election() -> Weight {
        Weight::from_parts(80_000_000, 4_096)
            .saturating_add(T::DbWeight::get().reads(3))
            .saturating_add(T::DbWeight::get().writes(3))
    }
    /// Worst-case: 3 read(s), 3 write(s).
    fn elect_branch_council() -> Weight {
        Weight::from_parts(80_000_000, 4_096)
            .saturating_add(T::DbWeight::get().reads(3))
            .saturating_add(T::DbWeight::get().writes(3))
    }
    /// Worst-case: 1 read(s), 1 write(s).
    fn confirm_branch_council() -> Weight {
        Weight::from_parts(25_000_000, 4_096)
            .saturating_add(T::DbWeight::get().reads(1))
            .saturating_add(T::DbWeight::get().writes(1))
    }
    /// Worst-case: 3 read(s), 3 write(s).
    fn elect_supreme_leader() -> Weight {
        Weight::from_parts(80_000_000, 4_096)
            .saturating_add(T::DbWeight::get().reads(3))
            .saturating_add(T::DbWeight::get().writes(3))
    }
    /// Worst-case: 1 read(s), 1 write(s).
    fn confirm_supreme_leader() -> Weight {
        Weight::from_parts(25_000_000, 4_096)
            .saturating_add(T::DbWeight::get().reads(1))
            .saturating_add(T::DbWeight::get().writes(1))
    }
    /// Worst-case: 3 read(s), 3 write(s).
    fn elect_khural_chairman() -> Weight {
        Weight::from_parts(80_000_000, 4_096)
            .saturating_add(T::DbWeight::get().reads(3))
            .saturating_add(T::DbWeight::get().writes(3))
    }
    /// Worst-case: 1 read(s), 1 write(s).
    fn confirm_khural_chairman() -> Weight {
        Weight::from_parts(25_000_000, 4_096)
            .saturating_add(T::DbWeight::get().reads(1))
            .saturating_add(T::DbWeight::get().writes(1))
    }
    /// Worst-case: 1 read(s), 1 write(s).
    fn reset_ballot() -> Weight {
        Weight::from_parts(25_000_000, 4_096)
            .saturating_add(T::DbWeight::get().reads(1))
            .saturating_add(T::DbWeight::get().writes(1))
    }
}

/// Unit weights for tests (zero cost).
impl WeightInfo for () {
    fn cast_vote() -> Weight {
        Weight::zero()
    }
    fn add_citizen_to_arbad() -> Weight {
        Weight::zero()
    }
    fn promote_leader() -> Weight {
        Weight::zero()
    }
    fn create_election() -> Weight {
        Weight::zero()
    }
    fn elect_branch_council() -> Weight {
        Weight::zero()
    }
    fn confirm_branch_council() -> Weight {
        Weight::zero()
    }
    fn elect_supreme_leader() -> Weight {
        Weight::zero()
    }
    fn confirm_supreme_leader() -> Weight {
        Weight::zero()
    }
    fn elect_khural_chairman() -> Weight {
        Weight::zero()
    }
    fn confirm_khural_chairman() -> Weight {
        Weight::zero()
    }
    fn reset_ballot() -> Weight {
        Weight::zero()
    }
}
