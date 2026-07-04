//! # Annual Profit Tax Pallet
//!
//! **Altan Network — Sovereign L1 Blockchain**
//!
//! Constitutional annual profit tax for sovereign organizations and citizens.
//!
//! ## Tax Rates
//!
//! | Category | Rate |
//! |---|---|
//! | Standard (organization / single / small family) | **10%** of annual net profit |
//! | Large family (3 or more children) | **5%** of annual net profit |
//!
//! ## Filing Window
//!
//! ```text
//! On-time period:  January 1 – April 15 (105 days)
//! Late period:     April 16 – December 31 (259 days)
//!
//! Filing year Y:
//!   Open:      Jan 1  Y  00:00:00 UTC
//!   On-time:   Apr 15 Y  23:59:59 UTC
//!   Late start: Apr 16 Y  00:00:00 UTC
//!   Deadline:  Dec 31 Y  23:59:59 UTC
//! ```
//!
//! ## Late Payment Penalty (Пени + Штраф)
//!
//! Two components apply when filing after April 15:
//!
//! ```text
//! 1. ШТРАФ (one-time fixed penalty): 5% of base_tax
//!
//! 2. ПЕНИ (daily accrual): 5% annual / 365 days = per-day rate
//!    days_late = days from April 16 to payment date (min 1, max 259)
//!    daily_penalty_rate = 5 / (100 × 365) per day
//!    total_peni = base_tax × 5 × days_late / 36_500
//!
//! total_late_charges = shtraf + peni
//! total_tax = base_tax + total_late_charges
//! ```
//!
//! Example (filing on July 1 = 76 days late):
//! ```text
//! profit    = 1,000,000 ALTAN
//! base_tax  = 100,000 ALTAN (10%)
//! штраф     = 5,000 ALTAN (5% flat)
//! пени      = 100,000 × 5 × 76 / 36,500 = 1,041.09 ≈ 1,041 ALTAN
//! total     = 106,041 ALTAN
//! ```
//!
//! ## Constitutional Split
//!
//! ```text
//! 70% → Regional Treasury    (region where org/citizen is registered)
//! 30% → Confederation Treasury
//! ```
//!
//! ## Large Family Benefit
//!
//! Citizens with 3+ children declare `is_large_family: true`.
//! This halves the base rate: 10% → 5%.
//! The `is_large_family` flag is self-declared here and should be
//! verified off-chain by cross-referencing `pallet-inomad-identity`
//! `children_count` before calling. A future upgrade will add an
//! automatic on-chain check via `IdentityInterface`.
//!
//! ## Dispatchables
//!
//! | Call | Origin | Description |
//! |---|---|---|
//! | `declare_annual_profit` | Signed | Submit profit + pay tax; include `is_large_family` flag |
//! | `set_regional_treasury`  | Root | Update regional treasury |
//! | `set_confederation_treasury` | Root | Update confederation treasury |

#![cfg_attr(not(feature = "std"), no_std)]

pub use pallet::*;

#[cfg(test)]
mod mock;

#[cfg(test)]
mod tests;

#[cfg(feature = "runtime-benchmarks")]
mod benchmarking;

#[frame_support::pallet]
pub mod pallet {
    use frame_support::{
        pallet_prelude::*,
        traits::{Currency, ExistenceRequirement, UnixTime},
    };
    use frame_system::pallet_prelude::*;

    // =========================================================================
    // Calendar Constants
    // =========================================================================

    /// Seconds in a non-leap year (365 × 86400).
    const SECONDS_PER_YEAR: u64 = 31_536_000;
    /// Seconds in the on-time filing window: Jan 1 → Apr 15 inclusive (105 days).
    const FILING_WINDOW_SECONDS: u64 = 105 * 86_400;
    /// Seconds from Jan 1 to the first late day (Apr 16 = 106th day).
    const LATE_START_OFFSET_SECS: u64 = 106 * 86_400;
    /// Maximum late-filing period: Apr 16 – Dec 31 = 259 days.
    const MAX_DAYS_LATE: u64 = 259;
    /// Divisor for daily peni calculation: 5% / 365 days = 5 / 36_500.
    const PENI_DIVISOR: u128 = 36_500;
    /// Numerator for daily peni calculation.
    const PENI_NUMERATOR: u128 = 5;

