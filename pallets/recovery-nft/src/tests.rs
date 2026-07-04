//! Unit tests for pallet-recovery-nft.
//!
//! Covers:
//! - `issue_recovery_nfts`: happy path (3-of-5), threshold validation, too many holders
//! - `initiate_recovery`: creates request, not-holder guard
//! - `veto_recovery`: owner blocks recovery, not-target-account guard
//! - `confirm_recovery`: double-confirm guard, veto-window active guard, vetoed guard

use crate::mock::*;
use crate::pallet::RecoveryDestination;
use frame_support::{assert_noop, assert_ok};

// ── Helpers ───────────────────────────────────────────────────────────────

fn group_id(s: &str) -> Vec<u8> {
    s.as_bytes().to_vec()
}

fn vault_addr() -> Vec<u8> {
    b"5GrwvaEF5zXb26Fz9rcQpDWS57CtERHpNehXCPcNoHGKutQY".to_vec()
}

/// Issue 3 Recovery NFTs (threshold 2) for TARGET, held by ALICE/BOB/CAROL.
fn issue_group(gid: &str) {
    assert_ok!(RecoveryNft::issue_recovery_nfts(
        RuntimeOrigin::signed(ISSUER),
        group_id(gid),
        TARGET,
        vec![ALICE, BOB, CAROL],
        RecoveryDestination::PersonalBankVault,
        vault_addr(),
        2, // threshold: 2 of 3
    ));
}

// ── 1. issue_recovery_nfts ───────────────────────────────────────────────

#[test]
fn issue_recovery_nfts_happy_path() {
    new_test_ext().execute_with(|| {
        System::set_block_number(1);

        issue_group("GROUP-A");

        // 3 NFTs should be minted (token IDs 0, 1, 2)
        assert_eq!(crate::pallet::NextRecoveryTokenId::<Test>::get(), 3);

        let nft0 = crate::pallet::RecoveryNfts::<Test>::get(0).expect("NFT 0 should exist");
        assert_eq!(nft0.holder, ALICE);
        assert_eq!(nft0.target_account, TARGET);
        assert_eq!(nft0.threshold, 2);
        assert_eq!(nft0.total_issued, 3);

        let nft2 = crate::pallet::RecoveryNfts::<Test>::get(2).expect("NFT 2 should exist");
        assert_eq!(nft2.holder, CAROL);
    });
}

#[test]
fn issue_recovery_nfts_holder_index_updated() {
    new_test_ext().execute_with(|| {
        System::set_block_number(1);
        issue_group("GROUP-B");

        let alice_nfts = crate::pallet::HolderRecoveryNfts::<Test>::get(ALICE);
        assert_eq!(alice_nfts.len(), 1);
        assert_eq!(alice_nfts[0], 0); // first minted token
    });
}

#[test]
fn issue_recovery_nfts_rejects_threshold_zero() {
    new_test_ext().execute_with(|| {
        assert_noop!(
            RecoveryNft::issue_recovery_nfts(
                RuntimeOrigin::signed(ISSUER),
                group_id("GROUP-T0"),
                TARGET,
                vec![ALICE, BOB],
                RecoveryDestination::PersonalBankVault,
                vault_addr(),
                0, // invalid threshold
            ),
            crate::pallet::Error::<Test>::InvalidThreshold
        );
    });
}

#[test]
fn issue_recovery_nfts_rejects_threshold_exceeds_group_size() {
    new_test_ext().execute_with(|| {
        // 2 holders but threshold 3 — impossible
        assert_noop!(
            RecoveryNft::issue_recovery_nfts(
                RuntimeOrigin::signed(ISSUER),
                group_id("GROUP-TEx"),
                TARGET,
                vec![ALICE, BOB],
                RecoveryDestination::PersonalBankVault,
                vault_addr(),
                3, // threshold > holders.len()
            ),
            crate::pallet::Error::<Test>::ThresholdExceedsGroupSize
        );
    });
}

