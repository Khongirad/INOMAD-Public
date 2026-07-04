//! WeightInfo trait and conservative placeholder weights for pallet_guilds.
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
//!   --pallet=pallet_guilds \
//!   --extrinsic="*" --steps=50 --repeat=20 \
//!   --output=pallets/guilds/src/weights.rs
//! ```

use core::marker::PhantomData;
use frame_support::traits::Get;
use frame_support::weights::Weight;

/// Weight functions needed for pallet_guilds.
pub trait WeightInfo {
    /// Weight of [`create_guild_from_relayer`] extrinsic.
    fn create_guild_from_relayer() -> Weight;
    /// Weight of [`update_council_from_relayer`] extrinsic.
    fn update_council_from_relayer() -> Weight;
    /// Weight of [`create_proposal_from_relayer`] extrinsic.
    fn create_proposal_from_relayer() -> Weight;
    /// Weight of [`vote_proposal_from_relayer`] extrinsic.
    fn vote_proposal_from_relayer() -> Weight;
    /// Weight of [`create_guild`] extrinsic.
    fn create_guild() -> Weight;
    /// Weight of [`join_guild`] extrinsic.
    fn join_guild() -> Weight;
    /// Weight of [`promote_member`] extrinsic.
    fn promote_member() -> Weight;
    /// Weight of [`propose_achievement`] extrinsic.
    fn propose_achievement() -> Weight;
    /// Weight of [`publish_quest`] extrinsic.
    fn publish_quest() -> Weight;
    /// Weight of [`assign_quest`] extrinsic.
    fn assign_quest() -> Weight;
    /// Weight of [`submit_quest`] extrinsic.
    fn submit_quest() -> Weight;
    /// Weight of [`complete_quest`] extrinsic.
    fn complete_quest() -> Weight;
    /// Weight of [`cancel_quest`] extrinsic.
    fn cancel_quest() -> Weight;
    /// Weight of [`subscribe_to_academy`] extrinsic.
    fn subscribe_to_academy() -> Weight;
    /// Weight of [`force_resolve_quest`] extrinsic.
    fn force_resolve_quest() -> Weight;
    /// Weight of [`form_guild_union`] extrinsic.
    fn form_guild_union() -> Weight;
    /// Weight of [`vote_for_grandmaster`] extrinsic.
    fn vote_for_grandmaster() -> Weight;
    /// Weight of [`elevate_union`] extrinsic.
    fn elevate_union() -> Weight;
    /// Weight of [`cleanup_account`] extrinsic.
    fn cleanup_account() -> Weight;
}

/// Placeholder weights for pallet_guilds.
///
/// Reference: conservative static analysis.
/// Replace with `cargo benchmark` output before mainnet.
pub struct SubstrateWeight<T>(PhantomData<T>);

