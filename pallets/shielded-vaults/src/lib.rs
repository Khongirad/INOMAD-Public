//! # pallet-shielded-vaults
//!
//! **Altan Network — Принцип Асимметричной Прозрачности**
//!
//! ## Конституционный Мандат
//!
//! | Субъект                    | Прозрачность | Право на тайну |
//! |----------------------------|-------------|----------------|
//! | Центральный Банк           | 100% PUBLIC | ✗ ЗАБЛОКИРОВАНО|
//! | Казначейства (Хурал, Регионы) | 100% PUBLIC | ✗ ЗАБЛОКИРОВАНО|
//! | Банк Сибири (гос. резерв)  | 100% PUBLIC | ✗ ЗАБЛОКИРОВАНО|
//! | Граждане                   | Публичный адрес | ✓ ПРАВО     |
//! | Гильдии / Организации      | Регистрация открытая | ✓ ПРАВО |
//!
//! ## Commitment-Based Shielded Pool
//!
//! Реализует протокол экранированных обязательств (commitment scheme),
//! аналогичный Zcash Sapling, но без off-chain ZK-схем.
//!
//! ### Формат Commitment
//!
//! ```text
//! commitment = blake2_256(amount_bytes || blinding_factor || owner_hash)
//! ```
//!
//! Commitment — опакный 32-байтный хэш. Публичная цепь видит только
//! хэш, а не сумму или получателя. Владелец хранит `amount` и
//! `blinding_factor` off-chain.
//!
//! ### Nullifier (защита от двойных трат)
//!
//! ```text
//! nullifier = blake2_256(commitment || owner_secret)
//! ```
//!
//! При «расчехлении» (`unshield_to_account`) или налоговом мосте
//! (`org_unshield_tax_payment`) нуллификатор публикуется => commitment
//! уничтожается. Попытка использовать нуллификатор дважды возвращает
//! `Error::NullifierAlreadySpent`.
//!
//! ### Поток средств
//!
//! ```text
//! PUBLIC (открытый баланс)
//!   ─► shield_funds(amount, commitment) ─► SHIELDED POOL
//!         shielded_transfer(nullifier, new_commitment) ─► (скрытый перевод в пуле)
//!         unshield_to_account(nullifier, amount, recipient) ─► PUBLIC
//!         org_unshield_tax_payment(nullifier, amount) ─► PUBLIC (RegionalTreasury)
//! ```
//!
//! ## TransparentStateGuard
//!
//! Государственные аккаунты математически ЛИШЕНЫ ПРАВА входить в пул.
//! `shield_funds` возвращает `Error::StateFundsMustRemainPublic` если
//! вызывающий является государственным аккаунтом.
//!
//! ## Dispatchables
//!
//! | Call | Origin | Description |
//! |---|---|---|
//! | `shield_funds` | Signed | Deposit public ALTAN into a private shielded vault |
//! | `shielded_transfer` | Signed | Transfer between shielded vaults without revealing amounts |
//! | `unshield_to_account` | Signed | Withdraw from a shielded vault to a public account |
//! | `org_unshield_tax_payment` | Signed (Organization) | Unshield funds specifically for tax payment obligations |

#![cfg_attr(not(feature = "std"), no_std)]

extern crate alloc;

pub mod migrations;
pub mod weights;
pub use pallet::*;

// =========================================================================
// Cross-pallet Interface — State Account Checker
// =========================================================================

/// Trait for checking whether an account is a state/government account
/// that must remain 100% transparent (cannot enter the shielded pool).
///
/// ## Constitutional Mandate (Асимметричная Прозрачность)
///
/// Accounts implementing this as `true` are constitutionally blocked
/// from calling `shield_funds`. The check is enforced at the pallet level:
/// no governance vote, no Sudo key can override it.
///
/// ## Accounts that must return `true`
///
/// - `Central Bank` system account
/// - `Bank_of_Siberia_Main_Reserve`
/// - `Confederation_Treasury`
/// - All 83 `Regional_Gov_Treasury` accounts
/// - All 83 `Regional_Citizen_Fund` accounts
/// - `Federal_Sweep_Account`
/// - Judicial / Khural treasury accounts
pub trait StateAccountChecker<AccountId> {
    /// Returns `true` if `who` is a state/government account that must
    /// remain fully transparent. Such accounts cannot shield their funds.
    fn is_state_account(who: &AccountId) -> bool;
}

