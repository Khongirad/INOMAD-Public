// This file is part of the Altan Network.
// SPDX-License-Identifier: Apache-2.0

// ─── Sprint L1-23: Canonical Lore Alignment & SBT Citizen Rule ──────────────────────────────
//
// CREATOR MANDATE: The Central Bank (System Account) is the sole Hard Cap generator.
// It holds exactly 2.1 Trillion ALTAN — zero inflation of the base asset ever.
// The full supply flows through the CB's `trigger_genesis_distribution` extrinsic,
// which the Creator (Citizen #1) signs as BankingOrigin (Sudo) until the CB Board
// is constitutionally formed by democratic election.
//
// CANONICAL FLOW (The Path of Money):
//
//   1. CENTRAL_BANK (keyless multisig)    ← Hard Cap: 2_100_000_000_000 ALTAN
//        └─► Creator signs trigger_genesis_distribution() as Sudo / BankingOrigin
//              ├─► Confederation_Treasury        — 3%  (Confederate Khural, frozen)
//              ├─► 83 × Regional_Gov_Treasury    — 7%  (Republican Khural, frozen)
//              └─► 83 × Regional_Citizen_Fund    — 90% (Bank of Siberia citizen reserves)
//
//   Integer dust from 83-way division → FEDERAL_SWEEP_ACCOUNT (padded to ≥ UNIT)
//
// SEPARATION OF POWERS — Creator Is NOT the Treasury:
//   The Creator (Khongirad, Citizen #1) holds only 100 ALTAN (SBT Level-0 seed).
//   He is the temporary signing authority (BankingOrigin via Sudo) until the
//   CB Board of Directors is democratically elected and Sudo is surrendered.
//   He never personally holds the 2.1T M0 — that belongs to the Republic.
//
// TOTAL EMISSION   : 2,100,000,000,000 ALTAN (2.1 Trillion, Hard Cap)
// 1 ALTAN          : 10^12 planck (PLANCK_FACTOR = 1_000_000_000_000)
//
// CONSTITUTIONAL BUDGET STRUCTURE:
//   90% → Regional_Citizen_Fund × 83    — Population reserves (Proof-of-Citizenship airdrops)
//    7% → Regional_Gov_Treasury  × 83   — Regional government frozen budgets
//    3% → Confederation_Treasury × 1    — Confederation frozen budget
//
// SBT CITIZEN RULE (L1-23):
//   All registered citizens — including dev pioneers — receive exactly 100 ALTAN
//   at registration. This is the Soulbound Token (SBT) Level 0 seed. No exceptions.
//
// ⚠  ED-SAFETY FIX (L1-22):
//   The dust from 83-way integer division is only 74 planck (< ED = 1e9 planck).
//   The FEDERAL_SWEEP_ACCOUNT is seeded with UNIT (1 ALTAN) so it satisfies
//   ExistentialDeposit. The 74-planck dust remainder is added on top.
//
// 83 Federal Subjects of the Russian Federation:
//   21 Republics · 6 Krais · 49 Oblasts · 2 Federal Cities
//   1 Autonomous Oblast · 4 Autonomous Okrugs (incl. in 83 total)
// ─────────────────────────────────────────────────────────────────────────────

use crate::{
    AccountId, AltanTaxConfig, BalancesConfig, CentralBankConfig, RuntimeGenesisConfig, SudoConfig,
    UNIT,
};
use alloc::{vec, vec::Vec};
use frame_support::build_struct_json_patch;
use serde_json::Value;
use sp_consensus_aura::sr25519::AuthorityId as AuraId;
use sp_consensus_grandpa::AuthorityId as GrandpaId;
use sp_core::crypto::Ss58Codec;
use sp_keyring::{Ed25519Keyring, Sr25519Keyring};
use sp_genesis_builder::{self, PresetId};
// ─── Macroeconomic Emission Constants ────────────────────────────────────────
//
// UNIT = 1 ALTAN = 10^12 planck (imported from runtime::lib).
// All balances expressed as `whole_altan * UNIT` for constitutional clarity.

/// Number of Official Federal Subjects of the Russian Federation.
/// 83 = official count per Constitution (Republics + Krais + Oblasts + Cities + AOs)
const REGIONS_COUNT: u128 = 83;

/// Total emission: 2.1 Trillion ALTAN × UNIT
/// = 2_100_000_000_000 × 10^12 planck.
const TOTAL_SUPPLY_PLANCK: u128 = 2_100_000_000_000 * UNIT;