    /// Known anchor: Jan 1, 2026, 00:00:00 UTC = 1_735_689_600 seconds.
    const ANCHOR_YEAR: u64 = 2026;
    const ANCHOR_TIMESTAMP: u64 = 1_735_689_600;

    // =========================================================================
    // Type Aliases
    // =========================================================================

    pub type BalanceOf<T> =
        <<T as Config>::Currency as Currency<<T as frame_system::Config>::AccountId>>::Balance;

    /// Calendar year (e.g. 2026).
    pub type TaxYear = u32;

    // =========================================================================
    // Pallet Struct
    // =========================================================================

    #[pallet::pallet]
    pub struct Pallet<T>(_);

    // =========================================================================
    // Configuration Trait
    // =========================================================================

    #[pallet::config]
    pub trait Config: frame_system::Config<RuntimeEvent: From<Event<Self>>> {
        /// Currency for profit tax payments.
        type Currency: Currency<Self::AccountId>;

        /// Unix timestamp provider (pallet-timestamp).
        type UnixTime: UnixTime;

        /// Standard annual profit tax rate in per-mille (default: 100 = 10%).
        ///
        /// Constitutional mandate: adjustable via runtime upgrade only.
        #[pallet::constant]
        type StandardTaxRatePermill: Get<u32>;

        /// Reduced annual profit tax rate for large families (3+ children) in per-mille.
        ///
        /// Default: 50 = 5% (half of standard rate).
        #[pallet::constant]
        type LargeFamilyTaxRatePermill: Get<u32>;
    }

    // =========================================================================
    // Storage
    // =========================================================================

    /// Profit declarations: (org_account, year) → ProfitRecord.
    ///
    /// Prevents double-filing: each (account, year) pair can only be declared once.
    #[pallet::storage]
    #[pallet::getter(fn profit_declarations)]
    pub type ProfitDeclarations<T: Config> = StorageDoubleMap<
        _,
        Blake2_128Concat,
        T::AccountId,
        Blake2_128Concat,
        TaxYear,
        ProfitRecord<BalanceOf<T>>,
        OptionQuery,
    >;

    /// Regional Treasury account — receives 70% of annual profit tax.
    #[pallet::storage]
    #[pallet::getter(fn regional_treasury)]
    pub type RegionalTreasury<T: Config> = StorageValue<_, T::AccountId, OptionQuery>;

    /// Confederation Treasury account — receives 30% of annual profit tax.
    #[pallet::storage]
    #[pallet::getter(fn confederation_treasury)]
    pub type ConfederationTreasury<T: Config> = StorageValue<_, T::AccountId, OptionQuery>;

    // =========================================================================
    // Records
    // =========================================================================

    /// Immutable on-chain record of a single annual profit declaration + tax payment.
    #[derive(Clone, Encode, Decode, MaxEncodedLen, TypeInfo, RuntimeDebug, PartialEq)]
    pub struct ProfitRecord<Balance> {
        /// Declared net profit for the tax year.
        pub declared_profit: Balance,
        /// Base tax before penalties (profit × applicable_rate%).
        pub base_tax: Balance,
        /// One-time штраф: 5% of base_tax (zero if on-time).
        pub shtraf: Balance,
        /// Accumulated пени: 5%/year daily accrual from Apr 16 (zero if on-time).
        pub peni: Balance,
        /// Total paid: base_tax + shtraf + peni.
        pub total_paid: Balance,
        /// Regional share (70% of total_paid).
        pub regional_share: Balance,
        /// Confederation share (30% of total_paid — absorbs dust).
        pub confederation_share: Balance,
        /// Unix timestamp (seconds) at payment time.
        pub paid_at: u64,
        /// Days late (0 = on-time, 1–259 = late days from Apr 16).
        pub days_late: u32,
        /// True if the large-family reduced rate (5%) was applied.
        pub large_family_rate: bool,
    }

    // =========================================================================
    // Genesis Configuration
    // =========================================================================

    #[pallet::genesis_config]
    pub struct GenesisConfig<T: Config> {
        pub regional_treasury: Option<T::AccountId>,
        pub confederation_treasury: Option<T::AccountId>,
    }

