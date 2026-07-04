use crate::{mock::*, Error, Event, RoleTier};
use frame_support::{assert_noop, assert_ok};

#[test]
fn it_creates_an_organization() {
	new_test_ext().execute_with(|| {
		assert_ok!(OrgRoles::create_organization(RuntimeOrigin::signed(1), b"Test Org".to_vec(), 5, 100));

		// Check storage
		assert_eq!(OrgRoles::next_org_id(), 1);
		
		let org = OrgRoles::organizations(0).expect("Org should exist");
		assert_eq!(org.owner, 1);
		assert_eq!(org.name.into_inner(), b"Test Org".to_vec());

		// Check ROOT key was minted
		let tier = OrgRoles::issued_keys(0, 1).expect("Root key should be minted");
		assert_eq!(tier, RoleTier::Root);

		// Event check
		System::assert_last_event(
			Event::OrganizationCreated { org_id: 0, owner: 1 }.into()
		);
	});
}

#[test]
fn root_can_mint_officer_and_member() {
	new_test_ext().execute_with(|| {
		assert_ok!(OrgRoles::create_organization(RuntimeOrigin::signed(1), b"Test Org".to_vec(), 5, 100));

		// Mint Officer
		assert_ok!(OrgRoles::mint_role_key(RuntimeOrigin::signed(1), 0, 2, RoleTier::Officer));
		assert_eq!(OrgRoles::issued_keys(0, 2), Some(RoleTier::Officer));

		// Mint Member
		assert_ok!(OrgRoles::mint_role_key(RuntimeOrigin::signed(1), 0, 3, RoleTier::Member));
		assert_eq!(OrgRoles::issued_keys(0, 3), Some(RoleTier::Member));
	});
}

#[test]
fn officer_can_mint_member_but_not_officer() {
	new_test_ext().execute_with(|| {
		assert_ok!(OrgRoles::create_organization(RuntimeOrigin::signed(1), b"Test Org".to_vec(), 5, 100));
		assert_ok!(OrgRoles::mint_role_key(RuntimeOrigin::signed(1), 0, 2, RoleTier::Officer));

		// Officer mints Member -> Success
		assert_ok!(OrgRoles::mint_role_key(RuntimeOrigin::signed(2), 0, 3, RoleTier::Member));
		assert_eq!(OrgRoles::issued_keys(0, 3), Some(RoleTier::Member));

		// Officer tries to mint Officer -> Fail
		assert_noop!(
			OrgRoles::mint_role_key(RuntimeOrigin::signed(2), 0, 4, RoleTier::Officer),
			Error::<Test>::InsufficientMintPermissions
		);
	});
}

#[test]
fn member_cannot_mint() {
	new_test_ext().execute_with(|| {
		assert_ok!(OrgRoles::create_organization(RuntimeOrigin::signed(1), b"Test Org".to_vec(), 5, 100));
		assert_ok!(OrgRoles::mint_role_key(RuntimeOrigin::signed(1), 0, 2, RoleTier::Member));

		// Member tries to mint Member -> Fail
		assert_noop!(
			OrgRoles::mint_role_key(RuntimeOrigin::signed(2), 0, 3, RoleTier::Member),
			Error::<Test>::InsufficientMintPermissions
		);
	});
}

#[test]
fn revocation_hierarchy_enforced() {
	new_test_ext().execute_with(|| {
		assert_ok!(OrgRoles::create_organization(RuntimeOrigin::signed(1), b"Test Org".to_vec(), 5, 100));
		assert_ok!(OrgRoles::mint_role_key(RuntimeOrigin::signed(1), 0, 2, RoleTier::Officer));
		assert_ok!(OrgRoles::mint_role_key(RuntimeOrigin::signed(1), 0, 3, RoleTier::Member));
		assert_ok!(OrgRoles::mint_role_key(RuntimeOrigin::signed(1), 0, 4, RoleTier::Member));

		// Officer revokes Member -> Success
		assert_ok!(OrgRoles::revoke_role_key(RuntimeOrigin::signed(2), 0, 3));
		assert_eq!(OrgRoles::issued_keys(0, 3), None);

		// Officer tries to revoke ROOT -> Fail
		assert_noop!(
			OrgRoles::revoke_role_key(RuntimeOrigin::signed(2), 0, 1),
			Error::<Test>::CannotRevokeRoot
		);

		// Member tries to revoke Member -> Fail
		assert_noop!(
			OrgRoles::revoke_role_key(RuntimeOrigin::signed(4), 0, 2),
			Error::<Test>::InsufficientRevokePermissions
		);

		// Root revokes Officer -> Success
		assert_ok!(OrgRoles::revoke_role_key(RuntimeOrigin::signed(1), 0, 2));
		assert_eq!(OrgRoles::issued_keys(0, 2), None);
	});
}
