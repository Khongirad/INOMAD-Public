//! WeightInfo trait and conservative placeholder weights for pallet_bank_operator.
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
//!   --pallet=pallet_bank_operator \
//!   --extrinsic="*" --steps=50 --repeat=20 \
//!   --output=pallets/bank-operator/src/weights.rs
//! ```

use core::marker::PhantomData;
use frame_support::traits::Get;
use frame_support::weights::Weight;

/// Weight functions needed for pallet_bank_operator.
pub trait WeightInfo {
    /// Weight of [`set_bank_account`] extrinsic.
    fn set_bank_account() -> Weight;
    /// Weight of [`assess_rwa`] extrinsic.
    fn assess_rwa() -> Weight;
    /// Weight of [`lock_collateral`] extrinsic.
    fn lock_collateral() -> Weight;
    /// Weight of [`request_credit`] extrinsic.
    fn request_credit() -> Weight;
    /// Weight of [`issue_credit`] extrinsic.
    fn issue_credit() -> Weight;
    /// Weight of [`accrue_interest`] extrinsic.
    fn accrue_interest() -> Weight;
    /// Weight of [`repay_credit`] extrinsic.
    fn repay_credit() -> Weight;
    /// Weight of [`declare_default`] extrinsic.
    fn declare_default() -> Weight;
    /// Weight of [`request_account_freeze`] extrinsic.
    fn request_account_freeze() -> Weight;
    /// Weight of [`calculate_accrued_interest`] extrinsic.
    fn calculate_accrued_interest() -> Weight;
    /// Weight of [`total_outstanding_debt_for`] extrinsic.
    fn total_outstanding_debt_for() -> Weight;
}

/// Placeholder weights for pallet_bank_operator.
///
/// Reference: conservative static analysis.
/// Replace with `cargo benchmark` output before mainnet.
pub struct SubstrateWeight<T>(PhantomData<T>);

impl<T: frame_system::Config> WeightInfo for SubstrateWeight<T> {
    /// Worst-case: 1 read(s), 1 write(s).
    fn set_bank_account() -> Weight {
        Weight::from_parts(25_000_000, 4_096)
            .saturating_add(T::DbWeight::get().reads(1))
            .saturating_add(T::DbWeight::get().writes(1))
    }
    /// Worst-case: 1 read(s), 1 write(s).
    fn assess_rwa() -> Weight {
        Weight::from_parts(25_000_000, 4_096)
            .saturating_add(T::DbWeight::get().reads(1))
            .saturating_add(T::DbWeight::get().writes(1))
    }
    /// Worst-case: 1 read(s), 1 write(s).
    fn lock_collateral() -> Weight {
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
    /// Worst-case: 3 read(s), 3 write(s).
    fn issue_credit() -> Weight {
        Weight::from_parts(80_000_000, 4_096)
            .saturating_add(T::DbWeight::get().reads(3))
            .saturating_add(T::DbWeight::get().writes(3))
    }
    /// Worst-case: 1 read(s), 1 write(s).
    fn accrue_interest() -> Weight {
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
    /// Worst-case: 1 read(s), 1 write(s).
    fn declare_default() -> Weight {
        Weight::from_parts(25_000_000, 4_096)
            .saturating_add(T::DbWeight::get().reads(1))
            .saturating_add(T::DbWeight::get().writes(1))
    }
    /// Worst-case: 1 read(s), 1 write(s).
    fn request_account_freeze() -> Weight {
        Weight::from_parts(25_000_000, 4_096)
            .saturating_add(T::DbWeight::get().reads(1))
            .saturating_add(T::DbWeight::get().writes(1))
    }
    /// Worst-case: 1 read(s), 0 write(s).
    fn calculate_accrued_interest() -> Weight {
        Weight::from_parts(5_000_000, 4_096).saturating_add(T::DbWeight::get().reads(1))
    }
    /// Worst-case: 1 read(s), 0 write(s).
    fn total_outstanding_debt_for() -> Weight {
        Weight::from_parts(5_000_000, 4_096).saturating_add(T::DbWeight::get().reads(1))
    }
}

/// Unit weights for tests (zero cost).
impl WeightInfo for () {
    fn set_bank_account() -> Weight {
        Weight::zero()
    }
    fn assess_rwa() -> Weight {
        Weight::zero()
    }
    fn lock_collateral() -> Weight {
        Weight::zero()
    }
    fn request_credit() -> Weight {
        Weight::zero()
    }
    fn issue_credit() -> Weight {
        Weight::zero()
    }
    fn accrue_interest() -> Weight {
        Weight::zero()
    }
    fn repay_credit() -> Weight {
        Weight::zero()
    }
    fn declare_default() -> Weight {
        Weight::zero()
    }
    fn request_account_freeze() -> Weight {
        Weight::zero()
    }
    fn calculate_accrued_interest() -> Weight {
        Weight::zero()
    }
    fn total_outstanding_debt_for() -> Weight {
        Weight::zero()
    }
}