#[test]
fn issue_recovery_nfts_rejects_too_many_holders() {
    new_test_ext().execute_with(|| {
        // MaxGroupSize = 10 — try 11
        let holders: Vec<AccountId> = (100u64..=110).collect();

        assert_noop!(
            RecoveryNft::issue_recovery_nfts(
                RuntimeOrigin::signed(ISSUER),
                group_id("GROUP-BIG"),
                TARGET,
                holders,
                RecoveryDestination::FamilyVault,
                vault_addr(),
                5,
            ),
            crate::pallet::Error::<Test>::TooManyHolders
        );
    });
}

// ── 2. initiate_recovery ─────────────────────────────────────────────────

#[test]
fn initiate_recovery_creates_request() {
    new_test_ext().execute_with(|| {
        System::set_block_number(1);
        issue_group("GROUP-C");

        // ALICE holds token 0 — she initiates recovery
        assert_ok!(RecoveryNft::initiate_recovery(
            RuntimeOrigin::signed(ALICE),
            0,
            group_id("REQ-001"),
        ));

        let req_id_bounded: frame_support::BoundedVec<u8, frame_support::traits::ConstU32<64>> =
            b"REQ-001".to_vec().try_into().unwrap();

        let req = crate::pallet::RecoveryRequests::<Test>::get(&req_id_bounded)
            .expect("request should be created");

        assert_eq!(req.target_account, TARGET);
        assert_eq!(req.threshold, 2);
        // Initiator's confirmation is counted automatically
        assert_eq!(req.confirmation_count, 1);
    });
}

#[test]
fn initiate_recovery_fails_not_holder() {
    new_test_ext().execute_with(|| {
        System::set_block_number(1);
        issue_group("GROUP-D");

        // DAVE does not hold any token — error
        assert_noop!(
            RecoveryNft::initiate_recovery(
                RuntimeOrigin::signed(DAVE),
                0, // token held by ALICE
                group_id("REQ-002"),
            ),
            crate::pallet::Error::<Test>::NotHolder
        );
    });
}

#[test]
fn initiate_recovery_fails_token_not_found() {
    new_test_ext().execute_with(|| {
        assert_noop!(
            RecoveryNft::initiate_recovery(RuntimeOrigin::signed(ALICE), 999, group_id("REQ-003"),),
            crate::pallet::Error::<Test>::TokenNotFound
        );
    });
}

// ── 3. veto_recovery ─────────────────────────────────────────────────────

#[test]
fn veto_recovery_sets_vetoed_status() {
    new_test_ext().execute_with(|| {
        System::set_block_number(1);
        issue_group("GROUP-E");

        assert_ok!(RecoveryNft::initiate_recovery(
            RuntimeOrigin::signed(ALICE),
            0,
            group_id("REQ-VETO"),
        ));

        // TARGET (owner) exercises their veto right
        assert_ok!(RecoveryNft::veto_recovery(
            RuntimeOrigin::signed(TARGET),
            group_id("REQ-VETO"),
        ));

        let req_id_bounded: frame_support::BoundedVec<u8, frame_support::traits::ConstU32<64>> =
            b"REQ-VETO".to_vec().try_into().unwrap();

        let req = crate::pallet::RecoveryRequests::<Test>::get(&req_id_bounded).unwrap();
        assert!(matches!(
            req.status,
            crate::pallet::RecoveryRequestStatus::OwnerVetoed
        ));
    });
}

#[test]
fn veto_recovery_fails_not_target_account() {
    new_test_ext().execute_with(|| {
        System::set_block_number(1);
        issue_group("GROUP-F");

        assert_ok!(RecoveryNft::initiate_recovery(
            RuntimeOrigin::signed(ALICE),
            0,
            group_id("REQ-NTA"),
        ));

        // ALICE is a holder, not the target — cannot veto
        assert_noop!(
            RecoveryNft::veto_recovery(RuntimeOrigin::signed(ALICE), group_id("REQ-NTA")),
            crate::pallet::Error::<Test>::NotTargetAccount
        );
    });
}