impl<T: frame_system::Config> WeightInfo for SubstrateWeight<T> {
    /// Worst-case: 3 read(s), 3 write(s).
    fn create_guild_from_relayer() -> Weight {
        Weight::from_parts(80_000_000, 4_096)
            .saturating_add(T::DbWeight::get().reads(3))
            .saturating_add(T::DbWeight::get().writes(3))
    }
    /// Worst-case: 1 read(s), 1 write(s).
    fn update_council_from_relayer() -> Weight {
        Weight::from_parts(25_000_000, 4_096)
            .saturating_add(T::DbWeight::get().reads(1))
            .saturating_add(T::DbWeight::get().writes(1))
    }
    /// Worst-case: 3 read(s), 3 write(s).
    fn create_proposal_from_relayer() -> Weight {
        Weight::from_parts(80_000_000, 4_096)
            .saturating_add(T::DbWeight::get().reads(3))
            .saturating_add(T::DbWeight::get().writes(3))
    }
    /// Worst-case: 1 read(s), 1 write(s).
    fn vote_proposal_from_relayer() -> Weight {
        Weight::from_parts(25_000_000, 4_096)
            .saturating_add(T::DbWeight::get().reads(1))
            .saturating_add(T::DbWeight::get().writes(1))
    }
    /// Worst-case: 3 read(s), 3 write(s).
    fn create_guild() -> Weight {
        Weight::from_parts(80_000_000, 4_096)
            .saturating_add(T::DbWeight::get().reads(3))
            .saturating_add(T::DbWeight::get().writes(3))
    }
    /// Worst-case: 1 read(s), 1 write(s).
    fn join_guild() -> Weight {
        Weight::from_parts(25_000_000, 4_096)
            .saturating_add(T::DbWeight::get().reads(1))
            .saturating_add(T::DbWeight::get().writes(1))
    }
    /// Worst-case: 1 read(s), 1 write(s).
    fn promote_member() -> Weight {
        Weight::from_parts(25_000_000, 4_096)
            .saturating_add(T::DbWeight::get().reads(1))
            .saturating_add(T::DbWeight::get().writes(1))
    }
    /// Worst-case: 1 read(s), 1 write(s).
    fn propose_achievement() -> Weight {
        Weight::from_parts(25_000_000, 4_096)
            .saturating_add(T::DbWeight::get().reads(1))
            .saturating_add(T::DbWeight::get().writes(1))
    }
    /// Worst-case: 1 read(s), 1 write(s).
    fn publish_quest() -> Weight {
        Weight::from_parts(25_000_000, 4_096)
            .saturating_add(T::DbWeight::get().reads(1))
            .saturating_add(T::DbWeight::get().writes(1))
    }
    /// Worst-case: 1 read(s), 1 write(s).
    fn assign_quest() -> Weight {
        Weight::from_parts(25_000_000, 4_096)
            .saturating_add(T::DbWeight::get().reads(1))
            .saturating_add(T::DbWeight::get().writes(1))
    }
    /// Worst-case: 1 read(s), 1 write(s).
    fn submit_quest() -> Weight {
        Weight::from_parts(25_000_000, 4_096)
            .saturating_add(T::DbWeight::get().reads(1))
            .saturating_add(T::DbWeight::get().writes(1))
    }
    /// Worst-case: 1 read(s), 1 write(s).
    fn complete_quest() -> Weight {
        Weight::from_parts(25_000_000, 4_096)
            .saturating_add(T::DbWeight::get().reads(1))
            .saturating_add(T::DbWeight::get().writes(1))
    }
    /// Worst-case: 1 read(s), 1 write(s).
    fn cancel_quest() -> Weight {
        Weight::from_parts(25_000_000, 4_096)
            .saturating_add(T::DbWeight::get().reads(1))
            .saturating_add(T::DbWeight::get().writes(1))
    }
    /// Worst-case: 1 read(s), 1 write(s).
    fn subscribe_to_academy() -> Weight {
        Weight::from_parts(25_000_000, 4_096)
            .saturating_add(T::DbWeight::get().reads(1))
            .saturating_add(T::DbWeight::get().writes(1))
    }
    /// Worst-case: 1 read(s), 1 write(s).
    fn force_resolve_quest() -> Weight {
        Weight::from_parts(25_000_000, 4_096)
            .saturating_add(T::DbWeight::get().reads(1))
            .saturating_add(T::DbWeight::get().writes(1))
    }
    /// Worst-case: 3 read(s), 3 write(s).
    fn form_guild_union() -> Weight {
        Weight::from_parts(80_000_000, 4_096)
            .saturating_add(T::DbWeight::get().reads(3))
            .saturating_add(T::DbWeight::get().writes(3))
    }
    /// Worst-case: 1 read(s), 1 write(s).
    fn vote_for_grandmaster() -> Weight {
        Weight::from_parts(25_000_000, 4_096)
            .saturating_add(T::DbWeight::get().reads(1))
            .saturating_add(T::DbWeight::get().writes(1))
    }
    /// Worst-case: 1 read(s), 1 write(s).
    fn elevate_union() -> Weight {
        Weight::from_parts(25_000_000, 4_096)
            .saturating_add(T::DbWeight::get().reads(1))
            .saturating_add(T::DbWeight::get().writes(1))
    }
    /// Worst-case: 1 read(s), 1 write(s).
    fn cleanup_account() -> Weight {
        Weight::from_parts(25_000_000, 4_096)
            .saturating_add(T::DbWeight::get().reads(1))
            .saturating_add(T::DbWeight::get().writes(1))
    }
}

/// Unit weights for tests (zero cost).
impl WeightInfo for () {
    fn create_guild_from_relayer() -> Weight {
        Weight::zero()
    }
    fn update_council_from_relayer() -> Weight {
        Weight::zero()
    }
    fn create_proposal_from_relayer() -> Weight {
        Weight::zero()
    }
    fn vote_proposal_from_relayer() -> Weight {
        Weight::zero()
    }
    fn create_guild() -> Weight {
        Weight::zero()
    }
    fn join_guild() -> Weight {
        Weight::zero()
    }
    fn promote_member() -> Weight {
        Weight::zero()
    }
    fn propose_achievement() -> Weight {
        Weight::zero()
    }
    fn publish_quest() -> Weight {
        Weight::zero()
    }
    fn assign_quest() -> Weight {
        Weight::zero()
    }
    fn submit_quest() -> Weight {
        Weight::zero()
    }
    fn complete_quest() -> Weight {
        Weight::zero()
    }
    fn cancel_quest() -> Weight {
        Weight::zero()
    }
    fn subscribe_to_academy() -> Weight {
        Weight::zero()
    }
    fn force_resolve_quest() -> Weight {
        Weight::zero()
    }
    fn form_guild_union() -> Weight {
        Weight::zero()
    }
    fn vote_for_grandmaster() -> Weight {
        Weight::zero()
    }
    fn elevate_union() -> Weight {
        Weight::zero()
    }
    fn cleanup_account() -> Weight {
        Weight::zero()
    }
}
