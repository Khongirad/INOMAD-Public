//! pallet-inheritance — Altan Network: Heritage Institute & Digital Notary
//!
//! Implements the Институт Наследства и Цифрового Нотариата.
//!
//! PURPOSE:
//!   When a citizen dies (`register_death` in pallet-inomad-identity), their
//!   free balance is reserved (frozen), preventing "dead soul" transactions.
//!   This pallet resolves those frozen funds according to a notarized Will,
//!   creating professional employment for Notary Guild members.
//!
//! WORKFLOW:
//!   1. Citizen drafts a Will (heirs + percentage shares, must sum to 100%).
//!   2. A licensed Notary (Professional/Master in the Notary Guild) certifies it.
//!   3. Anyone calls `execute_will` after the citizen is registered as Deceased.
//!   4. All reserved funds are distributed to heirs per their shares.
//!   5. If no notarized Will exists, funds fall back to StateTreasury.
//!
//! CONSTITUTIONAL INTEGRATION:
//!   - Notary verification: delegates to pallet-guilds via GuildsNotaryInterface.
//!   - Deceased status check: delegates to pallet-inomad-identity via
//!     IdentityInheritanceInterface.
//!   - Fund release: uses ReservableCurrency::unreserve + Currency::transfer.
//!
//! ## Dispatchables
//!
//! | Call | Origin | Description |
//! |---|---|---|
//! | `draft_will` | Signed (Citizen) | Create and publish a last will testament on-chain |
//! | `notarize_will` | Signed (Notary) | Notarize a citizen's will (required for execution) |
//! | `execute_will` | Signed (Executor/Notary) | Execute a notarized will after death registration |
//! | `trigger_inheritance` | Root (Chronicle hook) | Automatically trigger inheritance after death event |

#![cfg_attr(not(feature = "std"), no_std)]

extern crate alloc;

pub mod migrations;
pub mod weights;
pub use pallet::*;

#[cfg(test)]
mod mock;
#[cfg(test)]
mod tests;

// =========================================================================
// Cross-pallet Interfaces
// =========================================================================

/// Cross-pallet trait: check whether an account is a qualified professional
/// notary (Professional or Master in the designated Notary Guild).
///
/// Implement in the runtime glue (configs/mod.rs) — reads from pallet-guilds
/// storage. This keeps pallet-inheritance loosely coupled from pallet-guilds.
pub trait GuildsNotaryInterface<AccountId> {
    /// Returns `true` if `who` is a Professional or Master in the Notary Guild.
    fn is_valid_notary(who: &AccountId) -> bool;
}

/// Cross-pallet trait: interact with pallet-inomad-identity for inheritance.
///
/// Implement in the runtime glue (configs/mod.rs).
pub trait IdentityInheritanceInterface<AccountId> {
    /// Returns `true` if the citizen's `CitizenStatus` is `Deceased`.
    fn is_deceased(who: &AccountId) -> bool;
}

/// Cross-pallet trait: query and liquidate outstanding CDP debt from pallet-bank-operator.
///
/// [SECURITY VECTOR 1] — Dead Debt Liquidation.
///
/// Implement in the runtime glue (configs/mod.rs) by scanning
/// `pallet_bank_operator::CreditContracts` for Active credits owned by `who`.
///
/// Returns `u128` planck units (the universal balance denomination in the Altan runtime).
/// This avoids the need for a `Balance` type-parameter inside the FRAME Config macro.
pub trait BankDebtInterface<AccountId> {
    /// Returns the total outstanding (unpaid) CDP debt for `who` in planck units.
    ///
    /// Returns `0` if the citizen has no active CDP debts.
    fn total_outstanding_debt(who: &AccountId) -> u128;
}

#[cfg(feature = "runtime-benchmarks")]
mod benchmarking;
#[frame_support::pallet]
pub mod pallet {
    use crate::weights::WeightInfo as _;
    use alloc::vec::Vec;
    use frame_support::{
        pallet_prelude::*,
        traits::{Currency, ExistenceRequirement, ReservableCurrency},
    };
    use frame_system::pallet_prelude::*;
    use sp_runtime::traits::SaturatedConversion;

    use crate::{BankDebtInterface, GuildsNotaryInterface, IdentityInheritanceInterface};
    use sp_runtime::{traits::Zero, Saturating};

    // =========================================================================
    // Currency helper type
    // =========================================================================

    pub type BalanceOf<T> =
        <<T as Config>::Currency as Currency<<T as frame_system::Config>::AccountId>>::Balance;

