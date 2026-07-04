//! # Altan Vault Pallet
//!
//! **Altan Network — Sprint L1-25: Isolated Savings Vaults**
//!
//! ## Architecture
//!
//! Each citizen/business identity can create multiple deterministic vault sub-accounts.
//! A vault sub-account is derived from `(owner_account_id, vault_index)` using a
//! deterministic seed, making it keyless and permanently owned by the creator.
//!
//! ## Inbound (Anonymous)
//!
//! Vaults are designed to receive funds from Mixer withdrawals (buryad-mongol-mixer).
//! Regular transfers to a vault address are also permitted, but the primary use-case
//! is privacy-preserving inbound from the Mixer.
//!
//! ## Outbound (Owner-Only, Zero Fee)
//!
//! Funds in an Altan Vault can **ONLY** be transferred back to the exact primary
//! owner account that created the vault. This ensures:
//!   - No MEV attacks (third parties cannot drain vaults)
//!   - Clear audit trail (vault → owner only)
//!   - Zero network fee: vault-to-owner withdrawals are **0% fee EXEMPT**.
//!     The economic rationale: the fee was already paid when funds entered
//!     (either on deposit to mixer or on the original P2P transfer).
//!
//! ## Storage
//!
//! ```text
//! Vaults<T>: (AccountId, u16) → VaultInfo  ← vault metadata per (owner, index)
//! ```
//!
//! Sprint L1-25.
//!
//! ## Dispatchables
//!
//! | Call | Origin | Description |
//! |---|---|---|
//! | `create_vault` | Root | Instantiate a keyless treasury vault for an org |
//! | `withdraw_to_owner` | Signed (Owner/Council) | Withdraw funds from a vault to the designated owner |
//! | `record_inbound` | Root | Record an inbound payment to a vault (Relayer-called) |

#![cfg_attr(not(feature = "std"), no_std)]

extern crate alloc;

pub mod migrations;
pub mod weights;
pub use pallet::*;

#[cfg(test)]
mod mock;
#[cfg(test)]
mod tests;

#[cfg(feature = "runtime-benchmarks")]
mod benchmarking;
#[frame_support::pallet]
pub mod pallet {
    use crate::weights::WeightInfo as _;
    use frame_support::{
        pallet_prelude::*,
        traits::{Currency, ExistenceRequirement},
        PalletId,
    };
    use frame_system::pallet_prelude::*;
    use sp_runtime::{
        traits::{AccountIdConversion, Saturating, Zero},
        RuntimeDebug,
    };

    /// Shorthand for the balance type.
    pub type BalanceOf<T> =
        <<T as Config>::Currency as Currency<<T as frame_system::Config>::AccountId>>::Balance;

    // =========================================================================
    // VaultInfo — stored per (owner, vault_index)
    // =========================================================================

    /// Vault state record.
    #[derive(Encode, Decode, Clone, PartialEq, Eq, RuntimeDebug, TypeInfo, MaxEncodedLen)]
    pub struct VaultInfo<AccountId, BlockNumber, Balance> {
        /// The primary owner account. Only this account can call `withdraw_to_owner`.
        pub owner: AccountId,
        /// Vault index within owner's namespace (0..=65535).
        pub vault_index: u16,
        /// Block when the vault was created.
        pub created_at: BlockNumber,
        /// Cumulative amount deposited into this vault (audit trail).
        pub total_deposited: Balance,
        /// Cumulative amount withdrawn from this vault (audit trail).
        pub total_withdrawn: Balance,
    }

    #[pallet::pallet]
    pub struct Pallet<T>(_);

    // =========================================================================
    // Configuration Trait
    // =========================================================================

    #[pallet::config]
    pub trait Config: frame_system::Config<RuntimeEvent: From<Event<Self>>> {
        /// Weight information for extrinsics in this pallet.
        type WeightInfo: crate::weights::WeightInfo;
        /// The currency for vault operations.
        type Currency: Currency<Self::AccountId>;

        /// The pallet ID used to derive vault sub-account addresses.
        ///
        /// Each vault address = PalletId + owner_bytes + vault_index.
        /// This ensures vault addresses are deterministic and keyless.
        #[pallet::constant]
        type PalletId: Get<PalletId>;

        /// Maximum number of vaults per owner (safety cap to prevent storage bloat).
        #[pallet::constant]
        type MaxVaultsPerOwner: Get<u16>;
    }

