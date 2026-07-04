//! # Altan Fee Pallet
//!
//! **Altan Network — Sovereign L1 Blockchain**
//!
//! Constitutional fee collection and routing pallet.
//! Enforces the immutable 4-way fee split per AGENTS.md §1.2:
//!
//!   - Khural Foundation  → 54%  (`KhuralFoundationAccount`) — UBI, science, indigenous peoples
//!   - INOMAD AG          → 26%  (`AgTreasuryAccount`)        — R&D, operations
//!   - Creator / Citizen #1 → 10%  (`CreatorAccount`)          — sovereign founder royalty
//!   - Validator Pool     → 10%  (`ValidatorPoolAccount`)     — block producers
//!
//! ## Exclusive Fee Math
//!
//! When `transfer_with_fee(dest, amount)` is called:
//!   - `dest` receives **exactly** `amount` ALTAN
//!   - `sender` is debited `amount + fee` (fee = 0.03% of amount, cap 1,000 ALTAN)
//!   - If sender cannot cover `amount + fee` the extrinsic fails atomically
//!
//! Tax rate: 0.03% (`amount × 3 / 10_000`).
//! Fee Cap: 1,000 ALTAN per transaction (threshold: 3,333,333 ALTAN).
//!
//! Sprint L1-02 → Sprint L1-24 (constitutional fee reform).
//!
//! ## Dispatchables
//!
//! | Call | Origin | Description |
//! |---|---|---|
//! | `transfer_with_fee` | Signed | Transfer ALTAN with automatic 13% constitutional tax deduction |

#![cfg_attr(not(feature = "std"), no_std)]

extern crate alloc;

pub mod migrations;
pub use pallet::*;

#[cfg(feature = "runtime-benchmarks")]
mod benchmarking;

#[frame_support::pallet]
pub mod pallet {
    use alloc::vec::Vec;
    use frame_support::{
        pallet_prelude::*,
        traits::{Currency, ExistenceRequirement},
        weights::Weight,
    };
    use frame_system::pallet_prelude::*;

    // =========================================================================
    // Type Aliases
    // =========================================================================

    /// Shorthand for the balance type of `T::Currency`.
    pub type BalanceOf<T> =
        <<T as Config>::Currency as Currency<<T as frame_system::Config>::AccountId>>::Balance;

    // =========================================================================
    // Pallet struct
    // =========================================================================

    #[pallet::pallet]
    pub struct Pallet<T>(_);

    // =========================================================================
    // Configuration Trait
    // =========================================================================

    #[pallet::config]
    pub trait Config: frame_system::Config<RuntimeEvent: From<Event<Self>>> {
        /// The currency used for transfers and fee routing.
        type Currency: Currency<Self::AccountId>;
    }

    // =========================================================================
    // Storage
    // =========================================================================

    /// INOMAD AG treasury account — receives 26% of every fee.
    #[pallet::storage]
    #[pallet::getter(fn ag_treasury_account)]
    pub type AgTreasuryAccount<T: Config> = StorageValue<_, T::AccountId, OptionQuery>;

    /// Khural Foundation account — receives 54% of every fee (+ mathematical dust).
    ///
    /// Internal allocation governed by 86 Academy of Sciences representatives:
    ///   - 27% for INOMAD KHURAL OS technology, infrastructure, and grants
    ///   - 27% for indigenous peoples development (culture, science, grants)
    #[pallet::storage]
    #[pallet::getter(fn khural_foundation_account)]
    pub type KhuralFoundationAccount<T: Config> = StorageValue<_, T::AccountId, OptionQuery>;

    /// Validator pool account — receives 10% of every fee.
    #[pallet::storage]
    #[pallet::getter(fn validator_pool_account)]
    pub type ValidatorPoolAccount<T: Config> = StorageValue<_, T::AccountId, OptionQuery>;