    impl<T: Config> Default for GenesisConfig<T> {
        fn default() -> Self {
            Self {
                regional_treasury: None,
                confederation_treasury: None,
            }
        }
    }

    #[pallet::genesis_build]
    impl<T: Config> BuildGenesisConfig for GenesisConfig<T> {
        fn build(&self) {
            if let Some(ref acct) = self.regional_treasury {
                RegionalTreasury::<T>::put(acct);
            }
            if let Some(ref acct) = self.confederation_treasury {
                ConfederationTreasury::<T>::put(acct);
            }
        }
    }

    // =========================================================================
    // Events
    // =========================================================================

    #[pallet::event]
    #[pallet::generate_deposit(pub(super) fn deposit_event)]
    pub enum Event<T: Config> {
        /// Annual profit declared and tax paid.
        ProfitTaxPaid {
            org: T::AccountId,
            year: TaxYear,
            declared_profit: BalanceOf<T>,
            base_tax: BalanceOf<T>,
            shtraf: BalanceOf<T>,
            peni: BalanceOf<T>,
            total_paid: BalanceOf<T>,
            regional_share: BalanceOf<T>,
            confederation_share: BalanceOf<T>,
            days_late: u32,
            large_family_rate: bool,
        },
        /// Regional treasury updated by Root.
        RegionalTreasuryUpdated { new_account: T::AccountId },
        /// Confederation treasury updated by Root.
        ConfederationTreasuryUpdated { new_account: T::AccountId },
    }

    // =========================================================================
    // Errors
    // =========================================================================

    #[pallet::error]
    pub enum Error<T> {
        /// Treasury accounts not initialized at genesis.
        TreasuriesNotInitialized,
        /// This (org, year) combination has already been declared.
        AlreadyDeclared,
        /// Declared profit is zero — nothing to tax.
        ZeroProfitDeclared,
        /// Arithmetic overflow in tax or split calculation.
        MathOverflow,
        /// The filing year is in the future (cannot pre-file next year).
        FutureYearNotAllowed,
        /// Cannot declare for a year earlier than 2026 (genesis year).
        YearBeforeGenesis,
        /// Filing deadline December 31 has passed — tax authority takes over.
        FilingDeadlineExpired,
    }

    // =========================================================================
    // Calendar Helpers
    // =========================================================================

    impl<T: Config> Pallet<T> {
        /// Return the current Unix timestamp in **seconds**.
        pub fn now_secs() -> u64 {
            T::UnixTime::now().as_secs()
        }

        /// Unix timestamp of January 1, 00:00:00 UTC for a given `year`.
        ///
        /// Anchor: Jan 1, 2026 = 1_735_689_600 s (verified against UTC epoch).
        pub fn year_start_secs(year: TaxYear) -> u64 {
            let y = year as u64;
            let a = ANCHOR_YEAR;
            if y >= a {
                ANCHOR_TIMESTAMP + (y - a) * SECONDS_PER_YEAR
            } else {
                ANCHOR_TIMESTAMP.saturating_sub((a - y) * SECONDS_PER_YEAR)
            }
        }

        /// Returns `true` if `now` is within Jan 1 – Apr 15 of `tax_year`.
        pub fn is_on_time(now: u64, tax_year: TaxYear) -> bool {
            let start = Self::year_start_secs(tax_year);
            let end = start + FILING_WINDOW_SECONDS; // Apr 15 23:59:59
            now >= start && now <= end
        }

        /// Returns the current calendar year from the Unix timestamp.
        pub fn current_year(now: u64) -> TaxYear {
            let elapsed = if now >= ANCHOR_TIMESTAMP {
                (now - ANCHOR_TIMESTAMP) / SECONDS_PER_YEAR
            } else {
                0
            };
            (ANCHOR_YEAR + elapsed) as TaxYear
        }

        /// Compute the number of late days (1–259) for a payment made at `now`
        /// for `tax_year`. Returns 0 if on-time, capped at MAX_DAYS_LATE (259).
        ///
        /// Day 1 = April 16 00:00:00 UTC.
        /// Day 259 = December 31 23:59:59 UTC.
        pub fn days_late(now: u64, tax_year: TaxYear) -> u64 {
            let year_start = Self::year_start_secs(tax_year);
            let late_start = year_start + LATE_START_OFFSET_SECS; // Apr 16 00:00:00

            if now < late_start {
                return 0; // on-time
            }

            // How many complete days past Apr 16?
            let seconds_late = now.saturating_sub(late_start);
            let days = seconds_late / 86_400 + 1; // day 1 = first 24h of Apr 16
            days.min(MAX_DAYS_LATE)
        }

