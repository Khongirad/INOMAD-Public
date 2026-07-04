//! # Steppe Offline Pallet
//!
//! **Altan Network — Sovereign L1 Blockchain**
//! **Sprint L1-05 / L1-15 (Audit Fix): The Steppe Protocol & Offline Justice**
//!
//! This pallet implements the **Steppe Protocol** — Altan Network's offline mesh payment
//! settlement system.  When citizens venture off-grid (remote steppe regions, disaster zones,
//! or areas without internet connectivity), they can:
//!
//! 1. **Lock ALTAN** into an offline pocket vault before going offline.
//! 2. **Transact via QR-code IOUs** using cryptographic ed25519 / sr25519 signatures,
//!    peer-to-peer, with no internet required.
//! 3. **Sync and settle** all accumulated IOUs when back online via `settle_iou`.
//! 4. **ARMAGEDDON Protocol**: The network mathematically proves double-spend fraud and
//!    triggers an instant soulbound slash of the offending citizen via
//!    [`SlashInterface::slash_citizen`].
//!
//! ## Security Guarantees
//!
//! | Threat                  | Defence                                                   |
//! |-------------------------|-----------------------------------------------------------|
//! | Replay attack           | `ProcessedIous` double-map (sender, nonce) → bool        |
//! | Signature forgery       | `sp_runtime::traits::Verify` on-chain signature check    |
//! | Double-spend / FRAUD    | ARMAGEDDON: pocket emptied, citizen slashed, event emitted|
//!
//! ## ⚠ Audit Fix — Sprint L1-15: Steppe Protocol Economics
//!
//! The original implementation used `fungible::Mutate::burn_from` / `mint_into`.
//! This **temporarily destroys and recreates total supply**, which is economically
//! unsound — auditors flagged it as a "phantom money" vulnerability.
//!
//! The refactored vault uses `Currency::reserve` (lock_funds) and
//! `Currency::unreserve` + `Currency::transfer` (settle_iou).  Funds are never
//! destroyed; they remain in the sender's account as a *reserved* balance,
//! invisible to normal spends but 100% accounted in total issuance.
//!
//! ## Storage
//! - [`OfflinePockets`]: per-account reserved ALTAN vault balance.
//! - [`ProcessedIous`]: (sender, nonce) replay guard.
//!
//! ## Dispatchables
//!
//! | Call | Origin | Description |
//! |---|---|---|
//! | `lock_funds` | Signed | Lock ALTAN as collateral for an off-chain IOU agreement |
//! | `settle_iou` | Signed (Both Parties) | Settle a confirmed IOU and release locked collateral |

#![cfg_attr(not(feature = "std"), no_std)]

pub mod migrations;
pub mod weights;
pub use pallet::*;

// ═══════════════════════════════════════════════════════════════════════════════
// SlashInterface — the cross-pallet slashing hook
// ═══════════════════════════════════════════════════════════════════════════════

/// Cross-pallet slashing interface.
///
/// The runtime implements this trait (in `configs/mod.rs`) to bridge
/// `pallet-steppe-offline` → `pallet-inomad-identity`, keeping the pallets
/// themselves loosely coupled.
///
/// A correct implementation MUST:
/// - Set `CitizenRecord::is_active = false` for `who`.
/// - Emit the identity pallet's `CitizenSlashed` event.
/// - Return `Ok(())` on success or a `DispatchError` on failure.
pub trait SlashInterface<AccountId> {
    /// Permanently freeze an account's citizen identity due to fraud.
    fn slash_citizen(who: &AccountId) -> frame_support::dispatch::DispatchResult;
}

// ═══════════════════════════════════════════════════════════════════════════════
// Pallet implementation
// ═══════════════════════════════════════════════════════════════════════════════

#[cfg(feature = "runtime-benchmarks")]
mod benchmarking;
#[frame_support::pallet]
pub mod pallet {
    use super::SlashInterface;
    use crate::weights::WeightInfo as _;
    use codec::Encode;
    use frame_support::{
        pallet_prelude::*,
        traits::{Currency, ExistenceRequirement, ReservableCurrency},
    };
    use frame_system::pallet_prelude::*;
    use sp_runtime::{
        traits::{IdentifyAccount, Verify},
        Saturating,
    };

    // ─── Type aliases ────────────────────────────────────────────────────────

    /// Convenience alias for the pallet's balance type.
    ///
    /// Derived from `T::Currency` (which is `pallet_balances` in the runtime).
    pub type BalanceOf<T> =
        <<T as Config>::Currency as Currency<<T as frame_system::Config>::AccountId>>::Balance;

