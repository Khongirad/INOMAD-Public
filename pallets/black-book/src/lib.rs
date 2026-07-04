//! pallet-black-book — Altan Network: Чёрная Книга (Black Book of the Confederation)
//!
//! Constitutional Bounty Hunting & Warrant System.
//!
//! This pallet implements the "Wall of Shame" — the on-chain registry of
//! condemned criminals, fugitives, and traitors — along with a fully
//! public, crowdfunded bounty pool for fugitive capture.
//!
//! TIMELINE FOR A WARRANT:
//!   1. Root (Khural/Sudo) calls `condemn_and_issue_warrant`:
//!      - [VECTOR 3] Checks: target must NOT be an Academician or active Khural Delegate.
//!        If so, returns `RequiresImpeachment`. High officials can only be removed via
//!        the constitutional impeachment process, NOT by direct exile.
//!      - Target citizen status set to `Exiled` (via IdentityInterface bridge).
//!      - ALL target funds slashed (transferred) to StateTreasury.
//!      - Crime record written to `WallOfShame`.
//!      - If `is_fugitive == true`:
//!          * `FugitiveStatus::AtLarge` set.
//!          * `initial_bounty` transferred from Treasury → `BountyPools[target]`.
//!   2. Any citizen may call `donate_to_bounty`:
//!      - Donates from their free balance into `BountyPools[target]`.
//!      - Target must be `AtLarge` (rejected if already `Captured`).
//!   3. Root (or Police Guild via future upgrade) calls `register_capture_and_payout`:
//!      - [VECTOR 1] Instead of immediate payout, bounty is LOCKED for `BountyLockPeriod`
//!        blocks to prevent collusion (fake capture = bounty laundering).
//!      - Status flipped to `FugitiveStatus::Captured`.
//!      - Locked payout record created in `LockedBountyPayouts`.
//!   4. Bounty hunter calls `claim_bounty_payout` after lock period expires.
//!   5. Root may call `cancel_bounty_payout` to veto a payout if collusion is proven.
//!
//! ANTI-TYRANNY (VECTOR 3):
//!   - Academicians (pallet-guilds AcademyMembers) cannot be exiled directly.
//!   - Active Khural Delegates (KhuralDelegate / ConfederationDelegate role) cannot
//!     be exiled directly. They must first face impeachment via the Khural.
//!
//! ECONOMIC SECURITY:
//!   - Confiscation is done via `T::Currency::slash` — funds are destroyed then
//!     re-emitted to Treasury via `T::Currency::deposit_creating`. This is the
//!     same flow used by `pallet-treasury` slashes.
//!   - Bounty pool is kept as free balance on a deterministic PalletId-derived
//!     AccountId per fugitive (sub-account pattern), so the Treasury can top it up
//!     via a normal transfer and citizens donate via a normal transfer.
//!
//! ## Dispatchables
//!
//! | Call | Origin | Description |
//! |---|---|---|
//! | `condemn_and_issue_warrant` | Signed (Judicial Origin) | Condemn a citizen and post an arrest warrant with bounty |
//! | `donate_to_bounty` | Signed | Contribute additional ALTAN to an existing bounty |
//! | `register_capture_and_payout` | Signed (State Officer) | Record a successful capture and trigger bounty payout |
//! | `claim_bounty_payout` | Signed (Captor) | Claim the bounty reward after capture is verified |
//! | `cancel_bounty_payout` | Root | Cancel an expired or erroneous bounty (emergency) |

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
// BlackBookIdentityInterface — loose coupling to pallet-inomad-identity
// =========================================================================

/// Cross-pallet trait: allows `pallet-black-book` to exile a citizen without
/// tightly coupling to `pallet-inomad-identity` storage.
pub trait BlackBookIdentityInterface<AccountId> {
    /// Set `who`'s status to `CitizenStatus::Exiled` (terminal).
    fn exile_citizen(who: &AccountId) -> frame_support::dispatch::DispatchResult;
}

// =========================================================================
// KhuralDelegateInterface — loose coupling to pallet-khural-governance
// =========================================================================

