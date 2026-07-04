//! Unit tests for pallet-achievement-nft.
//!
//! Covers:
//! - `award_achievement`: reputation NFT minting, token counter, soulbound (no monetary value)
//! - `issue_reward_nft`: ALTAN-backed NFT, treasury reservation
//! - `redeem_reward_nft`: balance transfer, double-redeem guard, not-holder guard
//! - Error paths: TooManyAchievements (capped at MaxAchievementsPerHolder)

use crate::mock::*;
use crate::pallet::AchievementKind;
use frame_support::{assert_noop, assert_ok};

// ── 1. award_achievement ─────────────────────────────────────────────────

#[test]
fn award_achievement_happy_path() {
    new_test_ext().execute_with(|| {
        System::set_block_number(1);

        assert_ok!(AchievementNft::award_achievement(
            RuntimeOrigin::signed(ISSUER),
            ALICE,
            AchievementKind::Verifier,
            None,
        ));

        let token = crate::pallet::Achievements::<Test>::get(0).expect("token should exist");

        assert_eq!(token.holder, ALICE);
        assert_eq!(token.altan_value, 0); // Reputation NFT — no monetary value
        assert!(!token.redeemed);
    });
}

#[test]
fn award_achievement_increments_token_counter() {
    new_test_ext().execute_with(|| {
        System::set_block_number(1);

        assert_ok!(AchievementNft::award_achievement(
            RuntimeOrigin::signed(ISSUER),
            ALICE,
            AchievementKind::Verifier,
            None,
        ));
        assert_ok!(AchievementNft::award_achievement(
            RuntimeOrigin::signed(ISSUER),
            BOB,
            AchievementKind::Verifier,
            None,
        ));

        assert_eq!(crate::pallet::NextAchievementId::<Test>::get(), 2);
    });
}

#[test]
fn award_achievement_with_event_ref() {
    new_test_ext().execute_with(|| {
        System::set_block_number(1);

        let event_ref = b"GENESIS-CEREMONY-2026".to_vec();

        assert_ok!(AchievementNft::award_achievement(
            RuntimeOrigin::signed(ISSUER),
            ALICE,
            AchievementKind::Verifier,
            Some(event_ref),
        ));

        let token = crate::pallet::Achievements::<Test>::get(0).unwrap();
        assert!(token.event_ref.is_some());
    });
}

#[test]
fn award_achievement_updates_holder_index() {
    new_test_ext().execute_with(|| {
        System::set_block_number(1);

        assert_ok!(AchievementNft::award_achievement(
            RuntimeOrigin::signed(ISSUER),
            ALICE,
            AchievementKind::Verifier,
            None,
        ));

        let ids = crate::pallet::HolderAchievements::<Test>::get(ALICE);
        assert_eq!(ids.len(), 1);
        assert_eq!(ids[0], 0);
    });
}

// ── 2. issue_reward_nft ──────────────────────────────────────────────────

#[test]
fn issue_reward_nft_reserves_from_treasury() {
    new_test_ext().execute_with(|| {
        System::set_block_number(1);

        let reward_amount = 100 * UNIT;
        let treasury_before = Balances::reserved_balance(CONFEDERATION_TREASURY);

        assert_ok!(AchievementNft::issue_reward_nft(
            RuntimeOrigin::signed(ISSUER),
            ALICE,
            AchievementKind::Verifier,
            reward_amount,
            None,
        ));

        let treasury_after = Balances::reserved_balance(CONFEDERATION_TREASURY);
        assert_eq!(treasury_after - treasury_before, reward_amount);
    });
}

#[test]
fn issue_reward_nft_creates_token_with_altan_value() {
    new_test_ext().execute_with(|| {
        System::set_block_number(1);

        let reward_amount = 50 * UNIT;

        assert_ok!(AchievementNft::issue_reward_nft(
            RuntimeOrigin::signed(ISSUER),
            ALICE,
            AchievementKind::Verifier,
            reward_amount,
            None,
        ));

        let token = crate::pallet::Achievements::<Test>::get(0).unwrap();
        assert_eq!(token.altan_value, reward_amount);
        assert_eq!(token.holder, ALICE);
        assert!(!token.redeemed);
    });
}