    // ─── Pallet struct ───────────────────────────────────────────────────────

    #[pallet::pallet]
    pub struct Pallet<T>(_);

    // ─── Config ──────────────────────────────────────────────────────────────

    /// Pallet configuration trait.
    ///
    /// ## Audit Fix — Sprint L1-15
    ///
    /// `Currency` now requires both `Currency<AccountId>` (for `transfer`) and
    /// `ReservableCurrency<AccountId>` (for `reserve` / `unreserve`).
    /// In the runtime this is wired to `pallet_balances::Pallet<Runtime>`, which
    /// implements both traits.  This replaces the former `fungible::Mutate` bound
    /// that enabled the economically-unsound `burn_from` / `mint_into` pattern.
    #[pallet::config]
    pub trait Config: frame_system::Config<RuntimeEvent: From<Event<Self>>> {
        /// Weight information for extrinsics in this pallet.
        type WeightInfo: crate::weights::WeightInfo;
        /// The currency used for locking and settling offline funds.
        ///
        /// Must implement both `Currency` (for `transfer`) and
        /// `ReservableCurrency` (for `reserve` / `unreserve`).
        /// In the runtime this is wired to `pallet_balances::Pallet<Runtime>`.
        type Currency: Currency<Self::AccountId> + ReservableCurrency<Self::AccountId>;

        /// Cross-pallet slashing hook — calls `pallet-inomad-identity` to freeze
        /// a citizen account that has been proven to double-spend.
        ///
        /// The runtime provides the concrete implementation via a newtype struct.
        type IdentitySlashing: SlashInterface<Self::AccountId>;

        /// The signature type used to authenticate offline IOUs.
        ///
        /// In the runtime this is the `Signature = MultiSignature` alias, which
        /// verifies both sr25519 and ed25519 signatures transparently.
        ///
        /// The bound `Signer: IdentifyAccount<AccountId = Self::AccountId>` ensures
        /// that the signer identity can be resolved to the on-chain `AccountId`.
        type Signature: Parameter + Verify<Signer: IdentifyAccount<AccountId = Self::AccountId>>;
    }

    // ─── IOU Payload ─────────────────────────────────────────────────────────

    /// The data that a payer signs offline to create an IOU.
    ///
    /// The receiver brings this payload + the sender's signature on-chain via
    /// [`Pallet::settle_iou`].  The encoding **must** remain deterministic and
    /// identical on both the offline device and the verification node.
    #[derive(Encode, Decode, DecodeWithMemTracking, Clone, PartialEq, RuntimeDebug, TypeInfo)]
    pub struct IouPayload<Balance> {
        /// The amount of ALTAN planck the sender promises to pay.
        pub amount: Balance,
        /// Monotonically increasing nonce that prevents replay attacks.
        ///
        /// Each new IOU from the same sender must use a strictly higher nonce.
        /// `ProcessedIous[sender][nonce]` will be set to `true` on first
        /// settlement, blocking all subsequent replay attempts.
        pub nonce: u64,
    }

    // ─── Storage ─────────────────────────────────────────────────────────────

    /// Per-account offline pocket balance (ALTAN planck).
    ///
    /// This mirrors the *reserved* balance held by the sender via
    /// `Currency::reserve`.  Incremented by [`Pallet::lock_funds`]; decremented
    /// by successful [`Pallet::settle_iou`] calls or zeroed during ARMAGEDDON.
    ///
    /// ## Audit Note (L1-15)
    /// This value is a *shadow ledger* that tracks how much of the sender's
    /// reserved balance belongs to the Steppe vault.  The actual tokens remain
    /// on the sender's account (as reserved balance) at all times.
    #[pallet::storage]
    #[pallet::getter(fn offline_pockets)]
    pub type OfflinePockets<T: Config> =
        StorageMap<_, Blake2_128Concat, T::AccountId, BalanceOf<T>, ValueQuery>;

    /// Replay-attack guard: `(sender_account, iou_nonce) → already_settled`.
    ///
    /// A `true` value means this IOU has already been settled on-chain and any
    /// further settlement attempt with the same (sender, nonce) pair will be
    /// rejected with [`Error::IouAlreadyProcessed`].
    #[pallet::storage]
    #[pallet::getter(fn processed_ious)]
    pub type ProcessedIous<T: Config> = StorageDoubleMap<
        _,
        Blake2_128Concat,
        T::AccountId, // sender
        Blake2_128Concat,
        u64, // nonce
        bool,
        ValueQuery,
    >;

    // ─── Events ──────────────────────────────────────────────────────────────

