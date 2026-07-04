//! # pallet-achievement-nft
//!
//! **Altan Network — Achievement NFTs + Reward NFTs**
//!
//! ## Two NFT Categories
//!
//! ### 1. Achievement NFTs (Ачивки — репутационные)
//!
//! Non-monetary, soul-bound NFTs representing accomplishments.
//! Cannot be transferred or sold. Permanently attached to the citizen.
//!
//! | Achievement | Trigger | Value |
//! |------------|---------|-------|
//! | `Verifier` | Verified 10 citizens | Reputation ↑ |
//! | `TumedElected` | Elected to Tumed Khural | Title |
//! | `GuildMaster` | Grandmaster of guild | Title |
//! | `AcademyScientist` | Academy of Sciences | Title |
//! | `FirstCitizen` | First citizen of region | History |
//! | `FrontierBuilder` | Built infrastructure | Honor |
//! | `LegislativeAuthor` | Authored passed law | Honor |
//! | `CenturyMember` | 100th Arbad member | Honor |
//!
//! ### 2. Reward NFTs (Денежные призы — из бюджета конфедерации)
//!
//! ALTAN-backed redeemable NFTs. Source: Confederation Treasury budget.
//! Can be redeemed (burned) for ALTAN at any time.
//!
//! | Reward | Trigger | Amount |
//! |--------|---------|--------|
//! | `VerifierReward` | Per verified citizen | 10 ALTAN |
//! | `ElectionReward` | For running election | Variable |
//! | `QuestReward` | Guild quest completion | Variable |
//! | `BudgetPrize` | Confederation annual award | Variable |
//! | `TaxBonus` | Tax compliance reward | Variable |
//! | `ValidationBonus` | Validator performance | Variable |
//!
//! ## NFT Center Management
//!
//! The NFT Center is managed by designated humans (NFT Akademiks).
//! They can:
//!   - Define new achievement types (via governance)
//!   - Issue achievements on behalf of verified events
//!   - Set ALTAN amounts for reward NFTs (from confederation budget)
//!   - Revoke fraudulent achievements
//!
//! All issued NFTs are permanently recorded on Altan L1.
//!
//! ## Dispatchables
//!
//! | Call | Origin | Description |
//! |---|---|---|
//! | `award_achievement` | Signed (Officer) | Issue a non-transferable achievement badge |
//! | `issue_reward_nft` | Signed (Officer) | Issue a redeemable reward NFT to a citizen |
//! | `redeem_reward_nft` | Signed (Citizen) | Burn a reward NFT to claim its underlying benefit |

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
        traits::{Currency, ExistenceRequirement, ReservableCurrency},
    };
    use frame_system::pallet_prelude::*;

    pub type BalanceOf<T> =
        <<T as Config>::Currency as Currency<<T as frame_system::Config>::AccountId>>::Balance;

    // =========================================================================
    // Achievement Kind
    // =========================================================================

    /// Whether an achievement is purely reputational or carries ALTAN value.
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
    pub enum AchievementCategory {
        /// Soul-bound reputation NFT — no monetary value. Cannot be sold/transferred.
        Reputation,
        /// ALTAN-backed reward NFT — redeemable for ALTAN from confederation treasury.
        /// `reserved_amount` ALTAN is reserved at issuance, released on redemption.
        Reward,
    }

    /// Achievement type discriminant.
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
    pub enum AchievementKind {
        // ── Reputation (soul-bound, no monetary value) ─────────────────────
        Verifier,          // Верификатор 10 граждан
        TumedElected,      // Избран в Хурал Тумэда
        GuildMaster,       // Мастер-гроссмейстер гильдии
        AcademyScientist,  // Академик — Академия Наук
        FirstCitizen,      // Первый гражданин региона
        FrontierBuilder,   // Строитель инфраструктуры
        LegislativeAuthor, // Автор принятого закона
        CenturyMember,     // Сотый член Арбада

        // ── Reward (ALTAN-backed, redeemable) ─────────────────────────────
        VerifierReward,  // 10 ALTAN за каждого верифицированного гражданина
        ElectionReward,  // Награда за проведение выборов
        QuestReward,     // Награда за выполнение задания гильдии
        BudgetPrize,     // Ежегодная премия Конфедерации
        TaxBonus,        // Бонус за налоговое соответствие
        ValidationBonus, // Бонус за работу валидатора

        // ── Custom — defined by NFT Center admin ──────────────────────────
        Custom(u16), // Custom achievement ID (defined by NFT Akademik)
    }

    // =========================================================================
    // Achievement NFT Record
    // =========================================================================

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
    pub struct AchievementToken<AccountId, BlockNumber, Balance> {
        /// Global token ID.
        pub token_id: u64,
        /// Citizen who earned/received this achievement.
        pub holder: AccountId,
        /// Category: Reputation (soul-bound) or Reward (ALTAN-backed).
        pub category: AchievementCategory,
        /// Specific achievement kind.
        pub kind: AchievementKind,
        /// Optional: ALTAN amount reserved for Reward NFTs. Zero for Reputation.
        pub altan_value: Balance,
        /// Block when issued.
        pub issued_at: BlockNumber,
        /// Whether this reward has been redeemed (only for Reward category).
        pub redeemed: bool,
        /// Account that issued this NFT (NFT Akademik or system).
        pub issued_by: AccountId,
        /// Optional: linked event hash (quest ID, election ID, tx hash, etc.)
        pub event_ref: Option<BoundedVec<u8, ConstU32<64>>>,
    }

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
        /// ALTAN currency — used for reserving Reward NFT amounts.
        type Currency: Currency<Self::AccountId> + ReservableCurrency<Self::AccountId>;

        /// Origin that may issue achievement NFTs.
        /// In production: NFT Akademik collective (multi-sig) or the Creator.
        type NftIssuerOrigin: EnsureOrigin<Self::RuntimeOrigin, Success = Self::AccountId>;

        /// The confederation treasury account (source of Reward NFT ALTAN).
        #[pallet::constant]
        type ConfederationTreasury: Get<Self::AccountId>;

        /// Max achievements per citizen (anti-storage-spam).
        #[pallet::constant]
        type MaxAchievementsPerHolder: Get<u32>;
    }

    // =========================================================================
    // Storage
    // =========================================================================

    /// Auto-incrementing NFT token ID counter.
    #[pallet::storage]
    #[pallet::getter(fn next_achievement_id)]
    pub type NextAchievementId<T: Config> = StorageValue<_, u64, ValueQuery>;

    /// All achievement tokens stored by token_id.
    #[pallet::storage]
    #[pallet::getter(fn achievements)]
    pub type Achievements<T: Config> = StorageMap<
        _,
        Blake2_128Concat,
        u64,
        AchievementToken<T::AccountId, BlockNumberFor<T>, BalanceOf<T>>,
        OptionQuery,
    >;

    /// Index: holder → their achievement token IDs.
    #[pallet::storage]
    #[pallet::getter(fn holder_achievements)]
    pub type HolderAchievements<T: Config> = StorageMap<
        _,
        Blake2_128Concat,
        T::AccountId,
        BoundedVec<u64, T::MaxAchievementsPerHolder>,
        ValueQuery,
    >;

    // =========================================================================
    // Events
    // =========================================================================

    #[pallet::event]
    #[pallet::generate_deposit(pub(super) fn deposit_event)]
    pub enum Event<T: Config> {
        /// A reputation achievement was awarded (soul-bound, no transfer).
        AchievementAwarded {
            token_id: u64,
            holder: T::AccountId,
            kind: AchievementKind,
        },
        /// A reward NFT was issued (ALTAN reserved from confederation treasury).
        RewardNftIssued {
            token_id: u64,
            holder: T::AccountId,
            kind: AchievementKind,
            altan_amount: BalanceOf<T>,
        },
        /// A reward NFT was redeemed — ALTAN transferred to holder.
        RewardRedeemed {
            token_id: u64,
            holder: T::AccountId,
            altan_amount: BalanceOf<T>,
        },
    }

    // =========================================================================
    // Errors
    // =========================================================================

    #[pallet::error]
    pub enum Error<T> {
        /// Token not found in storage.
        TokenNotFound,
        /// Reputation NFTs cannot be redeemed (only Reward NFTs can).
        NotRedeemable,
        /// This reward has already been redeemed.
        AlreadyRedeemed,
        /// Caller is not the holder of this NFT.
        NotHolder,
        /// Holder has too many achievements (MaxAchievementsPerHolder exceeded).
        TooManyAchievements,
        /// The confederation treasury has insufficient ALTAN for reward.
        InsufficientTreasuryBalance,
    }

    // =========================================================================
    // Extrinsics
    // =========================================================================

    #[pallet::call]
    impl<T: Config> Pallet<T> {
        // ─── award_achievement ────────────────────────────────────────────────

        /// Award a reputation (soul-bound) achievement NFT to a citizen.
        ///
        /// Called by NFT Akademik or Creator when a citizen qualifies.
        /// Reputation NFTs are permanently attached — they CANNOT be transferred,
        /// sold, or redeemed. They are badges of honor with no monetary value.
        ///
        /// ## Example
        /// ```text
        /// award_achievement(Origin::NftAkademik, citizen, AchievementKind::Verifier, None)
        /// → citizen receives "Верификатор" badge (soul-bound)
        /// ```
        #[pallet::call_index(0)]
        #[pallet::weight(T::WeightInfo::award_achievement())]
        pub fn award_achievement(
            origin: OriginFor<T>,
            holder: T::AccountId,
            kind: AchievementKind,
            event_ref: Option<alloc::vec::Vec<u8>>,
        ) -> DispatchResult {
            let issuer = T::NftIssuerOrigin::ensure_origin(origin)?;

            let token_id = NextAchievementId::<T>::get();
            NextAchievementId::<T>::put(token_id.saturating_add(1));

            let now = frame_system::Pallet::<T>::block_number();
            let event_ref_bounded: Option<BoundedVec<u8, ConstU32<64>>> =
                event_ref.and_then(|r| r.try_into().ok());

            let token = AchievementToken {
                token_id,
                holder: holder.clone(),
                category: AchievementCategory::Reputation,
                kind: kind.clone(),
                altan_value: BalanceOf::<T>::default(),
                issued_at: now,
                redeemed: false,
                issued_by: issuer,
                event_ref: event_ref_bounded,
            };

            Achievements::<T>::insert(token_id, &token);
            HolderAchievements::<T>::try_mutate(&holder, |ids| {
                ids.try_push(token_id)
                    .map_err(|_| Error::<T>::TooManyAchievements)
            })?;

            Self::deposit_event(Event::AchievementAwarded {
                token_id,
                holder,
                kind,
            });
            Ok(())
        }

        // ─── issue_reward_nft ─────────────────────────────────────────────────

        /// Issue an ALTAN-backed Reward NFT to a citizen.
        ///
        /// The specified `altan_amount` is **reserved** from the Confederation Treasury
        /// at the time of issuance. The citizen can redeem it at any time via
        /// `redeem_reward_nft`, which unreserves and transfers the ALTAN to them.
        ///
        /// ## Constitutional Rule
        ///
        /// Reward NFTs must be budgeted by the Confederation Khural.
        /// The NFT Akademik collective approves each issuance batch.
        ///
        /// ## Example
        /// ```text
        /// issue_reward_nft(Origin::NftAkademik, verifier_citizen,
        ///                  AchievementKind::VerifierReward, 10_000_000_000_000)
        /// → reserves 10 ALTAN from ConfederationTreasury
        /// → citizen receives redeemable "Верификатор Приз" NFT worth 10 ALTAN
        /// ```
        #[pallet::call_index(1)]
        #[pallet::weight(T::WeightInfo::issue_reward_nft())]
        pub fn issue_reward_nft(
            origin: OriginFor<T>,
            holder: T::AccountId,
            kind: AchievementKind,
            altan_amount: BalanceOf<T>,
            event_ref: Option<alloc::vec::Vec<u8>>,
        ) -> DispatchResult {
            let issuer = T::NftIssuerOrigin::ensure_origin(origin)?;

            let treasury = T::ConfederationTreasury::get();

            // Reserve ALTAN from confederation treasury
            T::Currency::reserve(&treasury, altan_amount)
                .map_err(|_| Error::<T>::InsufficientTreasuryBalance)?;

            let token_id = NextAchievementId::<T>::get();
            NextAchievementId::<T>::put(token_id.saturating_add(1));

            let now = frame_system::Pallet::<T>::block_number();
            let event_ref_bounded: Option<BoundedVec<u8, ConstU32<64>>> =
                event_ref.and_then(|r| r.try_into().ok());

            let token = AchievementToken {
                token_id,
                holder: holder.clone(),
                category: AchievementCategory::Reward,
                kind: kind.clone(),
                altan_value: altan_amount,
                issued_at: now,
                redeemed: false,
                issued_by: issuer,
                event_ref: event_ref_bounded,
            };

            Achievements::<T>::insert(token_id, &token);
            HolderAchievements::<T>::try_mutate(&holder, |ids| {
                ids.try_push(token_id)
                    .map_err(|_| Error::<T>::TooManyAchievements)
            })?;

            Self::deposit_event(Event::RewardNftIssued {
                token_id,
                holder,
                kind,
                altan_amount,
            });

            Ok(())
        }

        // ─── redeem_reward_nft ────────────────────────────────────────────────

        /// Redeem a Reward NFT — burn it and receive the reserved ALTAN.
        ///
        /// The citizen calls this to convert their Reward NFT into actual ALTAN.
        /// The reserved amount from ConfederationTreasury is unreserved and
        /// transferred to the citizen's free balance.
        ///
        /// ## Constraints
        ///
        /// - Only the NFT holder can redeem it.
        /// - Reputation NFTs cannot be redeemed (they have no altan_value).
        /// - A reward can only be redeemed once.
        #[pallet::call_index(2)]
        #[pallet::weight(T::WeightInfo::redeem_reward_nft())]
        pub fn redeem_reward_nft(origin: OriginFor<T>, token_id: u64) -> DispatchResult {
            let caller = ensure_signed(origin)?;

            Achievements::<T>::try_mutate(token_id, |maybe_token| -> DispatchResult {
                let token = maybe_token.as_mut().ok_or(Error::<T>::TokenNotFound)?;

                ensure!(token.holder == caller, Error::<T>::NotHolder);
                ensure!(
                    matches!(token.category, AchievementCategory::Reward),
                    Error::<T>::NotRedeemable
                );
                ensure!(!token.redeemed, Error::<T>::AlreadyRedeemed);

                let treasury = T::ConfederationTreasury::get();

                // Unreserve from treasury and transfer to holder
                T::Currency::unreserve(&treasury, token.altan_value);
                T::Currency::transfer(
                    &treasury,
                    &token.holder,
                    token.altan_value,
                    ExistenceRequirement::KeepAlive,
                )?;

                token.redeemed = true;

                Self::deposit_event(Event::RewardRedeemed {
                    token_id,
                    holder: token.holder.clone(),
                    altan_amount: token.altan_value,
                });

                Ok(())
            })
        }
    }
}
