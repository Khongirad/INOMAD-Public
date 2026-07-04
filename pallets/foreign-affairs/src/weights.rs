//! WeightInfo trait and conservative placeholder weights for pallet_foreign_affairs.
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
//!   --pallet=pallet_foreign_affairs \
//!   --extrinsic="*" --steps=50 --repeat=20 \
//!   --output=pallets/foreign-affairs/src/weights.rs
//! ```

use core::marker::PhantomData;
use frame_support::traits::Get;
use frame_support::weights::Weight;

/// Weight functions needed for pallet_foreign_affairs.
pub trait WeightInfo {
    /// Weight of [`register_foreign_state`] extrinsic.
    fn register_foreign_state() -> Weight;
    /// Weight of [`update_diplomatic_status`] extrinsic.
    fn update_diplomatic_status() -> Weight;
    /// Weight of [`send_diplomatic_message`] extrinsic.
    fn send_diplomatic_message() -> Weight;
    /// Weight of [`submit_legalization_document`] extrinsic.
    fn submit_legalization_document() -> Weight;
    /// Weight of [`freeze_country_operations`] extrinsic.
    fn freeze_country_operations() -> Weight;
    /// Weight of [`acknowledge_message`] extrinsic.
    fn acknowledge_message() -> Weight;
    /// Weight of [`register_representative`] extrinsic.
    fn register_representative() -> Weight;
    /// Weight of [`appoint_council_member`] extrinsic.
    fn appoint_council_member() -> Weight;
    /// Weight of [`remove_council_member`] extrinsic.
    fn remove_council_member() -> Weight;
    /// Weight of [`is_council_authorized`] extrinsic.
    fn is_council_authorized() -> Weight;
}

/// Placeholder weights for pallet_foreign_affairs.
///
/// Reference: conservative static analysis.
/// Replace with `cargo benchmark` output before mainnet.
pub struct SubstrateWeight<T>(PhantomData<T>);

impl<T: frame_system::Config> WeightInfo for SubstrateWeight<T> {
    /// Worst-case: 3 read(s), 3 write(s).
    fn register_foreign_state() -> Weight {
        Weight::from_parts(80_000_000, 4_096)
            .saturating_add(T::DbWeight::get().reads(3))
            .saturating_add(T::DbWeight::get().writes(3))
    }
    /// Worst-case: 1 read(s), 1 write(s).
    fn update_diplomatic_status() -> Weight {
        Weight::from_parts(25_000_000, 4_096)
            .saturating_add(T::DbWeight::get().reads(1))
            .saturating_add(T::DbWeight::get().writes(1))
    }
    /// Worst-case: 1 read(s), 1 write(s).
    fn send_diplomatic_message() -> Weight {
        Weight::from_parts(25_000_000, 4_096)
            .saturating_add(T::DbWeight::get().reads(1))
            .saturating_add(T::DbWeight::get().writes(1))
    }
    /// Worst-case: 1 read(s), 1 write(s).
    fn submit_legalization_document() -> Weight {
        Weight::from_parts(25_000_000, 4_096)
            .saturating_add(T::DbWeight::get().reads(1))
            .saturating_add(T::DbWeight::get().writes(1))
    }
    /// Worst-case: 1 read(s), 1 write(s).
    fn freeze_country_operations() -> Weight {
        Weight::from_parts(25_000_000, 4_096)
            .saturating_add(T::DbWeight::get().reads(1))
            .saturating_add(T::DbWeight::get().writes(1))
    }
    /// Worst-case: 1 read(s), 1 write(s).
    fn acknowledge_message() -> Weight {
        Weight::from_parts(25_000_000, 4_096)
            .saturating_add(T::DbWeight::get().reads(1))
            .saturating_add(T::DbWeight::get().writes(1))
    }
    /// Worst-case: 3 read(s), 3 write(s).
    fn register_representative() -> Weight {
        Weight::from_parts(80_000_000, 4_096)
            .saturating_add(T::DbWeight::get().reads(3))
            .saturating_add(T::DbWeight::get().writes(3))
    }
    /// Worst-case: 1 read(s), 1 write(s).
    fn appoint_council_member() -> Weight {
        Weight::from_parts(25_000_000, 4_096)
            .saturating_add(T::DbWeight::get().reads(1))
            .saturating_add(T::DbWeight::get().writes(1))
    }
    /// Worst-case: 1 read(s), 1 write(s).
    fn remove_council_member() -> Weight {
        Weight::from_parts(25_000_000, 4_096)
            .saturating_add(T::DbWeight::get().reads(1))
            .saturating_add(T::DbWeight::get().writes(1))
    }
    /// Worst-case: 1 read(s), 0 write(s).
    fn is_council_authorized() -> Weight {
        Weight::from_parts(5_000_000, 4_096).saturating_add(T::DbWeight::get().reads(1))
    }
}

/// Unit weights for tests (zero cost).
impl WeightInfo for () {
    fn register_foreign_state() -> Weight {
        Weight::zero()
    }
    fn update_diplomatic_status() -> Weight {
        Weight::zero()
    }
    fn send_diplomatic_message() -> Weight {
        Weight::zero()
    }
    fn submit_legalization_document() -> Weight {
        Weight::zero()
    }
    fn freeze_country_operations() -> Weight {
        Weight::zero()
    }
    fn acknowledge_message() -> Weight {
        Weight::zero()
    }
    fn register_representative() -> Weight {
        Weight::zero()
    }
    fn appoint_council_member() -> Weight {
        Weight::zero()
    }
    fn remove_council_member() -> Weight {
        Weight::zero()
    }
    fn is_council_authorized() -> Weight {
        Weight::zero()
    }
}