/// No-op implementation — treats everyone as a non-state account.
/// ONLY use in unit tests. Production runtime MUST wire `StateAccountGuard`.
impl<AccountId> StateAccountChecker<AccountId> for () {
    fn is_state_account(_who: &AccountId) -> bool {
        false
    }
}

// =========================================================================
// Cross-pallet Interface — Shielded Vaults for other pallets
// =========================================================================

/// Interface for other pallets (e.g. pallet-organization) to interact with
/// the shielded pool without tight coupling.
pub trait ShieldedVaultsInterface<AccountId> {
    /// Check whether a nullifier has already been spent.
    fn is_nullifier_spent(nullifier: &[u8; 32]) -> bool;

    /// Spend a nullifier and materialize `amount` as a public transfer
    /// to `recipient`. Used for the ZK tax bridge in pallet-organization.
    ///
    /// Returns `Err(NullifierAlreadySpent)` if the nullifier was already used.
    /// Returns `Err(CommitmentNotFound)` if no matching commitment exists.
    fn unshield_for_tax(
        nullifier: &[u8; 32],
        commitment: &[u8; 32],
        amount: u128,
        recipient: &AccountId,
    ) -> frame_support::dispatch::DispatchResult;
}

/// No-op implementation for unit tests / mock runtimes.
impl<AccountId> ShieldedVaultsInterface<AccountId> for () {
    fn is_nullifier_spent(_nullifier: &[u8; 32]) -> bool {
        false
    }
    fn unshield_for_tax(
        _nullifier: &[u8; 32],
        _commitment: &[u8; 32],
        _amount: u128,
        _recipient: &AccountId,
    ) -> frame_support::dispatch::DispatchResult {
        Ok(())
    }
}

// =========================================================================
// Cross-pallet Interface — Org Region Resolver
// =========================================================================

/// Resolves the regional treasury account for an organization.
///
/// Called during `org_unshield_tax_payment` to determine which Regional
/// Treasury receives 7/10 of the tax payment. Implement in runtime glue
/// by reading from `pallet-organization`'s `OrgRegion` storage.
pub trait OrgRegionResolverTrait<AccountId> {
    /// Returns the regional treasury account for the given `org_id`.
    /// Returns `None` if the org is not registered or has no region.
    fn regional_treasury_for(org_id: u32) -> Option<AccountId>;
}

/// No-op implementation for unit tests — always returns None.
impl<AccountId> OrgRegionResolverTrait<AccountId> for () {
    fn regional_treasury_for(_org_id: u32) -> Option<AccountId> {
        None
    }
}

#[cfg(feature = "runtime-benchmarks")]
mod benchmarking;
#[frame_support::pallet]
pub mod pallet {
    use crate::weights::WeightInfo as _;
    use crate::{OrgRegionResolverTrait, StateAccountChecker};
    use frame_support::{
        pallet_prelude::*,
        traits::{Currency, ExistenceRequirement},
    };
    use frame_system::pallet_prelude::*;

    // =========================================================================
    // Type Aliases
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

    /// Shielded Vaults pallet configuration.
    ///
    /// ## Конституционная Архитектура
    ///
    /// - `Currency`: Used for `shield_funds` (deduct public balance) and
    ///   `unshield_to_account` (restore public balance via `deposit_creating`).
    ///
    /// - `TransparentStateGuard`: Checked on every `shield_funds` call.
    ///   State accounts that return `true` cannot enter the shielded pool.
    #[pallet::config]
    pub trait Config: frame_system::Config<RuntimeEvent: From<Event<Self>>> {
        /// Weight information for extrinsics in this pallet.
        type WeightInfo: crate::weights::WeightInfo;
        /// Native ALTAN currency. Used to transfer in/out of the shielded pool.
        type Currency: Currency<Self::AccountId>;

