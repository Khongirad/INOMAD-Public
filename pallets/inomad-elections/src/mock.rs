//! Mock runtime for pallet-inomad-elections unit tests.
//!
//! Covers the complete hierarchy:
//!   Arbad → Zun → Myangad → Tumed → [Peak Governance]
//!
//! ## Well-Known Accounts
//!
//! | Account | Role | Indigenous | Tier |
//! |---------|------|------------|------|
//! | ALICE   | Arbad citizen #1 | ✅ | Arbad |
//! | BOB     | Arbad citizen #2 | ✅ | Arbad |
//! | CHARLIE | No membership    | ❌ | None  |
//! | DAVE    | Arbad Leader (Zun rights) | ✅ | Zun |
//! | EVE     | Zun Leader (Myangad rights) | ✅ | Myangad |
//! | FRANK..OSCAR | **Tumed Leaders** | ✅ | Tumed |
//! | HEIDI   | Tumed + Khural Chairman candidate (has Arbad) | ✅ | Tumed |
//!
//! ## Constitutional constants (test values)
//!
//! | Constant | Value | Meaning |
//! |---|---|---|
//! | `MinArbadSize` | 10 | Arbad needs 10 citizens before leader can go to Zun |
//! | `MinZunSize` | 100 | Zun needs 100 Arbad leaders before leader can go to Myangad |
//! | `MinMyangadSize` | 1000 | Myangad needs 1000 Zun leaders before leader can go to Tumed |

use frame_support::{construct_runtime, derive_impl, parameter_types, traits::ConstU32};
use sp_runtime::BuildStorage;

// ── Types ──────────────────────────────────────────────────────────────────

pub type AccountId = u64;
pub type Block = frame_system::mocking::MockBlock<Test>;

// ── Well-known accounts ─────────────────────────────────────────────────────

// Decimal hierarchy
pub const ALICE: AccountId = 1;
pub const BOB: AccountId = 2;
pub const CHARLIE: AccountId = 3; // not indigenous — used to test Khural restriction
pub const DAVE: AccountId = 4;
pub const EVE: AccountId = 5;

// Tumed leaders (9 for filling a full council)
pub const FRANK: AccountId = 6;
pub const GRACE: AccountId = 7;
pub const HEIDI: AccountId = 8; // also has a CitizenArbad entry
pub const IVAN: AccountId = 9;
pub const JUDY: AccountId = 10;
pub const KARL: AccountId = 11;
pub const LENA: AccountId = 12;
pub const MIKE: AccountId = 13;
pub const NINA: AccountId = 14;
pub const OSCAR: AccountId = 15; // 10th Tumed — used to test council-member-only restriction

/// The 9 council members by default for Executive/Judicial/Banking tests.
pub const COUNCIL_9: [AccountId; 9] = [FRANK, GRACE, HEIDI, IVAN, JUDY, KARL, LENA, MIKE, NINA];

// Arbad / election IDs
pub const ARBAD_ID: u32 = 1;
pub const HEIDI_ARBAD_ID: u32 = 7; // Heidi's home Arbad — the "nation-representing" one

pub const ARBAD_ELECTION_ID: u32 = 10;
pub const ZUN_ELECTION_ID: u32 = 20;
pub const MYANGAD_ELECTION_ID: u32 = 30;
pub const TUMED_ELECTION_ID: u32 = 40;

// ── Indigenous hook ─────────────────────────────────────────────────────────

/// Test implementation: CHARLIE is NOT indigenous; all other accounts are.
pub struct TestIsIndigenous;
impl crate::pallet::IsIndigenousCitizen<AccountId> for TestIsIndigenous {
    fn is_indigenous(who: &AccountId) -> bool {
        *who != CHARLIE
    }
}

// ── construct_runtime ──────────────────────────────────────────────────────

construct_runtime!(
    pub enum Test {
        System:    frame_system,
        Elections: crate,
    }
);

#[derive_impl(frame_system::config_preludes::TestDefaultConfig)]
impl frame_system::Config for Test {
    type Block = Block;
}

parameter_types! {
    // Constitutional size requirements.
    pub const MinArbadSize: u32 = 10;      // Арбад: ≥ 10 членов
    pub const MinZunSize: u32 = 100;       // Зун: ≥ 100 членов (документация)
    pub const MinZunLeaders: u32 = 10;     // Зун: ≥ 10 Лидеров Арбадов (проверка)
    pub const MinMyangadSize: u32 = 1_000; // Мянгад: ≥ 1000 членов (документация)
    pub const MinMyangadLeaders: u32 = 10; // Мянгад: ≥ 10 Лидеров Зунов (проверка)
}

impl crate::Config for Test {
    type MaxCandidates = ConstU32<32>;
    type MaxBranchCandidates = ConstU32<32>;
    type WeightInfo = ();
    type MinArbadSize = MinArbadSize;         // 10 members per Arbad
    type MinZunSize = MinZunSize;             // 100 total members (docs)
    type MinZunLeaders = MinZunLeaders;       // 10 Arbad Leaders required
    type MinMyangadSize = MinMyangadSize;     // 1000 total members (docs)
    type MinMyangadLeaders = MinMyangadLeaders; // 10 Zun Leaders required
    type IsIndigenous = TestIsIndigenous;
}

