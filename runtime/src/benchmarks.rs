// SPDX-License-Identifier: Unlicense
// Altan Network — Runtime Benchmark Registration
//
// All 28 sovereign pallets are registered here so that the node binary can
// generate final `WeightInfo` constants via:
//
//   ./target/release/altan-network benchmark pallet \
//     --chain=dev \
//     --pallet="pallet_*" \
//     --extrinsic="*" \
//     --steps=50 \
//     --repeat=20 \
//     --output=runtime/src/weights/

frame_benchmarking::define_benchmarks!(
    // ── Substrate / Frame built-ins ──────────────────────────────────────────
    [frame_benchmarking, BaselineBench::<Runtime>]
    [frame_system, SystemBench::<Runtime>]
    [frame_system_extensions, SystemExtensionsBench::<Runtime>]
    [pallet_balances, Balances]
    [pallet_timestamp, Timestamp]
    [pallet_sudo, Sudo]
    [pallet_template, Template]

    // ── Altan Network — Sovereign Pallets ────────────────────────────────────
    // Index 8 — Tax collection and fee routing
    [pallet_altan_tax, AltanTax]
    // Index 9 — Soulbound citizen identity & Arbad (Digital DNA)
    [pallet_inomad_identity, InomadIdentity]
    // Index 10 — Khural democratic governance
    [pallet_khural_governance, KhuralGovernance]
    // Index 11 — Steppe Protocol — offline mesh payment vault & IOU settlement
    [pallet_steppe_offline, SteppeOffline]
    // Index 12 — Sovereign Judicial Courts
    [pallet_judicial_courts, JudicialCourts]
    // Index 13 — Banking Branch — collateral, CDP, fractional reserve credit
    [pallet_bank_operator, BankOperator]
    // Index 14 — Chronicles — decentralised IP registry
    [pallet_chronicles, Chronicles]
    // Index 15 — Guilds — Professional DAO Protocol
    [pallet_guilds, Guilds]
    // Index 16 — Inheritance — Heritage Institute & Digital Notary
    [pallet_inheritance, Inheritance]
    // Index 17 — Citizen Voice Protocol (Голос Гражданина)
    [pallet_citizen_voice, CitizenVoice]
    // Index 18 — Black Book — Constitutional Bounty Hunting
    [pallet_black_book, BlackBook]
    // Index 19 — Chancery — Universal Digital Chancellery
    [pallet_chancery, Chancery]
    // Index 20 — Central Bank — sole constitutional issuer of ALTAN
    [pallet_central_bank, CentralBank]
    // Index 21 — Bank of Siberia — credit operator, financial oracle, escrow
    [pallet_bank_of_siberia, BankOfSiberia]
    // Index 22 — Constitution — Layer 0 Bill of Rights and Habeas Corpus
    [pallet_constitution, Constitution]
    // Index 23 — Land Registry — sovereign land cadastre
    [pallet_land_registry, LandRegistry]
    // Index 24 — Organization Registry — Corporate SBTs
    [pallet_organization, Organization]
    // Index 25 — Shielded Vaults — Asymmetric Transparency
    [pallet_shielded_vaults, ShieldedVaults]
    // Index 26 — МИД — Sovereign Diplomatic Bridge
    [pallet_foreign_affairs, ForeignAffairs]
    // Index 27 — Decimal DAO Core — Generic Council Governance
    [pallet_decimal_dao, DecimalDao]
    // Index 28 — Licensing — Constitutional Executive/Khural Pipeline
    [pallet_licensing, Licensing]
    // Index 29 — Elections — Bottom-Up Hierarchical Meritocracy
    [pallet_inomad_elections, InomadElections]
    // Index 30 — Buryad-Mongol Mixer — privacy-preserving shielded pool
    [pallet_buryad_mongol_mixer, BuryadMongolMixer]
    // Index 31 — Altan Vault — isolated savings sub-accounts
    [pallet_altan_vault, AltanVault]
    // Index 32 — Access NFT — Universal NFT Access Keys
    [pallet_access_nft, AccessNft]
    // Index 33 — Achievement NFT — Reputation & Reward NFT System
    [pallet_achievement_nft, AchievementNft]
    // Index 34 — Recovery NFT — Arbad Social Account Recovery
    [pallet_recovery_nft, RecoveryNft]
    // Index 35 — Forums — Threaded Hierarchical Communication
    [pallet_forums, Forums]
    // Index 36 — Annual Profit Tax — Constitutional income tax (10%/5% large-family)
    [pallet_annual_profit_tax, AnnualProfitTax]
);