/// Cross-pallet trait: allows `pallet-black-book` to check whether an account
/// is an active Khural or Confederation Delegate without tight-coupling to
/// `pallet-khural-governance` or `pallet-inomad-identity` internals.
///
/// Wire at runtime:
/// ```ignore
/// pub struct KhuralDelegateBridge;
/// impl pallet_black_book::KhuralDelegateInterface<AccountId> for KhuralDelegateBridge {
///     fn is_active_khural_delegate(who: &AccountId) -> bool {
///         use pallet_inomad_identity::pallet::CitizenRole;
///         pallet_inomad_identity::Citizens::<Runtime>::get(who)
///             .map(|r| matches!(
///                 r.role,
///                 CitizenRole::KhuralDelegate | CitizenRole::ConfederationDelegate
///             ))
///             .unwrap_or(false)
///     }
/// }
/// ```
pub trait KhuralDelegateInterface<AccountId> {
    /// Returns `true` if `who` holds a `KhuralDelegate` or `ConfederationDelegate` role
    /// and has an active citizen status.
    fn is_active_khural_delegate(who: &AccountId) -> bool;
}

#[cfg(feature = "runtime-benchmarks")]
mod benchmarking;
#[frame_support::pallet]
pub mod pallet {
    use crate::weights::WeightInfo as _;
    use crate::BlackBookIdentityInterface;
    use frame_support::sp_runtime::Saturating;
    use frame_support::{
        pallet_prelude::*,
        traits::{Currency, ExistenceRequirement, ReservableCurrency},
    };
    use frame_system::pallet_prelude::*;
    use sp_core::H256;
    // Bring trait methods into scope so T::AcademyChecker::is_academician() resolves.
    use pallet_guilds::AcademyInterface;
    // Bring trait methods into scope so T::KhuralChecker::is_active_khural_delegate() resolves.
    use crate::KhuralDelegateInterface;

    // =========================================================================
    // Currency type alias
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
        /// Currency for confiscation, bounty pool management, and citizen donations.
        type Currency: Currency<Self::AccountId> + ReservableCurrency<Self::AccountId>;

        /// Bridge to `pallet-inomad-identity` — sets `CitizenStatus::Exiled`.
        type IdentityBridge: crate::BlackBookIdentityInterface<Self::AccountId>;

        /// [VECTOR 3] Bridge to check Academy membership (pallet-guilds).
        type AcademyChecker: pallet_guilds::AcademyInterface<Self::AccountId>;

        /// [VECTOR 3] Bridge to check Khural Delegate status.
        type KhuralChecker: crate::KhuralDelegateInterface<Self::AccountId>;

        /// The on-chain Treasury account — receives confiscated criminal funds
        /// and provides initial bounties for fugitives.
        #[pallet::constant]
        type StateTreasury: Get<Self::AccountId>;

