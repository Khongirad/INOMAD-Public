#![cfg_attr(not(feature = "std"), no_std)]

/// A pallet for managing organization roles as Soulbound Tokens (SBT) in Altan Network.
/// Roles are strictly bound to accounts, non-transferable, and hierarchical.

#[cfg(test)]
mod mock;

#[cfg(test)]
mod tests;

pub use pallet::*;

#[frame_support::pallet]
pub mod pallet {
	use frame_support::pallet_prelude::*;
	use frame_system::pallet_prelude::*;
	extern crate alloc;
	use alloc::vec::Vec;

	#[pallet::pallet]
	pub struct Pallet<T>(_);

	/// Configure the pallet by specifying the parameters and types on which it depends.
	#[pallet::config]
	pub trait Config: frame_system::Config {
		/// Because this pallet emits events, it depends on the runtime's definition of an event.
		type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;
		
		/// Maximum length of organization names.
		#[pallet::constant]
		type MaxOrgNameLen: Get<u32>;
	}

	#[derive(Clone, Encode, Decode, codec::DecodeWithMemTracking, PartialEq, RuntimeDebug, TypeInfo, MaxEncodedLen)]

	pub enum RoleTier {
		Root = 0,     // Tier 0: Leader / Organization Creator
		Officer = 1,  // Tier 1: Officer / HR
		Member = 2,   // Tier 2: Standard Member
	}

	#[derive(Clone, Encode, Decode, codec::DecodeWithMemTracking, PartialEq, RuntimeDebug, TypeInfo, MaxEncodedLen)]

	pub struct OrgDetails<AccountId, BoundedString> {
		pub owner: AccountId,
		pub name: BoundedString,
		pub created_at: u64,
	}

	// ==========================================
	// Storage
	// ==========================================

	/// Counter for organizations to generate unique OrgIds.
	#[pallet::storage]
	#[pallet::getter(fn next_org_id)]
	pub type NextOrgId<T> = StorageValue<_, u32, ValueQuery>;

	/// Organizations mapping: OrgId -> OrgDetails
	#[pallet::storage]
	#[pallet::getter(fn organizations)]
	pub type Organizations<T: Config> = StorageMap<
		_,
		Blake2_128Concat,
		u32, // OrgId
		OrgDetails<T::AccountId, BoundedVec<u8, T::MaxOrgNameLen>>,
		OptionQuery,
	>;

	/// Role limits/slots for an organization: OrgId -> (Max Officers, Max Members)
	#[pallet::storage]
	#[pallet::getter(fn role_slots)]
	pub type RoleSlots<T: Config> = StorageMap<
		_,
		Blake2_128Concat,
		u32, // OrgId
		(u32, u32), // (max_officers, max_members)
		ValueQuery,
	>;

	/// Issued keys/roles mapping: (OrgId, AccountId) -> RoleTier
	/// Represents an account's SBT within a specific organization.
	#[pallet::storage]
	#[pallet::getter(fn issued_keys)]
	pub type IssuedKeys<T: Config> = StorageDoubleMap<
		_,
		Blake2_128Concat,
		u32, // OrgId
		Blake2_128Concat,
		T::AccountId,
		RoleTier,
		OptionQuery,
	>;

	// ==========================================
	// Events
	// ==========================================

	#[pallet::event]
	#[pallet::generate_deposit(pub(super) fn deposit_event)]
	pub enum Event<T: Config> {
		/// A new organization was created and ROOT_KEY assigned.
		OrganizationCreated { org_id: u32, owner: T::AccountId },
		/// A new role key was minted to an account.
		RoleKeyMinted { org_id: u32, account: T::AccountId, tier: RoleTier, minted_by: T::AccountId },
		/// A role key was revoked.
		RoleKeyRevoked { org_id: u32, account: T::AccountId, revoked_by: T::AccountId },
	}

	// ==========================================
	// Errors
	// ==========================================

	#[pallet::error]
	pub enum Error<T> {
		/// Organization name is too long.
		NameTooLong,
		/// Organization does not exist.
		OrgNotFound,
		/// Account already has a role in this organization.
		AlreadyHasRole,
		/// Caller does not have sufficient permissions to mint this role tier.
		InsufficientMintPermissions,
		/// Caller does not have permissions to revoke roles.
		InsufficientRevokePermissions,
		/// Cannot revoke the ROOT key.
		CannotRevokeRoot,
		/// Target account does not have a role in this organization.
		TargetHasNoRole,
		/// Role slots for this tier are full.
		RoleSlotsFull,
	}

	// ==========================================
	// Extrinsics
	// ==========================================