/// 90% pool — Regional_Citizen_Fund accounts (Bank of Siberia citizen reserves).
/// = 1_890_000_000_000 ALTAN × UNIT
const TOTAL_RESERVE_PLANCK: u128 = 1_890_000_000_000 * UNIT;

/// 7% pool — Regional_Gov_Treasury accounts (83 Republican Khural, frozen).
/// = 147_000_000_000 ALTAN × UNIT
const TOTAL_REGIONAL_FROZEN_PLANCK: u128 = 147_000_000_000 * UNIT;

/// 3% pool — Confederation_Treasury account (Confederate Khural, frozen).
/// = 63_000_000_000 ALTAN × UNIT
const TOTAL_CONFEDERATION_FROZEN_PLANCK: u128 = 63_000_000_000 * UNIT;

/// Existential Deposit (matches runtime::EXISTENTIAL_DEPOSIT = MILLI_ALTAN).
/// Used to seed system accounts that have a zero-post-routing balance.
const EXISTENTIAL_DEPOSIT: u128 = 1_000_000_000; // 0.001 ALTAN (well below 100 ALTAN citizen seed)

/// Citizen starting balance for the Creator (Citizen #1).
/// 100 ALTAN — the SBT Level-0 seed. Equal for all citizens. No exceptions.
const CITIZEN_SEED: u128 = 100 * UNIT;

// ── Dev pioneer accounts removed (L1-CB Constitutional Mandate) ──────────────
//
// CONSTITUTIONAL MANDATE (Sprint L1-CB: Central Bank & Genezis):
// All liquidity is in the constitutional economy of the Republic:
//   - 83 × Regional_Citizen_Fund (90% of total supply)
//   - 83 × Regional_Gov_Treasury (7% of total supply)
//   - 1  ×  Confederation_Treasury (3% of total supply)
//
// Dev pioneer Buryat accounts (Bator, Ayur, etc.) are removed from genesis.
// They are NOT citizens of the Republic at genesis — they must register
// via the normal on-chain citizen onboarding flow.
//
// Alice remains as a pure technical key (Aura/Grandpa/Sudo validator),
// NOT a citizen. She receives a minimal operational seed only.

// ─── Compile-time Constitutional Invariants ───────────────────────────────────

// CONSTITUTIONAL INVARIANT: 90% + 7% + 3% = 100% of total emission
// Any arithmetic error here is a compile-time failure.
const _: () = assert!(
    TOTAL_RESERVE_PLANCK + TOTAL_REGIONAL_FROZEN_PLANCK + TOTAL_CONFEDERATION_FROZEN_PLANCK
        == TOTAL_SUPPLY_PLANCK,
    "CONSTITUTIONAL INVARIANT VIOLATED: 90% Reserve + 7% Regional + 3% Confederate must equal 100% of total supply"
);