        /// [VECTOR 1] The number of blocks a bounty payout is locked after capture
        /// is registered. This prevents bounty laundering via colluding "captures".
        /// Example: ~50400 blocks ≈ 7 days at 12s/block.
        #[pallet::constant]
        type BountyLockPeriod: Get<u32>;
    }

    // =========================================================================
    // Enums
    // =========================================================================

    /// Constitutional crime category recorded in the Black Book.
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
    pub enum CrimeCategory {
        /// Abuse of public office for personal gain.
        Corruption,
        /// Crimes against humanity committed during armed conflict.
        WarCrime,
        /// Betrayal of the Confederation (treason).
        HighTreason,
        /// Large-scale theft of public or private property.
        GrandTheft,
    }

    /// Whether the condemned criminal remains at large or has been apprehended.
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
    pub enum FugitiveStatus {
        /// The criminal has evaded justice — BountyPool is OPEN.
        AtLarge,
        /// The criminal has been apprehended — BountyPool has been locked for vesting.
        Captured,
    }

    // =========================================================================
    // Structs
    // =========================================================================

    /// On-chain crime record for a condemned citizen.
    ///
    /// Stored in `WallOfShame` keyed by the criminal's `AccountId`.
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
    pub struct CrimeRecord {
        /// Constitutional crime classification.
        pub category: CrimeCategory,
        /// H256 hash of the verdict document (stored on IPFS — only hash on-chain).
        pub verdict_hash: H256,
        /// Block number when the warrant was issued.
        pub timestamp: u32,
        /// Current fugitive status (`AtLarge` or `Captured`).
        /// `None` if the criminal is NOT a fugitive (e.g. already imprisoned).
        /// `Some(AtLarge)` / `Some(Captured)` if a BountyPool exists or existed.
        pub fugitive_status: Option<FugitiveStatus>,
    }

    /// [VECTOR 1] A vesting-locked bounty payout pending the anti-laundering cooldown.
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
    pub struct LockedPayout<T: Config> {
        /// The account that registered the capture and will receive the bounty.
        pub bounty_hunter: T::AccountId,
        /// Block number after which the payout can be claimed.
        pub unlock_block: u32,
        /// Total bounty amount locked.
        pub amount: BalanceOf<T>,
    }

    // =========================================================================
    // Storage
    // =========================================================================

    /// Wall of Shame — AccountId → CrimeRecord.
    ///
    /// Every condemned citizen who receives a Root-issued warrant is entered here.
    /// Records are NEVER deleted — the chain serves as a permanent public ledger.
    #[pallet::storage]
    #[pallet::getter(fn wall_of_shame)]
    pub type WallOfShame<T: Config> =
        StorageMap<_, Blake2_128Concat, T::AccountId, CrimeRecord, OptionQuery>;

    /// Bounty pools — AccountId (fugitive) → Balance (total reward pool).
    ///
    /// Funded by the Treasury (initial bounty) and public donations.
    /// Cleared and locked on successful capture registration.
    /// Only exists for fugitives with `FugitiveStatus::AtLarge`.
    #[pallet::storage]
    #[pallet::getter(fn bounty_pools)]
    pub type BountyPools<T: Config> =
        StorageMap<_, Blake2_128Concat, T::AccountId, BalanceOf<T>, OptionQuery>;

    /// [VECTOR 1] Locked bounty payouts awaiting anti-laundering cooldown.
    ///
    /// Once `register_capture_and_payout` is called, funds are locked for
    /// `T::BountyLockPeriod` blocks. Root can cancel via `cancel_bounty_payout`.
    /// After unlock, the hunter calls `claim_bounty_payout`.
    #[pallet::storage]
    #[pallet::getter(fn locked_bounty_payouts)]
    pub type LockedBountyPayouts<T: Config> =
        StorageMap<_, Blake2_128Concat, T::AccountId, LockedPayout<T>, OptionQuery>;

    // =========================================================================
    // Events
    // =========================================================================

    #[pallet::event]
    #[pallet::generate_deposit(pub(super) fn deposit_event)]
    pub enum Event<T: Config> {
        /// A citizen has been condemned and a warrant issued.
        WarrantIssued {
            target: T::AccountId,
            category: CrimeCategory,
            verdict_hash: H256,
            confiscated: BalanceOf<T>,
            is_fugitive: bool,
            initial_bounty: BalanceOf<T>,
        },
        /// A citizen donated to a fugitive's bounty pool.
        BountyDonation {
            donor: T::AccountId,
            fugitive: T::AccountId,
            amount: BalanceOf<T>,
            new_total: BalanceOf<T>,
        },
        /// [VECTOR 1] A fugitive has been captured — bounty locked for vesting period.
        FugitiveCapturedBountyLocked {
            fugitive: T::AccountId,
            bounty_hunter: T::AccountId,
            bounty_amount: BalanceOf<T>,
            unlock_block: u32,
        },
        /// [VECTOR 1] Bounty hunter successfully claimed their vested payout.
        BountyPayoutClaimed {
            fugitive: T::AccountId,
            bounty_hunter: T::AccountId,
            amount: BalanceOf<T>,
        },
        /// [VECTOR 1] Root cancelled a suspicious bounty payout (proven collusion).
        BountyPayoutCancelled {
            fugitive: T::AccountId,
            bounty_hunter: T::AccountId,
            amount: BalanceOf<T>,
        },
    }

    // =========================================================================
    // Errors
    // =========================================================================

    #[pallet::error]
    pub enum Error<T> {
        /// Target account has no citizen record in `WallOfShame`.
        NotInBlackBook,
        /// The criminal is not a fugitive or is already captured — cannot donate.
        NotAtLarge,
        /// The criminal has already been captured — operation invalid.
        AlreadyCaptured,
        /// Target account is not currently condemned (`WallOfShame` has no entry).
        TargetNotCondemned,
        /// Treasury does not have sufficient funds for the initial bounty.
        TreasuryInsufficientFunds,
        /// Donor does not have sufficient free balance for the donation.
        InsufficientBalance,
        /// Bounty pool is empty — nothing to pay out.
        EmptyBountyPool,
        /// [VECTOR 1] No locked payout found for this fugitive.
        BountyPayoutNotFound,
        /// [VECTOR 1] Bounty payout is still within the anti-laundering lock period.
        BountyPayoutStillLocked,
        /// [VECTOR 1] A locked payout already exists for this fugitive.
        BountyAlreadyLocked,
        /// [VECTOR 3] Target is an Academician or active Khural Delegate.
        /// High officials must face constitutional impeachment before exile.
        RequiresImpeachment,
    }

    // =========================================================================
    // Extrinsics
    // =========================================================================

    #[pallet::call]
    impl<T: Config> Pallet<T> {
        // ─── condemn_and_issue_warrant ─────────────────────────────────────────

        /// Issue a constitutional arrest warrant for a criminal citizen.
        ///
        /// **Origin: Root only** (Khural constitutional decree or Sudo in devnet).
        ///
        /// ## [VECTOR 3] Anti-Tyranny Guard
        /// If the target is an **Academician** (registered in `pallet-guilds::AcademyMembers`)
        /// or an **active Khural/Confederation Delegate**, this call returns
        /// `Error::RequiresImpeachment`. High officials CANNOT be exiled via this route;
        /// they must first be impeached through the constitutional Khural procedure.
        ///
        /// ## Effects
        /// 1. Sets citizen status to `Exiled` via `T::IdentityBridge`.
        /// 2. Slashes ALL free balance from `target` → state treasury.
        /// 3. Writes to `WallOfShame`.
        /// 4. If `is_fugitive == true`:
        ///    - Opens BountyPool and seeds it with `initial_bounty` from Treasury.
        ///
        /// ## Parameters
        /// - `target`:         The condemned citizen's AccountId.
        /// - `category`:       Constitutional crime classification.
        /// - `verdict_hash`:   H256 of the IPFS verdict document.
        /// - `is_fugitive`:    Whether the criminal evaded arrest (`AtLarge`).
        /// - `initial_bounty`: ALTAN transferred from Treasury to the bounty pool.
        ///                     Ignored (zero) if `is_fugitive == false`.
        #[pallet::call_index(0)]
        #[pallet::weight(T::WeightInfo::condemn_and_issue_warrant())]
        pub fn condemn_and_issue_warrant(
            origin: OriginFor<T>,
            target: T::AccountId,
            category: CrimeCategory,
            verdict_hash: H256,
            is_fugitive: bool,
            initial_bounty: BalanceOf<T>,
        ) -> DispatchResult {
            ensure_root(origin)?;

            // ── [VECTOR 3] Anti-Tyranny Guard ─────────────────────────────────
            // Academicians (GuildTumed Grandmasters) are protected by constitutional
            // mandate. They cannot be exiled without prior impeachment.
            ensure!(
                !T::AcademyChecker::is_academician(&target),
                Error::<T>::RequiresImpeachment
            );
            // Active Khural Delegates and Confederation Delegates are equally protected.
            ensure!(
                !T::KhuralChecker::is_active_khural_delegate(&target),
                Error::<T>::RequiresImpeachment
            );

            // ── Step 1: Set CitizenStatus::Exiled in pallet-inomad-identity ──
            T::IdentityBridge::exile_citizen(&target)?;

            // ── Step 2: Confiscate all free balance to State Treasury ─────────
            let target_free = T::Currency::free_balance(&target);
            if target_free > BalanceOf::<T>::default() {
                T::Currency::transfer(
                    &target,
                    &T::StateTreasury::get(),
                    target_free,
                    ExistenceRequirement::AllowDeath,
                )?;
            }
            let confiscated = target_free;

            // ── Step 3: Write CrimeRecord to WallOfShame ──────────────────────
            let timestamp: u32 = frame_system::Pallet::<T>::block_number()
                .try_into()
                .unwrap_or(u32::MAX);

            let fugitive_status = if is_fugitive {
                Some(FugitiveStatus::AtLarge)
            } else {
                None
            };

            WallOfShame::<T>::insert(
                &target,
                CrimeRecord {
                    category: category.clone(),
                    verdict_hash,
                    timestamp,
                    fugitive_status,
                },
            );

            // ── Step 4: Seed BountyPool from Treasury if fugitive ─────────────
            let seeded_bounty = if is_fugitive && initial_bounty > BalanceOf::<T>::default() {
                T::Currency::transfer(
                    &T::StateTreasury::get(),
                    &target, // We use the target account as the bounty pot key.
                    initial_bounty,
                    ExistenceRequirement::KeepAlive,
                )
                .map_err(|_| Error::<T>::TreasuryInsufficientFunds)?;
                BountyPools::<T>::insert(&target, initial_bounty);
                initial_bounty
            } else {
                BalanceOf::<T>::default()
            };

            Self::deposit_event(Event::WarrantIssued {
                target,
                category,
                verdict_hash,
                confiscated,
                is_fugitive,
                initial_bounty: seeded_bounty,
            });

            Ok(())
        }

        // ─── donate_to_bounty ──────────────────────────────────────────────────

        /// Donate ALTAN to the bounty pool of a fugitive criminal.
        ///
        /// Any citizen (Signed origin) may call this to increase the reward
        /// for capturing a specific fugitive.
        ///
        /// ## Checks
        /// - `target_fugitive` must exist in `WallOfShame` with `FugitiveStatus::AtLarge`.
        /// - Donor must have sufficient free balance.
        ///
        /// ## Parameters
        /// - `target_fugitive`: The fugitive's AccountId.
        /// - `amount`:          ALTAN to donate from the caller's free balance.
        #[pallet::call_index(1)]
        #[pallet::weight(T::WeightInfo::donate_to_bounty())]
        pub fn donate_to_bounty(
            origin: OriginFor<T>,
            target_fugitive: T::AccountId,
            amount: BalanceOf<T>,
        ) -> DispatchResult {
            let donor = ensure_signed(origin)?;

            // ── Verify fugitive is AtLarge ────────────────────────────────────
            let record =
                WallOfShame::<T>::get(&target_fugitive).ok_or(Error::<T>::TargetNotCondemned)?;

            match record.fugitive_status {
                Some(FugitiveStatus::AtLarge) => {} // OK
                Some(FugitiveStatus::Captured) => return Err(Error::<T>::AlreadyCaptured.into()),
                None => return Err(Error::<T>::NotAtLarge.into()),
            }

            // ── Transfer donation to bounty pot ───────────────────────────────
            // The bounty pot is identified by the fugitive's AccountId itself.
            // Funds are kept as free balance on that account.
            T::Currency::transfer(
                &donor,
                &target_fugitive,
                amount,
                ExistenceRequirement::KeepAlive,
            )
            .map_err(|_| Error::<T>::InsufficientBalance)?;

            // ── Update pool total ─────────────────────────────────────────────
            let new_total = BountyPools::<T>::mutate(&target_fugitive, |maybe| {
                let current = maybe.unwrap_or(BalanceOf::<T>::default());
                let updated = current.saturating_add(amount);
                *maybe = Some(updated);
                updated
            });

            Self::deposit_event(Event::BountyDonation {
                donor,
                fugitive: target_fugitive,
                amount,
                new_total,
            });

            Ok(())
        }

        // ─── register_capture_and_payout ──────────────────────────────────────

        /// Register the capture of a fugitive and initiate the vested bounty payout.
        ///
        /// **Origin: Root only** (Policing Authority or Khural decree).
        ///
        /// ## [VECTOR 1] Bounty Vesting / Anti-Laundering Protection
        /// Instead of immediate payout, the bounty is **locked** for `T::BountyLockPeriod`
        /// blocks. This prevents the "fake capture" attack where an accomplice pretends
        /// to catch the fugitive and launders the crowdfunded bounty pool.
        ///
        /// During the lock period:
        /// - Root can call `cancel_bounty_payout` if collusion is proven.
        /// - After the period, the hunter calls `claim_bounty_payout` to receive funds.
        ///
        /// ## Effects
        /// 1. Sets `FugitiveStatus::Captured` in `WallOfShame`.
        /// 2. Creates a `LockedBountyPayouts` entry with `unlock_block = now + BountyLockPeriod`.
        /// 3. Removes the open `BountyPools` entry.
        ///
        /// ## Parameters
        /// - `target_fugitive`: The captured criminal's AccountId.
        /// - `bounty_hunter`:   The account that apprehended the fugitive.
        #[pallet::call_index(2)]
        #[pallet::weight(T::WeightInfo::register_capture_and_payout())]
        pub fn register_capture_and_payout(
            origin: OriginFor<T>,
            target_fugitive: T::AccountId,
            bounty_hunter: T::AccountId,
        ) -> DispatchResult {
            ensure_root(origin)?;

            // ── Verify fugitive is AtLarge ────────────────────────────────────
            let mut record =
                WallOfShame::<T>::get(&target_fugitive).ok_or(Error::<T>::NotInBlackBook)?;

            ensure!(
                record.fugitive_status == Some(FugitiveStatus::AtLarge),
                Error::<T>::NotAtLarge
            );

            // ── Verify no duplicate locked payout exists ───────────────────────
            ensure!(
                !LockedBountyPayouts::<T>::contains_key(&target_fugitive),
                Error::<T>::BountyAlreadyLocked
            );

            // ── Mark as Captured ──────────────────────────────────────────────
            record.fugitive_status = Some(FugitiveStatus::Captured);
            WallOfShame::<T>::insert(&target_fugitive, &record);

            // ── Lock bounty pool for vesting ──────────────────────────────────
            let pool_amount =
                BountyPools::<T>::get(&target_fugitive).ok_or(Error::<T>::EmptyBountyPool)?;

            ensure!(
                pool_amount > BalanceOf::<T>::default(),
                Error::<T>::EmptyBountyPool
            );

            let current_block: u32 = frame_system::Pallet::<T>::block_number()
                .try_into()
                .unwrap_or(u32::MAX);
            let unlock_block = current_block.saturating_add(T::BountyLockPeriod::get());

            LockedBountyPayouts::<T>::insert(
                &target_fugitive,
                LockedPayout::<T> {
                    bounty_hunter: bounty_hunter.clone(),
                    unlock_block,
                    amount: pool_amount,
                },
            );

            // ── Close the open pool ───────────────────────────────────────────
            BountyPools::<T>::remove(&target_fugitive);

            Self::deposit_event(Event::FugitiveCapturedBountyLocked {
                fugitive: target_fugitive,
                bounty_hunter,
                bounty_amount: pool_amount,
                unlock_block,
            });

            Ok(())
        }

        // ─── claim_bounty_payout ──────────────────────────────────────────────

        /// [VECTOR 1] Claim a vested bounty payout after the anti-laundering lock period.
        ///
        /// Any signed caller may trigger this (the hunter's account is recorded in
        /// `LockedBountyPayouts`). The transfer goes to the registered `bounty_hunter`
        /// regardless of who calls this extrinsic.
        ///
        /// ## Conditions
        /// - A locked payout must exist for `target_fugitive`.
        /// - `current_block >= unlock_block`.
        ///
        /// # Origin: Signed
        #[pallet::call_index(3)]
        #[pallet::weight(T::WeightInfo::claim_bounty_payout())]
        pub fn claim_bounty_payout(
            origin: OriginFor<T>,
            target_fugitive: T::AccountId,
        ) -> DispatchResult {
            ensure_signed(origin)?;

            let locked = LockedBountyPayouts::<T>::get(&target_fugitive)
                .ok_or(Error::<T>::BountyPayoutNotFound)?;

            let current_block: u32 = frame_system::Pallet::<T>::block_number()
                .try_into()
                .unwrap_or(u32::MAX);

            ensure!(
                current_block >= locked.unlock_block,
                Error::<T>::BountyPayoutStillLocked
            );

            // ── Transfer bounty from fugitive's account to hunter ─────────────
            T::Currency::transfer(
                &target_fugitive,
                &locked.bounty_hunter,
                locked.amount,
                ExistenceRequirement::AllowDeath,
            )?;

            LockedBountyPayouts::<T>::remove(&target_fugitive);

            Self::deposit_event(Event::BountyPayoutClaimed {
                fugitive: target_fugitive,
                bounty_hunter: locked.bounty_hunter,
                amount: locked.amount,
            });

            Ok(())
        }

        // ─── cancel_bounty_payout ─────────────────────────────────────────────

        /// [VECTOR 1] Cancel a vested bounty payout if collusion is proven.
        ///
        /// **Origin: Root only**.
        ///
        /// If evidence of collusion between the "bounty hunter" and the fugitive
        /// emerges during the lock period, Root can cancel the payout. The funds
        /// remain on the fugitive's account and can be re-seized via a second
        /// `condemn_and_issue_warrant` or confiscated manually.
        ///
        /// # Origin: Root
        #[pallet::call_index(4)]
        #[pallet::weight(T::WeightInfo::cancel_bounty_payout())]
        pub fn cancel_bounty_payout(
            origin: OriginFor<T>,
            target_fugitive: T::AccountId,
        ) -> DispatchResult {
            ensure_root(origin)?;

            let locked = LockedBountyPayouts::<T>::get(&target_fugitive)
                .ok_or(Error::<T>::BountyPayoutNotFound)?;

            LockedBountyPayouts::<T>::remove(&target_fugitive);

            Self::deposit_event(Event::BountyPayoutCancelled {
                fugitive: target_fugitive,
                bounty_hunter: locked.bounty_hunter,
                amount: locked.amount,
            });

            Ok(())
        }
    }
}