    // =========================================================================
    // Pallet
    // =========================================================================

    #[pallet::pallet]
    pub struct Pallet<T>(_);

    // =========================================================================
    // Config
    // =========================================================================

    #[pallet::config]
    pub trait Config: frame_system::Config<RuntimeEvent: From<Event<Self>>> {
        /// Weight information for extrinsics in this pallet.
        type WeightInfo: crate::weights::WeightInfo;
        /// The currency for balance reservation and distribution.
        ///
        /// Must be the same instance used in pallet-inomad-identity (Balances).
        type Currency: Currency<Self::AccountId> + ReservableCurrency<Self::AccountId>;

        /// Cross-pallet bridge to pallet-guilds for notary validation.
        ///
        /// Implement in runtime/configs/mod.rs as `GuildsBridge`.
        type GuildsChecker: GuildsNotaryInterface<Self::AccountId>;

        /// Cross-pallet bridge to pallet-inomad-identity for death status.
        ///
        /// Implement in runtime/configs/mod.rs as `IdentityInheritanceBridge`.
        type IdentityChecker: IdentityInheritanceInterface<Self::AccountId>;

        /// [SECURITY VECTOR 1] Cross-pallet bridge to pallet-bank-operator for
        /// outstanding CDP debt queries.
        ///
        /// Implement in runtime/configs/mod.rs as `BankDebtBridge`.
        /// Before distributing inheritance, the pallet will query this bridge for
        /// any unpaid credit and liquidate it from the deceased's reserved balance,
        /// preventing ghost liabilities from persisting after death.
        type BankInterface: crate::BankDebtInterface<Self::AccountId>;

        /// The account to receive assets when no valid notarized Will exists.
        ///
        /// Configure to the Confederation Treasury or a dedicated State Escrow account.
        #[pallet::constant]
        type StateTreasury: Get<Self::AccountId>;

        /// Maximum number of heirs in a single Will.
        ///
        /// Bounded to prevent unbounded storage and gas usage in `execute_will`.
        #[pallet::constant]
        type MaxHeirs: Get<u32>;
    }

    // =========================================================================
    // Types
    // =========================================================================

