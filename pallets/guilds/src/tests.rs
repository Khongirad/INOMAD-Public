//! Unit tests for pallet-guilds.
//!
//! Covers:
//! - Guild lifecycle: create → join → promote
//! - Quest escrow: publish → assign → complete/cancel (escrow unreserved)
//! - Ghost-state cascade:
//!     · `cleanup_account` cancels open quests and unreserves escrow
//!     · `OnTerminalStatus::on_deceased` / `on_exiled` trigger cleanup
//!     · Academy membership stripped on exile/death
//! - Error paths: NotMember, NotMaster, CannotPromoteSelf, AlreadyMember

use super::*;
use crate::mock::*;
use frame_support::{assert_noop, assert_ok};

// Re-export the runtime pallet as a short alias for readability
use GuildsPallet as Guilds;

// =========================================================================
// Helpers
// =========================================================================

const MASTER: AccountId = 1; // Alice
const MEMBER: AccountId = 2; // Bob
const CHARLIE: AccountId = 3; // Charlie

/// Create a guild as MASTER. Returns guild_id (always 0 first time).
fn create_guild() -> u32 {
    let guild_id = GuildsPallet::next_guild_id();
    assert_ok!(GuildsPallet::create_guild(
        RuntimeOrigin::signed(MASTER),
        b"Test Guild".to_vec(),
        b"Blacksmithing".to_vec(),
        None::<Vec<u8>>,
        1u32, // region_tag
    ));
    guild_id
}

/// Publish a quest from `employer` for `guild_id`. Returns quest_id.
fn publish_quest(employer: AccountId, guild_id: u32, reward: Balance) -> u32 {
    let quest_id = GuildsPallet::next_quest_id();
    assert_ok!(GuildsPallet::publish_quest(
        RuntimeOrigin::signed(employer),
        guild_id,
        reward,
        b"QmDescCid".to_vec(),
    ));
    quest_id
}

// =========================================================================
// 1. Guild Lifecycle
// =========================================================================

#[test]
fn create_guild_sets_creator_as_master() {
    new_test_ext().execute_with(|| {
        let guild_id = create_guild();
        assert_eq!(
            Guilds::guild_members(guild_id, MASTER),
            Some(MemberRole::Master),
        );
    });
}

#[test]
fn join_guild_adds_member_as_apprentice() {
    new_test_ext().execute_with(|| {
        let guild_id = create_guild();
        assert_ok!(Guilds::join_guild(RuntimeOrigin::signed(MEMBER), guild_id));
        assert_eq!(
            Guilds::guild_members(guild_id, MEMBER),
            Some(MemberRole::Apprentice),
        );
    });
}

#[test]
fn join_guild_increments_member_count() {
    new_test_ext().execute_with(|| {
        let guild_id = create_guild();
        let before = Guilds::guilds(guild_id).unwrap().member_count;
        assert_ok!(Guilds::join_guild(RuntimeOrigin::signed(MEMBER), guild_id));
        assert_eq!(Guilds::guilds(guild_id).unwrap().member_count, before + 1);
    });
}

#[test]
fn double_join_guild_fails() {
    new_test_ext().execute_with(|| {
        let guild_id = create_guild();
        assert_ok!(Guilds::join_guild(RuntimeOrigin::signed(MEMBER), guild_id));
        assert_noop!(
            Guilds::join_guild(RuntimeOrigin::signed(MEMBER), guild_id),
            Error::<Test>::AlreadyMember,
        );
    });
}

#[test]
fn promote_member_to_professional() {
    new_test_ext().execute_with(|| {
        let guild_id = create_guild();
        assert_ok!(Guilds::join_guild(RuntimeOrigin::signed(MEMBER), guild_id));
        assert_ok!(Guilds::promote_member(
            RuntimeOrigin::signed(MASTER),
            guild_id,
            MEMBER,
            MemberRole::Professional,
        ));
        assert_eq!(
            Guilds::guild_members(guild_id, MEMBER),
            Some(MemberRole::Professional),
        );
    });
}

#[test]
fn master_cannot_promote_self() {
    new_test_ext().execute_with(|| {
        let guild_id = create_guild();
        assert_noop!(
            Guilds::promote_member(
                RuntimeOrigin::signed(MASTER),
                guild_id,
                MASTER,
                MemberRole::Professional,
            ),
            Error::<Test>::CannotPromoteSelf,
        );
    });
}

#[test]
fn non_master_cannot_promote() {
    new_test_ext().execute_with(|| {
        let guild_id = create_guild();
        assert_ok!(Guilds::join_guild(RuntimeOrigin::signed(MEMBER), guild_id));
        assert_ok!(Guilds::join_guild(RuntimeOrigin::signed(CHARLIE), guild_id));
        assert_noop!(
            Guilds::promote_member(
                RuntimeOrigin::signed(MEMBER),
                guild_id,
                CHARLIE,
                MemberRole::Professional,
            ),
            Error::<Test>::NotMaster,
        );
    });
}

// =========================================================================
// 2. Quest Escrow
// =========================================================================

#[test]
fn publish_quest_reserves_escrow() {
    new_test_ext().execute_with(|| {
        let guild_id = create_guild();
        let free_before = Balances::free_balance(MASTER);
        publish_quest(MASTER, guild_id, 100 * UNIT);
        // Reserved = held in escrow — freed balance decreases
        assert_eq!(Balances::free_balance(MASTER), free_before - 100 * UNIT);
        assert_eq!(Balances::reserved_balance(MASTER), 100 * UNIT);
    });
}

