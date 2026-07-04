//! WeightInfo trait and conservative placeholder weights for pallet_bank_of_siberia.
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
//!   --pallet=pallet_bank_of_siberia \
//!   --extrinsic="*" --steps=50 --repeat=20 \
//!   --output=pallets/bank-of-siberia/src/weights.rs
//! ```

use core::marker::PhantomData;
use frame_support::traits::Get;
use frame_support::weights::Weight;

/// Weight functions needed for pallet_bank_of_siberia.
pub trait WeightInfo {
    /// Weight of [`open_master_account`] extrinsic.
    fn open_master_account() -> Weight;
    /// Weight of [`open_sub_account`] extrinsic.
    fn open_sub_account() -> Weight;
    /// Weight of [`withdraw_from_savings`] extrinsic.
    fn withdraw_from_savings() -> Weight;
    /// Weight of [`pay_credit`] extrinsic.
    fn pay_credit() -> Weight;
    /// Weight of [`close_sub_account`] extrinsic.
    fn close_sub_account() -> Weight;
    /// Weight of [`request_loan`] extrinsic.
    fn request_loan() -> Weight;
    /// Weight of [`create_escrow`] extrinsic.
    fn create_escrow() -> Weight;
    /// Weight of [`release_escrow`] extrinsic.
    fn release_escrow() -> Weight;
    /// Weight of [`refund_escrow`] extrinsic.
    fn refund_escrow() -> Weight;
    /// Weight of [`open_time_deposit`] extrinsic.
    fn open_time_deposit() -> Weight;
    /// Weight of [`claim_time_deposit`] extrinsic.
    fn claim_time_deposit() -> Weight;
    /// Weight of [`approve_loan`] extrinsic.
    fn approve_loan() -> Weight;
    /// Weight of [`cancel_loan_request`] extrinsic.
    fn cancel_loan_request() -> Weight;
    /// Weight of [`fund_treasury`] extrinsic.
    fn fund_treasury() -> Weight;
}

/// Placeholder weights for pallet_bank_of_siberia.
///
/// Reference: conservative static analysis.
/// Replace with `cargo benchmark` output before mainnet.
pub struct SubstrateWeight<T>(PhantomData<T>);

impl<T: frame_system::Config> WeightInfo for SubstrateWeight<T> {
    /// Worst-case: 3 read(s), 3 write(s).
    fn open_master_account() -> Weight {
        Weight::from_parts(80_000_000, 4_096)
            .saturating_add(T::DbWeight::get().reads(3))
            .saturating_add(T::DbWeight::get().writes(3))
    }
    /// Worst-case: 3 read(s), 3 write(s).
    fn open_sub_account() -> Weight {
        Weight::from_parts(80_000_000, 4_096)
            .saturating_add(T::DbWeight::get().reads(3))
            .saturating_add(T::DbWeight::get().writes(3))
    }
    /// Worst-case: 1 read(s), 1 write(s).
    fn withdraw_from_savings() -> Weight {
        Weight::from_parts(25_000_000, 4_096)
            .saturating_add(T::DbWeight::get().reads(1))
            .saturating_add(T::DbWeight::get().writes(1))
    }
    /// Worst-case: 1 read(s), 1 write(s).
    fn pay_credit() -> Weight {
        Weight::from_parts(25_000_000, 4_096)
            .saturating_add(T::DbWeight::get().reads(1))
            .saturating_add(T::DbWeight::get().writes(1))
    }
    /// Worst-case: 1 read(s), 1 write(s).
    fn close_sub_account() -> Weight {
        Weight::from_parts(25_000_000, 4_096)
            .saturating_add(T::DbWeight::get().reads(1))
            .saturating_add(T::DbWeight::get().writes(1))
    }
    /// Worst-case: 1 read(s), 1 write(s).
    fn request_loan() -> Weight {
        Weight::from_parts(25_000_000, 4_096)
            .saturating_add(T::DbWeight::get().reads(1))
            .saturating_add(T::DbWeight::get().writes(1))
    }
    /// Worst-case: 3 read(s), 3 write(s).
    fn create_escrow() -> Weight {
        Weight::from_parts(80_000_000, 4_096)
            .saturating_add(T::DbWeight::get().reads(3))
            .saturating_add(T::DbWeight::get().writes(3))
    }
    /// Worst-case: 1 read(s), 1 write(s).
    fn release_escrow() -> Weight {
        Weight::from_parts(25_000_000, 4_096)
            .saturating_add(T::DbWeight::get().reads(1))
            .saturating_add(T::DbWeight::get().writes(1))
    }
    /// Worst-case: 1 read(s), 1 write(s).
    fn refund_escrow() -> Weight {
        Weight::from_parts(25_000_000, 4_096)
            .saturating_add(T::DbWeight::get().reads(1))
            .saturating_add(T::DbWeight::get().writes(1))
    }
    /// Worst-case: 3 read(s), 3 write(s).
    fn open_time_deposit() -> Weight {
        Weight::from_parts(80_000_000, 4_096)
            .saturating_add(T::DbWeight::get().reads(3))
            .saturating_add(T::DbWeight::get().writes(3))
    }
    /// Worst-case: 1 read(s), 1 write(s).
    fn claim_time_deposit() -> Weight {
        Weight::from_parts(25_000_000, 4_096)
            .saturating_add(T::DbWeight::get().reads(1))
            .saturating_add(T::DbWeight::get().writes(1))
    }
    /// Worst-case: 3 read(s), 4 write(s) — treasury check + transfer + status + index.
    fn approve_loan() -> Weight {
        Weight::from_parts(80_000_000, 4_096)
            .saturating_add(T::DbWeight::get().reads(3))
            .saturating_add(T::DbWeight::get().writes(4))
    }
    /// Worst-case: 2 read(s), 3 write(s) — lock remove + collateral + status.
    fn cancel_loan_request() -> Weight {
        Weight::from_parts(50_000_000, 4_096)
            .saturating_add(T::DbWeight::get().reads(2))
            .saturating_add(T::DbWeight::get().writes(3))
    }
    /// Worst-case: 1 read(s), 1 write(s) — treasury transfer.
    fn fund_treasury() -> Weight {
        Weight::from_parts(40_000_000, 4_096)
            .saturating_add(T::DbWeight::get().reads(1))
            .saturating_add(T::DbWeight::get().writes(1))
    }
}

/// Unit weights for tests (zero cost).
impl WeightInfo for () {
    fn open_master_account() -> Weight {
        Weight::zero()
    }
    fn open_sub_account() -> Weight {
        Weight::zero()
    }
    fn withdraw_from_savings() -> Weight {
        Weight::zero()
    }
    fn pay_credit() -> Weight {
        Weight::zero()
    }
    fn close_sub_account() -> Weight {
        Weight::zero()
    }
    fn request_loan() -> Weight {
        Weight::zero()
    }
    fn create_escrow() -> Weight {
        Weight::zero()
    }
    fn release_escrow() -> Weight {
        Weight::zero()
    }
    fn refund_escrow() -> Weight {
        Weight::zero()
    }
    fn open_time_deposit() -> Weight {
        Weight::zero()
    }
    fn claim_time_deposit() -> Weight {
        Weight::zero()
    }
    fn approve_loan() -> Weight {
        Weight::zero()
    }
    fn cancel_loan_request() -> Weight {
        Weight::zero()
    }
    fn fund_treasury() -> Weight {
        Weight::zero()
    }
}
