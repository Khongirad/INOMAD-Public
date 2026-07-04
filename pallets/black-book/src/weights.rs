//! WeightInfo trait and conservative placeholder weights for pallet_black_book.
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
//!   --pallet=pallet_black_book \
//!   --extrinsic="*" --steps=50 --repeat=20 \
//!   --output=pallets/black-book/src/weights.rs
//! ```

use core::marker::PhantomData;
use frame_support::traits::Get;
use frame_support::weights::Weight;

/// Weight functions needed for pallet_black_book.
pub trait WeightInfo {
    /// Weight of [`condemn_and_issue_warrant`] extrinsic.
    fn condemn_and_issue_warrant() -> Weight;
    /// Weight of [`donate_to_bounty`] extrinsic.
    fn donate_to_bounty() -> Weight;
    /// Weight of [`register_capture_and_payout`] extrinsic.
    fn register_capture_and_payout() -> Weight;
    /// Weight of [`claim_bounty_payout`] extrinsic.
    fn claim_bounty_payout() -> Weight;
    /// Weight of [`cancel_bounty_payout`] extrinsic.
    fn cancel_bounty_payout() -> Weight;
}

/// Placeholder weights for pallet_black_book.
///
/// Reference: conservative static analysis.
/// Replace with `cargo benchmark` output before mainnet.
pub struct SubstrateWeight<T>(PhantomData<T>);

impl<T: frame_system::Config> WeightInfo for SubstrateWeight<T> {
    /// Worst-case: 1 read(s), 1 write(s).
    fn condemn_and_issue_warrant() -> Weight {
        Weight::from_parts(25_000_000, 4_096)
            .saturating_add(T::DbWeight::get().reads(1))
            .saturating_add(T::DbWeight::get().writes(1))
    }
    /// Worst-case: 1 read(s), 1 write(s).
    fn donate_to_bounty() -> Weight {
        Weight::from_parts(25_000_000, 4_096)
            .saturating_add(T::DbWeight::get().reads(1))
            .saturating_add(T::DbWeight::get().writes(1))
    }
    /// Worst-case: 3 read(s), 3 write(s).
    fn register_capture_and_payout() -> Weight {
        Weight::from_parts(80_000_000, 4_096)
            .saturating_add(T::DbWeight::get().reads(3))
            .saturating_add(T::DbWeight::get().writes(3))
    }
    /// Worst-case: 1 read(s), 1 write(s).
    fn claim_bounty_payout() -> Weight {
        Weight::from_parts(25_000_000, 4_096)
            .saturating_add(T::DbWeight::get().reads(1))
            .saturating_add(T::DbWeight::get().writes(1))
    }
    /// Worst-case: 1 read(s), 1 write(s).
    fn cancel_bounty_payout() -> Weight {
        Weight::from_parts(25_000_000, 4_096)
            .saturating_add(T::DbWeight::get().reads(1))
            .saturating_add(T::DbWeight::get().writes(1))
    }
}

/// Unit weights for tests (zero cost).
impl WeightInfo for () {
    fn condemn_and_issue_warrant() -> Weight {
        Weight::zero()
    }
    fn donate_to_bounty() -> Weight {
        Weight::zero()
    }
    fn register_capture_and_payout() -> Weight {
        Weight::zero()
    }
    fn claim_bounty_payout() -> Weight {
        Weight::zero()
    }
    fn cancel_bounty_payout() -> Weight {
        Weight::zero()
    }
}