        /// **Transparent State Guard** — constitutional gate for `shield_funds`.
        ///
        /// Wire to `StateAccountGuard` in the runtime. This struct reads the
        /// canonical list of state accounts (Central Bank, Bank of Siberia,
        /// all Confederation and Regional Treasury accounts) and returns `true`
        /// for any account that must remain transparent.
        ///
        /// When `is_state_account` returns `true`, `shield_funds` fails with
        /// `Error::StateFundsMustRemainPublic`. This is enforced at the type
        /// level and cannot be bypassed by governance or Sudo.
        type TransparentStateGuard: crate::StateAccountChecker<Self::AccountId>;

        /// The Confederation Treasury account that receives 3/10 of the ZK tax payment.
        ///
        /// Constitutional mandate (Ст. XI §3): 3/10 of corporate tax → Confederation.
        #[pallet::constant]
        type ConfederationTreasury: Get<Self::AccountId>;

        /// Resolves a region's treasury account from an `org_id`.
        ///
        /// Called during `org_unshield_tax_payment` to route 7/10 of the tax payment
        /// to the correct regional treasury. Implement in runtime configs as an
        /// `OrgRegionResolver` that reads from `pallet-organization`.
        type OrgRegionResolver: OrgRegionResolverTrait<Self::AccountId>;
    }

    // =========================================================================
    // Structs
    // =========================================================================

    /// A ZK-style tax proof for the `file_tax_return` ZK bridge in pallet-organization.
    ///
    /// The Director publishes this proof to "unshield" part of the guild's
    /// shielded balance and materialize it directly on the Regional Treasury.
    /// The guild's internal financial flows remain hidden.
    ///
    /// ## Fields
    ///
    /// - `nullifier`: Destroys the guild's shielded commitment.
    ///   `blake2_256(commitment || owner_secret)`. Prevents double-spending.
    /// - `commitment`: The shielded commitment being spent.
    ///   `blake2_256(amount_bytes || blinding_factor || owner_hash)`.
    /// - `amount_claimed`: The amount the Director declares as the tax payment.
    ///   This becomes publicly visible on the Regional Treasury. The internal
    ///   guild balance (shielded pool) remains hidden.
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
    pub struct ZkTaxProof {
        /// Nullifier that destroys the shielded commitment (prevents double-spend).
        /// format: blake2_256(commitment || owner_secret)
        pub nullifier: [u8; 32],
        /// The shielded commitment being spent.
        /// format: blake2_256(amount_bytes || blinding_factor || owner_hash)
        pub commitment: [u8; 32],
        /// The tax amount declared publicly. This hits the Regional Treasury.
        /// The guild's total internal balance is NOT disclosed.
        pub amount_claimed: u128,
    }

    // =========================================================================
    // Storage
    // =========================================================================

    /// Shielded Commitments registry — the leaves of the shielded pool.
    ///
    /// Key: 32-byte commitment hash = `blake2_256(amount || blinding || owner_hash)`.
    /// Value: `true` = active commitment; `false` = commitment has been spent.
    ///
    /// ## Privacy Property
    ///
    /// The commitment is opaque: an observer sees the hash but cannot derive
    /// the amount or owner without knowledge of the blinding factor.
    #[pallet::storage]
    #[pallet::getter(fn shielded_commitments)]
    pub type ShieldedCommitments<T: Config> =
        StorageMap<_, Blake2_128Concat, [u8; 32], bool, ValueQuery>;

    /// Spent Nullifiers — cryptographic proof of commitment destruction.
    ///
    /// Key: 32-byte nullifier = `blake2_256(commitment || owner_secret)`.
    /// Value: block number when the nullifier was spent (for audit trail).
    ///
    /// ## Double-Spend Prevention
    ///
    /// Before any unshielding operation, the pallet checks that the nullifier
    /// is NOT in this map. Once added, a nullifier is permanently spent —
    /// the commitment cannot be used again.
    #[pallet::storage]
    #[pallet::getter(fn spent_nullifiers)]
    pub type SpentNullifiers<T: Config> =
        StorageMap<_, Blake2_128Concat, [u8; 32], BlockNumberFor<T>, OptionQuery>;

