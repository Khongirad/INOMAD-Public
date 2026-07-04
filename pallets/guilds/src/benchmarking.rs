#![cfg(feature = "runtime-benchmarks")]
use super::*;
use frame_benchmarking::v2::*;
use frame_support::{
    pallet_prelude::*,
    traits::{Currency, ReservableCurrency},
};
use frame_system::RawOrigin;

/// Fund an account with enough balance for quest escrow.
fn fund<T: Config>(who: &T::AccountId) {
    let amount: BalanceOf<T> = (1_000_000_000_000u128 * 1_000u128)
        .try_into()
        .unwrap_or_default();
    T::Currency::make_free_balance_be(who, amount);
}

/// Seed a Guild with a Master caller, returns guild_id = 0.
fn seed_guild<T: Config>(master: &T::AccountId) -> u32 {
    let name: BoundedVec<u8, T::MaxNameLength> =
        b"Benchmark Guild".to_vec().try_into().unwrap_or_default();
    let industry: BoundedVec<u8, T::MaxNameLength> =
        b"Technology".to_vec().try_into().unwrap_or_default();
    let guild_id = NextGuildId::<T>::get();
    Guilds::<T>::insert(
        guild_id,
        Guild::<T> {
            founder: master.clone(),
            name,
            industry_tag: industry,
            description_hash: None,
            region_tag: 1u32,
            quest_count: 0u32,
            member_count: 1u32,
        },
    );
    GuildMembers::<T>::insert(guild_id, master, MemberRole::Master);
    NextGuildId::<T>::put(guild_id.saturating_add(1));
    guild_id
}

#[benchmarks]
mod benchmarks {
    use super::*;

    // create_guild(name, industry_tag, description_cid, region_tag) — Signed
    #[benchmark]
    fn create_guild() {
        let caller: T::AccountId = whitelisted_caller();

        #[extrinsic_call]
        create_guild(
            RawOrigin::Signed(caller.clone()),
            b"Rust Engineers".to_vec(),
            b"Software Engineering".to_vec(),
            None,
            1u32, // region_tag
        );

        assert_eq!(NextGuildId::<T>::get(), 1);
    }

    // join_guild(guild_id) — Signed
    #[benchmark]
    fn join_guild() {
        let master: T::AccountId = account("master", 0, 0);
        let guild_id = seed_guild::<T>(&master);
        let joiner: T::AccountId = whitelisted_caller();

        #[extrinsic_call]
        join_guild(RawOrigin::Signed(joiner.clone()), guild_id);

        assert_eq!(
            GuildMembers::<T>::get(guild_id, &joiner),
            Some(MemberRole::Apprentice)
        );
    }

    // promote_member(guild_id, target, new_role) — Signed(Master)
    #[benchmark]
    fn promote_member() {
        let master: T::AccountId = whitelisted_caller();
        let guild_id = seed_guild::<T>(&master);
        let member: T::AccountId = account("member", 0, 0);
        GuildMembers::<T>::insert(guild_id, &member, MemberRole::Apprentice);

        #[extrinsic_call]
        promote_member(
            RawOrigin::Signed(master.clone()),
            guild_id,
            member.clone(),
            MemberRole::Professional,
        );

        assert_eq!(
            GuildMembers::<T>::get(guild_id, &member),
            Some(MemberRole::Professional)
        );
    }

    // propose_achievement(guild_id, target, title, required_quests, proof_cid) — Signed(Master)
    #[benchmark]
    fn propose_achievement() {
        let master: T::AccountId = whitelisted_caller();
        let guild_id = seed_guild::<T>(&master);
        let target: T::AccountId = account("target", 0, 0);
        GuildMembers::<T>::insert(guild_id, &target, MemberRole::Professional);

        #[extrinsic_call]
        propose_achievement(
            RawOrigin::Signed(master.clone()),
            guild_id,
            target.clone(),
            b"Rust Expert".to_vec(),
            0u32,                 // required_quests
            b"QmXyz123".to_vec(), // proof_cid
        );
    }

    // publish_quest(guild_id, reward, description_cid) — Signed(member)
    #[benchmark]
    fn publish_quest() {
        let master: T::AccountId = whitelisted_caller();
        fund::<T>(&master);
        let guild_id = seed_guild::<T>(&master);

        let reward: BalanceOf<T> = 1_000_000u128.try_into().unwrap_or_default();

        #[extrinsic_call]
        publish_quest(
            RawOrigin::Signed(master.clone()),
            guild_id,
            reward,
            b"QmDescriptionHash".to_vec(),
        );

        assert_eq!(NextQuestId::<T>::get(), 1);
    }

    // assign_quest(quest_id) — Signed(member)
    #[benchmark]
    fn assign_quest() {
        let master: T::AccountId = account("master", 0, 0);
        fund::<T>(&master);
        let guild_id = seed_guild::<T>(&master);

        let assignee: T::AccountId = whitelisted_caller();
        GuildMembers::<T>::insert(guild_id, &assignee, MemberRole::Apprentice);

        let reward: BalanceOf<T> = 1_000_000u128.try_into().unwrap_or_default();
        T::Currency::reserve(&master, reward).ok();
        let quest_id = NextQuestId::<T>::get();
        Quests::<T>::insert(
            quest_id,
            Quest::<T> {
                guild_id,
                employer: master.clone(),
                reward,
                status: QuestStatus::Open,
                assignee: None,
                description_hash: b"QmDesc".to_vec().try_into().unwrap_or_default(),
                assignee_quest_count: 0,
            },
        );
        NextQuestId::<T>::put(quest_id.saturating_add(1));

        #[extrinsic_call]
        assign_quest(RawOrigin::Signed(assignee.clone()), quest_id);

        let q = Quests::<T>::get(quest_id).unwrap();
        assert_eq!(q.status, QuestStatus::InProgress);
    }

    impl_benchmark_test_suite!(Pallet, sp_io::TestExternalities::default(), T);
}