// ── Storage seeding helpers ────────────────────────────────────────────────

pub fn seed_in_arbad(who: AccountId, arbad_id: u32) {
    crate::pallet::CitizenArbad::<Test>::insert(who, arbad_id);
    // Also increment the constitutional size counter so tests that call
    // promote_leader with ElectionLevel::Zun don't hit ArbadTooSmall.
    crate::pallet::ArbadMemberCount::<Test>::mutate(arbad_id, |c| *c = c.saturating_add(1));
}

pub fn seed_leader(who: AccountId, level: crate::pallet::ElectionLevel) {
    crate::pallet::ElectedLeaders::<Test>::insert(who, level);
}

pub fn seed_election(
    election_id: u32,
    level: crate::pallet::ElectionLevel,
    candidates: Vec<AccountId>,
) {
    use frame_support::BoundedVec;
    let bounded: BoundedVec<AccountId, ConstU32<32>> = candidates
        .try_into()
        .expect("seed_election: too many candidates");
    crate::pallet::Elections::<Test>::insert(
        election_id,
        crate::pallet::Election {
            level,
            is_active: true,
            candidates: bounded,
        },
    );
}

/// Seed an Executive/Judicial/Banking council directly (skips voting phase).
pub fn seed_council(branch: crate::pallet::GovernmentBranch, members: Vec<AccountId>) {
    use frame_support::BoundedVec;
    let bounded: BoundedVec<AccountId, ConstU32<9>> =
        members.try_into().expect("seed_council: must be exactly 9");
    crate::pallet::BranchCouncils::<Test>::insert(&branch, bounded);
}

/// Seed Zun leader count so promote_leader(Myangad) does not hit ZunTooSmall.
/// Sets ZunLeaderCount to MinZunLeaders (10) — meaning 10 Arbad Leaders are
/// present in this Zun zone, implying ≥ 100 total citizens.
pub fn seed_zun_ready(zun_id: u32) {
    crate::pallet::ZunLeaderCount::<Test>::insert(zun_id, MinZunLeaders::get());
}

/// Seed Myangad leader count so promote_leader(Tumed) does not hit MyangadTooSmall.
/// Sets MyangadLeaderCount to MinMyangadLeaders (10) — meaning 10 Zun Leaders are
/// present in this Myangad zone, implying ≥ 1 000 total citizens.
pub fn seed_myangad_ready(myangad_id: u32) {
    crate::pallet::MyangadLeaderCount::<Test>::insert(myangad_id, MinMyangadLeaders::get());
}

// ── Genesis ────────────────────────────────────────────────────────────────

pub fn new_test_ext() -> sp_io::TestExternalities {
    let t = frame_system::GenesisConfig::<Test>::default()
        .build_storage()
        .unwrap();

    let mut ext = sp_io::TestExternalities::new(t);
    ext.execute_with(|| {
        System::set_block_number(1);

        // Decimal hierarchy citizens
        seed_in_arbad(ALICE, ARBAD_ID);
        seed_in_arbad(BOB, ARBAD_ID);
        seed_in_arbad(DAVE, ARBAD_ID);

        // Pre-populate ArbadMemberCount to MinArbadSize so existing tests
        // that call promote_leader(Zun) don't fail on ArbadTooSmall.
        // (seed_in_arbad already increments per call, but we set it directly
        // to the required minimum here for the full Arbad unit.)
        crate::pallet::ArbadMemberCount::<Test>::insert(ARBAD_ID, MinArbadSize::get());

        seed_leader(DAVE, crate::pallet::ElectionLevel::Zun);
        seed_leader(EVE, crate::pallet::ElectionLevel::Myangad);

        // All 9+1 Tumed leaders
        for &who in &[
            FRANK, GRACE, HEIDI, IVAN, JUDY, KARL, LENA, MIKE, NINA, OSCAR,
        ] {
            seed_leader(who, crate::pallet::ElectionLevel::Tumed);
        }

        // HEIDI has a CitizenArbad entry (required to become Khural Chairman)
        seed_in_arbad(HEIDI, HEIDI_ARBAD_ID);

        // Standard decimal elections
        seed_election(
            ARBAD_ELECTION_ID,
            crate::pallet::ElectionLevel::Arbad,
            vec![ALICE, BOB, DAVE],
        );
        seed_election(
            ZUN_ELECTION_ID,
            crate::pallet::ElectionLevel::Zun,
            vec![DAVE, EVE],
        );
        seed_election(
            MYANGAD_ELECTION_ID,
            crate::pallet::ElectionLevel::Myangad,
            vec![EVE, FRANK],
        );
        seed_election(
            TUMED_ELECTION_ID,
            crate::pallet::ElectionLevel::Tumed,
            vec![FRANK],
        );
    });
    ext
}