    /// Shielded balance per Organization (OrgId → total shielded planks).
    ///
    /// Tracks how much ALTAN an organization has entered into the shielded pool
    /// in total (net of unshielding). This allows the tax authority to verify
    /// that an org's declared tax `amount_claimed` cannot exceed their shielded pool.
    ///
    /// ## Constitutional Note
    ///
    /// This value is an accumulator — NOT a per-commitment breakdown.
    /// The individual commitments remain opaque even with this value visible.
    #[pallet::storage]
    #[pallet::getter(fn org_vault_balance)]
    pub type OrgVaultBalance<T: Config> = StorageMap<
        _,
        Blake2_128Concat,
        u32, // OrgId
        u128,
        ValueQuery,
    >;

    /// Total amount of ALTAN currently in the shielded pool across all participants.
    ///
    /// Acts as a constitutional audit counter. Increases on `shield_funds`,
    /// decreases on `unshield_to_account` and `org_unshield_tax_payment`.
    #[pallet::storage]
    #[pallet::getter(fn total_shielded)]
    pub type TotalShielded<T: Config> = StorageValue<_, u128, ValueQuery>;

    // =========================================================================
    // Events
    // =========================================================================

    #[pallet::event]
    #[pallet::generate_deposit(pub(super) fn deposit_event)]
    pub enum Event<T: Config> {
        /// Funds were shielded: moved from public balance into the ZK pool.
        ///
        /// The `commitment` is publicly visible; the amount and recipient are hidden.
        /// Anyone can verify that `amount` left the public balance, but cannot
        /// determine who receives it within the shielded pool.
        FundsShielded {
            /// The account that shielded the funds (publicly visible).
            who: T::AccountId,
            /// Opaque commitment hash — hides amount and blinding factor.
            commitment: [u8; 32],
            /// If shielded on behalf of an organization.
            org_id: Option<u32>,
        },

        /// A shielded transfer occurred within the ZK pool.
        ///
        /// The nullifier (destroyed commitment) and new_commitment are visible,
        /// but the amount transferred remains hidden.
        ShieldedTransferExecuted {
            /// Destroyed commitment (now unspendable).
            nullifier: [u8; 32],
            /// New commitment created for the recipient.
            new_commitment: [u8; 32],
        },

        /// Funds were unshielded: moved from the ZK pool back to a public account.
        ///
        /// The `amount` is now visible on-chain at `recipient`'s public balance.
        FundsUnshielded {
            /// Nullifier that destroyed the commitment.
            nullifier: [u8; 32],
            /// Public account that received the unshielded funds.
            recipient: T::AccountId,
            /// Amount that became publicly visible.
            amount: u128,
        },

        /// A guild paid taxes by unshielding from their ZK vault.
        ///
        /// The treasury received `amount_paid` publicly; the guild's total
        /// internal shielded balance remains hidden.
        OrgTaxPaidFromVault {
            /// Organization that paid the tax.
            org_id: u32,
            /// The nullifier used (commitment destroyed).
            nullifier: [u8; 32],
            /// Amount publicly visible on the Regional Treasury.
            amount_paid: u128,
        },

        /// A state account attempted to shield its funds — BLOCKED.
        ///
        /// Constitutional violation attempt logged on-chain for transparency.
        StateAccountShieldingAttemptBlocked {
            /// The state account that tried to enter the shielded pool.
            who: T::AccountId,
        },
    }

    // =========================================================================
    // Errors
    // =========================================================================

    #[pallet::error]
    pub enum Error<T> {
        /// A government/state account attempted to shield its funds.
        ///
        /// ## Constitutional Mandate (Асимметричная Прозрачность)
        ///
        /// State accounts (Central Bank, Bank of Siberia, Confederation Treasury,
        /// Regional Treasuries) MUST remain 100% transparent. Shielding state
        /// funds is a constitutional violation. All state transactions are public.
        StateFundsMustRemainPublic,

        /// The nullifier has already been spent — double-spend attempt blocked.
        ///
        /// A nullifier can only be published once. Once spent, the corresponding
        /// commitment is permanently destroyed and cannot be unshielded again.
        NullifierAlreadySpent,

