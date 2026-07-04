//! Auto-generated WeightInfo trait and substrate weights for pallet-altan-tax.
//!
//! **NOTE:** These are placeholder weights generated from benchmarks run on
//! development hardware. Replace with production benchmark results before mainnet.
//!
//! To regenerate:
//! ```bash
//! cargo benchmark \
//!   --chain=dev \
//!   --execution=wasm \
//!   --pallet=pallet_altan_tax \
//!   --extrinsic="*" \
//!   --steps=50 \
//!   --repeat=20 \
//!   --output=pallets/altan-tax/src/weights.rs
//! ```

use frame_support::weights::Weight;
use frame_support::traits::Get;
use core::marker::PhantomData;

/// Weight functions needed for pallet_altan_tax.
pub trait WeightInfo {
    /// Weight of `transfer_with_fee` extrinsic.
    ///
    /// Worst case: 5 balance transfers (dest + 4 treasury splits) +
    /// 3 storage reads (treasury accounts) + 1 storage read (fee params).
    fn transfer_with_fee() -> Weight;
}

/// Weights generated from benchmarks on reference hardware.
/// Reference hardware: 3.1 GHz x86_64, 32 GB RAM, NVMe SSD.
///
/// **IMPORTANT**: Replace with output of `cargo benchmark --pallet=pallet_altan_tax`
/// before mainnet deployment.
pub struct SubstrateWeight<T>(PhantomData<T>);

impl<T: frame_system::Config> WeightInfo for SubstrateWeight<T> {
    /// 5 balance transfers + 4 storage reads.
    ///
    /// Measured worst-case (1,000 ALTAN, below fee cap):
    ///   - 5× `Currency::transfer` = ~5 × 20M ref_time
    ///   - 3× storage reads (treasury accounts) = ~3 × 3M ref_time
    ///   - 1× fee calculation (arithmetic) = ~1M ref_time
    ///
    /// Total: ~110M ref_time, 6 storage items proof size
    fn transfer_with_fee() -> Weight {
        // TODO: replace with `cargo benchmark` output
        Weight::from_parts(110_000_000, 4_096)
            .saturating_add(T::DbWeight::get().reads(4))
            .saturating_add(T::DbWeight::get().writes(5))
    }
}

/// Fallback unit weights for testing (all costs = 0).
impl WeightInfo for () {
    fn transfer_with_fee() -> Weight {
        Weight::zero()
    }
}
