#![cfg(feature = "runtime-benchmarks")]
use super::*;
use frame_benchmarking::v2::*;
use frame_support::pallet_prelude::*;
use frame_system::RawOrigin;

#[benchmarks(
    where
        T::IssuerOrigin: frame_support::traits::EnsureOrigin<
            T::RuntimeOrigin,
            Success = T::AccountId,
        >,
)]
mod benchmarks {
    use super::*;

    // ── 0. issue_access_key ───────────────────────────────────────────────────
    // issue_access_key(origin, entity_kind, entity_id, holder, role, access_level, portal_mask)
    // IssuerOrigin is the gating type — in mock it's configured as EnsureRoot.
    #[benchmark]
    fn issue_access_key() {
        let holder: T::AccountId = account("holder", 0, 0);
        let entity_id: Vec<u8> = b"test-entity-001".to_vec();
        let token_id = NextTokenId::<T>::get();

        #[extrinsic_call]
        issue_access_key(
            RawOrigin::Root,
            EntityKind::Organization,
            entity_id,
            holder.clone(),
            1u16, // role: officer
            5u8,  // access_level: mid-tier (1-10)
            3u8,  // portal_mask: 0b00000011
        );

        assert!(AccessKeys::<T>::contains_key(token_id));
    }

    // ── 1. revoke_access_key ──────────────────────────────────────────────────
    // revoke_access_key(origin, token_id: u64)
    #[benchmark]
    fn revoke_access_key() {
        let issuer: T::AccountId = account("issuer", 0, 0);
        let holder: T::AccountId = account("holder", 0, 0);
        let token_id: u64 = 999u64;
        let now = frame_system::Pallet::<T>::block_number();

        // Pre-seed a non-revoked AccessKeyInfo
        AccessKeys::<T>::insert(
            token_id,
            AccessKeyInfo {
                token_id,
                entity_kind: EntityKind::Organization,
                entity_id: BoundedVec::try_from(b"test".to_vec()).unwrap_or_default(),
                holder: holder.clone(),
                role: 1u16,
                access_level: 5u8,
                portal_mask: 3u8,
                issued_at: now,
                revoked: false,
                issued_by: issuer.clone(),
            },
        );
        NextTokenId::<T>::put(token_id + 1);

        #[extrinsic_call]
        revoke_access_key(RawOrigin::Root, token_id);

        let key = AccessKeys::<T>::get(token_id).expect("key exists");
        assert!(key.revoked);
    }

    impl_benchmark_test_suite!(Pallet, crate::mock::new_test_ext(), crate::mock::Test);
}