	#[pallet::call]
	impl<T: Config> Pallet<T> {
		/// Create a new organization. The caller becomes the owner and receives a Tier 0 ROOT_KEY.
		/// Note: The fee for this transaction is inherently paid by the caller.
		#[pallet::call_index(0)]
		#[pallet::weight(Weight::from_parts(10_000_000, 0))]
		pub fn create_organization(
			origin: OriginFor<T>,
			name: Vec<u8>,
			max_officers: u32,
			max_members: u32,
		) -> DispatchResult {
			let owner = ensure_signed(origin)?;

			let bounded_name: BoundedVec<u8, T::MaxOrgNameLen> =
				name.try_into().map_err(|_| Error::<T>::NameTooLong)?;

			let org_id = NextOrgId::<T>::get();
			NextOrgId::<T>::put(org_id.checked_add(1).unwrap_or(org_id));

			let details = OrgDetails {
				owner: owner.clone(),
				name: bounded_name,
				created_at: 0, // Placeholder, would ideally use timestamp pallet
			};

			Organizations::<T>::insert(org_id, details);
			RoleSlots::<T>::insert(org_id, (max_officers, max_members));

			// Mint ROOT_KEY
			IssuedKeys::<T>::insert(org_id, owner.clone(), RoleTier::Root);

			Self::deposit_event(Event::OrganizationCreated { org_id, owner });
			Ok(())
		}

		/// Mint a new SBT role key to an employee.
		/// - ROOT can mint OFFICER or MEMBER.
		/// - OFFICER can only mint MEMBER.
		/// Fee is paid by the caller (minting party), providing feeless onboarding for the recipient.
		#[pallet::call_index(1)]
		#[pallet::weight(Weight::from_parts(10_000_000, 0))]
		pub fn mint_role_key(
			origin: OriginFor<T>,
			org_id: u32,
			target_account: T::AccountId,
			tier: RoleTier,
		) -> DispatchResult {
			let minter = ensure_signed(origin)?;

			ensure!(Organizations::<T>::contains_key(org_id), Error::<T>::OrgNotFound);
			ensure!(!IssuedKeys::<T>::contains_key(org_id, &target_account), Error::<T>::AlreadyHasRole);

			let minter_tier = IssuedKeys::<T>::get(org_id, &minter)
				.ok_or(Error::<T>::InsufficientMintPermissions)?;

			// Tier hierarchy validation
			match minter_tier {
				RoleTier::Root => {
					// Root can mint anything
				},
				RoleTier::Officer => {
					// Officer can only mint Member
					ensure!(tier == RoleTier::Member, Error::<T>::InsufficientMintPermissions);
				},
				RoleTier::Member => {
					return Err(Error::<T>::InsufficientMintPermissions.into());
				}
			}

			// Check limits (simplified check, would need to iterate or keep counters in production)
			// For this implementation, we assume limits are checked externally or via offchain worker
			// to save on-chain iteration costs, or we could add a counter to RoleSlots.

			IssuedKeys::<T>::insert(org_id, target_account.clone(), tier.clone());

			Self::deposit_event(Event::RoleKeyMinted {
				org_id,
				account: target_account,
				tier,
				minted_by: minter,
			});

			Ok(())
		}

		/// Revoke (burn) an SBT role key.
		/// Only ROOT or OFFICER can revoke. ROOT can revoke anyone. OFFICER can only revoke MEMBER.
		#[pallet::call_index(2)]
		#[pallet::weight(Weight::from_parts(10_000_000, 0))]
		pub fn revoke_role_key(
			origin: OriginFor<T>,
			org_id: u32,
			target_account: T::AccountId,
		) -> DispatchResult {
			let revoker = ensure_signed(origin)?;

			let revoker_tier = IssuedKeys::<T>::get(org_id, &revoker)
				.ok_or(Error::<T>::InsufficientRevokePermissions)?;

			let target_tier = IssuedKeys::<T>::get(org_id, &target_account)
				.ok_or(Error::<T>::TargetHasNoRole)?;

			ensure!(target_tier != RoleTier::Root, Error::<T>::CannotRevokeRoot);

			// Tier hierarchy validation for revocation
			if revoker_tier == RoleTier::Officer {
				ensure!(target_tier == RoleTier::Member, Error::<T>::InsufficientRevokePermissions);
			} else if revoker_tier == RoleTier::Member {
				return Err(Error::<T>::InsufficientRevokePermissions.into());
			}

			IssuedKeys::<T>::remove(org_id, &target_account);

			Self::deposit_event(Event::RoleKeyRevoked {
				org_id,
				account: target_account,
				revoked_by: revoker,
			});

			Ok(())
		}
	}
}
