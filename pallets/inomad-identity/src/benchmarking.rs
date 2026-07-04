//! Benchmarks for pallet-inomad-identity
//!
//! Covers all 23 extrinsics (23 weight placeholders):
//!   0.  register_citizen           (Root)
//!   1.  verify_citizen             (Root)
//!   2.  bootstrap_verify_creator   (Root)
//!   3.  update_role                (Root)
//!   4.  freeze_citizen             (Root)
//!   5.  unfreeze_citizen           (Root)
//!   6.  form_arbad                 (Signed — verified Regular)
//!   7.  form_family_arbad          (Signed — verified Regular)
//!   8.  register_birth             (Root)
//!   9.  register_death             (Root)
//!   10. form_zun                   (Signed — ArbadLeader)
//!   11. form_myangad               (Signed — ZunLeader)
//!   12. form_tumed                 (Signed — MyangadLeader)
//!   13. form_khural                (Signed — TumedLeader, Legislative)
//!   14. form_confederation         (Signed — KhuralDelegate)
//!   15. assign_guardian            (Root)
//!   16. leave_arbad                (Signed — member)
//!   17. claim_repatriation         (Root)
//!   18. claim_birthright           (Signed — registered)
//!   19. register_marriage          (Signed)
//!   20. register_divorce           (Signed)
//!   21. register_nickname          (Signed)
//!   22. clear_nickname             (Signed)

#![cfg(feature = "runtime-benchmarks")]

use super::*;
use alloc::vec::Vec;
use frame_benchmarking::v2::*;
use frame_support::pallet_prelude::*;
use frame_support::traits::{Currency, ReservableCurrency};
use frame_system::RawOrigin;
use sp_core::H256;

const UNIT: u128 = 1_000_000_000_000u128;
const NATION: u32 = 1u32;

// ---------------------------------------------------------------------------
// Minimal CitizenRecord constructor
// ---------------------------------------------------------------------------

