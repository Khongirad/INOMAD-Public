//! WeightInfo trait and conservative placeholder weights for pallet_citizen_voice.
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
//!   --pallet=pallet_citizen_voice \
//!   --extrinsic="*" --steps=50 --repeat=20 \
//!   --output=pallets/citizen-voice/src/weights.rs
//! ```

use core::marker::PhantomData;
use frame_support::traits::Get;
use frame_support::weights::Weight;

/// Weight functions needed for pallet_citizen_voice.
pub trait WeightInfo {
    /// Weight of [`submit_ticket`] extrinsic.
    fn submit_ticket() -> Weight;
    /// Weight of [`resolve_ticket`] extrinsic.
    fn resolve_ticket() -> Weight;
    /// Weight of [`mark_in_review`] extrinsic.
    fn mark_in_review() -> Weight;
    /// Weight of [`escalate_ticket`] extrinsic.
    fn escalate_ticket() -> Weight;
    /// Weight of [`request_sting_operation`] extrinsic.
    fn request_sting_operation() -> Weight;
    /// Weight of [`approve_sting`] extrinsic.
    fn approve_sting() -> Weight;
    /// Weight of [`reveal_and_spring_trap`] extrinsic.
    fn reveal_and_spring_trap() -> Weight;
}

/// Placeholder weights for pallet_citizen_voice.
///
/// Reference: conservative static analysis.
/// Replace with `cargo benchmark` output before mainnet.
pub struct SubstrateWeight<T>(PhantomData<T>);

impl<T: frame_system::Config> WeightInfo for SubstrateWeight<T> {
    /// Worst-case: 1 read(s), 1 write(s).
    fn submit_ticket() -> Weight {
        Weight::from_parts(25_000_000, 4_096)
            .saturating_add(T::DbWeight::get().reads(1))
            .saturating_add(T::DbWeight::get().writes(1))
    }
    /// Worst-case: 1 read(s), 1 write(s).
    fn resolve_ticket() -> Weight {
        Weight::from_parts(25_000_000, 4_096)
            .saturating_add(T::DbWeight::get().reads(1))
            .saturating_add(T::DbWeight::get().writes(1))
    }
    /// Worst-case: 1 read(s), 1 write(s).
    fn mark_in_review() -> Weight {
        Weight::from_parts(25_000_000, 4_096)
            .saturating_add(T::DbWeight::get().reads(1))
            .saturating_add(T::DbWeight::get().writes(1))
    }
    /// Worst-case: 1 read(s), 1 write(s).
    fn escalate_ticket() -> Weight {
        Weight::from_parts(25_000_000, 4_096)
            .saturating_add(T::DbWeight::get().reads(1))
            .saturating_add(T::DbWeight::get().writes(1))
    }
    /// Worst-case: 1 read(s), 1 write(s).
    fn request_sting_operation() -> Weight {
        Weight::from_parts(25_000_000, 4_096)
            .saturating_add(T::DbWeight::get().reads(1))
            .saturating_add(T::DbWeight::get().writes(1))
    }
    /// Worst-case: 1 read(s), 1 write(s).
    fn approve_sting() -> Weight {
        Weight::from_parts(25_000_000, 4_096)
            .saturating_add(T::DbWeight::get().reads(1))
            .saturating_add(T::DbWeight::get().writes(1))
    }
    /// Worst-case: 1 read(s), 1 write(s).
    fn reveal_and_spring_trap() -> Weight {
        Weight::from_parts(25_000_000, 4_096)
            .saturating_add(T::DbWeight::get().reads(1))
            .saturating_add(T::DbWeight::get().writes(1))
    }
}

/// Unit weights for tests (zero cost).
impl WeightInfo for () {
    fn submit_ticket() -> Weight {
        Weight::zero()
    }
    fn resolve_ticket() -> Weight {
        Weight::zero()
    }
    fn mark_in_review() -> Weight {
        Weight::zero()
    }
    fn escalate_ticket() -> Weight {
        Weight::zero()
    }
    fn request_sting_operation() -> Weight {
        Weight::zero()
    }
    fn approve_sting() -> Weight {
        Weight::zero()
    }
    fn reveal_and_spring_trap() -> Weight {
        Weight::zero()
    }
}