// ─── 83 Official Federal Subjects (region code, name) ────────────────────────
//
// Codes follow the Russian OKATO classification system.
// Used for on-chain indexing: region_code → AccountId seed.
//
pub const REGIONS_83: &[(u8, &str)] = &[
    // ─── 21 Republics ──────────────────────────────────────────────────────
    (01, "Республика Адыгея"),
    (02, "Республика Башкортостан"),
    (03, "Республика Бурятия (Буряад-Монгол Улас)"),
    (04, "Республика Алтай"),
    (05, "Республика Дагестан"),
    (06, "Республика Ингушетия"),
    (07, "Кабардино-Балкарская Республика"),
    (08, "Республика Калмыкия"),
    (09, "Карачаево-Черкесская Республика"),
    (10, "Республика Карелия"),
    (11, "Республика Коми"),
    (12, "Республика Марий Эл"),
    (13, "Республика Мордовия"),
    (14, "Республика Саха (Якутия)"),
    (15, "Республика Северная Осетия-Алания"),
    (16, "Республика Татарстан"),
    (17, "Республика Тыва"),
    (18, "Удмуртская Республика"),
    (19, "Республика Хакасия"),
    (20, "Чеченская Республика"),
    (21, "Чувашская Республика"),
    // ─── 6 Krais ───────────────────────────────────────────────────────────
    (22, "Алтайский край"),
    (23, "Краснодарский край"),
    (24, "Красноярский край"),
    (25, "Приморский край"),
    (26, "Ставропольский край"),
    (27, "Хабаровский край"),
    // ─── 49 Oblasts ────────────────────────────────────────────────────────
    (28, "Амурская область"),
    (29, "Архангельская область"),
    (30, "Астраханская область"),
    (31, "Белгородская область"),
    (32, "Брянская область"),
    (33, "Владимирская область"),
    (34, "Волгоградская область"),
    (35, "Вологодская область"),
    (36, "Воронежская область"),
    (37, "Ивановская область"),
    (38, "Иркутская область"),
    (39, "Калининградская область"),
    (40, "Калужская область"),
    (41, "Камчатский край"),
    (42, "Кемеровская область"),
    (43, "Кировская область"),
    (44, "Костромская область"),
    (45, "Курганская область"),
    (46, "Курская область"),
    (47, "Ленинградская область"),
    (48, "Липецкая область"),
    (49, "Магаданская область"),
    (50, "Московская область"),
    (51, "Мурманская область"),
    (52, "Нижегородская область"),
    (53, "Новгородская область"),
    (54, "Новосибирская область"),
    (55, "Омская область"),
    (56, "Оренбургская область"),
    (57, "Орловская область"),
    (58, "Пензенская область"),
    (59, "Пермский край"),
    (60, "Псковская область"),
    (61, "Ростовская область"),
    (62, "Рязанская область"),
    (63, "Самарская область"),
    (64, "Саратовская область"),
    (65, "Сахалинская область"),
    (66, "Свердловская область"),
    (67, "Смоленская область"),
    (68, "Тамбовская область"),
    (69, "Тверская область"),
    (70, "Томская область"),
    (71, "Тульская область"),
    (72, "Тюменская область"),
    (73, "Ульяновская область"),
    (74, "Челябинская область"),
    (75, "Забайкальский край"),
    (76, "Ярославская область"),
    // ─── 2 Federal Cities ──────────────────────────────────────────────────
    (77, "Москва"),
    (78, "Санкт-Петербург"),
    // ─── 1 Autonomous Oblast ───────────────────────────────────────────────
    (79, "Еврейская автономная область"),
    // ─── 4 Autonomous Okrugs (included in 83 total count) ──────────────────
    (83, "Ненецкий автономный округ"),
    (86, "Ханты-Мансийский АО — Югра"),
    (87, "Чукотский автономный округ"),
    (89, "Ямало-Ненецкий автономный округ"),
];

// Sanity: must be exactly 83 regions at compile time.
// Note: array length check at runtime in testnet_genesis via assertion.

use codec::Encode;

// Helper to deterministically generate a 7-of-10 multisig account
fn derive_7_of_10_multisig(entity_name: &[u8]) -> AccountId {
    let mut signatories: alloc::vec::Vec<AccountId> = alloc::vec::Vec::new();
    for i in 1..=10u8 {
        let mut seed = [0u8; 32];
        let len = entity_name.len().min(30);
        seed[..len].copy_from_slice(&entity_name[..len]);
        seed[30] = b'_';
        seed[31] = i;
        signatories.push(AccountId::from(seed));
    }
    // Substrate requires signatories to be sorted
    signatories.sort();

    // pallet_multisig derives addresses via blake2_256(b"modlpy/utilisig" ++ signatories ++ threshold)
    let threshold: u16 = 7;
    let entropy = (b"modlpy/utilisig", signatories, threshold).encode();
    let hash = sp_core::hashing::blake2_256(&entropy);
    AccountId::from(hash)
}

// ─── Account Factories ────────────────────────────────────────────────────────

/// The Central Bank — apex issuer of the 2.1T ALTAN supply.
fn central_bank_account() -> AccountId {
    derive_7_of_10_multisig(b"CENTRAL_BANK")
}

/// Bank_of_Siberia_Main_Reserve — correspondent account of the Central Bank.
fn bank_of_siberia_main_reserve_account() -> AccountId {
    derive_7_of_10_multisig(b"BANK_OF_SIBERIA_MASTER")
}

/// Confederation_Treasury — holds the 3% confederation frozen budget.
fn confederation_treasury_account() -> AccountId {
    derive_7_of_10_multisig(b"CONFEDERATION_BUDGET")
}

/// Federal Sweep Account — receives all integer dust remainders PLUS a UNIT seed.
///
/// ⚠  ED SAFETY: The dust from 83-way division is only 74 planck — far below
///    ExistentialDeposit (1_000_000_000 planck).  We add UNIT (1 ALTAN) as a
///    constitutional seed so this account is always alive and ready to receive dust.
fn federal_sweep_account() -> AccountId {
    let mut seed = [b'_'; 32];
    let label = b"FEDERAL_SWEEP_ACCOUNT";
    seed[..label.len()].copy_from_slice(label);
    AccountId::from(seed)
}