#[test]
fn complete_quest_releases_escrow_to_assignee() {
    new_test_ext().execute_with(|| {
        let guild_id = create_guild();
        assert_ok!(Guilds::join_guild(RuntimeOrigin::signed(MEMBER), guild_id));

        let quest_id = publish_quest(MASTER, guild_id, 100 * UNIT);
        assert_ok!(Guilds::assign_quest(
            RuntimeOrigin::signed(MEMBER),
            quest_id
        ));
        assert_ok!(Guilds::submit_quest(
            RuntimeOrigin::signed(MEMBER),
            quest_id
        ));

        let member_before = Balances::free_balance(MEMBER);
        assert_ok!(Guilds::complete_quest(
            RuntimeOrigin::signed(MASTER),
            quest_id
        ));

        assert_eq!(Balances::free_balance(MEMBER), member_before + 100 * UNIT);
        assert_eq!(Balances::reserved_balance(MASTER), 0);
    });
}

#[test]
fn cancel_quest_returns_escrow_to_employer() {
    new_test_ext().execute_with(|| {
        let guild_id = create_guild();
        let free_before = Balances::free_balance(MASTER);
        let quest_id = publish_quest(MASTER, guild_id, 100 * UNIT);

        assert_ok!(Guilds::cancel_quest(
            RuntimeOrigin::signed(MASTER),
            quest_id
        ));

        // Escrow returned — free balance fully restored (minus dust)
        assert_eq!(Balances::free_balance(MASTER), free_before);
        assert_eq!(Balances::reserved_balance(MASTER), 0);
    });
}

// =========================================================================
// 3. Ghost-State Cascade: cleanup_account
// =========================================================================

#[test]
fn cleanup_cancels_open_quests_and_unreserves_escrow() {
    new_test_ext().execute_with(|| {
        let guild_id = create_guild();

        // Alice (MASTER) publishes 3 open quests
        publish_quest(MASTER, guild_id, 100 * UNIT);
        publish_quest(MASTER, guild_id, 50 * UNIT);
        publish_quest(MASTER, guild_id, 25 * UNIT);

        // Reserved: 175 UNIT
        assert_eq!(Balances::reserved_balance(MASTER), 175 * UNIT);

        // Trigger ghost-state cleanup (simulates exile/death)
        Guilds::cleanup_account(&MASTER);

        // Escrow fully unreserved
        assert_eq!(Balances::reserved_balance(MASTER), 0);

        // All 3 quests are now Cancelled
        for quest_id in 0..3u32 {
            assert_eq!(
                Guilds::quests(quest_id).unwrap().status,
                QuestStatus::Cancelled,
            );
        }
    });
}

#[test]
fn cleanup_does_not_cancel_in_progress_quests_escrow() {
    new_test_ext().execute_with(|| {
        let guild_id = create_guild();
        assert_ok!(Guilds::join_guild(RuntimeOrigin::signed(MEMBER), guild_id));

        // Alice publishes, Bob assigns
        let quest_id = publish_quest(MASTER, guild_id, 50 * UNIT);
        assert_ok!(Guilds::assign_quest(
            RuntimeOrigin::signed(MEMBER),
            quest_id
        ));

        // Quest is InProgress — cleanup should still cancel it and unreserve
        Guilds::cleanup_account(&MASTER);

        assert_eq!(Balances::reserved_balance(MASTER), 0);
        assert_eq!(
            Guilds::quests(quest_id).unwrap().status,
            QuestStatus::Cancelled,
        );
    });
}

#[test]
fn cleanup_strips_academy_membership() {
    new_test_ext().execute_with(|| {
        // Manually plant an Academy entry for Alice
        let tag: frame_support::BoundedVec<u8, frame_support::traits::ConstU32<64>> =
            b"Engineering".to_vec().try_into().unwrap();
        AcademyMembers::<Test>::insert(MASTER, tag);
        assert!(Guilds::academy_members(MASTER).is_some());

        Guilds::cleanup_account(&MASTER);

        assert!(Guilds::academy_members(MASTER).is_none());
    });
}

// =========================================================================
// 4. OnTerminalStatus — constitutional cascade hooks
// =========================================================================

#[test]
fn on_exiled_triggers_cleanup() {
    new_test_ext().execute_with(|| {
        let guild_id = create_guild();

        // Alice publishes a quest
        publish_quest(MASTER, guild_id, 100 * UNIT);
        assert_eq!(Balances::reserved_balance(MASTER), 100 * UNIT);

        // Simulate exile cascade (called by pallet-judicial-courts via identity hook)
        <Guilds as pallet_inomad_identity::OnTerminalStatus<AccountId>>::on_exiled(&MASTER);

        // Quest cancelled, escrow released
        assert_eq!(Balances::reserved_balance(MASTER), 0);
        assert_eq!(Guilds::quests(0).unwrap().status, QuestStatus::Cancelled);
    });
}

#[test]
fn on_deceased_triggers_cleanup() {
    new_test_ext().execute_with(|| {
        let guild_id = create_guild();

        // Alice publishes a quest
        publish_quest(MASTER, guild_id, 75 * UNIT);
        assert_eq!(Balances::reserved_balance(MASTER), 75 * UNIT);

        // Simulate death cascade
        <Guilds as pallet_inomad_identity::OnTerminalStatus<AccountId>>::on_deceased(&MASTER);

        // Quest cancelled, escrow released
        assert_eq!(Balances::reserved_balance(MASTER), 0);
    });
}

#[test]
fn on_exiled_with_no_quests_is_noop() {
    new_test_ext().execute_with(|| {
        // No quests published — cleanup should be silent
        <Guilds as pallet_inomad_identity::OnTerminalStatus<AccountId>>::on_exiled(&MEMBER);
        assert_eq!(Balances::reserved_balance(MEMBER), 0);
    });
}
