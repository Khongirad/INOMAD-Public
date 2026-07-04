use crate::{mock::*, Error, Event};
use frame_support::{assert_noop, assert_ok};
use pallet_inomad_identity::pallet::{
    CitizenRecord, CitizenRole, CitizenStatus, CitizenshipStatus, PassportType, VerificationStatus,
};
use sp_core::H256;

// Helper: register a citizen directly in identity storage
fn register_citizen(who: u64, nation_id: u32) {
    pallet_inomad_identity::Citizens::<Test>::insert(
        who,
        CitizenRecord {
            citizen_id: who,
            nation_id,
            naturalized_people_id: None,
            role: CitizenRole::Regular,
            status: CitizenStatus::Active,
            verification: VerificationStatus::Unverified,
            vesting_level: None,
            branch: None,
            term_end: None,
            khural_terms_served: 0,
            is_indigenous: false,
            citizenship_status: CitizenshipStatus::Naturalized,
            region_id: None,
            birth_region_id: None,
            passport_type: PassportType::Internal,
            document_hash: H256::zero(),
            birth_page_hash: H256::zero(),
            email_hash: H256::zero(),
        },
    );
}

fn forum_id(s: &str) -> frame_support::BoundedVec<u8, frame_support::traits::ConstU32<128>> {
    s.as_bytes().to_vec().try_into().unwrap()
}

// ─── post_message ─────────────────────────────────────────────────────────────

#[test]
fn post_top_level_message_works() {
    new_test_ext().execute_with(|| {
        register_citizen(1, 1);
        let fid = forum_id("khural:1");
        let hash = [42u8; 32];

        assert_ok!(crate::Pallet::<Test>::post_message(
            frame_system::RawOrigin::Signed(1).into(),
            fid.clone(),
            hash,
            None,
        ));

        let msg = crate::Messages::<Test>::get(0).unwrap();
        assert_eq!(msg.author, 1);
        assert_eq!(msg.content_hash, hash);
        assert_eq!(msg.parent_id, None);
        assert!(!msg.pinned);

        // ForumIndex populated
        assert!(crate::ForumIndex::<Test>::get(&fid, 0).is_some());
        // NextMessageId incremented
        assert_eq!(crate::NextMessageId::<Test>::get(), 1);

        System::assert_last_event(
            Event::MessagePosted {
                message_id: 0,
                author: 1,
                forum_id: b"khural:1".to_vec(),
                parent_id: None,
                content_hash: hash,
                posted_at: 1,
            }
            .into(),
        );
    });
}

#[test]
fn reply_to_message_works() {
    new_test_ext().execute_with(|| {
        register_citizen(1, 1);
        register_citizen(2, 1);
        let fid = forum_id("arbad:1:5");

        // Post root
        assert_ok!(crate::Pallet::<Test>::post_message(
            frame_system::RawOrigin::Signed(1).into(),
            fid.clone(),
            [1u8; 32],
            None,
        ));

        // Reply to root (message_id = 0)
        assert_ok!(crate::Pallet::<Test>::post_message(
            frame_system::RawOrigin::Signed(2).into(),
            fid.clone(),
            [2u8; 32],
            Some(0),
        ));

        let reply = crate::Messages::<Test>::get(1).unwrap();
        assert_eq!(reply.parent_id, Some(0));

        // ThreadReplies indexed
        assert!(crate::ThreadReplies::<Test>::get(0, 1).is_some());
        // Reply is NOT in ForumIndex
        assert!(crate::ForumIndex::<Test>::get(&fid, 1).is_none());
    });
}

#[test]
fn unregistered_citizen_cannot_post() {
    new_test_ext().execute_with(|| {
        assert_noop!(
            crate::Pallet::<Test>::post_message(
                frame_system::RawOrigin::Signed(99).into(),
                forum_id("khural:1"),
                [0u8; 32],
                None,
            ),
            Error::<Test>::NotRegistered,
        );
    });
}

#[test]
fn frozen_citizen_cannot_post() {
    new_test_ext().execute_with(|| {
        register_citizen(1, 1);
        pallet_inomad_identity::Citizens::<Test>::mutate(1, |r| {
            if let Some(rec) = r {
                rec.status = CitizenStatus::Frozen;
            }
        });

        assert_noop!(
            crate::Pallet::<Test>::post_message(
                frame_system::RawOrigin::Signed(1).into(),
                forum_id("khural:1"),
                [0u8; 32],
                None,
            ),
            Error::<Test>::CitizenInactive,
        );
    });
}

#[test]
fn reply_to_nonexistent_message_fails() {
    new_test_ext().execute_with(|| {
        register_citizen(1, 1);
        assert_noop!(
            crate::Pallet::<Test>::post_message(
                frame_system::RawOrigin::Signed(1).into(),
                forum_id("khural:1"),
                [0u8; 32],
                Some(999),
            ),
            Error::<Test>::ParentMessageNotFound,
        );
    });
}

#[test]
fn cross_forum_reply_is_forbidden() {
    new_test_ext().execute_with(|| {
        register_citizen(1, 1);
        register_citizen(2, 1);

        // Root in forum A
        assert_ok!(crate::Pallet::<Test>::post_message(
            frame_system::RawOrigin::Signed(1).into(),
            forum_id("khural:1"),
            [1u8; 32],
            None,
        ));

        // Reply in forum B — forbidden
        assert_noop!(
            crate::Pallet::<Test>::post_message(
                frame_system::RawOrigin::Signed(2).into(),
                forum_id("proposal:7"),
                [2u8; 32],
                Some(0),
            ),
            Error::<Test>::CrossForumReplyForbidden,
        );
    });
}

// ─── pin_message ──────────────────────────────────────────────────────────────

#[test]
fn root_can_pin_message() {
    new_test_ext().execute_with(|| {
        register_citizen(1, 1);
        assert_ok!(crate::Pallet::<Test>::post_message(
            frame_system::RawOrigin::Signed(1).into(),
            forum_id("grand_khural"),
            [3u8; 32],
            None,
        ));

        assert_ok!(crate::Pallet::<Test>::pin_message(
            frame_system::RawOrigin::Root.into(),
            0,
        ));

        assert!(crate::Messages::<Test>::get(0).unwrap().pinned);

        System::assert_last_event(Event::MessagePinned { message_id: 0 }.into());
    });
}

#[test]
fn pin_nonexistent_message_fails() {
    new_test_ext().execute_with(|| {
        assert_noop!(
            crate::Pallet::<Test>::pin_message(frame_system::RawOrigin::Root.into(), 9999,),
            Error::<Test>::MessageNotFound,
        );
    });
}