// ─── Dev helpers ──────────────────────────────────────────────────────────────

/// Generate all nation/diplomatic genesis accounts via the sovereign_accounts module.
///
/// Replaces the legacy `dev_nation_treasuries()` placeholder that used `[i; 32]` seeds.
/// Now uses proper PalletId derivation for constitutional compliance.
fn all_sovereign_nation_accounts() -> alloc::vec::Vec<(AccountId, u128)> {
    use crate::sovereign_accounts::{
        all_indigenous_accounts, all_naturalized_accounts, diplomatic_account, DIPLOMATIC_SLOTS,
        UN_MEMBER_STATES,
    };
    let ed = EXISTENTIAL_DEPOSIT;
    let mut accounts = alloc::vec::Vec::new();

    // 79 коренных народов: treasury (UNIT для налогового роутинга) + council (ED)
    for (_id, treasury, council) in all_indigenous_accounts() {
        accounts.push((treasury, UNIT)); // UNIT: tax routing
        accounts.push((council, ed)); // ED: stays alive
    }

    // 88 натурализованных групп: treasury только (ED — минимальный)
    for (_id, treasury) in all_naturalized_accounts() {
        accounts.push((treasury, ed));
    }

    // 193 иностранных государства: 13 дипломатических слотов каждому (ED)
    for &(country_code, _, _) in UN_MEMBER_STATES {
        for slot in 0u8..DIPLOMATIC_SLOTS {
            accounts.push((diplomatic_account(country_code, slot), ed));
        }
    }

    accounts
}

/// Genesis bootstrap: only the Creator account (Citizen #1 / Bair).
///
/// ╔══════════════════════════════════════════════════════════════════════╗
/// ║  СОЗДАТЕЛЬ / FOUNDER                                                ║
/// ║                                                                      ║
/// ║  Баир Иванов Хонгирад  (Буряад-Монгол)                             ║
/// ║  Bair Ivanov Khongirad (Buryad-Mongol)                              ║
/// ║                                                                      ║
/// ║  Клан / Clan : Хонгирад (Khongirad)                                 ║
/// ║  Народ / Nation : Буряад-Монгол (Buryad-Mongol)                     ║
/// ║                                                                      ║
/// ║  Гражданин #1 · Citizen #1 · SUDO · Genesis Block · Altan L1        ║
/// ║  SS58: 5FTZYAh4tCCXKc8Pu7KYZrD9F3fGeu3YNYhk3gbrGC9n39Wv           ║
/// ╚══════════════════════════════════════════════════════════════════════╝
///
/// The Creator is the ONLY account that gets a pre-seeded balance in genesis.
/// Their balance of `CITIZEN_SEED = 100 * UNIT` covers:
///   - Transaction fees to call `bootstrap_verify_creator` (Root verifies them)
///   - Fees to call `verify_citizen` for the first wave of citizens
///
/// ALL other citizens are NOT pre-seeded.
/// They register via the normal on-chain citizen onboarding flow.
fn dev_creator_account() -> (AccountId, u128) {
    // Citizen #1 — Создатель / Founder: Bair Ivanov Khongirad (Buryad-Mongol)
    // CREATOR_SUDO — docs/ALPHA_KEYS.md §5 (gitignored, never commit mnemonic)
    let creator = AccountId::from_ss58check("5FTZYAh4tCCXKc8Pu7KYZrD9F3fGeu3YNYhk3gbrGC9n39Wv")
        .expect("CREATOR_SUDO SS58 address must be valid");
    (creator, CITIZEN_SEED)
}

// ─── Institutional Account Factories (Alpha Keys) ─────────────────────────────
//
// Real SS58 addresses generated for the Altan Alpha network.
// See docs/ALPHA_KEYS.md for mnemonic phrases and import instructions.
//
// These replace the dev keyring accounts (Charlie, Dave, Eve) used in earlier sprints.

/// INOMAD AG Treasury — receives 26% of every transaction fee.
///
/// The Creator (Citizen #1) is the permanent owner of INOMAD AG.
/// Fee share reduced from 36% → 26% per constitutional amendment (2026-04-16).
/// Key details: see docs/ALPHA_KEYS.md (gitignored)
fn inomad_ag_treasury_account() -> AccountId {
    AccountId::from_ss58check("5DrCgbEEpN1T1AjgJsaNz914T2q3p39RNGmRh9GqdVd4YdGJ")
        .expect("INOMAD_AG SS58 address must be valid")
}

