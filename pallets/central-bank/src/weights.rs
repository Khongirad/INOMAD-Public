//! WeightInfo trait and conservative placeholder weights for pallet_central_bank.
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
//!   --pallet=pallet_central_bank \
//!   --extrinsic="*" --steps=50 --repeat=20 \
//!   --output=pallets/central-bank/src/weights.rs
//! ```

use core::marker::PhantomData;
use frame_support::traits::Get;
use frame_support::weights::Weight;

/// Weight functions needed for pallet_central_bank.
pub trait WeightInfo {
    /// Weight of [`grant_operator_license`] extrinsic.
    fn grant_operator_license() -> Weight;
    /// Weight of [`revoke_operator_license`] extrinsic.
    fn revoke_operator_license() -> Weight;
    /// Weight of [`mint_to_operator`] extrinsic.
    fn mint_to_operator() -> Weight;
    /// Weight of [`burn`] extrinsic.
    fn burn() -> Weight;
    /// Weight of [`set_current_rate`] extrinsic.
    fn set_current_rate() -> Weight;
    /// Weight of [`set_next_epoch_rate`] extrinsic.
    fn set_next_epoch_rate() -> Weight;
    /// Weight of [`request_credit`] extrinsic.
    fn request_credit() -> Weight;
    /// Weight of [`repay_credit`] extrinsic.
    fn repay_credit() -> Weight;
    /// Weight of [`transition_epoch`] extrinsic.
    fn transition_epoch() -> Weight;
    /// Weight of [`set_base_rate`] extrinsic.
    fn set_base_rate() -> Weight;
    /// Weight of [`issue_genesis_grant`] extrinsic.
    fn issue_genesis_grant() -> Weight;
}

/// Placeholder weights for pallet_central_bank.
///
/// Reference: conservative static analysis.
/// Replace with `cargo benchmark` output before mainnet.
pub struct SubstrateWeight<T>(PhantomData<T>);

impl<T: frame_system::Config> WeightInfo for SubstrateWeight<T> {
    /// Worst-case: 1 read(s), 1 write(s).
    fn grant_operator_license() -> Weight {
        Weight::from_parts(25_000_000, 4_096)
            .saturating_add(T::DbWeight::get().reads(1))
            .saturating_add(T::DbWeight::get().writes(1))
    }
    /// Worst-case: 1 read(s), 1 write(s).
    fn revoke_operator_license() -> Weight {
        Weight::from_parts(25_000_000, 4_096)
            .saturating_add(T::DbWeight::get().reads(1))
            .saturating_add(T::DbWeight::get().writes(1))
    }
    /// Worst-case: 3 read(s), 3 write(s).
    fn mint_to_operator() -> Weight {
        Weight::from_parts(80_000_000, 4_096)
            .saturating_add(T::DbWeight::get().reads(3))
            .saturating_add(T::DbWeight::get().writes(3))
    }
    /// Worst-case: 1 read(s), 1 write(s).
    fn burn() -> Weight {
        Weight::from_parts(25_000_000, 4_096)
            .saturating_add(T::DbWeight::get().reads(1))
            .saturating_add(T::DbWeight::get().writes(1))
    }
    /// Worst-case: 1 read(s), 1 write(s).
    fn set_current_rate() -> Weight {
        Weight::from_parts(25_000_000, 4_096)
            .saturating_add(T::DbWeight::get().reads(1))
            .saturating_add(T::DbWeight::get().writes(1))
    }
    /// Worst-case: 1 read(s), 1 write(s).
    fn set_next_epoch_rate() -> Weight {
        Weight::from_parts(25_000_000, 4_096)
            .saturating_add(T::DbWeight::get().reads(1))
            .saturating_add(T::DbWeight::get().writes(1))
    }
    /// Worst-case: 1 read(s), 1 write(s).
    fn request_credit() -> Weight {
        Weight::from_parts(25_000_000, 4_096)
            .saturating_add(T::DbWeight::get().reads(1))
            .saturating_add(T::DbWeight::get().writes(1))
    }
    /// Worst-case: 1 read(s), 1 write(s).
    fn repay_credit() -> Weight {
        Weight::from_parts(25_000_000, 4_096)
            .saturating_add(T::DbWeight::get().reads(1))
            .saturating_add(T::DbWeight::get().writes(1))
    }
    /// Worst-case: 3 read(s), 3 write(s) — epoch close + new epoch create + rate update.
    fn transition_epoch() -> Weight {
        Weight::from_parts(50_000_000, 4_096)
            .saturating_add(T::DbWeight::get().reads(3))
            .saturating_add(T::DbWeight::get().writes(3))
    }
    /// Worst-case: 2 read(s), 2 write(s) — rate storage + epoch update.
    fn set_base_rate() -> Weight {
        Weight::from_parts(30_000_000, 4_096)
            .saturating_add(T::DbWeight::get().reads(2))
            .saturating_add(T::DbWeight::get().writes(2))
    }
    /// Worst-case: 2 read(s), 2 write(s) — deposit_creating + TotalEmitted update.
    fn issue_genesis_grant() -> Weight {
        Weight::from_parts(60_000_000, 4_096)
            .saturating_add(T::DbWeight::get().reads(2))
            .saturating_add(T::DbWeight::get().writes(2))
    }
}

/// Unit weights for tests (zero cost).
impl WeightInfo for () {
    fn grant_operator_license() -> Weight {
        Weight::zero()
    }
    fn revoke_operator_license() -> Weight {
        Weight::zero()
    }
    fn mint_to_operator() -> Weight {
        Weight::zero()
    }
    fn burn() -> Weight {
        Weight::zero()
    }
    fn set_current_rate() -> Weight {
        Weight::zero()
    }
    fn set_next_epoch_rate() -> Weight {
        Weight::zero()
    }
    fn request_credit() -> Weight {
        Weight::zero()
    }
    fn repay_credit() -> Weight {
        Weight::zero()
    }
    fn transition_epoch() -> Weight {
        Weight::zero()
    }
    fn set_base_rate() -> Weight {
        Weight::zero()
    }
    fn issue_genesis_grant() -> Weight {
        Weight::zero()
    }
}