    /// The 79 Indigenous Nation treasury accounts.
    /// Used for governance proposals (pallet-khural-governance) and direct transfers.
    /// NOT used in automatic fee routing (Khural Foundation governs internally).
    /// Bounded at 150 to allow future governance expansion without a storage migration.
    #[pallet::storage]
    #[pallet::getter(fn nation_treasuries)]
    pub type NationTreasuries<T: Config> =
        StorageValue<_, BoundedVec<T::AccountId, ConstU32<150>>, ValueQuery>;

    // =========================================================================
    // Genesis Configuration
    // =========================================================================

    #[pallet::genesis_config]
    pub struct GenesisConfig<T: Config> {
        /// INOMAD AG treasury account.
        pub ag_treasury: Option<T::AccountId>,
        /// Khural Foundation account.
        pub khural_foundation: Option<T::AccountId>,
        /// Validator pool account.
        pub validator_pool: Option<T::AccountId>,
        /// 79 Indigenous Nation treasury accounts.
        pub nation_treasuries: Vec<T::AccountId>,
    }

    impl<T: Config> Default for GenesisConfig<T> {
        fn default() -> Self {
            Self {
                ag_treasury: None,
                khural_foundation: None,
                validator_pool: None,
                nation_treasuries: Vec::new(),
            }
        }
    }

    #[pallet::genesis_build]
    impl<T: Config> BuildGenesisConfig for GenesisConfig<T> {
        fn build(&self) {
            if let Some(ref acct) = self.ag_treasury {
                AgTreasuryAccount::<T>::put(acct);
            }
            if let Some(ref acct) = self.khural_foundation {
                KhuralFoundationAccount::<T>::put(acct);
            }
            if let Some(ref acct) = self.validator_pool {
                ValidatorPoolAccount::<T>::put(acct);
            }
            let bounded: BoundedVec<T::AccountId, ConstU32<150>> = self
                .nation_treasuries
                .clone()
                .try_into()
                .expect("nation_treasuries must not exceed 150 entries");
            NationTreasuries::<T>::put(bounded);
        }
    }

    // =========================================================================
    // Events
    // =========================================================================

    #[pallet::event]
    #[pallet::generate_deposit(pub(super) fn deposit_event)]
    pub enum Event<T: Config> {
        /// Constitutional fee was routed (v2: 54/36/10 split).
        ///
        /// Exclusive fee: dest received exactly `amount`. Sender paid `amount + total_fee`.
        FeeRouted {
            /// The account initiating the transfer.
            sender: T::AccountId,
            /// The ultimate recipient — received exactly `amount`.
            recipient: T::AccountId,
            /// The net amount received by dest (exclusive fee).
            amount: BalanceOf<T>,
            /// Total fee deducted FROM sender on top of `amount`.
            total_fee: BalanceOf<T>,
            /// Share sent to Khural Foundation (54% + mathematical dust).
            khural_share: BalanceOf<T>,
            /// Share sent to INOMAD AG (36% of fee — includes Creator compensation).
            ag_share: BalanceOf<T>,
            /// Share sent to Validator Pool (10% of fee).
            validator_share: BalanceOf<T>,
        },
    }

    // =========================================================================
    // Errors
    // =========================================================================

    #[pallet::error]
    pub enum Error<T> {
        /// An arithmetic operation overflowed or underflowed.
        MathOverflow,
        /// One or more treasury accounts have not been initialized at genesis.
        TreasuriesNotInitialized,
    }

    // =========================================================================
    // Extrinsics
    // =========================================================================