/// KHURAL Foundation — receives 54% of every transaction fee + 90% genesis allocation.
///
/// Sovereign fund for citizen GDP, UBI distribution, science & technology.
/// Managed by the State Khural (86 Academy of Sciences representatives).
/// Key details: see docs/ALPHA_KEYS.md (gitignored)
fn khural_foundation_account() -> AccountId {
    AccountId::from_ss58check("5G11UBehntN5pPMoi7m7s6GTayb3T9iEAJFThAUmuF8V2fna")
        .expect("KHURAL_FOUNDATION SS58 address must be valid")
}

/// Confederation Government — receives 3% genesis allocation.
///
/// Confederate budget — cross-national infrastructure, common defense.
/// Released only by Confederate Khural vote. Frozen until first elections.
/// Key details: see docs/ALPHA_KEYS.md (gitignored)
#[allow(dead_code)] // Future use: direct 3% genesis allocation
fn confederation_gov_account() -> AccountId {
    AccountId::from_ss58check("5HjfW69nTUDzA26ZoACc2SQRJ8fp9KK55zLtDok8uNa7eTvu")
        .expect("CONFEDERATION_GOV SS58 address must be valid")
}

/// Republics Treasury — receives 7% genesis allocation.
///
/// Regional government budgets (83 Federal Subjects).
/// Each Republican Khural votes on allocation within their jurisdiction.
/// Key details: see docs/ALPHA_KEYS.md (gitignored)
#[allow(dead_code)] // Future use: direct 7% genesis allocation
fn republics_treasury_account() -> AccountId {
    AccountId::from_ss58check("5EeaVbc9AWVR5zq57XybpSdbCLD3M3wQHD4Smg2a7sBVFziP")
        .expect("REPUBLICS_TREASURY SS58 address must be valid")
}

/// CREATOR_SUDO — the sole sudo account at genesis.
///
/// The Creator (Citizen #1 / Bair Ivanov Khongirad) serves as Gamemaster: acting head of all
/// four branches of power until the first democratic elections.
/// SS58: 5FTZYAh4tCCXKc8Pu7KYZrD9F3fGeu3YNYhk3gbrGC9n39Wv (docs/ALPHA_KEYS.md §5)
fn creator_sudo_account() -> AccountId {
    AccountId::from_ss58check("5FTZYAh4tCCXKc8Pu7KYZrD9F3fGeu3YNYhk3gbrGC9n39Wv")
        .expect("CREATOR_SUDO SS58 address must be valid")
}

/// SIBERIAN_NATIONAL_FOUNDATION — Sovereign wealth fund.
///
/// Аналог Norwegian Government Pension Fund Global.
/// Собирает прибыль с продажи природных ресурсов Конфедерации.
/// Управляется 79 суверенными Хуралами (по 1 голосу на народ).
/// Решения принимаются квалифицированным большинством (2/3 = 53 из 79).
/// Key details: see docs/ALPHA_KEYS.md (gitignored)
fn siberian_national_foundation_account() -> AccountId {
    AccountId::from_ss58check("5G1z35pmQ32megbYcHxFeRxdBr41cKf6h8MzgGPkghki4LFz")
        .expect("SIBERIAN_NATIONAL_FOUNDATION SS58 address must be valid")
}

// ─── Core Genesis Builder ─────────────────────────────────────────────────────

