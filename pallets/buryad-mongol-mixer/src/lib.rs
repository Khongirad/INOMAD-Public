//! # pallet-buryad-mongol-mixer — Buryad-Mongol Privacy Mixer
//!
//! Implements the constitutional **ZK Anonymity Pool** for the Altan Network.
//!
//! ## Overview
//!
//! The Buryad-Mongol Mixer provides privacy-preserving transfers using a
//! commitment-nullifier scheme compatible with Groth16 ZK proofs (snarkjs/circom).
//!
//! Deposits are locked in the pallet account and assigned a commitment hash.
//! Withdrawals burn a nullifier (preventing double-spend) and release funds
//! to any recipient via the authorized `RelayerOrigin` (Bank of Siberia).
//!
//! ## Constitutional Constraints
//!
//! - **Denomination**: Deposits must be exact multiples of `MixerDenomination` (1,000 ALTAN)
//!   to maintain anonymity set integrity.
//! - **Fee Split**: `BaseFeePermill + MixerFeePermill` (effective 800 ppm) routed 54/26/10/10
//!   to Khural Foundation / INOMAD AG / Creator / Validators.
//! - **Fee Cap**: `MaxMixerFee` (10 ALTAN) — prevents proportional privacy tax.
//! - **Recharge Attack**: `CommitmentAlreadyExists` blocks re-depositing a spent commitment.
//!
//! ## Governance Origins
//!
//! | Origin | Account | Permission |
//! |---|---|---|
//! | `RelayerOrigin` | Bank of Siberia | `withdraw` |
//! | `BankBoardOrigin` | Bank Board | `reveal_transaction` |
//! | `KhuralOrigin` | Khural | `submit_quarterly_audit` |
//!
//! ## Security
//!
//! Off-chain ZK proof verification is performed by the relayer before calling `withdraw`.
//! The on-chain pallet trusts the `RelayerOrigin` for proof validity — the relayer
//! is a licensed institution bound by the Altan Constitution.
//!
//! ## Dispatchables
//!
//! | Call | Origin | Description |
//! |---|---|---|
//! | `deposit` | Signed | Commit ALTAN into the mixer pool with a blinded commitment |
//! | `withdraw` | Signed | Initiate withdrawal from the mixer pool |
//! | `reveal_transaction` | Signed | Reveal the blinded commitment to complete a transfer |
//! | `submit_quarterly_audit` | Root (Auditor) | Submit an aggregate audit report for compliance |

#![cfg_attr(not(feature = "std"), no_std)]
extern crate alloc;
pub mod migrations;
pub mod weights;
pub use pallet::*;

#[cfg(feature = "runtime-benchmarks")]
mod benchmarking;

#[cfg(test)]
mod mock;
#[cfg(test)]
mod tests;

#[frame_support::pallet]
pub mod pallet {
    use crate::weights::WeightInfo as _;
    use frame_support::{
        pallet_prelude::*,
        traits::{Currency, EnsureOrigin, ExistenceRequirement, Get},
        PalletId,
    };
    use frame_system::pallet_prelude::*;
    use sp_runtime::{
        traits::{AccountIdConversion, Saturating, Zero},
        Permill, RuntimeDebug,
    };

    pub type BalanceOf<T> =
        <<T as Config>::Currency as Currency<<T as frame_system::Config>::AccountId>>::Balance;

    // ── CommitmentState ───────────────────────────────────────────────────────
    /// Защита от Recharge Attack: `Spent` — необратимый финальный статус.
    #[derive(Encode, Decode, Clone, Copy, PartialEq, Eq, RuntimeDebug, TypeInfo, MaxEncodedLen)]
    pub enum CommitmentState {
        Active,
        Spent,
    }

    // ── RevealRecord ──────────────────────────────────────────────────────────
    /// Запись о раскрытии транзакции по судебному ордеру (BankBoard).
    #[derive(Encode, Decode, Clone, PartialEq, Eq, RuntimeDebug, TypeInfo, MaxEncodedLen)]
    pub struct RevealRecord<BlockNumber, AccountId> {
        pub timestamp: BlockNumber,
        pub authorized_by: AccountId,
        pub target_commitment: [u8; 32],
        pub warrant_id: BoundedVec<u8, ConstU32<256>>,
    }