    #[pallet::call]
    impl<T: Config> Pallet<T> {
        /// Transfer `amount` from the caller to `dest` using **exclusive fee math**.
        ///
        /// ## Exclusive Fee Math (v2: 54/36/10 split)
        ///
        /// `dest` receives **exactly** `amount`. The sender is debited `amount + fee`.
        ///
        /// ```text
        /// fee             = min(amount × 3 / 10_000, FEE_CAP)   ← 0.03%, max 1,000 ALTAN
        /// sender debited  = amount + fee
        /// dest credited   = amount                               ← exact, no deduction
        ///
        /// Constitutional 54/36/10 split of fee (v2):
        ///   khural_share    = fee − ag_share − validator_share  (54% + dust)
        ///   ag_share        = fee × 36 / 100   → AgTreasuryAccount (INOMAD AG Swiss)
        ///   validator_share = fee × 10 / 100   → ValidatorPoolAccount
        /// ```
        ///
        /// All arithmetic is checked. All transfers use `KeepAlive` to prevent account reaping.
        /// The extrinsic fails atomically if the sender cannot cover `amount + fee`.
        #[pallet::call_index(0)]
        #[pallet::weight(Weight::from_parts(100_000_000, 0))]
        pub fn transfer_with_fee(
            origin: OriginFor<T>,
            dest: T::AccountId,
            amount: BalanceOf<T>,
        ) -> DispatchResult {
            let sender = ensure_signed(origin)?;

            // ── Fetch treasury accounts ────────────────────────────────────
            let ag_account =
                AgTreasuryAccount::<T>::get().ok_or(Error::<T>::TreasuriesNotInitialized)?;
            let khural_account =
                KhuralFoundationAccount::<T>::get().ok_or(Error::<T>::TreasuriesNotInitialized)?;
            let validator_account =
                ValidatorPoolAccount::<T>::get().ok_or(Error::<T>::TreasuriesNotInitialized)?;

            // ── Constitutional Math (Exclusive Fee) ─────────────────────────
            //
            // Fee = min(amount × 3 / 10_000, FEE_CAP).
            let raw_fee = amount
                .checked_mul(&3u32.into())
                .and_then(|v| v.checked_div(&10_000u32.into()))
                .ok_or(Error::<T>::MathOverflow)?;

            // 1,000 ALTAN fee cap
            let fee_cap: BalanceOf<T> = 1_000_000_000_000_000u128
                .try_into()
                .unwrap_or_else(|_| raw_fee);
            let fee = raw_fee.min(fee_cap);

            // Exclusive: sender pays amount + fee. Dest receives exactly amount.
            let _total_debit = amount.checked_add(&fee).ok_or(Error::<T>::MathOverflow)?;

            // Constitutional 54/36/10 split of fee (v2)
            let ag_share = fee
                .checked_mul(&36u32.into())
                .and_then(|v| v.checked_div(&100u32.into()))
                .ok_or(Error::<T>::MathOverflow)?;

            let validator_share = fee
                .checked_mul(&10u32.into())
                .and_then(|v| v.checked_div(&100u32.into()))
                .ok_or(Error::<T>::MathOverflow)?;

            // Khural Foundation gets the remainder (54% + mathematical dust).
            let khural_share = fee
                .checked_sub(&ag_share)
                .and_then(|v| v.checked_sub(&validator_share))
                .ok_or(Error::<T>::MathOverflow)?;

            // ── 4 Transfers (exclusive fee: dest gets exactly `amount`) ────
            //
            // 1. Net amount → destination
            T::Currency::transfer(&sender, &dest, amount, ExistenceRequirement::KeepAlive)?;

            // 2. AG share (36%) from sender — includes Creator compensation
            T::Currency::transfer(
                &sender,
                &ag_account,
                ag_share,
                ExistenceRequirement::KeepAlive,
            )?;

            // 3. Validator Pool (10%) from sender
            T::Currency::transfer(
                &sender,
                &validator_account,
                validator_share,
                ExistenceRequirement::KeepAlive,
            )?;

            // 4. Khural Foundation (54% + dust) from sender
            T::Currency::transfer(
                &sender,
                &khural_account,
                khural_share,
                ExistenceRequirement::KeepAlive,
            )?;

            // ── Emit Event ───────────────────────────────────────
            Self::deposit_event(Event::FeeRouted {
                sender,
                recipient: dest,
                amount,
                total_fee: fee,
                khural_share,
                ag_share,
                validator_share,
            });

            Ok(())
        }
    }
}

// =========================================================================
// Unit Tests
// =========================================================================

#[cfg(test)]
mod mock;

#[cfg(test)]
mod tests;