        /// The provided commitment was not found in the shielded pool.
        ///
        /// Either the commitment was never shielded, or it was already spent.
        CommitmentNotFound,

        /// The commitment is already marked as inactive (already spent).
        CommitmentAlreadySpent,

        /// The amount claimed in a ZK tax proof exceeds the org's shielded balance.
        ///
        /// The declared tax amount cannot exceed the total the org has shielded.
        AmountExceedsShieldedBalance,

        /// Arithmetic overflow in shielded balance accounting.
        ArithmeticOverflow,

        /// The transfer of funds out of the shielded pool to a public account failed.
        UnshieldTransferFailed,
    }

    // =========================================================================
    // Extrinsics
    // =========================================================================

    #[pallet::call]
    impl<T: Config> Pallet<T> {
        // ─── shield_funds ────────────────────────────────────────────────────

        /// Move funds from a public account into the shielded ZK pool.
        ///
        /// ## Transparent State Guard
        ///
        /// **This is the constitutional firewall of the shielded pool.**
        /// If `T::TransparentStateGuard::is_state_account(&caller)` returns `true`,
        /// this extrinsic IMMEDIATELY fails with `Error::StateFundsMustRemainPublic`.
        ///
        /// State accounts (Central Bank, all treasury accounts) CANNOT enter the pool.
        /// This check is enforced at the pallet level — no origin, no governance,
        /// no Sudo key can bypass it.
        ///
        /// ## Commitment Security
        ///
        /// The caller generates the `commitment` off-chain as:
        ///   `blake2_256(amount_bytes || blinding_factor || owner_hash)`
        ///
        /// The pallet stores only the commitment hash. The amount and blinding
        /// factor remain known only to the caller (and anyone they share them with).
        ///
        /// ## Parameters
        /// - `amount`: Amount to move from public balance to shielded pool.
        /// - `commitment`: 32-byte opaque commitment hash.
        /// - `org_id`: Optional — if shielding on behalf of an organization.
        #[pallet::call_index(0)]
        #[pallet::weight(T::WeightInfo::shield_funds())]
        pub fn shield_funds(
            origin: OriginFor<T>,
            amount: BalanceOf<T>,
            commitment: [u8; 32],
            org_id: Option<u32>,
        ) -> DispatchResult {
            let caller = ensure_signed(origin)?;

            // ── TRANSPARENT STATE GUARD ───────────────────────────────────────
            // Constitutional firewall: state accounts cannot shield their funds.
            if T::TransparentStateGuard::is_state_account(&caller) {
                // Log the attempt for constitutional audit trail.
                Self::deposit_event(Event::StateAccountShieldingAttemptBlocked { who: caller });
                return Err(Error::<T>::StateFundsMustRemainPublic.into());
            }

            // Guard: commitment must not already be registered.
            ensure!(
                !ShieldedCommitments::<T>::get(&commitment),
                Error::<T>::CommitmentAlreadySpent
            );

            // ── Physical transfer: public balance → shielded pool ─────────────
            // We burn the amount from the caller's public balance.
            // The shielded pool is an accounting entry (no single "pool account").
            let amount_u128: u128 = TryInto::<u128>::try_into(amount).unwrap_or(0);

            let _imbalance = T::Currency::withdraw(
                &caller,
                amount,
                frame_support::traits::WithdrawReasons::TRANSFER,
                ExistenceRequirement::KeepAlive,
            )
            .map_err(|_| Error::<T>::UnshieldTransferFailed)?;

            // ── Register commitment ───────────────────────────────────────────
            ShieldedCommitments::<T>::insert(&commitment, true);

            // ── Update org vault balance if applicable ────────────────────────
            if let Some(id) = org_id {
                OrgVaultBalance::<T>::mutate(id, |bal| {
                    *bal = bal.saturating_add(amount_u128);
                });
            }

            // ── Update total shielded counter ─────────────────────────────────
            TotalShielded::<T>::mutate(|total| {
                *total = total.saturating_add(amount_u128);
            });

            Self::deposit_event(Event::FundsShielded {
                who: caller,
                commitment,
                org_id,
            });
            Ok(())
        }