    #[pallet::pallet]
    pub struct Pallet<T>(_);

    // ── Config ────────────────────────────────────────────────────────────────
    #[pallet::config]
    pub trait Config: frame_system::Config<RuntimeEvent: From<Event<Self>>> {
        /// Weight information for extrinsics in this pallet.
        type WeightInfo: crate::weights::WeightInfo;
        type Currency: Currency<Self::AccountId>;

        #[pallet::constant]
        type PalletId: Get<PalletId>;

        /// Denomination step (must be multiple of 10 ALTAN = 10 × 10^12 planck).
        /// Protects the ZK anonymity set by enforcing uniform deposit sizes.
        /// CONSTITUTIONAL: deposits must be multiples of 10 ALTAN.
        #[pallet::constant]
        type MixerDenomination: Get<BalanceOf<Self>>;

        /// Privacy fee: 0.05% = Permill::from_parts(500).
        /// Combined with the base 0.03% fee → total 0.08% charged to depositor.
        #[pallet::constant]
        type MixerFeePermill: Get<Permill>;

        /// Base network fee: 0.03% = Permill::from_parts(300).
        /// Charged in addition to the mixer privacy fee.
        #[pallet::constant]
        type BaseFeePermill: Get<Permill>;

        /// Hard cap on total fee per deposit (e.g. 1000 ALTAN).
        #[pallet::constant]
        type MaxMixerFee: Get<BalanceOf<Self>>;

        // ── Constitutional Fee Recipients (54/26/10/10 — IMMUTABLE) ──────────
        /// Khural Foundation — receives 54% of every mixer fee.
        #[pallet::constant]
        type KhuralFoundationAccount: Get<Self::AccountId>;

        /// INOMAD AG — receives 26% of every mixer fee.
        #[pallet::constant]
        type InomadAgAccount: Get<Self::AccountId>;

        /// Creator / Citizen #1 — receives 10% of every mixer fee.
        /// SS58: 5FTZYAh4tCCXKc8Pu7KYZrD9F3fGeu3YNYhk3gbrGC9n39Wv
        #[pallet::constant]
        type CreatorAccount: Get<Self::AccountId>;

        /// Validator pool — receives 10% of every mixer fee.
        #[pallet::constant]
        type ValidatorsPoolAccount: Get<Self::AccountId>;

        // ── Origins (Разделение Властей) ─────────────────────────────────────
        /// Bank of Siberia — единственный origin для `withdraw` (MEV защита).
        type RelayerOrigin: EnsureOrigin<Self::RuntimeOrigin, Success = Self::AccountId>;

        /// Правление Банка — раскрытие транзакций по судебному ордеру.
        type BankBoardOrigin: EnsureOrigin<Self::RuntimeOrigin, Success = Self::AccountId>;

        /// Хурал — подписание квартальных аудитов.
        type KhuralOrigin: EnsureOrigin<Self::RuntimeOrigin>;
    }

    // ── Helpers ───────────────────────────────────────────────────────────────
    impl<T: Config> Pallet<T> {
        pub fn pool_account() -> T::AccountId {
            T::PalletId::get().into_account_truncating()
        }

        /// Total fee = base (0.03%) + privacy (0.05%) = 0.08%, capped at MaxMixerFee.
        ///
        /// Exclusive fee math: pool receives exactly `amount`.
        /// Depositor pays `amount + total_fee`.
        pub fn calculate_total_fee(amount: BalanceOf<T>) -> BalanceOf<T> {
            let base_fee = T::BaseFeePermill::get().mul_floor(amount);
            let privacy_fee = T::MixerFeePermill::get().mul_floor(amount);
            let raw = base_fee.saturating_add(privacy_fee);
            core::cmp::min(raw, T::MaxMixerFee::get())
        }
    }

    // ── Storage ───────────────────────────────────────────────────────────────
    /// None=неизвестен | Some(Active)=в пуле | Some(Spent)=потрачен навсегда.
    #[pallet::storage]
    #[pallet::getter(fn commitments)]
    pub type Commitments<T: Config> =
        StorageMap<_, Blake2_128Concat, [u8; 32], CommitmentState, OptionQuery>;