        /// Compute пени (daily accrual penalty).
        ///
        /// ```text
        /// peni = base_tax × 5 × days_late / 36_500
        /// ```
        ///
        /// This is 5% annual rate prorated to daily: 5% / 365 = 1/730 per day.
        pub fn compute_peni(base_tax: BalanceOf<T>, days_late: u64) -> Option<BalanceOf<T>> {
            if days_late == 0 {
                return Some(BalanceOf::<T>::from(0u32));
            }
            // base_tax × PENI_NUMERATOR × days_late / PENI_DIVISOR
            // All in u128 via Balance. Intermediate: max ~2.1e12 * 1e12 * 5 * 259 ≈ 2.7e27 < u128
            let days = BalanceOf::<T>::from(days_late as u32);
            let num = BalanceOf::<T>::from(PENI_NUMERATOR as u32);
            let den = BalanceOf::<T>::from(PENI_DIVISOR as u32);
            base_tax
                .checked_mul(&num)
                .and_then(|v| v.checked_mul(&days))
                .and_then(|v| v.checked_div(&den))
        }

        /// Compute штраф (fixed 5% one-time penalty on base tax).
        pub fn compute_shtraf(base_tax: BalanceOf<T>) -> Option<BalanceOf<T>> {
            base_tax
                .checked_mul(&BalanceOf::<T>::from(5u32))
                .and_then(|v| v.checked_div(&BalanceOf::<T>::from(100u32)))
        }
    }

    // =========================================================================
    // Extrinsics
    // =========================================================================