        // ─── shielded_transfer ───────────────────────────────────────────────

        /// Transfer ownership within the shielded pool (e.g. guild salary payments).
        ///
        /// ## Commercial Privacy — Guild Salary Payments
        ///
        /// A Director can pay crew members (`CrewMembers`) without revealing
        /// the amount on the public blockchain. The transfer stays inside the
        /// shielded pool: an old commitment is destroyed and a new one is created.
        ///
        /// Both commitments are visible on-chain as opaque hashes.
        /// The amount transferred is known only to the parties involved.
        ///
        /// ## Double-Spend Prevention
        ///
        /// The `nullifier` permanently marks the input commitment as consumed.
        /// Reusing a nullifier fails with `Error::NullifierAlreadySpent`.
        ///
        /// ## Parameters
        /// - `nullifier`: Destroys the sender's commitment. Must be fresh.
        /// - `input_commitment`: The sender's existing commitment (being destroyed).
        /// - `new_commitment`: New commitment for the recipient.
        /// - `org_id`: If the transfer is between org members.
        #[pallet::call_index(1)]
        #[pallet::weight(T::WeightInfo::shielded_transfer())]
        pub fn shielded_transfer(
            origin: OriginFor<T>,
            nullifier: [u8; 32],
            input_commitment: [u8; 32],
            new_commitment: [u8; 32],
            org_id: Option<u32>,
        ) -> DispatchResult {
            let _caller = ensure_signed(origin)?;

            // Guard: nullifier must not be spent.
            ensure!(
                SpentNullifiers::<T>::get(&nullifier).is_none(),
                Error::<T>::NullifierAlreadySpent
            );

            // Guard: input commitment must exist and be active.
            ensure!(
                ShieldedCommitments::<T>::get(&input_commitment),
                Error::<T>::CommitmentNotFound
            );

            let now = frame_system::Pallet::<T>::block_number();

            // ── Destroy the input commitment ──────────────────────────────────
            ShieldedCommitments::<T>::insert(&input_commitment, false);
            SpentNullifiers::<T>::insert(&nullifier, now);

            // ── Create the new commitment ─────────────────────────────────────
            // Guard: new commitment must not already exist.
            ensure!(
                !ShieldedCommitments::<T>::get(&new_commitment),
                Error::<T>::CommitmentAlreadySpent
            );
            ShieldedCommitments::<T>::insert(&new_commitment, true);

            // Note: org_id is for cross-org tracking (future feature).
            let _ = org_id;

            Self::deposit_event(Event::ShieldedTransferExecuted {
                nullifier,
                new_commitment,
            });
            Ok(())
        }

        // ─── unshield_to_account ─────────────────────────────────────────────