/// Build the canonical genesis config reflecting the post-routing state.
///
/// # Canonical Flow: Path of Money (Creator Lore)
///
/// ```text
/// System Account (CENTRAL_BANK)  →  Bank_of_Siberia_Main_Reserve
///                                        ├─ Confederation_Treasury    (3%)
///                                        ├─ 83 × Regional_Gov_Treasury (7%)
///                                        └─ 83 × Regional_Citizen_Fund (90%)
/// ```
///
/// # Constitutional Budget Structure (post-genesis state)
///
/// | Account                         | Canonical Name             | Pool    | Count | Each (planck)                      |
/// |---------------------------------|----------------------------|---------|-------|------------------------------------|
/// | CONFEDERATION_BUDGET            | Confederation_Treasury     | 3%      |   1   | total_confederation                |
/// | REGIONAL_SPECIAL_{1..83}        | Regional_Gov_Treasury_{i}  | 7%      |  83   | total_regional / 83                |
/// | BANK_OF_SIBERIA_RES_{1..83}     | Regional_Citizen_Fund_{i}  | 90%     |  83   | total_reserve / 83                 |
/// | FEDERAL_SWEEP_ACCOUNT           | Federal Sweep              | dust+ED |   1   | UNIT + (reserve%83 + regional%83)  |
/// | CENTRAL_BANK                    | System Account             | 0       |   1   | EXISTENTIAL_DEPOSIT (alive)        |
/// | BANK_OF_SIBERIA_MASTER          | Bank_of_Siberia_Main_Reserve | 0     |   1   | EXISTENTIAL_DEPOSIT (alive)        |
/// | dev pioneers + creator          | SBT Citizens               | seed    |   6   | 100 * UNIT each (SBT Level-0 rule) |
///
/// # ED Safety
///
/// All accounts are guaranteed to have balance > EXISTENTIAL_DEPOSIT (0.001 ALTAN).
/// The `federal_sweep_account` receives `UNIT + dust` (not just dust which is 74 planck).
fn testnet_genesis(initial_authorities: Vec<(AuraId, GrandpaId)>, root: AccountId) -> Value {
    // ── Constitutional Math ────────────────────────────────────────────────────
    //
    //   total_reserve           = 1_890_000_000_000 * UNIT
    //   total_regional_frozen   =   147_000_000_000 * UNIT
    //   total_confederation     =    63_000_000_000 * UNIT
    //
    //   reserve_per_region      = total_reserve / 83
    //   regional_frozen_per_reg = total_regional_frozen / 83
    //   dust                    = (total_reserve % 83) + (total_regional_frozen % 83)
    //
    //   ⚠ CRITICAL: dust = 74 planck < ExistentialDeposit(1e9 planck).
    //   FIX: FEDERAL_SWEEP_ACCOUNT receives UNIT + dust to ensure it stays alive.
    //
    let total_reserve: u128 = TOTAL_RESERVE_PLANCK;
    let total_regional: u128 = TOTAL_REGIONAL_FROZEN_PLANCK;
    let _total_confederation: u128 = TOTAL_CONFEDERATION_FROZEN_PLANCK;

    let _reserve_per_region: u128 = total_reserve / REGIONS_COUNT;
    let _regional_per_region: u128 = total_regional / REGIONS_COUNT;

    // Raw dust from integer division (only 74 planck — below ED!)
    let _raw_dust: u128 = (total_reserve % REGIONS_COUNT) + (total_regional % REGIONS_COUNT);

    // ── System Accounts ────────────────────────────────────────────────────────
    // Canonical names per Creator Lore (L1-23):
    let central_bank = central_bank_account(); // System Account — Hard Cap issuer
    let bank_of_siberia_main_reserve = bank_of_siberia_main_reserve_account(); // Bank_of_Siberia_Main_Reserve
    let confederation_treasury = confederation_treasury_account(); // Confederation_Treasury (3%)
    let federal_sweep = federal_sweep_account();

    // Tax-routing accounts (used by pallet-altan-tax — hybrid fee split)
    // Real institutional keys from docs/ALPHA_KEYS.md
    let ag_treasury: AccountId = inomad_ag_treasury_account();
    let khural_foundation: AccountId = khural_foundation_account();
    // Validator pool: receives 10% of transaction fees via InomadFeeSplitter
    // Uses deterministic system account seed matching configs/mod.rs::InomadFeeSplitter
    let validator_pool: AccountId = {
        let mut seed = [b'_'; 32];
        let label = b"VALIDATOR_POOL_ACCT";
        seed[..label.len()].copy_from_slice(label);
        AccountId::from(seed)
    };

    // All sovereign nation + diplomatic genesis accounts (PalletId-derived)
    let sovereign_accounts = all_sovereign_nation_accounts();

    // Единственный bootstrap-аккаунт в genesis — Создатель (Citizen #1).
    // Все остальные граждане присоединяются через verify_citizen на-чейне.
    let (creator_account, _creator_balance) = dev_creator_account();

    // ── Build Balances Array ───────────────────────────────────────────────────
    let mut balances: Vec<(AccountId, u128)> = Vec::new();

    // [1] Confederation_Treasury — EXISTENTIAL_DEPOSIT
    //     Funds will be routed later by the Creator signing the genesis distribution.
    balances.push((confederation_treasury.clone(), EXISTENTIAL_DEPOSIT));

    // [2] Federal Sweep — UNIT seed. Keeps it alive for later dust sweeps.
    balances.push((federal_sweep.clone(), UNIT));

    // [4] System identities — existential deposit only (all funds routed out).
    //     These accounts must stay alive in the system account registry.
    //
    // ⚠️  CONSTITUTIONAL CHANGE (Genesis L1-CB-Fix):
    //     CENTRAL_BANK now receives the ENTIRE M0 supply (2.1T ALTAN).
    //     It is a KEYLESS 7-of-10 multisig — no private key exists.
    //     Only pallet-central-bank extrinsics (BankingOrigin = Sudo/Creator) can move funds.
    //     This satisfies the constitutional mandate: "ЦБ — юридическое тело эмиссии".
    //
    //     Bank_of_Siberia_Main_Reserve stays at ED — it receives funds via
    //     trigger_genesis_distribution() in the first constitutional extrinsic.
    balances.push((central_bank.clone(), TOTAL_SUPPLY_PLANCK));     // ← M0: 2.1T ALTAN (ЦБ)
    balances.push((bank_of_siberia_main_reserve.clone(), EXISTENTIAL_DEPOSIT)); // Bank_of_Siberia_Main_Reserve

    // [5] Tax-routing system accounts — existential deposit
    for sys_acct in [
        ag_treasury.clone(),
        khural_foundation.clone(),
        validator_pool.clone(),
    ] {
        balances.push((sys_acct, EXISTENTIAL_DEPOSIT));
    }

    // [5b] Siberian National Foundation — sovereign wealth fund (79 Khurals).
    //      Receives revenue from resource sales. Seeded with UNIT to stay alive.
    balances.push((siberian_national_foundation_account(), UNIT));

    // [6] SOVEREIGN ACCOUNTS — 79 Indigenous Nations + 88 Naturalized + 193 Foreign States
    //
    // Each sovereign entity receives a PalletId-derived deterministic keyless account.
    // Constitutional rules:
    //   - Indigenous (1–79): treasury (1 ALTAN) + council (ED) — FULL political sovereignty
    //   - Naturalized (1001–1088): treasury (ED) — economic rights ONLY, no Khural
    //   - Foreign states (193 × 13 slots): diplomatic accounts (ED) — Sanctioned states blocked
    //
    // Total sovereign accounts added to genesis:
    //   79×2 + 88 + 193×13 = 158 + 88 + 2509 = 2755 accounts
    for (acct, balance) in &sovereign_accounts {
        balances.push((acct.clone(), *balance));
    }

    // [7] Technical system key: Alice (Aura / Grandpa block validator).
    //
    // ⚠️  MAINNET LAUNCH SECURITY CHECK:
    //
    //  1. DEV / TESTNET (current): Alice is kept here for local block production.
    //     This is intentional and safe for non-production environments.
    //
    //  2. PRODUCTION — REMOVE ALICE:
    //     Alice's mnemonic is publicly known ("Alice"). NEVER use her on mainnet.
    //     Generate a dedicated headless Session Key via subkey:
    //       $ subkey generate --scheme sr25519   # → Aura session key
    //       $ subkey generate --scheme ed25519   # → Grandpa session key
    //     Store in the node's encrypted keystore (--keystore-path), NOT in source code.
    //
    //  3. CREATOR KEY SEPARATION:
    //     DO NOT use the Creator's SubWallet mnemonic (5FBvV2KC...) for Aura/Grandpa.
    //     The Creator's key is for Sudo / Account operations ONLY and must NEVER
    //     reside in the node's keystore. Mixing keys violates OpSec and risks
    //     private key exposure if the node is compromised.
    //
    // Alice is NOT a citizen of the Republic. No constitutional rights. No SBT.
    // She receives a minimal operational seed only (to pay for extrinsic fees).
    balances.push((
        AccountId::from_ss58check("5GrwvaEF5zXb26Fz9rcQpDWS57CtERHpNehXCPcNoHGKutQY")
            .expect("Alice SS58 must be valid"),
        1_000 * UNIT, // operational only; Alice is a technical validator key, NOT a citizen
    ));

    // [8] Creator (Citizen #1 / Bair Khongirad) — receives ONLY the SBT Level-0 seed.
    //
    // CONSTITUTIONAL MANDATE: The Creator is NOT a bank, NOT a treasury.
    // He is Citizen #1 — equal to all other citizens in his personal balance.
    //
    // His authority is POSITIONAL (Sudo key = BankingOrigin temporarily):
    //   - He signs trigger_genesis_distribution() on behalf of the CB Board
    //   - Until the CB Board of Directors is elected, he acts as the interim authority
    //   - After elections, Sudo is surrendered and BankingOrigin → Banking Collective
    //
    // CITIZEN_SEED = 100 ALTAN covers transaction fees for the first governance acts.
    balances.push((creator_account, CITIZEN_SEED)); // 100 ALTAN — same as every citizen

    build_struct_json_patch!(RuntimeGenesisConfig {
        balances: BalancesConfig { balances },
        aura: pallet_aura::GenesisConfig {
            authorities: initial_authorities
                .iter()
                .map(|x| x.0.clone())
                .collect::<Vec<_>>(),
        },
        grandpa: pallet_grandpa::GenesisConfig {
            authorities: initial_authorities
                .iter()
                .map(|x| (x.1.clone(), 1))
                .collect::<Vec<_>>(),
        },
        sudo: SudoConfig { key: Some(root) },
        altan_tax: AltanTaxConfig {
            ag_treasury: Some(ag_treasury),
            khural_foundation: Some(khural_foundation),
            validator_pool: Some(validator_pool),
            // 79 коренных народов — конституционный налоговый роутинг.
            // Используем PalletId-derived treasury accounts из sovereign_accounts.
            nation_treasuries: {
                use crate::sovereign_accounts::indigenous_treasury;
                (1u16..=79u16).map(|id| indigenous_treasury(id)).collect()
            },
        },
        // ── Central Bank: Genesis — Licensed Operators ────────────────────────
        //
        // Bank_of_Siberia_Main_Reserve is the FIRST and ONLY licensed operator
        // at genesis. The Central Bank's `mint_to_operator` extrinsic can only
        // send new emission to this account (or others added later by BankingOrigin).
        //
        // This wiring implements the Правило Независимости: the Central Bank
        // knows WHO can receive its emission before the first block is produced.
        central_bank: CentralBankConfig {
            licensed_operators: vec![(bank_of_siberia_main_reserve, true)],
        },
    })
}