    #[pallet::event]
    #[pallet::generate_deposit(pub(super) fn deposit_event)]
    pub enum Event<T: Config> {
        /// A citizen locked ALTAN into their offline pocket vault.
        PocketFunded {
            /// The citizen who locked funds.
            who: T::AccountId,
            /// Amount locked (in ALTAN planck).
            amount: BalanceOf<T>,
        },

        /// An offline IOU was verified on-chain and settled to the receiver.
        IouSettled {
            /// The original IOU sender (payer).
            sender: T::AccountId,
            /// The receiver / merchant who brought the IOU online.
            receiver: T::AccountId,
            /// Amount transferred.
            amount: BalanceOf<T>,
            /// IOU nonce — identifies this IOU uniquely per sender.
            nonce: u64,
        },

        /// **ARMAGEDDON** — a double-spend was cryptographically proven.
        ///
        /// The fraudulent sender has been soulbound-slashed by the identity
        /// pallet.  Whatever remained in their reserved pocket was transferred
        /// to the receiver (best-effort).
        ArmageddonTriggered {
            /// The citizen convicted of double-spending.
            sinner: T::AccountId,
            /// The amount by which the IOU exceeded the sender's pocket balance
            /// (i.e. the "counterfeit" portion of the payment).
            deficit: BalanceOf<T>,
        },
    }

    // ─── Errors ──────────────────────────────────────────────────────────────

    #[pallet::error]
    pub enum Error<T> {
        /// The caller does not have enough free balance to lock into the pocket.
        InsufficientFreeBalance,
        /// The IOU signature is invalid — either forged or payload was tampered.
        InvalidSignature,
        /// This (sender, nonce) IOU has already been settled on-chain.
        IouAlreadyProcessed,
    }

    // ─── Extrinsics ──────────────────────────────────────────────────────────

    #[pallet::call]
    impl<T: Config> Pallet<T> {
        // ─── lock_funds ───────────────────────────────────────────────────────

        /// Lock ALTAN into the offline pocket vault before going off-grid.
        ///
        /// ## Audit Fix — Sprint L1-15 (Steppe Protocol Economics)
        ///
        /// Previously this extrinsic called `Currency::burn_from`, which
        /// **destroys** the total token supply temporarily.  This is economically
        /// unsound — auditors flagged it as a "phantom money" vulnerability.
        ///
        /// The new implementation calls `Currency::reserve`, which moves `amount`
        /// from the caller's **free** balance into their **reserved** balance.
        /// Funds remain on the account — they are simply locked from being spent
        /// freely.  Total issuance is unchanged at all times.
        ///
        /// # Origin: Signed (any account holder)
        #[pallet::call_index(0)]
        #[pallet::weight(T::WeightInfo::lock_funds())]
        pub fn lock_funds(origin: OriginFor<T>, amount: BalanceOf<T>) -> DispatchResult {
            let caller = ensure_signed(origin)?;

            // Ensure the caller has sufficient reducible free balance.
            let free = T::Currency::free_balance(&caller);
            ensure!(free >= amount, Error::<T>::InsufficientFreeBalance);

            // ── AUDIT FIX (L1-15): reserve instead of burn_from ──────────────
            // Moves `amount` from free → reserved on the caller's account.
            // Total supply is UNCHANGED.  The Steppe vault holds the *reserved*
            // portion; it cannot be spent but it exists on-chain.
            T::Currency::reserve(&caller, amount)?;

            // Credit the logical pocket shadow-ledger.
            OfflinePockets::<T>::mutate(&caller, |pocket| {
                *pocket = pocket.saturating_add(amount);
            });

            Self::deposit_event(Event::PocketFunded {
                who: caller,
                amount,
            });
            Ok(())
        }

        // ─── settle_iou ───────────────────────────────────────────────────────