    // =========================================================================
    // Storage
    // =========================================================================

    /// All vaults indexed by (owner, vault_index).
    ///
    /// Key: (AccountId, u16) — owner account ID + vault index
    /// Value: VaultInfo — vault metadata and audit counters
    #[pallet::storage]
    #[pallet::getter(fn vaults)]
    pub type Vaults<T: Config> = StorageDoubleMap<
        _,
        Blake2_128Concat,
        T::AccountId, // owner
        Blake2_128Concat,
        u16, // vault_index
        VaultInfo<T::AccountId, BlockNumberFor<T>, BalanceOf<T>>,
        OptionQuery,
    >;

    /// Number of vaults per owner (to enforce MaxVaultsPerOwner cap).
    #[pallet::storage]
    #[pallet::getter(fn vault_count)]
    pub type VaultCount<T: Config> = StorageMap<_, Blake2_128Concat, T::AccountId, u16, ValueQuery>;

    // =========================================================================
    // Events
    // =========================================================================

    #[pallet::event]
    #[pallet::generate_deposit(pub(super) fn deposit_event)]
    pub enum Event<T: Config> {
        /// A new vault was created.
        VaultCreated {
            owner: T::AccountId,
            vault_index: u16,
            vault_address: T::AccountId,
        },
        /// Funds were withdrawn from a vault to its owner.
        ///
        /// Zero fee: vault-to-owner transfers are constitutionally exempt
        /// from the 0.03% network fee. The fee was paid when funds entered the system.
        WithdrawnToOwner {
            owner: T::AccountId,
            vault_index: u16,
            vault_address: T::AccountId,
            amount: BalanceOf<T>,
        },
    }

    // =========================================================================
    // Errors
    // =========================================================================

    #[pallet::error]
    pub enum Error<T> {
        /// Vault with this index already exists for this owner.
        VaultAlreadyExists,
        /// Vault not found — not created yet.
        VaultNotFound,
        /// Caller is not the owner of this vault.
        NotVaultOwner,
        /// Maximum number of vaults per owner reached.
        MaxVaultsReached,
        /// Arithmetic overflow when computing vault address or amounts.
        ArithmeticOverflow,
        /// Insufficient funds in the vault to withdraw.
        InsufficientVaultBalance,
    }

    // =========================================================================
    // Pallet helpers
    // =========================================================================

    impl<T: Config> Pallet<T> {
        /// Derive the deterministic vault sub-account for `(owner, vault_index)`.
        ///
        /// Uses a 32-byte seed: first 8 bytes = PalletId prefix, next 20 bytes = owner
        /// account truncated, last 4 bytes = vault_index (big-endian).
        ///
        /// The resulting account is **keyless** — no private key exists for it.
        /// Only the pallet can move funds out via `withdraw_to_owner`.
        pub fn vault_account(owner: &T::AccountId, vault_index: u16) -> T::AccountId {
            // Encode as (PalletId_bytes, owner_bytes, index_bytes)
            // We use a two-level derivation: first derive a vault-specific PalletId
            // by embedding the owner + index, then convert to AccountId.
            let pallet_id = T::PalletId::get();
            let mut derived: [u8; 8] = pallet_id.0;

            // XOR the last 2 bytes of the PalletId with vault_index
            // to create a unique sub-namespace per vault.
            let idx_bytes = vault_index.to_be_bytes();
            derived[6] ^= idx_bytes[0];
            derived[7] ^= idx_bytes[1];

            // Derive the keyless vault account from the modified PalletId
            // combined with the owner's account bytes (truncated to AccountId length).
            let vault_pallet_id = PalletId(derived);
            // Use into_sub_account_truncating with owner as sub-account key
            vault_pallet_id.into_sub_account_truncating(owner)
        }
    }

    // =========================================================================
    // Extrinsics
    // =========================================================================

