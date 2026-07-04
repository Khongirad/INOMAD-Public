//! WeightInfo trait and conservative placeholder weights for pallet_shielded_vaults.
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
//!   --pallet=pallet_shielded_vaults \
//!   --extrinsic="*" --steps=50 --repeat=20 \
//!   --output=pallets/shielded-vaults/src/weights.rs
//! ```

use core::marker::PhantomData;
use frame_support::traits::Get;
use frame_support::weights::Weight;

/// Weight functions needed for pallet_shielded_vaults.
pub trait WeightInfo {
    /// Weight of [`shield_funds`] extrinsic.
    fn shield_funds() -> Weight;
    /// Weight of [`shielded_transfer`] extrinsic.
    fn shielded_transfer() -> Weight;
    /// Weight of [`unshield_to_account`] extrinsic.
    fn unshield_to_account() -> Weight;
    /// Weight of [`org_unshield_tax_payment`] extrinsic.
    fn org_unshield_tax_payment() -> Weight;
}

/// Placeholder weights for pallet_shielded_vaults.
///
/// Reference: conservative static analysis.
/// Replace with `cargo benchmark` output before mainnet.
pub struct SubstrateWeight<T>(PhantomData<T>);

impl<T: frame_system::Config> WeightInfo for SubstrateWeight<T> {
    /// Worst-case: 1 read(s), 1 write(s).
    fn shield_funds() -> Weight {
        Weight::from_parts(25_000_000, 4_096)
            .saturating_add(T::DbWeight::get().reads(1))
            .saturating_add(T::DbWeight::get().writes(1))
    }
    /// Worst-case: 1 read(s), 1 write(s).
    fn shielded_transfer() -> Weight {
        Weight::from_parts(25_000_000, 4_096)
            .saturating_add(T::DbWeight::get().reads(1))
            .saturating_add(T::DbWeight::get().writes(1))
    }
    /// Worst-case: 1 read(s), 1 write(s).
    fn unshield_to_account() -> Weight {
        Weight::from_parts(25_000_000, 4_096)
            .saturating_add(T::DbWeight::get().reads(1))
            .saturating_add(T::DbWeight::get().writes(1))
    }
    /// Worst-case: 1 read(s), 1 write(s).
    fn org_unshield_tax_payment() -> Weight {
        Weight::from_parts(25_000_000, 4_096)
            .saturating_add(T::DbWeight::get().reads(1))
            .saturating_add(T::DbWeight::get().writes(1))
    }
}

/// Unit weights for tests (zero cost).
impl WeightInfo for () {
    fn shield_funds() -> Weight {
        Weight::zero()
    }
    fn shielded_transfer() -> Weight {
        Weight::zero()
    }
    fn unshield_to_account() -> Weight {
        Weight::zero()
    }
    fn org_unshield_tax_payment() -> Weight {
        Weight::zero()
    }
}