        /// Settle a signed offline IOU — the receiver brings it on-chain.
        ///
        /// The caller (origin) is the **receiver** / merchant who accepted the
        /// IOU offline.  They submit the sender's `payload` and the cryptographic
        /// `signature` the sender produced over that payload.
        ///
        /// ## Security Chain
        ///
        /// ### CHECK 1 — Replay Prevention
        /// `ProcessedIous[(sender, nonce)]` must be `false`.  It is atomically
        /// set to `true` before any funds move, blocking all future replays.
        ///
        /// ### CHECK 2 — Cryptographic Signature Verification
        /// The signature is verified against `payload.encode()` using the
        /// sender's on-chain public key (their `AccountId`).  Invalid signatures
        /// return [`Error::InvalidSignature`].
        ///
        /// ### THE ARMAGEDDON CHECK — Double-Spend Detection
        ///
        /// | Condition                      | Outcome                                     |
        /// |--------------------------------|---------------------------------------------|
        /// | `payload.amount ≤ pocket`      | Honest: unreserve → transfer, emit `IouSettled`|
        /// | `payload.amount > pocket`      | **FRAUD**: drain pocket to receiver,         |
        /// |                                | call `IdentitySlashing::slash_citizen`,     |
        /// |                                | emit `ArmageddonTriggered`                  |
        ///
        /// ## Audit Fix — Sprint L1-15 (Steppe Protocol Economics)
        ///
        /// Previously this extrinsic called `mint_into` on the receiver, creating
        /// funds from thin air.  The new implementation:
        ///
        /// **Honest path**: `unreserve(sender, amount)` + `transfer(sender → receiver)`.
        /// **ARMAGEDDON**:  `unreserve(sender, pocket)` + `transfer(sender → receiver, AllowDeath)`.
        ///
        /// # Origin: Signed (receiver / merchant)
        #[pallet::call_index(1)]
        #[pallet::weight(T::WeightInfo::settle_iou())]
        pub fn settle_iou(
            origin: OriginFor<T>,
            sender: T::AccountId,
            payload: IouPayload<BalanceOf<T>>,
            signature: T::Signature,
        ) -> DispatchResult {
            let receiver = ensure_signed(origin)?;

            // ── Security Check 1: Replay prevention ──────────────────────────
            // Atomically mark as processed — reject if already seen.
            ensure!(
                !ProcessedIous::<T>::get(&sender, payload.nonce),
                Error::<T>::IouAlreadyProcessed
            );
            ProcessedIous::<T>::insert(&sender, payload.nonce, true);

            // ── Security Check 2: Cryptographic signature verification ────────
            //
            // SCALE-encode the IOU payload deterministically and verify the
            // sender's signature over that byte slice.
            //
            // `sp_runtime::traits::Verify::verify` dispatches over MultiSignature
            // covering sr25519, ed25519, and ecdsa transparently.
            let encoded_payload = payload.encode();
            ensure!(
                signature.verify(encoded_payload.as_slice(), &sender),
                Error::<T>::InvalidSignature
            );

            // ── The Armageddon Check ──────────────────────────────────────────
            let pocket = OfflinePockets::<T>::get(&sender);

            if payload.amount <= pocket {
                // ── AUDIT FIX (L1-15): unreserve + transfer (honest path) ─────
                //
                // Step 1: Free the reserved funds back to the sender's free balance.
                // Step 2: Transfer from sender's now-free balance to receiver.
                //
                // Net effect: sender loses `amount`, receiver gains `amount`,
                // total supply is UNCHANGED throughout.
                T::Currency::unreserve(&sender, payload.amount);
                T::Currency::transfer(
                    &sender,
                    &receiver,
                    payload.amount,
                    ExistenceRequirement::KeepAlive,
                )?;

                // Update the shadow ledger.
                OfflinePockets::<T>::insert(&sender, pocket.saturating_sub(payload.amount));

                Self::deposit_event(Event::IouSettled {
                    sender,
                    receiver,
                    amount: payload.amount,
                    nonce: payload.nonce,
                });
            } else {
                // ── FRAUD DETECTED — ARMAGEDDON ───────────────────────────────
                //
                // The sender cryptographically committed to paying more than they
                // had locked.  By CHECK 2, this signature is authentic — the fraud
                // is mathematically proven, not asserted.
                //
                // Best-effort: give whatever remains in the pocket to the receiver.
                //
                // ── AUDIT FIX (L1-15): unreserve + transfer (ARMAGEDDON path) ─
                let deficit = payload.amount.saturating_sub(pocket);
                let zero = BalanceOf::<T>::default();

                if pocket > zero {
                    // Unreserve the locked balance so it becomes free again,
                    // then immediately transfer it to the receiver.
                    T::Currency::unreserve(&sender, pocket);
                    // AllowDeath: the sender's account may be killed if pocket == ED.
                    let _ = T::Currency::transfer(
                        &sender,
                        &receiver,
                        pocket,
                        ExistenceRequirement::AllowDeath,
                    );
                }

                // Zero out the fraudulent sender's shadow-ledger pocket.
                OfflinePockets::<T>::insert(&sender, zero);

                // EXECUTE ARMAGEDDON: permanently freeze the sinner's soulbound identity.
                //
                // A failed slash (sender not in identity registry) is silently ignored —
                // the mathematical fraud proof and pocket drain are permanent regardless.
                let _ = T::IdentitySlashing::slash_citizen(&sender);

                Self::deposit_event(Event::ArmageddonTriggered {
                    sinner: sender,
                    deficit,
                });
            }

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