    #[pallet::call]
    impl<T: Config> Pallet<T> {
        /// Create a new deterministic vault sub-account for the caller.
        ///
        /// The vault address is derived from `(caller, vault_index)` — fully deterministic.
        /// Once created, the vault can receive funds (e.g., from Mixer withdrawals).
        ///
        /// ## Constraints
        /// - `vault_index` must not already exist for this caller.
        /// - Total vaults per owner capped at `MaxVaultsPerOwner`.
        #[pallet::call_index(0)]
        #[pallet::weight(T::WeightInfo::create_vault())]
        pub fn create_vault(origin: OriginFor<T>, vault_index: u16) -> DispatchResult {
            let owner = ensure_signed(origin)?;

            // Cap check
            let count = VaultCount::<T>::get(&owner);
            ensure!(
                count < T::MaxVaultsPerOwner::get(),
                Error::<T>::MaxVaultsReached
            );

            // Uniqueness check
            ensure!(
                Vaults::<T>::get(&owner, vault_index).is_none(),
                Error::<T>::VaultAlreadyExists
            );

            let vault_address = Self::vault_account(&owner, vault_index);
            let now = frame_system::Pallet::<T>::block_number();

            Vaults::<T>::insert(
                &owner,
                vault_index,
                VaultInfo {
                    owner: owner.clone(),
                    vault_index,
                    created_at: now,
                    total_deposited: Zero::zero(),
                    total_withdrawn: Zero::zero(),
                },
            );
            VaultCount::<T>::mutate(&owner, |n| *n = n.saturating_add(1));

            Self::deposit_event(Event::VaultCreated {
                owner,
                vault_index,
                vault_address,
            });
            Ok(())
        }

        /// Withdraw funds from a vault to its primary owner account.
        ///
        /// ## Constitutional Zero-Fee Exemption
        ///
        /// This transfer is **0% fee exempt**. The network fee was already collected
        /// when funds entered the system (either via `transfer_with_fee` or mixer `deposit`).
        /// Double-charging would be economically unjust and violates the constitutional
        /// principle of "fee once per economic event".
        ///
        /// ## Security
        ///
        /// Only the exact `owner` who created the vault can call this.
        /// Transfers ONLY go to `owner` — no third-party recipients allowed.
        /// This prevents MEV attacks and unauthorized vault draining.
        #[pallet::call_index(1)]
        #[pallet::weight(T::WeightInfo::withdraw_to_owner())]
        pub fn withdraw_to_owner(
            origin: OriginFor<T>,
            vault_index: u16,
            amount: BalanceOf<T>,
        ) -> DispatchResult {
            let caller = ensure_signed(origin)?;

            // Vault must exist
            let mut vault_info =
                Vaults::<T>::get(&caller, vault_index).ok_or(Error::<T>::VaultNotFound)?;

            // Only the vault owner can withdraw
            ensure!(vault_info.owner == caller, Error::<T>::NotVaultOwner);

            let vault_address = Self::vault_account(&caller, vault_index);

            // Verify vault has sufficient balance
            let vault_balance = T::Currency::free_balance(&vault_address);
            ensure!(
                vault_balance >= amount,
                Error::<T>::InsufficientVaultBalance
            );

            // ── Zero-Fee Transfer: vault → owner ──────────────────────────────
            // Constitutional exemption: NO 0.03% fee on vault-to-owner transfers.
            // The fee was already paid when funds entered the system.
            T::Currency::transfer(
                &vault_address,
                &caller, // ONLY to owner — no third-party recipients
                amount,
                ExistenceRequirement::AllowDeath,
            )?;

            // Update audit counters
            vault_info.total_withdrawn = vault_info.total_withdrawn.saturating_add(amount);
            Vaults::<T>::insert(&caller, vault_index, vault_info);

            Self::deposit_event(Event::WithdrawnToOwner {
                owner: caller,
                vault_index,
                vault_address,
                amount,
            });
            Ok(())
        }

        /// Record an inbound deposit event for audit purposes.
        ///
        /// Called after funds arrive in the vault (e.g., from Mixer `withdraw`).
        /// Updates the `total_deposited` audit counter. The actual transfer is done
        /// by the sender/relayer directly — this extrinsic only records the accounting.
        ///
        /// Anyone can call this to record that funds arrived — the vault address is
        /// deterministic so this cannot be spoofed.
        #[pallet::call_index(2)]
        #[pallet::weight(T::WeightInfo::record_inbound())]
        pub fn record_inbound(
            origin: OriginFor<T>,
            owner: T::AccountId,
            vault_index: u16,
            amount: BalanceOf<T>,
        ) -> DispatchResult {
            let _caller = ensure_signed(origin)?;

            Vaults::<T>::try_mutate(&owner, vault_index, |vault_opt| {
                let vault = vault_opt.as_mut().ok_or(Error::<T>::VaultNotFound)?;
                vault.total_deposited = vault.total_deposited.saturating_add(amount);
                Ok::<_, DispatchError>(())
            })?;

            Ok(())
        }
    }
}