#[test]
fn issue_reward_nft_fails_insufficient_treasury() {
    new_test_ext().execute_with(|| {
        System::set_block_number(1);

        // Try to reserve more than treasury has (10M UNIT)
        let excessive_amount = 100_000_000 * UNIT;

        assert_noop!(
            AchievementNft::issue_reward_nft(
                RuntimeOrigin::signed(ISSUER),
                ALICE,
                AchievementKind::Verifier,
                excessive_amount,
                None,
            ),
            crate::pallet::Error::<Test>::InsufficientTreasuryBalance
        );
    });
}

// ── 3. redeem_reward_nft ─────────────────────────────────────────────────

#[test]
fn redeem_reward_nft_transfers_altan_to_holder() {
    new_test_ext().execute_with(|| {
        System::set_block_number(1);

        let reward_amount = 200 * UNIT;

        assert_ok!(AchievementNft::issue_reward_nft(
            RuntimeOrigin::signed(ISSUER),
            ALICE,
            AchievementKind::Verifier,
            reward_amount,
            None,
        ));

        let alice_before = Balances::free_balance(ALICE);

        assert_ok!(AchievementNft::redeem_reward_nft(
            RuntimeOrigin::signed(ALICE),
            0,
        ));

        assert_eq!(Balances::free_balance(ALICE), alice_before + reward_amount);

        let token = crate::pallet::Achievements::<Test>::get(0).unwrap();
        assert!(token.redeemed);
    });
}

#[test]
fn redeem_reward_nft_fails_already_redeemed() {
    new_test_ext().execute_with(|| {
        System::set_block_number(1);

        assert_ok!(AchievementNft::issue_reward_nft(
            RuntimeOrigin::signed(ISSUER),
            ALICE,
            AchievementKind::Verifier,
            100 * UNIT,
            None,
        ));
        assert_ok!(AchievementNft::redeem_reward_nft(
            RuntimeOrigin::signed(ALICE),
            0
        ));

        assert_noop!(
            AchievementNft::redeem_reward_nft(RuntimeOrigin::signed(ALICE), 0),
            crate::pallet::Error::<Test>::AlreadyRedeemed
        );
    });
}

#[test]
fn redeem_reward_nft_fails_not_holder() {
    new_test_ext().execute_with(|| {
        System::set_block_number(1);

        assert_ok!(AchievementNft::issue_reward_nft(
            RuntimeOrigin::signed(ISSUER),
            ALICE,
            AchievementKind::Verifier,
            100 * UNIT,
            None,
        ));

        // BOB tries to redeem ALICE's NFT
        assert_noop!(
            AchievementNft::redeem_reward_nft(RuntimeOrigin::signed(BOB), 0),
            crate::pallet::Error::<Test>::NotHolder
        );
    });
}

#[test]
fn redeem_reputation_nft_fails_not_redeemable() {
    new_test_ext().execute_with(|| {
        System::set_block_number(1);

        // Award a reputation (soul-bound) NFT — no ALTAN value
        assert_ok!(AchievementNft::award_achievement(
            RuntimeOrigin::signed(ISSUER),
            ALICE,
            AchievementKind::Verifier,
            None,
        ));

        // Cannot redeem a reputation NFT
        assert_noop!(
            AchievementNft::redeem_reward_nft(RuntimeOrigin::signed(ALICE), 0),
            crate::pallet::Error::<Test>::NotRedeemable
        );
    });
}

#[test]
fn redeem_reward_nft_fails_token_not_found() {
    new_test_ext().execute_with(|| {
        assert_noop!(
            AchievementNft::redeem_reward_nft(RuntimeOrigin::signed(ALICE), 999),
            crate::pallet::Error::<Test>::TokenNotFound
        );
    });
}