    #[pallet::call]
    impl<T: Config> Pallet<T> {
        /// Declare annual net profit for `tax_year` and pay the constitutional tax.
        ///
        /// ## Parameters
        ///
        /// - `tax_year`: Year being declared (must be ≥ 2026, ≤ current year).
        /// - `declared_profit`: Net annual profit in planck.
        /// - `is_large_family`: If true, 5% reduced rate applies (3+ children).
        ///   Self-declared; cross-check with `pallet-inomad-identity` off-chain.
        ///
        /// ## Tax Calculation
        ///
        /// ```text
        /// rate      = 10% (standard) or 5% (large family)
        /// base_tax  = declared_profit × rate
        ///
        /// IF on-time (Jan 1 – Apr 15):
        ///   total = base_tax
        ///
        /// IF late (Apr 16 – Dec 31):
        ///   shtraf = base_tax × 5%               ← one-time fixed penalty
        ///   peni   = base_tax × 5% / 365 × days  ← daily accrual
        ///   total  = base_tax + shtraf + peni
        ///
        /// regional_share      = total × 70%
        /// confederation_share = total × 30% (+ dust)
        /// ```
        #[pallet::call_index(0)]
        #[pallet::weight(Weight::from_parts(200_000_000, 0))]
        pub fn declare_annual_profit(
            origin: OriginFor<T>,
            tax_year: TaxYear,
            declared_profit: BalanceOf<T>,
            is_large_family: bool,
        ) -> DispatchResult {
            let org = ensure_signed(origin)?;

            // ── Fetch treasury accounts ──────────────────────────────────────
            let regional_account =
                RegionalTreasury::<T>::get().ok_or(Error::<T>::TreasuriesNotInitialized)?;
            let confed_account =
                ConfederationTreasury::<T>::get().ok_or(Error::<T>::TreasuriesNotInitialized)?;

            // ── Validate inputs ──────────────────────────────────────────────
            ensure!(
                declared_profit > BalanceOf::<T>::from(0u32),
                Error::<T>::ZeroProfitDeclared
            );

            let now = Self::now_secs();
            let current_year = Self::current_year(now);

            ensure!(tax_year <= current_year, Error::<T>::FutureYearNotAllowed);
            ensure!(tax_year >= 2026, Error::<T>::YearBeforeGenesis);

            // Block filing after December 31 (enforcement moves to tax authority)
            let year_end = Self::year_start_secs(tax_year) + SECONDS_PER_YEAR;
            ensure!(now < year_end, Error::<T>::FilingDeadlineExpired);

            ensure!(
                ProfitDeclarations::<T>::get(&org, tax_year).is_none(),
                Error::<T>::AlreadyDeclared
            );

            // ── Select applicable tax rate ───────────────────────────────────
            let rate_permill = if is_large_family {
                T::LargeFamilyTaxRatePermill::get()
            } else {
                T::StandardTaxRatePermill::get()
            };

            // ── Base tax = profit × rate / 1000 ─────────────────────────────
            let base_tax = declared_profit
                .checked_mul(&BalanceOf::<T>::from(rate_permill))
                .and_then(|v| v.checked_div(&BalanceOf::<T>::from(1000u32)))
                .ok_or(Error::<T>::MathOverflow)?;

            // ── Late penalties ───────────────────────────────────────────────
            let days_late_count = Self::days_late(now, tax_year);
            let on_time = days_late_count == 0;

            let shtraf = if !on_time {
                Self::compute_shtraf(base_tax).ok_or(Error::<T>::MathOverflow)?
            } else {
                BalanceOf::<T>::from(0u32)
            };

            let peni =
                Self::compute_peni(base_tax, days_late_count).ok_or(Error::<T>::MathOverflow)?;

            let total_paid = base_tax
                .checked_add(&shtraf)
                .and_then(|v| v.checked_add(&peni))
                .ok_or(Error::<T>::MathOverflow)?;

            // ── 70/30 Split (regional gets remainder to absorb dust) ─────────
            let regional_share = total_paid
                .checked_mul(&BalanceOf::<T>::from(70u32))
                .and_then(|v| v.checked_div(&BalanceOf::<T>::from(100u32)))
                .ok_or(Error::<T>::MathOverflow)?;

            let confederation_share = total_paid
                .checked_sub(&regional_share)
                .ok_or(Error::<T>::MathOverflow)?;

            // ── 2 Transfers ──────────────────────────────────────────────────
            T::Currency::transfer(
                &org,
                &regional_account,
                regional_share,
                ExistenceRequirement::KeepAlive,
            )?;
            T::Currency::transfer(
                &org,
                &confed_account,
                confederation_share,
                ExistenceRequirement::KeepAlive,
            )?;

            // ── Store Declaration ────────────────────────────────────────────
            ProfitDeclarations::<T>::insert(
                &org,
                tax_year,
                ProfitRecord {
                    declared_profit,
                    base_tax,
                    shtraf,
                    peni,
                    total_paid,
                    regional_share,
                    confederation_share,
                    paid_at: now,
                    days_late: days_late_count as u32,
                    large_family_rate: is_large_family,
                },
            );

            // ── Emit Event ───────────────────────────────────────────────────
            Self::deposit_event(Event::ProfitTaxPaid {
                org,
                year: tax_year,
                declared_profit,
                base_tax,
                shtraf,
                peni,
                total_paid,
                regional_share,
                confederation_share,
                days_late: days_late_count as u32,
                large_family_rate: is_large_family,
            });

            Ok(())
        }

        /// Update the regional treasury account. Root only.
        #[pallet::call_index(1)]
        #[pallet::weight(Weight::from_parts(10_000_000, 0))]
        pub fn set_regional_treasury(
            origin: OriginFor<T>,
            new_account: T::AccountId,
        ) -> DispatchResult {
            ensure_root(origin)?;
            RegionalTreasury::<T>::put(&new_account);
            Self::deposit_event(Event::RegionalTreasuryUpdated { new_account });
            Ok(())
        }

        /// Update the confederation treasury account. Root only.
        #[pallet::call_index(2)]
        #[pallet::weight(Weight::from_parts(10_000_000, 0))]
        pub fn set_confederation_treasury(
            origin: OriginFor<T>,
            new_account: T::AccountId,
        ) -> DispatchResult {
            ensure_root(origin)?;
            ConfederationTreasury::<T>::put(&new_account);
            Self::deposit_event(Event::ConfederationTreasuryUpdated { new_account });
            Ok(())
        }
    }
}