// ── 4. confirm_recovery ──────────────────────────────────────────────────

#[test]
fn confirm_recovery_during_veto_window_fails() {
    new_test_ext().execute_with(|| {
        System::set_block_number(1);
        issue_group("GROUP-G");

        // ALICE initiates (block 1)
        assert_ok!(RecoveryNft::initiate_recovery(
            RuntimeOrigin::signed(ALICE),
            0,
            group_id("REQ-WIN"),
        ));

        // BOB tries to confirm at block 2 — still within VetoWindowBlocks (43_200)
        System::set_block_number(2);
        assert_noop!(
            RecoveryNft::confirm_recovery(
                RuntimeOrigin::signed(BOB),
                1, // BOB's token
                group_id("REQ-WIN"),
            ),
            crate::pallet::Error::<Test>::RecoveryVetoWindowActive
        );
    });
}

#[test]
fn confirm_recovery_after_veto_window_reaches_threshold() {
    new_test_ext().execute_with(|| {
        System::set_block_number(1);
        issue_group("GROUP-H");

        // ALICE initiates (token 0)
        assert_ok!(RecoveryNft::initiate_recovery(
            RuntimeOrigin::signed(ALICE),
            0,
            group_id("REQ-THRESH"),
        ));

        // Skip past veto window (43_200 blocks)
        System::set_block_number(50_000);

        // BOB confirms (token 1) — this is the 2nd confirmation, reaches threshold=2
        assert_ok!(RecoveryNft::confirm_recovery(
            RuntimeOrigin::signed(BOB),
            1,
            group_id("REQ-THRESH"),
        ));

        let req_id_bounded: frame_support::BoundedVec<u8, frame_support::traits::ConstU32<64>> =
            b"REQ-THRESH".to_vec().try_into().unwrap();

        let req = crate::pallet::RecoveryRequests::<Test>::get(&req_id_bounded).unwrap();
        assert!(matches!(
            req.status,
            crate::pallet::RecoveryRequestStatus::Executed
        ));
        assert_eq!(req.confirmation_count, 2);
    });
}

#[test]
fn confirm_recovery_fails_already_confirmed() {
    new_test_ext().execute_with(|| {
        System::set_block_number(1);
        issue_group("GROUP-I");

        assert_ok!(RecoveryNft::initiate_recovery(
            RuntimeOrigin::signed(ALICE),
            0,
            group_id("REQ-DBL"),
        ));

        System::set_block_number(50_000);

        assert_ok!(RecoveryNft::confirm_recovery(
            RuntimeOrigin::signed(BOB),
            1,
            group_id("REQ-DBL"),
        ));

        // BOB tries to confirm again with the same token
        assert_noop!(
            RecoveryNft::confirm_recovery(RuntimeOrigin::signed(BOB), 1, group_id("REQ-DBL"),),
            crate::pallet::Error::<Test>::AlreadyConfirmed
        );
    });
}

#[test]
fn confirm_recovery_fails_after_veto() {
    new_test_ext().execute_with(|| {
        System::set_block_number(1);
        issue_group("GROUP-J");

        assert_ok!(RecoveryNft::initiate_recovery(
            RuntimeOrigin::signed(ALICE),
            0,
            group_id("REQ-VA"),
        ));

        // TARGET vetoes immediately
        assert_ok!(RecoveryNft::veto_recovery(
            RuntimeOrigin::signed(TARGET),
            group_id("REQ-VA"),
        ));

        // Skip past veto window
        System::set_block_number(50_000);

        // BOB tries to confirm after veto — should fail
        assert_noop!(
            RecoveryNft::confirm_recovery(RuntimeOrigin::signed(BOB), 1, group_id("REQ-VA"),),
            crate::pallet::Error::<Test>::RecoveryVetoedByOwner
        );
    });
}