    /// Потраченные nullifiers → блок вывода (аудит).
    #[pallet::storage]
    #[pallet::getter(fn spent_nullifiers)]
    pub type SpentNullifiers<T: Config> =
        StorageMap<_, Blake2_128Concat, [u8; 32], BlockNumberFor<T>, OptionQuery>;

    /// Число активных листьев.
    #[pallet::storage]
    #[pallet::getter(fn pool_leaf_count)]
    pub type PoolLeafCount<T: Config> = StorageValue<_, u64, ValueQuery>;

    /// Монотонный счётчик записей AuditLog.
    #[pallet::storage]
    pub type AuditLogId<T: Config> = StorageValue<_, u64, ValueQuery>;

    /// Журнал аудита: log_id → RevealRecord.
    #[pallet::storage]
    #[pallet::getter(fn audit_logs)]
    pub type AuditLogs<T: Config> = StorageMap<
        _,
        Twox64Concat,
        u64,
        RevealRecord<BlockNumberFor<T>, T::AccountId>,
        OptionQuery,
    >;

    /// Квартальные аудиты Хурала: quarter_id (YYYYQ) → report_hash.
    #[pallet::storage]
    #[pallet::getter(fn quarterly_audits)]
    pub type QuarterlyAudits<T: Config> = StorageMap<_, Twox64Concat, u32, [u8; 32], OptionQuery>;

    // ── Events ────────────────────────────────────────────────────────────────
    #[pallet::event]
    #[pallet::generate_deposit(pub(super) fn deposit_event)]
    pub enum Event<T: Config> {
        /// Deposit accepted. Exclusive fee: pool received exactly `amount`.
        /// Depositor paid `amount + total_fee`.
        Deposited {
            depositor: T::AccountId,
            commitment: [u8; 32],
            /// Pool received exactly this (exclusive fee math).
            amount: BalanceOf<T>,
            /// Total fee paid on top of `amount` (0.08% = 0.03% base + 0.05% privacy).
            total_fee: BalanceOf<T>,
        },
        Withdrawn {
            nullifier: [u8; 32],
            recipient: T::AccountId,
            relayer: T::AccountId,
            amount: BalanceOf<T>,
        },
        /// Секретность транзакции снята: BankBoard + судебный ордер.
        SecrecyLifted {
            target_commitment: [u8; 32],
            authorized_by: T::AccountId,
            warrant_id: BoundedVec<u8, ConstU32<256>>,
        },
        /// Квартальный аудит подписан Хуралом.
        QuarterlyAuditSigned {
            quarter_id: u32,
            report_hash: [u8; 32],
        },
    }

    // ── Errors ────────────────────────────────────────────────────────────────
    #[pallet::error]
    pub enum Error<T> {
        CommitmentAlreadyExists,
        CommitmentNotFound,
        NullifierAlreadySpent,
        ArithmeticOverflow,
        InsufficientFunds,
        PoolTransferFailed,
        DepositTooSmall,
        /// Deposit amount is not a multiple of 10 ALTAN.
        /// Required to protect the ZK Anonymity Set (uniform deposit sizes).
        InvalidDenomination,
        /// Квартальный аудит уже подписан для данного quarter_id.
        AuditAlreadySigned,
    }