        /// Move funds from the shielded pool back to a public account.
        ///
        /// ## Unshielding
        ///
        /// The caller provides a nullifier (destroying the commitment) and declares
        /// the amount to unshield. The `amount` becomes visible on-chain as a credit
        /// to `recipient`'s public balance.
        ///
        /// This is a voluntary act: the commitment owner chooses when to surface
        /// their shielded funds back into the public economy.
        ///
        /// ## Parameters
        /// - `nullifier`: Destroys the commitment (prevents reuse).
        /// - `input_commitment`: The commitment being spent.
        /// - `amount`: Amount to credit to `recipient`. Must match the shielded amount.
        /// - `recipient`: Public account to receive the unshielded funds.
        /// - `org_id`: If unshielding from an organization's vault.
        #[pallet::call_index(2)]
        #[pallet::weight(T::WeightInfo::unshield_to_account())]
        pub fn unshield_to_account(
            origin: OriginFor<T>,
            nullifier: [u8; 32],
            input_commitment: [u8; 32],
            amount: u128,
            recipient: T::AccountId,
            org_id: Option<u32>,
        ) -> DispatchResult {
            let _caller = ensure_signed(origin)?;

            // Guard: nullifier must not be spent.
            ensure!(
                SpentNullifiers::<T>::get(&nullifier).is_none(),
                Error::<T>::NullifierAlreadySpent
            );

            // Guard: commitment must be active.
            ensure!(
                ShieldedCommitments::<T>::get(&input_commitment),
                Error::<T>::CommitmentNotFound
            );

            // Guard: if org vault, amount must not exceed org vault balance.
            if let Some(id) = org_id {
                let vault_bal = OrgVaultBalance::<T>::get(id);
                ensure!(
                    amount <= vault_bal,
                    Error::<T>::AmountExceedsShieldedBalance
                );
            }

            let now = frame_system::Pallet::<T>::block_number();

            // ── Destroy commitment, record nullifier ──────────────────────────
            ShieldedCommitments::<T>::insert(&input_commitment, false);
            SpentNullifiers::<T>::insert(&nullifier, now);

            // ── Restore public balance (deposit_creating) ─────────────────────
            // Convert u128 amount back to BalanceOf<T>.
            // Safety: if the pallet is used correctly, this will not fail.
            let balance_amount: BalanceOf<T> = amount
                .try_into()
                .map_err(|_| Error::<T>::ArithmeticOverflow)?;

            let _imbalance = T::Currency::deposit_creating(&recipient, balance_amount);
            // PositiveImbalance dropped → balance restored. ✓

            // ── Update accounting ─────────────────────────────────────────────
            if let Some(id) = org_id {
                OrgVaultBalance::<T>::mutate(id, |bal| {
                    *bal = bal.saturating_sub(amount);
                });
            }
            TotalShielded::<T>::mutate(|total| {
                *total = total.saturating_sub(amount);
            });

            Self::deposit_event(Event::FundsUnshielded {
                nullifier,
                recipient,
                amount,
            });
            Ok(())
        }

        // ─── org_unshield_tax_payment ────────────────────────────────────────