    /// A citizen's last Will and Testament.
    ///
    /// `heirs` is a bounded list of `(AccountId, percent)` pairs where
    /// `percent` is 0–100 and all percents must sum to exactly 100.
    ///
    /// `is_notarized` is set to `true` only by a qualified Notary calling
    /// `notarize_will`. Un-notarized wills are NOT executed — funds go to
    /// the StateTreasury instead.
    #[derive(
        Encode,
        Decode,
        DecodeWithMemTracking,
        Clone,
        PartialEq,
        Eq,
        RuntimeDebug,
        TypeInfo,
        MaxEncodedLen,
    )]
    #[scale_info(skip_type_params(T))]
    pub struct Will<T: Config> {
        /// List of (heir account, share in percent 0–100).
        pub heirs: BoundedVec<(T::AccountId, u8), T::MaxHeirs>,
        /// Whether this Will has been certified by a licensed Notary.
        pub is_notarized: bool,
        /// The Notary who certified this Will (None until notarized).
        pub notary: Option<T::AccountId>,
    }

    // =========================================================================
    // Storage
    // =========================================================================

    /// Registry of active Wills: citizen AccountId → Will.
    ///
    /// A citizen may update their Draft Will at any time (before notarization
    /// or by drafting a fresh one after revocation). After notarization, the
    /// Will can only be superseded by a new `draft_will` call, which resets
    /// `is_notarized` to `false` and clears the `notary` field.
    #[pallet::storage]
    #[pallet::getter(fn active_wills)]
    pub type ActiveWills<T: Config> =
        StorageMap<_, Blake2_128Concat, T::AccountId, Will<T>, OptionQuery>;

    // =========================================================================
    // Events
    // =========================================================================

    #[pallet::event]
    #[pallet::generate_deposit(pub(super) fn deposit_event)]
    pub enum Event<T: Config> {
        /// A citizen drafted (or re-drafted) a Will.
        WillDrafted {
            citizen: T::AccountId,
            heir_count: u32,
        },
        /// A Notary certified a citizen's Will and received their fee.
        WillNotarized {
            citizen: T::AccountId,
            notary: T::AccountId,
            fee: BalanceOf<T>,
        },
        /// A Will was executed — reserved funds distributed to heirs.
        WillExecuted {
            deceased: T::AccountId,
            total_distributed: BalanceOf<T>,
        },
        /// No notarized Will found — reserved funds sent to StateTreasury.
        FallbackToTreasury {
            deceased: T::AccountId,
            amount: BalanceOf<T>,
        },
        /// [SECURITY VECTOR 1] CDP debt liquidated from deceased's reserved balance
        /// before inheritance distribution. The `debt_settled` amount was burned
        /// (dropped as NegativeImbalance), reducing TotalIssuance permanently.
        /// Heirs receive only the `remainder` after debt settlement.
        DeadDebtLiquidated {
            deceased: T::AccountId,
            /// Amount of CDP debt burned from the reserved balance.
            debt_settled: BalanceOf<T>,
            /// Remaining reserved balance available for heir distribution.
            remainder: BalanceOf<T>,
        },
    }

    // =========================================================================
    // Errors
    // =========================================================================

    #[pallet::error]
    pub enum Error<T> {
        /// The declared heir shares do not sum to exactly 100%.
        HeirsShareNot100,
        /// The heirs list is empty.
        NoHeirsDeclared,
        /// Caller is not a Professional or Master in the Notary Guild.
        NotValidNotary,
        /// The target citizen's status is not `Deceased`.
        CitizenNotDeceased,
        /// No Will found for the deceased citizen.
        NoActiveWill,
        /// The notary fee transfer failed (insufficient balance in citizen's account).
        InsufficientFeeBalance,
        /// The heirs list exceeds the `MaxHeirs` bound.
        TooManyHeirs,
        /// A citizen cannot notarize their own Will.
        CannotNotarizeSelf,
    }

    // =========================================================================
    // Extrinsics
    // =========================================================================

    #[pallet::call]
    impl<T: Config> Pallet<T> {
        // ─── draft_will ───────────────────────────────────────────────────────

        /// Draft (or re-draft) a last Will and Testament.
        ///
        /// The caller specifies a list of `(heir_account, share_percent)` pairs.
        /// The total of all share_percent values MUST equal exactly 100.
        ///
        /// Re-drafting clears any prior notarization — the Will must be
        /// re-certified by a Notary after any modification.
        ///
        /// ## Parameters
        /// - `heirs`: Vec of (AccountId, u8 percent), sum must == 100, max `MaxHeirs` entries.
        ///
        /// # Origin: Signed (any citizen)
        #[pallet::call_index(0)]
        #[pallet::weight(T::WeightInfo::draft_will())]
        pub fn draft_will(origin: OriginFor<T>, heirs: Vec<(T::AccountId, u8)>) -> DispatchResult {
            let caller = ensure_signed(origin)?;

            // Validate: at least one heir.
            ensure!(!heirs.is_empty(), Error::<T>::NoHeirsDeclared);

            // Validate: shares sum to 100%.
            let total: u32 = heirs.iter().map(|(_, pct)| *pct as u32).sum();
            ensure!(total == 100u32, Error::<T>::HeirsShareNot100);

            // Bound the heirs list.
            let bounded: BoundedVec<(T::AccountId, u8), T::MaxHeirs> =
                heirs.try_into().map_err(|_| Error::<T>::TooManyHeirs)?;

            let heir_count = bounded.len() as u32;

            // Store Will (resets notarization if re-drafting).
            ActiveWills::<T>::insert(
                &caller,
                Will {
                    heirs: bounded,
                    is_notarized: false,
                    notary: None,
                },
            );

            Self::deposit_event(Event::WillDrafted {
                citizen: caller,
                heir_count,
            });
            Ok(())
        }

        // ─── notarize_will ────────────────────────────────────────────────────

        /// Certify a citizen's Will as a licensed Notary.
        ///
        /// The caller must be a Professional or Master in the Notary Guild,
        /// verified via the `GuildsChecker` cross-pallet bridge.
        ///
        /// A notarization fee is transferred from the `target_citizen` to the
        /// Notary, creating GDP and professional employment for Guild members.
        ///
        /// ## Parameters
        /// - `target_citizen`: The citizen whose Will is being notarized.
        /// - `fee`: The professional fee transferred from citizen → notary.
        ///
        /// ## Security
        /// - Notary CANNOT notarize their own Will (prevents self-certification).
        /// - Caller must be a valid notary per `GuildsChecker`.
        ///
        /// # Origin: Signed (must be Guild Professional/Master Notary)
        #[pallet::call_index(1)]
        #[pallet::weight(T::WeightInfo::notarize_will())]
        pub fn notarize_will(
            origin: OriginFor<T>,
            target_citizen: T::AccountId,
            fee: BalanceOf<T>,
        ) -> DispatchResult {
            let notary = ensure_signed(origin)?;

            // **SECURITY** Notary cannot notarize their own Will.
            ensure!(notary != target_citizen, Error::<T>::CannotNotarizeSelf);

            // **SECURITY** Caller must be a valid Notary Guild Professional/Master.
            ensure!(
                T::GuildsChecker::is_valid_notary(&notary),
                Error::<T>::NotValidNotary
            );

            // The target citizen must have drafted a Will.
            let mut will =
                ActiveWills::<T>::get(&target_citizen).ok_or(Error::<T>::NoActiveWill)?;

            // Transfer notarization fee: citizen → notary.
            // This creates GDP and a professional income for the Notary.
            T::Currency::transfer(
                &target_citizen,
                &notary,
                fee,
                ExistenceRequirement::KeepAlive,
            )
            .map_err(|_| Error::<T>::InsufficientFeeBalance)?;

            // Mark Will as notarized.
            will.is_notarized = true;
            will.notary = Some(notary.clone());
            ActiveWills::<T>::insert(&target_citizen, will);

            Self::deposit_event(Event::WillNotarized {
                citizen: target_citizen,
                notary,
                fee,
            });
            Ok(())
        }

        // ─── execute_will ─────────────────────────────────────────────────────

        /// Execute a deceased citizen's Will, distributing their frozen funds.
        ///
        /// Any signed caller may trigger execution — allowing the State, heirs,
        /// or automated pallets to trigger the procedure after `register_death`.
        ///
        /// ## Execution Logic
        /// 1. Verifies the `deceased_citizen` has `CitizenStatus::Deceased`.
        /// 2. Checks for an existing notarized Will.
        /// 3. **If notarized Will found**: unreserves ALL reserved funds and
        ///    distributes them proportionally to heirs by their declared percent.
        /// 4. **If no notarized Will**: sends all reserved funds to StateTreasury
        ///    (constitutional fallback — no dead-soul wealth accumulation).
        ///
        /// ## Dust Handling
        /// Rounding dust (from integer percent division) is sent to the final
        /// heir to ensure all funds are fully distributed.
        ///
        /// # Origin: Signed (any citizen or automated caller)
        #[pallet::call_index(2)]
        #[pallet::weight(T::WeightInfo::execute_will())]
        pub fn execute_will(
            origin: OriginFor<T>,
            deceased_citizen: T::AccountId,
        ) -> DispatchResult {
            let _caller = ensure_signed(origin)?;

            // Verify the citizen is registered as Deceased.
            ensure!(
                T::IdentityChecker::is_deceased(&deceased_citizen),
                Error::<T>::CitizenNotDeceased
            );

            Self::do_execute_will(&deceased_citizen)
        }
    }

    // =========================================================================
    // Internal / Hook-callable logic
    // =========================================================================

    impl<T: Config> Pallet<T> {
        /// Internal inheritance execution — called without an origin.
        ///
        /// ## Entry Points
        ///
        /// 1. **`execute_will` extrinsic** — public, signed, anyone may call.
        ///    Verifies `CitizenNotDeceased` guard before delegating here.
        /// 2. **`trigger_inheritance` hook** — called by `TerminalHookImpl::on_deceased`
        ///    synchronously inside `register_death`. No origin needed; the identity
        ///    pallet guarantees the citizen is already Deceased at call time.
        ///
        /// ## Execution Logic
        ///
        /// 1. Reads reserved (frozen) balance from the deceased's account.
        /// 2. Queries `BankInterface` for outstanding CDP debt → liquidates first.
        /// 3. If a notarized Will exists: distributes remainder to declared heirs.
        /// 4. Fallback (no notarized Will): sends all funds to `StateTreasury`.
        /// 5. Dust from integer percent division goes to the last heir.
        pub(crate) fn do_execute_will(deceased: &T::AccountId) -> DispatchResult {
            // Collect all reserved (frozen) funds of the deceased.
            let reserved = T::Currency::reserved_balance(deceased);

            // Check for a notarized Will.
            let maybe_will = ActiveWills::<T>::get(deceased);
            let has_notarized_will = maybe_will.as_ref().map(|w| w.is_notarized).unwrap_or(false);

            if has_notarized_will {
                // ── Notarized Will: distribute to heirs ───────────────────────
                let will = maybe_will.expect("checked above; qed");

                if reserved == BalanceOf::<T>::default() {
                    // No reserved funds to distribute (already zero).
                    Self::deposit_event(Event::WillExecuted {
                        deceased: deceased.clone(),
                        total_distributed: BalanceOf::<T>::default(),
                    });
                    return Ok(());
                }

                // ── [SECURITY VECTOR 1] CDP Debt Liquidation ─────────────────
                // Query outstanding bank credit BEFORE releasing funds to heirs.
                // Dead debt is burned from reserved balance (NegativeImbalance drop).
                let outstanding_planck: u128 = T::BankInterface::total_outstanding_debt(deceased);
                let outstanding: BalanceOf<T> = outstanding_planck.saturated_into();
                let net_for_heirs = if outstanding > Zero::zero() {
                    let debt_to_burn = outstanding.min(reserved);
                    let (neg_imbalance, _unslashed) =
                        T::Currency::slash_reserved(deceased, debt_to_burn);
                    drop(neg_imbalance); // burn: NegativeImbalance reduces TotalIssuance

                    let remainder = reserved.saturating_sub(debt_to_burn);
                    Self::deposit_event(Event::DeadDebtLiquidated {
                        deceased: deceased.clone(),
                        debt_settled: debt_to_burn,
                        remainder,
                    });
                    remainder
                } else {
                    reserved
                };

                // Unreserve the net remainder (post-debt) from the deceased's account.
                T::Currency::unreserve(deceased, net_for_heirs);

                // Distribute to heirs proportionally.
                let total_u128: u128 = net_for_heirs.saturated_into::<u128>();
                let mut distributed: u128 = 0u128;
                let heir_count = will.heirs.len();

                for (idx, (heir, pct)) in will.heirs.iter().enumerate() {
                    let share: u128 = if idx == heir_count.saturating_sub(1) {
                        // Last heir gets all remaining dust.
                        total_u128.saturating_sub(distributed)
                    } else {
                        total_u128
                            .saturating_mul(*pct as u128)
                            .saturating_div(100u128)
                    };

                    if share == 0 {
                        continue;
                    }

                    let heir_amount: BalanceOf<T> = share.saturated_into();
                    // Use AllowDeath — the deceased account is terminal.
                    let _ = T::Currency::transfer(
                        deceased,
                        heir,
                        heir_amount,
                        ExistenceRequirement::AllowDeath,
                    );
                    distributed = distributed.saturating_add(share);
                }

                // Remove the executed Will from storage.
                ActiveWills::<T>::remove(deceased);

                Self::deposit_event(Event::WillExecuted {
                    deceased: deceased.clone(),
                    total_distributed: distributed.saturated_into(),
                });
            } else {
                // ── Fallback: no notarized Will → StateTreasury ───────────────
                //
                // Constitutional principle: no dead-soul wealth accumulation.
                // Un-willed or un-notarized estates revert to the State.
                let treasury = T::StateTreasury::get();

                if reserved > BalanceOf::<T>::default() {
                    T::Currency::unreserve(deceased, reserved);
                    let _ = T::Currency::transfer(
                        deceased,
                        &treasury,
                        reserved,
                        ExistenceRequirement::AllowDeath,
                    );
                }

                // Clean up any un-notarized Will draft.
                ActiveWills::<T>::remove(deceased);

                Self::deposit_event(Event::FallbackToTreasury {
                    deceased: deceased.clone(),
                    amount: reserved,
                });
            }

            Ok(())
        }

        /// Hook called by `TerminalHookImpl::on_deceased` inside `register_death`.
        ///
        /// Synchronously triggers Will execution immediately on death registration.
        /// This closes the inheritance loop without requiring a separate user transaction.
        ///
        /// ## Rationale for Synchronous Execution
        ///
        /// The alternative (emit event → backend listener → user calls `execute_will`)
        /// creates a temporal gap where:
        ///   - The deceased's funds are frozen in limbo
        ///   - Network instability or backend downtime delays heir payout
        ///   - "Dead soul" state persists longer than constitutionally necessary
        ///
        /// Synchronous execution at `register_death` is ACID-correct and follows the
        /// constitutional principle of immediate inheritance resolution.
        ///
        /// ## Error Handling
        ///
        /// Uses `let _ = do_execute_will(...)` — errors are silently swallowed.
        /// This prevents a failing Will execution from reverting the `register_death`
        /// (which must always succeed for constitutional compliance).
        /// Execution failures are observable via the absence of `WillExecuted` or
        /// `FallbackToTreasury` events — the backend can re-trigger via `execute_will`.
        pub fn trigger_inheritance(deceased: &T::AccountId) {
            // Best-effort: never abort the parent register_death extrinsic.
            let _ = Self::do_execute_will(deceased);
        }
    }
}