    // ── Extrinsics ────────────────────────────────────────────────────────────
    #[pallet::call]
    impl<T: Config> Pallet<T> {
        // ─── deposit ─────────────────────────────────────────────────────────
        /// Deposit `amount` into the mixer pool with **exclusive fee math**.
        ///
        /// ## Exclusive Fee + Double Fee (0.08%)
        ///
        /// ```text
        /// base_fee      = 0.03% × amount    (network fee, same as transfer_with_fee)
        /// privacy_fee   = 0.05% × amount    (mixer anonymity premium)
        /// total_fee     = min(base + privacy, MaxMixerFee cap)
        ///
        /// Pool receives:   amount            ← exactly, no deduction
        /// Depositor pays:  amount + total_fee ← exclusive: fee is additive
        ///
        /// Constitutional 54/26/10/10 split of total_fee:
        ///   ag_share      = 26% → InomadAgAccount
        ///   creator_share = 10% → CreatorAccount (Citizen #1)
        ///   val_share     = 10% → ValidatorsPoolAccount
        ///   khural_share  = remainder (54% + dust) → KhuralFoundationAccount
        /// ```
        ///
        /// Denomination constraint: `amount` must be a multiple of 10 ALTAN.
        /// This protects the ZK anonymity set by ensuring uniform deposit sizes.
        #[pallet::call_index(0)]
        #[pallet::weight(T::WeightInfo::deposit())]
        pub fn deposit(
            origin: OriginFor<T>,
            amount: BalanceOf<T>,
            commitment: [u8; 32],
            encrypted_audit_payload: BoundedVec<u8, ConstU32<64>>,
        ) -> DispatchResult {
            let depositor = ensure_signed(origin)?;
            let denom = T::MixerDenomination::get();

            // Denomination guard: must be ≥ 10 ALTAN and a multiple of 10 ALTAN.
            // CONSTITUTIONAL: protects ZK anonymity set (uniform deposit sizes).
            ensure!(amount >= denom, Error::<T>::DepositTooSmall);
            ensure!(
                amount % denom == Zero::zero(),
                Error::<T>::InvalidDenomination
            );
            ensure!(
                Commitments::<T>::get(&commitment).is_none(),
                Error::<T>::CommitmentAlreadyExists
            );

            // ── Exclusive Fee Math ──────────────────────────────────────────
            // Total fee = 0.03% (base) + 0.05% (privacy) = 0.08%, capped at MaxMixerFee.
            // Pool receives exactly `amount`. Depositor pays amount + total_fee.
            let total_fee = Self::calculate_total_fee(amount);

            // Constitutional 54/26/10/10 split of total_fee
            let ag_share = Permill::from_percent(26).mul_floor(total_fee);
            let creator_share = Permill::from_percent(10).mul_floor(total_fee);
            let val_share = Permill::from_percent(10).mul_floor(total_fee);
            let khural_share = total_fee
                .saturating_sub(ag_share)
                .saturating_sub(creator_share)
                .saturating_sub(val_share);

            let pool = Self::pool_account();

            // 1. Pool receives exactly `amount` (exclusive fee — depositor pays extra)
            T::Currency::transfer(&depositor, &pool, amount, ExistenceRequirement::KeepAlive)
                .map_err(|_| Error::<T>::InsufficientFunds)?;

            // 2. INOMAD AG (26%)
            if ag_share > Zero::zero() {
                T::Currency::transfer(
                    &depositor,
                    &T::InomadAgAccount::get(),
                    ag_share,
                    ExistenceRequirement::KeepAlive,
                )
                .map_err(|_| Error::<T>::InsufficientFunds)?;
            }

            // 3. Creator / Citizen #1 (10% — sovereign founder royalty)
            if creator_share > Zero::zero() {
                T::Currency::transfer(
                    &depositor,
                    &T::CreatorAccount::get(),
                    creator_share,
                    ExistenceRequirement::KeepAlive,
                )
                .map_err(|_| Error::<T>::InsufficientFunds)?;
            }

            // 4. Validator Pool (10%)
            if val_share > Zero::zero() {
                T::Currency::transfer(
                    &depositor,
                    &T::ValidatorsPoolAccount::get(),
                    val_share,
                    ExistenceRequirement::KeepAlive,
                )
                .map_err(|_| Error::<T>::InsufficientFunds)?;
            }

            // 5. Khural Foundation (54% + dust)
            if khural_share > Zero::zero() {
                T::Currency::transfer(
                    &depositor,
                    &T::KhuralFoundationAccount::get(),
                    khural_share,
                    ExistenceRequirement::KeepAlive,
                )
                .map_err(|_| Error::<T>::InsufficientFunds)?;
            }

            Commitments::<T>::insert(&commitment, CommitmentState::Active);
            PoolLeafCount::<T>::mutate(|n| *n = n.saturating_add(1));
            let _ = encrypted_audit_payload;

            Self::deposit_event(Event::Deposited {
                depositor,
                commitment,
                amount,
                total_fee,
            });
            Ok(())
        }