        /// Pay corporate tax from an organization's shielded vault.
        ///
        /// ## Constitutional ZK Tax Bridge (Ст. XI §3)
        ///
        /// Allows an organization Director to pay annual tax without revealing
        /// the org's total shielded balance. The payment is publicly visible on
        /// the regional and confederation treasuries; all other guild financials
        /// remain private in the shielded pool.
        ///
        /// ## Tax Split
        ///
        /// ```text
        /// amount_claimed × 7/10 → Regional Treasury (of the org's registered region)
        /// amount_claimed × 3/10 → Confederation Treasury
        /// ```
        ///
        /// Constitutional mandate: 3/10 Confederation + 7/10 Region.
        /// These proportions are HARD-CODED — cannot be modified by governance.
        ///
        /// ## Double-Spend Prevention
        ///
        /// The `nullifier` permanently marks the `input_commitment` as consumed.
        /// Any attempt to reuse the nullifier fails with `NullifierAlreadySpent`.
        ///
        /// ## Parameters
        /// - `org_id`: The organization paying taxes.
        /// - `nullifier`: Destroys the org's shielded commitment.
        ///   `blake2_256(commitment || owner_secret)`. Must be fresh.
        /// - `input_commitment`: The commitment being spent (must be active).
        /// - `amount_claimed`: Publicly declared tax payment in planck.
        ///   Must not exceed `OrgVaultBalance[org_id]`.
        #[pallet::call_index(3)]
        #[pallet::weight(T::WeightInfo::org_unshield_tax_payment())]
        pub fn org_unshield_tax_payment(
            origin: OriginFor<T>,
            org_id: u32,
            nullifier: [u8; 32],
            input_commitment: [u8; 32],
            amount_claimed: u128,
        ) -> DispatchResult {
            let _caller = ensure_signed(origin)?;

            // Guard: nullifier must not be spent.
            ensure!(
                SpentNullifiers::<T>::get(&nullifier).is_none(),
                Error::<T>::NullifierAlreadySpent
            );

            // Guard: commitment must be active.
            ensure!(
                ShieldedCommitments::<T>::get(&input_commitment),
                Error::<T>::CommitmentNotFound
            );

            // Guard: declared amount must not exceed org's shielded balance.
            let vault_bal = OrgVaultBalance::<T>::get(org_id);
            ensure!(
                amount_claimed <= vault_bal,
                Error::<T>::AmountExceedsShieldedBalance
            );

            let now = frame_system::Pallet::<T>::block_number();

            // ── Destroy commitment, record nullifier (double-spend protection) ─
            ShieldedCommitments::<T>::insert(&input_commitment, false);
            SpentNullifiers::<T>::insert(&nullifier, now);

            // ── Constitutional Tax Split: 7/10 Region + 3/10 Confederation ─────
            //
            // Hard-coded proportions per BLOCKCHAIN_CONSTITUTION.md Ст. XI §3.
            // NEVER modify these ratios — constitutional violation.
            let region_amount = amount_claimed.saturating_mul(7).saturating_div(10);
            let confederation_amount = amount_claimed.saturating_sub(region_amount); // Remainder to Confederation (ensures no dust)

            let confederation_account = T::ConfederationTreasury::get();

            let balance_confederation: BalanceOf<T> = confederation_amount
                .try_into()
                .map_err(|_| Error::<T>::ArithmeticOverflow)?;
            let _imbalance_conf =
                T::Currency::deposit_creating(&confederation_account, balance_confederation);

            // Regional treasury (7/10): resolved via OrgRegionResolver.
            // If org has no registered region → full amount goes to Confederation.
            if let Some(regional_treasury) = T::OrgRegionResolver::regional_treasury_for(org_id) {
                let balance_region: BalanceOf<T> = region_amount
                    .try_into()
                    .map_err(|_| Error::<T>::ArithmeticOverflow)?;
                let _imbalance_region =
                    T::Currency::deposit_creating(&regional_treasury, balance_region);
            } else {
                // No region registered: route region share to Confederation too.
                let balance_region: BalanceOf<T> = region_amount
                    .try_into()
                    .map_err(|_| Error::<T>::ArithmeticOverflow)?;
                let _imbalance_region =
                    T::Currency::deposit_creating(&confederation_account, balance_region);
            }

            // ── Update accounting ─────────────────────────────────────────────
            OrgVaultBalance::<T>::mutate(org_id, |bal| {
                *bal = bal.saturating_sub(amount_claimed);
            });
            TotalShielded::<T>::mutate(|total| {
                *total = total.saturating_sub(amount_claimed);
            });

            Self::deposit_event(Event::OrgTaxPaidFromVault {
                org_id,
                nullifier,
                amount_paid: amount_claimed,
            });
            Ok(())
        }
    }

    // =========================================================================
    // ShieldedVaultsInterface implementation (for pallet-organization bridge)
    // =========================================================================

    impl<T: Config> crate::ShieldedVaultsInterface<T::AccountId> for Pallet<T> {
        fn is_nullifier_spent(nullifier: &[u8; 32]) -> bool {
            SpentNullifiers::<T>::get(nullifier).is_some()
        }

        fn unshield_for_tax(
            nullifier: &[u8; 32],
            commitment: &[u8; 32],
            amount: u128,
            recipient: &T::AccountId,
        ) -> frame_support::dispatch::DispatchResult {
            // Guard: nullifier must not be spent.
            ensure!(
                SpentNullifiers::<T>::get(nullifier).is_none(),
                Error::<T>::NullifierAlreadySpent
            );
            // Guard: commitment must exist and be active.
            ensure!(
                ShieldedCommitments::<T>::get(commitment),
                Error::<T>::CommitmentNotFound
            );

            let now = frame_system::Pallet::<T>::block_number();

            // Destroy commitment; record nullifier.
            ShieldedCommitments::<T>::insert(commitment, false);
            SpentNullifiers::<T>::insert(nullifier, now);

            // Materialize funds on the public recipient (RegionalTreasury).
            let balance_amount: BalanceOf<T> = amount
                .try_into()
                .map_err(|_| Error::<T>::ArithmeticOverflow)?;
            let _imbalance = T::Currency::deposit_creating(recipient, balance_amount);

            // Update total shielded counter.
            TotalShielded::<T>::mutate(|total| {
                *total = total.saturating_sub(amount);
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