// ─── Preset Entrypoints ──────────────────────────────────────────────────────

/// Return the development genesis config (single Alice validator).
///
/// ⚠️  SECURITY: Alice is used for dev convenience ONLY.
/// For production: generate a headless session key via `subkey` and load it
/// from the node's keystore. Do NOT use the Creator's mnemonic for Aura/Grandpa.
/// See MAINNET LAUNCH SECURITY CHECKLIST in `node/src/chain_spec.rs`.
///
/// NOTE: SUDO is CREATOR_SUDO (5FBvV2KC...), NOT Alice.
pub fn development_config_genesis() -> Value {
    testnet_genesis(
        vec![(
            // Alice Aura key (sr25519 //Alice)
            AuraId::from(Sr25519Keyring::Alice.public()),
            // Alice Grandpa key (ed25519 //Alice)
            GrandpaId::from(Ed25519Keyring::Alice.public()),
        )],
        creator_sudo_account(),
    )
}

/// Return the local testnet genesis config (Alice + Bob validators).
///
/// ⚠️  SECURITY: Alice + Bob are well-known public keys. Local testing ONLY.
/// For production: replace with real session keys from the node's keystore.
/// See MAINNET LAUNCH SECURITY CHECKLIST in `node/src/chain_spec.rs`.
pub fn local_config_genesis() -> Value {
    testnet_genesis(
        vec![
            (
                // Alice Aura key (sr25519 //Alice)
                AuraId::from(Sr25519Keyring::Alice.public()),
                // Alice Grandpa key (ed25519 //Alice)
                GrandpaId::from(Ed25519Keyring::Alice.public()),
            ),
            (
                // Bob Aura key (sr25519 //Bob)
                AuraId::from(Sr25519Keyring::Bob.public()),
                // Bob Grandpa key (ed25519 //Bob)
                GrandpaId::from(Ed25519Keyring::Bob.public()),
            ),
        ],
        creator_sudo_account(),
    )
}

/// Provides the JSON representation of predefined genesis config for given `id`.
pub fn get_preset(id: &PresetId) -> Option<Vec<u8>> {
    let patch = match id.as_ref() {
        sp_genesis_builder::DEV_RUNTIME_PRESET => development_config_genesis(),
        sp_genesis_builder::LOCAL_TESTNET_RUNTIME_PRESET => local_config_genesis(),
        _ => return None,
    };
    Some(
        serde_json::to_string(&patch)
            .expect("serialization to json is expected to work. qed.")
            .into_bytes(),
    )
}

/// List of supported presets.
pub fn preset_names() -> Vec<PresetId> {
    vec![
        PresetId::from(sp_genesis_builder::DEV_RUNTIME_PRESET),
        PresetId::from(sp_genesis_builder::LOCAL_TESTNET_RUNTIME_PRESET),
    ]
}