        // ─── withdraw ────────────────────────────────────────────────────────
        /// Withdraw `amount` from the pool to `recipient`.
        ///
        /// **Only `RelayerOrigin`** (Bank of Siberia — MEV protection).
        /// No fee on withdrawal — fee was collected at deposit time (exclusive math).
        /// Recipient receives exactly `amount`.
        #[pallet::call_index(1)]
        #[pallet::weight(T::WeightInfo::withdraw())]
        pub fn withdraw(
            origin: OriginFor<T>,
            nullifier: [u8; 32],
            commitment: [u8; 32],
            recipient: T::AccountId,
            amount: BalanceOf<T>,
        ) -> DispatchResult {
            let relayer = T::RelayerOrigin::ensure_origin(origin)?;
            let denom = T::MixerDenomination::get();

            ensure!(amount >= denom, Error::<T>::DepositTooSmall);
            ensure!(
                amount % denom == Zero::zero(),
                Error::<T>::InvalidDenomination
            );
            ensure!(
                SpentNullifiers::<T>::get(&nullifier).is_none(),
                Error::<T>::NullifierAlreadySpent
            );
            ensure!(
                Commitments::<T>::get(&commitment) == Some(CommitmentState::Active),
                Error::<T>::CommitmentNotFound
            );

            let now = frame_system::Pallet::<T>::block_number();
            let pool = Self::pool_account();

            // Mark as Spent — irreversible (Recharge Attack protection)
            Commitments::<T>::insert(&commitment, CommitmentState::Spent);
            SpentNullifiers::<T>::insert(&nullifier, now);
            PoolLeafCount::<T>::mutate(|n| *n = n.saturating_sub(1));

            // Pool → recipient: exactly `amount` (fee was paid at deposit, not here)
            T::Currency::transfer(&pool, &recipient, amount, ExistenceRequirement::KeepAlive)
                .map_err(|_| Error::<T>::PoolTransferFailed)?;

            Self::deposit_event(Event::Withdrawn {
                nullifier,
                recipient,
                relayer,
                amount,
            });
            Ok(())
        }

        // ─── reveal_transaction ───────────────────────────────────────────────
        /// Раскрыть транзакцию по судебному ордеру.
        ///
        /// **Только `BankBoardOrigin`** (Правление Банка Сибири).
        #[pallet::call_index(2)]
        #[pallet::weight(T::WeightInfo::reveal_transaction())]
        pub fn reveal_transaction(
            origin: OriginFor<T>,
            target_commitment: [u8; 32],
            warrant_id: BoundedVec<u8, ConstU32<256>>,
        ) -> DispatchResult {
            let authorized_by = T::BankBoardOrigin::ensure_origin(origin)?;
            let now = frame_system::Pallet::<T>::block_number();
            let log_id = AuditLogId::<T>::get();

            AuditLogs::<T>::insert(
                log_id,
                RevealRecord {
                    timestamp: now,
                    authorized_by: authorized_by.clone(),
                    target_commitment,
                    warrant_id: warrant_id.clone(),
                },
            );
            AuditLogId::<T>::put(log_id.saturating_add(1));

            Self::deposit_event(Event::SecrecyLifted {
                target_commitment,
                authorized_by,
                warrant_id,
            });
            Ok(())
        }

        // ─── submit_quarterly_audit ───────────────────────────────────────────
        /// Подписать квартальный аудиторский отчёт.
        ///
        /// **Только `KhuralOrigin`** (Законодательная ветвь).
        #[pallet::call_index(3)]
        #[pallet::weight(T::WeightInfo::submit_quarterly_audit())]
        pub fn submit_quarterly_audit(
            origin: OriginFor<T>,
            quarter_id: u32,
            report_hash: [u8; 32],
        ) -> DispatchResult {
            T::KhuralOrigin::ensure_origin(origin)?;
            ensure!(
                QuarterlyAudits::<T>::get(&quarter_id).is_none(),
                Error::<T>::AuditAlreadySigned
            );

            QuarterlyAudits::<T>::insert(&quarter_id, report_hash);
            Self::deposit_event(Event::QuarterlyAuditSigned {
                quarter_id,
                report_hash,
            });
            Ok(())
        }
    }
}
