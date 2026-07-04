//! WeightInfo trait and conservative placeholder weights for pallet_organization.
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
//!   --pallet=pallet_organization \
//!   --extrinsic="*" --steps=50 --repeat=20 \
//!   --output=pallets/organization/src/weights.rs
//! ```

use core::marker::PhantomData;
use frame_support::traits::Get;
use frame_support::weights::Weight;

/// Weight functions needed for pallet_organization.
pub trait WeightInfo {
    /// Weight of [`register_organization`] extrinsic.
    fn register_organization() -> Weight;
    /// Weight of [`activate_organization`] extrinsic.
    fn activate_organization() -> Weight;
    /// Weight of [`add_crew_member`] extrinsic.
    fn add_crew_member() -> Weight;
    /// Weight of [`file_tax_return`] extrinsic.
    fn file_tax_return() -> Weight;
    /// Weight of [`report_tax_evasion`] extrinsic.
    fn report_tax_evasion() -> Weight;
    /// Weight of [`freeze_organization`] extrinsic.
    fn freeze_organization() -> Weight;
    /// Weight of [`file_tax_return_zk`] extrinsic.
    fn file_tax_return_zk() -> Weight;
}

/// Placeholder weights for pallet_organization.
///
/// Reference: conservative static analysis.
/// Replace with `cargo benchmark` output before mainnet.
pub struct SubstrateWeight<T>(PhantomData<T>);

impl<T: frame_system::Config> WeightInfo for SubstrateWeight<T> {
    /// Worst-case: 3 read(s), 3 write(s).
    fn register_organization() -> Weight {
        Weight::from_parts(80_000_000, 4_096)
            .saturating_add(T::DbWeight::get().reads(3))
            .saturating_add(T::DbWeight::get().writes(3))
    }
    /// Worst-case: 1 read(s), 1 write(s).
    fn activate_organization() -> Weight {
        Weight::from_parts(25_000_000, 4_096)
            .saturating_add(T::DbWeight::get().reads(1))
            .saturating_add(T::DbWeight::get().writes(1))
    }
    /// Worst-case: 1 read(s), 1 write(s).
    fn add_crew_member() -> Weight {
        Weight::from_parts(25_000_000, 4_096)
            .saturating_add(T::DbWeight::get().reads(1))
            .saturating_add(T::DbWeight::get().writes(1))
    }
    /// Worst-case: 1 read(s), 1 write(s).
    fn file_tax_return() -> Weight {
        Weight::from_parts(25_000_000, 4_096)
            .saturating_add(T::DbWeight::get().reads(1))
            .saturating_add(T::DbWeight::get().writes(1))
    }
    /// Worst-case: 1 read(s), 1 write(s).
    fn report_tax_evasion() -> Weight {
        Weight::from_parts(25_000_000, 4_096)
            .saturating_add(T::DbWeight::get().reads(1))
            .saturating_add(T::DbWeight::get().writes(1))
    }
    /// Worst-case: 1 read(s), 1 write(s).
    fn freeze_organization() -> Weight {
        Weight::from_parts(25_000_000, 4_096)
            .saturating_add(T::DbWeight::get().reads(1))
            .saturating_add(T::DbWeight::get().writes(1))
    }
    /// Worst-case: 1 read(s), 1 write(s).
    fn file_tax_return_zk() -> Weight {
        Weight::from_parts(25_000_000, 4_096)
            .saturating_add(T::DbWeight::get().reads(1))
            .saturating_add(T::DbWeight::get().writes(1))
    }
}

/// Unit weights for tests (zero cost).
impl WeightInfo for () {
    fn register_organization() -> Weight {
        Weight::zero()
    }
    fn activate_organization() -> Weight {
        Weight::zero()
    }
    fn add_crew_member() -> Weight {
        Weight::zero()
    }
    fn file_tax_return() -> Weight {
        Weight::zero()
    }
    fn report_tax_evasion() -> Weight {
        Weight::zero()
    }
    fn freeze_organization() -> Weight {
        Weight::zero()
    }
    fn file_tax_return_zk() -> Weight {
        Weight::zero()
    }
}