fn base_citizen(citizen_id: u64) -> CitizenRecord {
    CitizenRecord {
        citizen_id,
        nation_id: NATION,
        naturalized_people_id: None,
        role: CitizenRole::Regular,
        status: CitizenStatus::Active,
        verification: VerificationStatus::Unverified,
        vesting_level: None,
        branch: None,
        term_end: None,
        khural_terms_served: 0,
        is_indigenous: false,
        citizenship_status: CitizenshipStatus::Indigenous,
        region_id: None,
        birth_region_id: None,
        passport_type: PassportType::Internal,
        document_hash: H256::zero(),
        birth_page_hash: H256::zero(),
        email_hash: H256::zero(),
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn fund<T: Config>(who: &T::AccountId) {
    let amount: BalanceOf<T> = (UNIT * 1_000).try_into().unwrap_or_default();
    <T as Config>::Currency::make_free_balance_be(who, amount);
}

/// Insert a verified Active Regular citizen.
fn seed_verified<T: Config>(who: &T::AccountId) {
    let id = NextCitizenId::<T>::get();
    NextCitizenId::<T>::put(id + 1);
    let mut rec = base_citizen(id);
    rec.verification = VerificationStatus::Verified;
    Citizens::<T>::insert(who, rec);
}

/// Insert an unverified Active Regular citizen.
fn seed_unverified<T: Config>(who: &T::AccountId) {
    let id = NextCitizenId::<T>::get();
    NextCitizenId::<T>::put(id + 1);
    Citizens::<T>::insert(who, base_citizen(id));
}

/// Insert a Minor (status = Minor) citizen — used for register_birth / assign_guardian.
fn seed_minor<T: Config>(who: &T::AccountId) {
    let id = NextCitizenId::<T>::get();
    NextCitizenId::<T>::put(id + 1);
    let mut rec = base_citizen(id);
    rec.status = CitizenStatus::Minor;
    Citizens::<T>::insert(who, rec);
}

/// Seed a complete Arbad in storage (bypasses form_arbad extrinsic overhead).
/// Returns `arbad_id`.
fn seed_arbad<T: Config>(leader: &T::AccountId, members: &[T::AccountId]) -> u32 {
    let aid = NextArbadId::<T>::get();
    NextArbadId::<T>::put(aid + 1);

    let bounded: BoundedVec<T::AccountId, ConstU32<9>> =
        members.to_vec().try_into().expect("≤9 members");

    Arbads::<T>::insert(
        aid,
        ArbadRecord::<T> {
            leader: leader.clone(),
            members: bounded,
            nation_id: NATION,
            marriage_credential_hash: None,
        },
    );
    ArbadByLeader::<T>::insert(leader, aid);

    // Promote leader to ArbadLeader
    Citizens::<T>::mutate(leader, |maybe| {
        if let Some(r) = maybe.as_mut() {
            r.role = CitizenRole::ArbadLeader;
        }
    });
    aid
}

/// Seed a Zun from 10 Arbad IDs.
fn seed_zun<T: Config>(leader: &T::AccountId, arbad_ids: &[u32]) -> u32 {
    let zid = NextZunId::<T>::get();
    NextZunId::<T>::put(zid + 1);

    let bounded: BoundedVec<u32, ConstU32<10>> = arbad_ids.to_vec().try_into().expect("≤10");

    Zuns::<T>::insert(
        zid,
        ZunRecord::<T> {
            leader: leader.clone(),
            arbads: bounded,
            nation_id: NATION,
        },
    );
    ZunByLeader::<T>::insert(leader, zid);

    Citizens::<T>::mutate(leader, |maybe| {
        if let Some(r) = maybe.as_mut() {
            r.role = CitizenRole::ZunLeader;
        }
    });
    zid
}

/// Seed a Myangad from 10 Zun IDs.
fn seed_myangad<T: Config>(leader: &T::AccountId, zun_ids: &[u32], branch: BranchOfPower) -> u32 {
    let mid = NextMyangadId::<T>::get();
    NextMyangadId::<T>::put(mid + 1);

    let bounded: BoundedVec<u32, ConstU32<10>> = zun_ids.to_vec().try_into().expect("≤10");

    Myangads::<T>::insert(
        mid,
        MyangadRecord::<T> {
            leader: leader.clone(),
            zuns: bounded,
            branch: branch.clone(),
            nation_id: NATION,
        },
    );
    MyangadByLeader::<T>::insert(leader, mid);

    Citizens::<T>::mutate(leader, |maybe| {
        if let Some(r) = maybe.as_mut() {
            r.role = CitizenRole::MyangadLeader;
            r.branch = Some(branch);
        }
    });
    mid
}

/// Seed a Tumed from 10 Myangad IDs.
fn seed_tumed<T: Config>(leader: &T::AccountId, myangad_ids: &[u32], branch: BranchOfPower) -> u32 {
    let tid = NextTumedId::<T>::get();
    NextTumedId::<T>::put(tid + 1);

    let bounded: BoundedVec<u32, ConstU32<10>> = myangad_ids.to_vec().try_into().expect("≤10");

    Tumeds::<T>::insert(
        tid,
        TumedRecord::<T> {
            leader: leader.clone(),
            myangads: bounded,
            branch: branch.clone(),
            nation_id: NATION,
        },
    );
    TumedByLeader::<T>::insert(leader, tid);

    Citizens::<T>::mutate(leader, |maybe| {
        if let Some(r) = maybe.as_mut() {
            r.role = CitizenRole::TumedLeader;
            r.branch = Some(branch);
        }
    });
    tid
}

/// Seed a Khural from Tumed IDs.
fn seed_khural<T: Config>(leader: &T::AccountId, tumed_ids: &[u32]) -> u32 {
    let kid = NextKhuralId::<T>::get();
    NextKhuralId::<T>::put(kid + 1);

    let bounded: BoundedVec<u32, ConstU32<100>> = tumed_ids.to_vec().try_into().expect("≤100");

    Khurals::<T>::insert(
        kid,
        KhuralRecord::<T> {
            leader: leader.clone(),
            tumeds: bounded,
            nation_id: NATION,
        },
    );
    KhuralByLeader::<T>::insert(leader, kid);

    Citizens::<T>::mutate(leader, |maybe| {
        if let Some(r) = maybe.as_mut() {
            r.role = CitizenRole::KhuralDelegate;
            r.branch = Some(BranchOfPower::Legislative);
        }
    });
    kid
}

// ---------------------------------------------------------------------------
// Benchmark suite
// ---------------------------------------------------------------------------

#[benchmarks]
mod benchmarks {
    use super::*;

    // ── register_citizen ─────────────────────────────────────────────────────

    #[benchmark]
    fn register_citizen() {
        let target: T::AccountId = account("new_citizen", 0, 0);
        let doc = H256::from([0x01u8; 32]);
        let birth = H256::from([0x02u8; 32]);
        let email = H256::from([0x03u8; 32]);

        #[extrinsic_call]
        register_citizen(
            RawOrigin::Root,
            target.clone(),
            NATION,
            CitizenRole::Regular,
            true,
            None,
            None,
            PassportType::Internal,
            doc,
            birth,
            email,
        );

        assert!(Citizens::<T>::contains_key(&target));
    }

    // ── verify_citizen ───────────────────────────────────────────────────────

    #[benchmark]
    fn verify_citizen() {
        let verifier: T::AccountId = whitelisted_caller();
        let new_citizen: T::AccountId = account("citizen", 0, 0);
        seed_verified::<T>(&verifier);
        seed_unverified::<T>(&new_citizen);

        #[extrinsic_call]
        verify_citizen(RawOrigin::Root, new_citizen.clone());

        let rec = Citizens::<T>::get(&new_citizen).expect("exists");
        assert_eq!(rec.verification, VerificationStatus::Verified);
    }

    // ── bootstrap_verify_creator ─────────────────────────────────────────────

    #[benchmark]
    fn bootstrap_verify_creator() {
        let creator: T::AccountId = account("creator", 0, 0);
        seed_unverified::<T>(&creator);

        #[extrinsic_call]
        bootstrap_verify_creator(RawOrigin::Root, creator.clone());

        let rec = Citizens::<T>::get(&creator).expect("exists");
        assert_eq!(rec.verification, VerificationStatus::Verified);
    }

    // ── update_role ──────────────────────────────────────────────────────────

    #[benchmark]
    fn update_role() {
        let target: T::AccountId = account("citizen", 0, 0);
        seed_verified::<T>(&target);

        #[extrinsic_call]
        update_role(RawOrigin::Root, target.clone(), CitizenRole::ArbadLeader);

        let rec = Citizens::<T>::get(&target).expect("exists");
        assert_eq!(rec.role, CitizenRole::ArbadLeader);
    }

    // ── freeze_citizen ───────────────────────────────────────────────────────

    #[benchmark]
    fn freeze_citizen() {
        let target: T::AccountId = account("citizen", 0, 0);
        seed_verified::<T>(&target);

        #[extrinsic_call]
        freeze_citizen(RawOrigin::Root, target.clone());

        let rec = Citizens::<T>::get(&target).expect("exists");
        assert_eq!(rec.status, CitizenStatus::Frozen);
    }

    // ── unfreeze_citizen ─────────────────────────────────────────────────────

    #[benchmark]
    fn unfreeze_citizen() {
        let target: T::AccountId = account("citizen", 0, 0);
        seed_verified::<T>(&target);
        Citizens::<T>::mutate(&target, |maybe| {
            if let Some(r) = maybe.as_mut() {
                r.status = CitizenStatus::Frozen;
            }
        });

        #[extrinsic_call]
        unfreeze_citizen(RawOrigin::Root, target.clone());

        let rec = Citizens::<T>::get(&target).expect("exists");
        assert_eq!(rec.status, CitizenStatus::Active);
    }

    // ── form_arbad ───────────────────────────────────────────────────────────

    #[benchmark]
    fn form_arbad() {
        let caller: T::AccountId = whitelisted_caller();
        fund::<T>(&caller);
        seed_verified::<T>(&caller);

        // 1 additional member minimum
        let member: T::AccountId = account("member", 0, 0);
        seed_verified::<T>(&member);

        let members = alloc::vec![member.clone()];

        #[extrinsic_call]
        form_arbad(RawOrigin::Signed(caller.clone()), members);

        assert!(ArbadByLeader::<T>::contains_key(&caller));
    }

    // ── form_family_arbad ────────────────────────────────────────────────────

    #[benchmark]
    fn form_family_arbad() {
        let caller: T::AccountId = whitelisted_caller();
        let spouse: T::AccountId = account("spouse", 0, 0);
        fund::<T>(&caller);
        fund::<T>(&spouse);
        seed_verified::<T>(&caller);
        seed_verified::<T>(&spouse);

        #[extrinsic_call]
        form_family_arbad(
            RawOrigin::Signed(caller.clone()),
            spouse.clone(),
            caller.clone(), // head = caller
            None,           // no marriage credential for benchmark
        );

        assert!(ArbadByLeader::<T>::contains_key(&caller));
    }

    // ── register_birth ───────────────────────────────────────────────────────

    #[benchmark]
    fn register_birth() {
        let child: T::AccountId = account("child", 0, 0);
        let parent: T::AccountId = account("parent", 0, 0);
        seed_verified::<T>(&parent);
        let child_hash = H256::from([0xBBu8; 32]);

        #[extrinsic_call]
        register_birth(
            RawOrigin::Root,
            child.clone(),
            parent.clone(),
            None,
            child_hash,
            1u8, // birth_region_id
        );

        assert!(Citizens::<T>::contains_key(&child));
    }

    // ── register_death ───────────────────────────────────────────────────────

    #[benchmark]
    fn register_death() {
        let target: T::AccountId = account("citizen", 0, 0);
        seed_verified::<T>(&target);
        fund::<T>(&target);
        let death_hash = H256::from([0xDEu8; 32]);

        #[extrinsic_call]
        register_death(RawOrigin::Root, target.clone(), death_hash);

        let rec = Citizens::<T>::get(&target).expect("exists");
        assert_eq!(rec.status, CitizenStatus::Deceased);
    }

    // ── form_zun ─────────────────────────────────────────────────────────────

    #[benchmark]
    fn form_zun() {
        let caller: T::AccountId = whitelisted_caller();
        seed_verified::<T>(&caller);

        // Create 10 Arbads (minimum required by form_zun)
        let mut arbad_ids: Vec<u32> = Vec::new();
        for i in 0..10u32 {
            let leader: T::AccountId = account("arbad_leader", i, 0);
            seed_verified::<T>(&leader);
            let member: T::AccountId = account("arbad_member", i, 0);
            seed_verified::<T>(&member);
            let aid = seed_arbad::<T>(&leader, &[member]);
            arbad_ids.push(aid);
        }

        // Promote caller to ArbadLeader with their own Arbad
        let my_member: T::AccountId = account("my_member", 99, 0);
        seed_verified::<T>(&my_member);
        seed_arbad::<T>(&caller, &[my_member]);

        #[extrinsic_call]
        form_zun(RawOrigin::Signed(caller.clone()), arbad_ids);

        assert!(ZunByLeader::<T>::contains_key(&caller));
    }

    // ── form_myangad ─────────────────────────────────────────────────────────

    #[benchmark]
    fn form_myangad() {
        let caller: T::AccountId = whitelisted_caller();
        seed_verified::<T>(&caller);

        // Seed 10 Zuns
        let mut zun_ids: Vec<u32> = Vec::new();
        for i in 0..10u32 {
            let zleader: T::AccountId = account("z_leader", i, 0);
            seed_verified::<T>(&zleader);
            Citizens::<T>::mutate(&zleader, |m| {
                if let Some(r) = m.as_mut() {
                    r.role = CitizenRole::ArbadLeader;
                }
            });
            let zid = seed_zun::<T>(&zleader, &[i]);
            zun_ids.push(zid);
        }

        // Caller must be ZunLeader
        let my_arbad: T::AccountId = account("my_arbad_l", 99, 0);
        seed_verified::<T>(&my_arbad);
        seed_arbad::<T>(&my_arbad, &[]);
        seed_zun::<T>(&caller, &[99u32]);

        #[extrinsic_call]
        form_myangad(
            RawOrigin::Signed(caller.clone()),
            BranchOfPower::Legislative,
            zun_ids,
        );

        assert!(MyangadByLeader::<T>::contains_key(&caller));
    }

    // ── form_tumed ───────────────────────────────────────────────────────────

    #[benchmark]
    fn form_tumed() {
        let caller: T::AccountId = whitelisted_caller();
        seed_verified::<T>(&caller);

        // Seed 10 Legislative Myangads
        let mut myangad_ids: Vec<u32> = Vec::new();
        for i in 0..10u32 {
            let ml: T::AccountId = account("m_leader", i, 0);
            seed_verified::<T>(&ml);
            let mid = seed_myangad::<T>(&ml, &[i], BranchOfPower::Legislative);
            myangad_ids.push(mid);
        }

        // Caller must be MyangadLeader
        seed_myangad::<T>(&caller, &[99u32], BranchOfPower::Legislative);

        #[extrinsic_call]
        form_tumed(RawOrigin::Signed(caller.clone()), myangad_ids);

        assert!(TumedByLeader::<T>::contains_key(&caller));
    }

    // ── form_khural ──────────────────────────────────────────────────────────

    #[benchmark]
    fn form_khural() {
        let caller: T::AccountId = whitelisted_caller();
        seed_verified::<T>(&caller);

        // Seed 1 Legislative Tumed (minimum)
        let tumed_leader: T::AccountId = account("tumed_l", 0, 0);
        seed_verified::<T>(&tumed_leader);
        let tid = seed_tumed::<T>(&tumed_leader, &[0u32], BranchOfPower::Legislative);

        // Caller must be Legislative TumedLeader
        seed_tumed::<T>(&caller, &[99u32], BranchOfPower::Legislative);

        #[extrinsic_call]
        form_khural(RawOrigin::Signed(caller.clone()), alloc::vec![tid]);

        assert!(KhuralByLeader::<T>::contains_key(&caller));
    }

    // ── form_confederation ───────────────────────────────────────────────────

    #[benchmark]
    fn form_confederation() {
        let caller: T::AccountId = whitelisted_caller();
        seed_verified::<T>(&caller);

        // Seed 1 Khural from a different nation
        let k_leader: T::AccountId = account("khural_l", 0, 0);
        seed_verified::<T>(&k_leader);
        let kid = seed_khural::<T>(&k_leader, &[0u32]);

        // Caller must be KhuralDelegate
        seed_khural::<T>(&caller, &[99u32]);

        #[extrinsic_call]
        form_confederation(RawOrigin::Signed(caller.clone()), alloc::vec![kid]);

        assert!(ConfederationByLeader::<T>::contains_key(&caller));
    }

    // ── assign_guardian ──────────────────────────────────────────────────────

    #[benchmark]
    fn assign_guardian() {
        let minor: T::AccountId = account("minor", 0, 0);
        let guardian: T::AccountId = account("guardian", 0, 0);
        seed_minor::<T>(&minor);
        seed_verified::<T>(&guardian);

        #[extrinsic_call]
        assign_guardian(RawOrigin::Root, minor.clone(), guardian.clone());

        assert_eq!(GuardianOf::<T>::get(&minor), Some(guardian));
    }

    // ── leave_arbad ──────────────────────────────────────────────────────────

    #[benchmark]
    fn leave_arbad() {
        let caller: T::AccountId = whitelisted_caller();
        let leader: T::AccountId = account("leader", 0, 0);
        seed_verified::<T>(&caller);
        seed_verified::<T>(&leader);

        // Seed Arbad with caller as member
        let arbad_id = seed_arbad::<T>(&leader, &[caller.clone()]);

        #[extrinsic_call]
        leave_arbad(RawOrigin::Signed(caller.clone()), arbad_id);

        // Caller should have a cooldown timestamp set
        assert!(LastArbadLeave::<T>::contains_key(&caller));
    }

    // ── claim_repatriation ───────────────────────────────────────────────────

    #[benchmark]
    fn claim_repatriation() {
        let citizen: T::AccountId = account("citizen", 0, 0);
        seed_unverified::<T>(&citizen);
        // Set citizenship_status to Foreigner — can claim repatriation via lineage proof
        Citizens::<T>::mutate(&citizen, |m| {
            if let Some(r) = m.as_mut() {
                r.citizenship_status = CitizenshipStatus::Foreigner;
            }
        });

        let lineage_proof = [0x42u8; 32];
        // ConstitutionHashProvider returns None in mock — any hash accepted.
        let constitution_hash = [0u8; 32];

        #[extrinsic_call]
        claim_repatriation(
            RawOrigin::Root,
            citizen.clone(),
            lineage_proof,
            constitution_hash,
        );

        let rec = Citizens::<T>::get(&citizen).expect("exists");
        assert_eq!(rec.citizenship_status, CitizenshipStatus::Indigenous);
    }

    // ── claim_birthright ─────────────────────────────────────────────────────

    #[benchmark]
    fn claim_birthright() {
        let caller: T::AccountId = whitelisted_caller();
        seed_unverified::<T>(&caller);
        Citizens::<T>::mutate(&caller, |m| {
            if let Some(r) = m.as_mut() {
                r.citizenship_status = CitizenshipStatus::Foreigner;
            }
        });

        let constitution_hash = [0u8; 32];

        #[extrinsic_call]
        claim_birthright(RawOrigin::Signed(caller.clone()), constitution_hash);

        let rec = Citizens::<T>::get(&caller).expect("exists");
        assert_eq!(rec.citizenship_status, CitizenshipStatus::Naturalized);
    }

    // ── register_marriage ────────────────────────────────────────────────────

    #[benchmark]
    fn register_marriage() {
        let caller: T::AccountId = whitelisted_caller();
        let partner_b: T::AccountId = account("partner_b", 0, 0);
        fund::<T>(&caller);
        fund::<T>(&partner_b);
        // Pre-fund the civil fee treasury
        let treasury = T::CivilFeeTreasury::get();
        fund::<T>(&treasury);

        seed_verified::<T>(&caller);
        seed_verified::<T>(&partner_b);

        let ceremony_block = frame_system::Pallet::<T>::block_number();

        #[extrinsic_call]
        register_marriage(
            RawOrigin::Signed(caller.clone()),
            partner_b.clone(),
            ceremony_block,
        );

        assert!(Marriages::<T>::contains_key(&caller));
    }

    // ── register_divorce ─────────────────────────────────────────────────────

    #[benchmark]
    fn register_divorce() {
        let caller: T::AccountId = whitelisted_caller();
        let partner_b: T::AccountId = account("partner_b", 0, 0);
        seed_verified::<T>(&caller);
        seed_verified::<T>(&partner_b);

        // Seed marriage directly
        Marriages::<T>::insert(&caller, partner_b.clone());
        Marriages::<T>::insert(&partner_b, caller.clone());

        #[extrinsic_call]
        register_divorce(RawOrigin::Signed(caller.clone()));

        assert!(!Marriages::<T>::contains_key(&caller));
    }

    // ── register_nickname ────────────────────────────────────────────────────

    #[benchmark]
    fn register_nickname() {
        let caller: T::AccountId = whitelisted_caller();
        seed_verified::<T>(&caller);

        let name: BoundedVec<u8, ConstU32<24>> =
            b"BenchmarkNick".to_vec().try_into().expect("fits 24");

        #[extrinsic_call]
        register_nickname(RawOrigin::Signed(caller.clone()), name.clone());

        assert!(NicknameOf::<T>::contains_key(&caller));
    }

    // ── clear_nickname ───────────────────────────────────────────────────────

    #[benchmark]
    fn clear_nickname() {
        let caller: T::AccountId = whitelisted_caller();
        seed_verified::<T>(&caller);

        let name: BoundedVec<u8, ConstU32<24>> =
            b"BenchmarkNick".to_vec().try_into().expect("fits 24");

        // Seed nickname directly
        NicknameOf::<T>::insert(&caller, name.clone());
        AccountByNickname::<T>::insert(&name, caller.clone());

        #[extrinsic_call]
        clear_nickname(RawOrigin::Signed(caller.clone()));

        assert!(!NicknameOf::<T>::contains_key(&caller));
    }

    impl_benchmark_test_suite!(Pallet, crate::mock::new_test_ext(), crate::mock::Test);
}
