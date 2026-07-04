#![cfg_attr(not(feature = "std"), no_std)]
// The `#[frame_support::runtime]` macro expands to deeply nested types.
// With 24+ pallets, the default limit of 128 is exceeded; 256 is sufficient.
#![recursion_limit = "256"]

#[cfg(feature = "std")]
include!(concat!(env!("OUT_DIR"), "/wasm_binary.rs"));

pub mod apis;
#[cfg(feature = "runtime-benchmarks")]
mod benchmarks;
pub mod configs;
/// Constitutional origin types: production-ready guards backed by on-chain
/// `pallet-inomad-elections` branch council and supreme leader state.
///
/// Replaces `EnsureRoot` and `EnsureSigned` dev stubs used during Sprint 1–8.
/// See `origins.rs` for full documentation and bootstrap strategy.
pub mod origins;

extern crate alloc;
use alloc::vec::Vec;
use sp_runtime::{
    generic, impl_opaque_keys,
    traits::{BlakeTwo256, IdentifyAccount, Verify},
    MultiAddress, MultiSignature,
};
#[cfg(feature = "std")]
use sp_version::NativeVersion;
use sp_version::RuntimeVersion;

pub use frame_system::Call as SystemCall;
pub use pallet_balances::Call as BalancesCall;
pub use pallet_timestamp::Call as TimestampCall;
#[cfg(any(feature = "std", test))]
pub use sp_runtime::BuildStorage;

pub mod genesis_config_presets;
/// Deterministic PalletId-derived accounts for all sovereign entities:
/// - 79 Indigenous Nations (Коренные Народы) — treasury + council
/// - 88 Naturalized Groups — treasury
/// - 193 UN Foreign States — 13 diplomatic slots each
pub mod sovereign_accounts;

/// Opaque types. These are used by the CLI to instantiate machinery that don't need to know
/// the specifics of the runtime. They can then be made to be agnostic over specific formats
/// of data like extrinsics, allowing for them to continue syncing the network through upgrades
/// to even the core data structures.
pub mod opaque {
    use super::*;
    use sp_runtime::{
        generic,
        traits::{BlakeTwo256, Hash as HashT},
    };

    pub use sp_runtime::OpaqueExtrinsic as UncheckedExtrinsic;

    /// Opaque block header type.
    pub type Header = generic::Header<BlockNumber, BlakeTwo256>;
    /// Opaque block type.
    pub type Block = generic::Block<Header, UncheckedExtrinsic>;
    /// Opaque block identifier type.
    pub type BlockId = generic::BlockId<Block>;
    /// Opaque block hash type.
    pub type Hash = <BlakeTwo256 as HashT>::Output;
}

impl_opaque_keys! {
    pub struct SessionKeys {
        pub aura: Aura,
        pub grandpa: Grandpa,
    }
}

// To learn more about runtime versioning, see:
// https://docs.substrate.io/main-docs/build/upgrade#runtime-versioning
#[sp_version::runtime_version]
pub const VERSION: RuntimeVersion = RuntimeVersion {
    spec_name: alloc::borrow::Cow::Borrowed("altan-network-runtime"),
    impl_name: alloc::borrow::Cow::Borrowed("altan-network-runtime"),
    authoring_version: 1,
    // The version of the runtime specification. A full node will not attempt to use its native
    //   runtime in substitute for the on-chain Wasm runtime unless all of `spec_name`,
    //   `spec_version`, and `authoring_version` are the same between Wasm and native.
    // This value is set to 100 to notify Polkadot-JS App (https://polkadot.js.org/apps) to use
    //   the compatible custom types.
    spec_version: 106,
    impl_version: 1,
    apis: apis::RUNTIME_API_VERSIONS,
    transaction_version: 1,
    system_version: 1,
};

mod block_times {
    /// This determines the average expected block time that we are targeting. Blocks will be
    /// produced at a minimum duration defined by `SLOT_DURATION`. `SLOT_DURATION` is picked up by
    /// `pallet_timestamp` which is in turn picked up by `pallet_aura` to implement `fn
    /// slot_duration()`.
    ///
    /// Change this to adjust the block time.
    pub const MILLI_SECS_PER_BLOCK: u64 = 6000;

    // NOTE: Currently it is not possible to change the slot duration after the chain has started.
    // Attempting to do so will brick block production.
    pub const SLOT_DURATION: u64 = MILLI_SECS_PER_BLOCK;
}
pub use block_times::*;

// Time is measured by number of blocks.
pub const MINUTES: BlockNumber = 60_000 / (MILLI_SECS_PER_BLOCK as BlockNumber);
pub const HOURS: BlockNumber = MINUTES * 60;
pub const DAYS: BlockNumber = HOURS * 24;

pub const BLOCK_HASH_COUNT: BlockNumber = 2400;

// ALTAN token denomination constants (12 decimal places: 1 ALTAN = 10^12 planck)
pub const ALTAN: Balance = 1_000_000_000_000; // 1 ALTAN
pub const MILLI_ALTAN: Balance = 1_000_000_000; // 0.001 ALTAN
pub const MICRO_ALTAN: Balance = 1_000_000; // 0.000001 ALTAN

// Legacy aliases for pallets that reference UNIT internally
pub const UNIT: Balance = ALTAN;
pub const MILLI_UNIT: Balance = MILLI_ALTAN;
pub const MICRO_UNIT: Balance = MICRO_ALTAN;

/// Existential deposit: minimum balance to keep an account alive (0.001 ALTAN).
pub const EXISTENTIAL_DEPOSIT: Balance = MILLI_ALTAN;

/// The version information used to identify this runtime when compiled natively.
#[cfg(feature = "std")]
pub fn native_version() -> NativeVersion {
    NativeVersion {
        runtime_version: VERSION,
        can_author_with: Default::default(),
    }
}

/// Alias to 512-bit hash when used in the context of a transaction signature on the chain.
pub type Signature = MultiSignature;

/// Some way of identifying an account on the chain. We intentionally make it equivalent
/// to the public key of our transaction signing scheme.
pub type AccountId = <<Signature as Verify>::Signer as IdentifyAccount>::AccountId;

/// Balance of an account.
pub type Balance = u128;

/// Index of a transaction in the chain.
pub type Nonce = u32;

/// A hash of some data used by the chain.
pub type Hash = sp_core::H256;

/// An index to a block.
pub type BlockNumber = u32;

/// The address format for describing accounts.
pub type Address = MultiAddress<AccountId, ()>;

/// Block header type as expected by this runtime.
pub type Header = generic::Header<BlockNumber, BlakeTwo256>;

/// Block type as expected by this runtime.
pub type Block = generic::Block<Header, UncheckedExtrinsic>;

/// A Block signed with a Justification
pub type SignedBlock = generic::SignedBlock<Block>;

/// BlockId type as expected by this runtime.
pub type BlockId = generic::BlockId<Block>;

/// The `TransactionExtension` to the basic transaction logic.
pub type TxExtension = (
    frame_system::AuthorizeCall<Runtime>,
    frame_system::CheckNonZeroSender<Runtime>,
    frame_system::CheckSpecVersion<Runtime>,
    frame_system::CheckTxVersion<Runtime>,
    frame_system::CheckGenesis<Runtime>,
    frame_system::CheckEra<Runtime>,
    frame_system::CheckNonce<Runtime>,
    frame_system::CheckWeight<Runtime>,
    pallet_transaction_payment::ChargeTransactionPayment<Runtime>,
    frame_metadata_hash_extension::CheckMetadataHash<Runtime>,
    frame_system::WeightReclaim<Runtime>,
);

/// Unchecked extrinsic type as expected by this runtime.
pub type UncheckedExtrinsic =
    generic::UncheckedExtrinsic<Address, RuntimeCall, Signature, TxExtension>;

/// The payload being signed in transactions.
pub type SignedPayload = generic::SignedPayload<RuntimeCall, TxExtension>;

/// Ordered list of runtime migrations.
///
/// Each entry is a pallet's [`OnRuntimeUpgrade`] implementation.  On a runtime
/// upgrade, FRAME executes these in order, calling `pre_upgrade`, then
/// `on_runtime_upgrade`, then `post_upgrade` (when compiled with
/// `--features try-runtime`).
///
/// **How to add a real migration:**
/// 1. Implement `vN::Migration<Runtime>` in `pallet-X/src/migrations.rs`.
/// 2. Replace `pallet_x::migrations::NoopMigration<Runtime>` with
///    `pallet_x::migrations::vN::Migration<Runtime>` below.
/// 3. Bump `spec_version` in `VERSION`.
pub type Migrations = (
    // ── Sovereign identity layer ──────────────────────────────────────────
    pallet_inomad_identity::migrations::NoopMigration<Runtime>,
    pallet_inomad_elections::migrations::NoopMigration<Runtime>,
    // ── Banking & finance ─────────────────────────────────────────────────
    pallet_central_bank::migrations::NoopMigration<Runtime>,
    pallet_bank_of_siberia::migrations::NoopMigration<Runtime>,
    pallet_bank_operator::migrations::NoopMigration<Runtime>,
    pallet_altan_vault::migrations::NoopMigration<Runtime>,
    pallet_shielded_vaults::migrations::NoopMigration<Runtime>,
    pallet_buryad_mongol_mixer::migrations::NoopMigration<Runtime>,
    pallet_steppe_offline::migrations::NoopMigration<Runtime>,
    // ── Governance & law ──────────────────────────────────────────────────
    pallet_constitution::migrations::NoopMigration<Runtime>,
    pallet_decimal_dao::migrations::NoopMigration<Runtime>,
    pallet_khural_governance::migrations::NoopMigration<Runtime>,
    pallet_licensing::migrations::NoopMigration<Runtime>,
    pallet_judicial_courts::migrations::NoopMigration<Runtime>,
    pallet_black_book::migrations::NoopMigration<Runtime>,
    pallet_citizen_voice::migrations::NoopMigration<Runtime>,
    pallet_chancery::migrations::NoopMigration<Runtime>,
    // ── Economy & organizations ───────────────────────────────────────────
    pallet_altan_tax::migrations::NoopMigration<Runtime>,
    pallet_organization::migrations::NoopMigration<Runtime>,
    pallet_guilds::migrations::NoopMigration<Runtime>,
    pallet_inheritance::migrations::NoopMigration<Runtime>,
    pallet_land_registry::migrations::NoopMigration<Runtime>,
    pallet_chronicles::migrations::NoopMigration<Runtime>,
    pallet_foreign_affairs::migrations::NoopMigration<Runtime>,
    // ── NFTs & credentials ────────────────────────────────────────────────
    pallet_access_nft::migrations::NoopMigration<Runtime>,
    pallet_achievement_nft::migrations::NoopMigration<Runtime>,
    pallet_recovery_nft::migrations::NoopMigration<Runtime>,
    // ── Communication ─────────────────────────────────────────────────────
    pallet_forums::migrations::NoopMigration<Runtime>,
);

/// Executive: handles dispatch to the various modules.
pub type Executive = frame_executive::Executive<
    Runtime,
    Block,
    frame_system::ChainContext<Runtime>,
    Runtime,
    AllPalletsWithSystem,
    Migrations,
>;

// Create the runtime by composing the FRAME pallets that were previously configured.
#[frame_support::runtime]
mod runtime {
    #[runtime::runtime]
    #[runtime::derive(
        RuntimeCall,
        RuntimeEvent,
        RuntimeError,
        RuntimeOrigin,
        RuntimeFreezeReason,
        RuntimeHoldReason,
        RuntimeSlashReason,
        RuntimeLockId,
        RuntimeTask,
        RuntimeViewFunction
    )]
    pub struct Runtime;

    #[runtime::pallet_index(0)]
    pub type System = frame_system;

    #[runtime::pallet_index(1)]
    pub type Timestamp = pallet_timestamp;

    #[runtime::pallet_index(2)]
    pub type Aura = pallet_aura;

    #[runtime::pallet_index(3)]
    pub type Grandpa = pallet_grandpa;

    #[runtime::pallet_index(4)]
    pub type Balances = pallet_balances;

    #[runtime::pallet_index(5)]
    pub type TransactionPayment = pallet_transaction_payment;

    #[runtime::pallet_index(6)]
    pub type Sudo = pallet_sudo;

    // Include the custom logic from the pallet-template in the runtime.
    #[runtime::pallet_index(7)]
    pub type Template = pallet_template;

    // Altan Network: Tax collection and fee routing pallet.
    #[runtime::pallet_index(8)]
    pub type AltanTax = pallet_altan_tax;

    // Altan Network: Soulbound citizen identity & Arbad (Digital DNA) pallet.
    #[runtime::pallet_index(9)]
    pub type InomadIdentity = pallet_inomad_identity;

    // Altan Network: Khural democratic governance — proposal voting & nation treasury execution.
    #[runtime::pallet_index(10)]
    pub type KhuralGovernance = pallet_khural_governance;

    // Altan Network: Steppe Protocol — offline mesh payment vault & ARMAGEDDON IOU settlement.
    #[runtime::pallet_index(11)]
    pub type SteppeOffline = pallet_steppe_offline;

    // Altan Network: Sovereign Judicial Courts — nation-scoped due process & verdicts.
    #[runtime::pallet_index(12)]
    pub type JudicialCourts = pallet_judicial_courts;

    // Altan Network: Banking Branch — collateral management, CDP, fractional reserve credit.
    #[runtime::pallet_index(13)]
    pub type BankOperator = pallet_bank_operator;

    // Altan Network: Chronicles (Летописи Государства) — decentralised IP registry.
    // Anchors Science, Media, History, and Law on-chain (H256 hash + IPFS CID).
    // Includes citizen-to-author ALTAN tipping economy.
    #[runtime::pallet_index(14)]
    pub type Chronicles = pallet_chronicles;

    // Altan Network: Guilds — Professional DAO Protocol.
    // Decentralised replacement for LinkedIn + Upwork + Academy.
    // Escrow quests, meritocracy achievements, and on-chain course subscriptions.
    #[runtime::pallet_index(15)]
    pub type Guilds = pallet_guilds;

    // Altan Network: Inheritance — Heritage Institute & Digital Notary.
    // Resolves frozen funds from register_death via notarized Wills.
    // Creates professional employment for Notary Guild members.
    #[runtime::pallet_index(16)]
    pub type Inheritance = pallet_inheritance;

    // Altan Network: Citizen Voice Protocol (Голос Гражданина).
    // Decentralised complaints, suggestions, and whistleblowing.
    // Anti-spam deposit (1 ALTAN) reserved on submission; returned or burned on resolution.
    // Guild Masters, Entity owners, and Government (Root) are the authorised resolvers.
    #[runtime::pallet_index(17)]
    pub type CitizenVoice = pallet_citizen_voice;

    // Altan Network: Black Book (Чёрная Книга) — Constitutional Bounty Hunting.
    // Root-only warrants exile criminals, confiscate their funds, and open
    // crowdfunded BountyPools for fugitive capture. Hunters paid on capture proof.
    #[runtime::pallet_index(18)]
    pub type BlackBook = pallet_black_book;

    // Altan Network: Chancery (Универсальная Цифровая Канцелярия) — Universal Digital Chancellery.
    // Multi-party Ricardian contracts anchored on-chain via Blake2_256 document hashes.
    // Professional validators (Lawyers, Notaries, Mediators) gated via pallet-guilds.
    #[runtime::pallet_index(19)]
    pub type Chancery = pallet_chancery;

    // Altan Network: Central Bank — sole constitutional issuer of ALTAN (GDP-tranche emission).
    // Managed by Confederate Khural; direct citizen operations strictly forbidden.
    #[runtime::pallet_index(20)]
    pub type CentralBank = pallet_central_bank;

    // Altan Network: Bank of Siberia — credit operator, financial oracle, escrow gateway.
    // Uses funds allocated by the Central Bank; never calls deposit_creating directly.
    #[runtime::pallet_index(21)]
    pub type BankOfSiberia = pallet_bank_of_siberia;

    // Altan Network: Constitution -- Layer 0 Bill of Rights and Habeas Corpus.
    // Stores the immutable IPFS CID of the constitutional document.
    // Habeas Corpus: auto-releases frozen citizens when max_lockup_block expires.
    #[runtime::pallet_index(22)]
    pub type Constitution = pallet_constitution;

    // Altan Network: Land Registry -- sovereign land cadastre.
    // Non-Alienation Law: foreign ownership of Altan land is constitutionally forbidden.
    #[runtime::pallet_index(23)]
    pub type LandRegistry = pallet_land_registry;

    // Altan Network: Organization Registry (Организация) — Corporate SBTs.
    // Manual tax filing by Directors, automatic digital punishment for evasion.
    // Penalty Engine: OrgStatus::Delinquent + debt accrual + Director CitizenFreeze.
    #[runtime::pallet_index(24)]
    pub type Organization = pallet_organization;

    // Altan Network: Shielded Vaults — Принцип Асимметричной Прозрачности.
    // Commitment-based ZK shielded pool for guilds and citizens.
    // Constitutional Guard: state/government accounts are mathematically blocked from shielding.
    //
    // CONSTITUTIONAL MANDATE:
    //   - State accounts (Central Bank, Bank of Siberia, Confederation Treasury,
    //     Regional Treasuries) → 100% PUBLIC (cannot call shield_funds)
    //   - Citizens & Guilds → right to commercial privacy via commitments
    //
    // Enables: org salary payments (shielded_transfer) and ZK tax bridge
    // (unshield_for_tax → materialises tax amount on RegionalTreasury).
    #[runtime::pallet_index(25)]
    pub type ShieldedVaults = pallet_shielded_vaults;

    // ── МИД — Sovereign Diplomatic Bridge ────────────────────────────────
    //
    // On-chain registry of 193 UN-recognized sovereign states.
    // Each state gets: MFA, Embassy, Passport Office, 10 banking wallets,
    // plus closed diplomatic and document legalization channels.
    #[runtime::pallet_index(26)]
    pub type ForeignAffairs = pallet_foreign_affairs;

    // Altan Network: Decimal DAO Core — Generic Council Governance & Keyless Treasuries.
    //
    // Powers all 4 branches of the INOMAD State:
    //   Legislative (Khural), Executive (Ministries), Judicial (Courts), Audit.
    // Also serves Enterprises, Guilds, and any arbitrary org identified by Blake2-256 UUID.
    //
    // Architecture:
    //   - Each org is identified by Blake2-256(off-chain UUID) → OrgId: [u8; 32]
    //   - Councils replaced annually by Relayer after Arbad→Tumed elections
    //   - Keyless treasury = PalletId(*b"inm/ddao").into_sub_account_truncating(org_id)
    //   - Strict majority (>50%) required to pass and execute treasury proposals
    #[runtime::pallet_index(27)]
    pub type DecimalDao = pallet_decimal_dao;

    // Altan Network: Licensing — Constitutional Executive/Khural Licensing Pipeline.
    //
    // Enforces the constitutional branch separation for licensing:
    //   1. Authorized Ministries (Executive, Root-whitelisted) submit applications.
    //   2. Khural Delegates (National) vote within 7-day window.
    //   3. `on_initialize` auto-enacts when voting closes.
    //   4. Judicial Courts (Root-proxied verdict) are the ONLY revocation authority.
    //
    // Constitutional Constraints:
    //   - Max license duration: 10 years (renewable via new application)
    //   - WeaponsManufacture + NuclearOperator: Confederate Khural, 2/3 supermajority
    //   - Indigenous land veto: `indigenous_veto_land_allocation` extrinsic
    //   - No administrative revocation (§2.3 — judicial process only)
    #[runtime::pallet_index(28)]
    pub type Licensing = pallet_licensing;

    // Altan Network: Elections — Bottom-Up Hierarchical Meritocracy (Peak Governance).
    //
    // Implements the 4-tier Mongol decimal hierarchy:
    //   Arbad (10) → Zun (100) → Myangad (1,000) → Tumed (10,000)
    //
    // Peak Governance split (Tumed leaders elect):
    //   • Executive, Judicial, Banking: BranchCouncil (9 seats) → SupremeLeader
    //   • Legislative: Khural Chairman (bound to Arbad ID — constitutional mandate)
    //
    // Constitutional Security:
    //   • H-1: MaxBranchCandidates=100 prevents DOS on BranchVoteCounts storage
    //   • H-2: Only Tumed-level leaders may stand as Branch Council candidates
    //   • H-4: create_election is atomic — overwrites are rejected on-chain
    //   • C-1: confirm_branch_council validates every winner has ≥1 on-chain vote
    //   • C-2: reset_ballot enables clean multi-cycle governance
    //   • C-3: Legislative is constitutionally blocked from SupremeLeader storage
    #[runtime::pallet_index(29)]
    pub type InomadElections = pallet_inomad_elections;

    // Altan Network: Buryad-Mongol Mixer — privacy-preserving shielded transfer pool.
    // Commitment/nullifier scheme. Double fee: 0.08% (0.03% base + 0.05% privacy).
    // Denomination: multiples of 10 ALTAN (ZK anonymity set). Constitutional 54/26/10/10.
    // Exclusive fee: pool receives exactly `amount`. Withdrawal: RelayerOrigin only (MEV).
    // Sprint L1-24.
    #[runtime::pallet_index(30)]
    pub type BuryadMongolMixer = pallet_buryad_mongol_mixer;

    // Altan Network: Altan Vault — isolated savings sub-accounts (Sprint L1-25).
    // Deterministic keyless vaults per (owner, vault_index). Anonymous inbound from Mixer.
    // Outbound ONLY to primary owner. Constitutional Zero-Fee Exemption on withdrawal.
    #[runtime::pallet_index(31)]
    pub type AltanVault = pallet_altan_vault;

    // Altan Network: Access NFT — Universal NFT Access Keys for all legal entities.
    // Soulbound access credentials issued per entity (org, bank, guild, region).
    // Encodes: entity_kind, entity_id, holder, role, access_level, portal_mask.
    // Used by all modules to verify member access without off-chain lookups.
    #[runtime::pallet_index(32)]
    pub type AccessNft = pallet_access_nft;

    // Altan Network: Achievement NFT — Reputation & Reward NFT System.
    //
    // Two categories:
    //   - Reputation: soul-bound badges (Verifier, TumedElected, GuildMaster, etc.)
    //     Cannot be transferred, sold, or redeemed. Proof of civic merit.
    //   - Reward: ALTAN-backed redeemable NFTs (VerifierReward, QuestReward, etc.)
    //     Reserved from ConfederationTreasury; redeemable by holder at any time.
    //
    // Issued by NFT Akademik collective (Root-gated in dev).
    #[runtime::pallet_index(33)]
    pub type AchievementNft = pallet_achievement_nft;

    // Altan Network: Recovery NFT — Arbad Social Account Recovery.
    //
    // Mechanism: Owner mints 5 soul-bound NFTs → distributes to 5 trusted members.
    // Each NFT contains: target_account, group_id, destination vault, threshold (3).
    //
    // Recovery flow:
    //   1. Holder calls initiate_recovery → 72h owner veto window begins
    //   2. Owner may veto_recovery to block
    //   3. 3 holders call confirm_recovery → RecoveryExecuted event emitted
    //   4. Backend listener moves funds to pre-programmed vault_address
    //
    // Constitutional: destination is immutable at mint time — no fund hijacking possible.
    #[runtime::pallet_index(34)]
    pub type RecoveryNft = pallet_recovery_nft;

    // Altan Network: Forums — Threaded Hierarchical Communication.
    //
    // Sovereign L1 deliberation layer for all levels:
    //   Arbad · Zun · Myangad · Tumed · National Khural · Grand Khural
    //
    // Chain of Legitimacy (Вложенное Общение):
    //   - Every post is permanently anchored by author AccountId + content_hash (blake2_256).
    //   - No message can ever be deleted — Habeas Corpus of speech.
    //   - Infinite-depth reply trees via parent_id linkage.
    //   - Root may pin official notices (administrative only — cannot delete).
    //   - Only Active citizens may post.
    //
    // forum_id context: "arbad:3:17" | "khural:5" | "grand_khural" | "proposal:42"
    #[runtime::pallet_index(35)]
    pub type Forums = pallet_forums;

    // Altan Network: Annual Profit Tax — Constitutional income tax for sovereign citizens and orgs.
    // Standard rate: 10% (100‰) | Large family benefit (3+ children): 5% (50‰).
    // Filing window: January 1 – April 15 on-time | April 16 – December 31 late (штраф + пени).
    // Constitutional split: 70% regional treasury / 30% confederation treasury.
    #[runtime::pallet_index(36)]
    pub type AnnualProfitTax = pallet_annual_profit_tax;

    // Altan Network: Migration Center — Sovereign application anchoring on L1.
    //
    // Every migration application is permanently anchored by applicant AccountId + blake2_256 hash.
    // Officer claim signatures are stored on-chain — immutable proof of review.
    //
    // Flow:
    //   1. Backend submits anchor hash (submit_application)
    //   2. Officer claims the application with their Sr25519 signature (claim_application)
    //   3. Officer finalises (approve/reject) or Root revokes (revoke_application)
    //
    // Constitutional guarantee: even if the backend DB is wiped, the wallet + application hash
    // remain permanently anchored on Altan L1.
    #[runtime::pallet_index(37)]
    pub type MigrationCenter = pallet_migration_center;

    // Altan Network: Multisig for sovereign entities
    #[runtime::pallet_index(38)]
    pub type Multisig = pallet_multisig;

    // Altan Network: Proxy (used for Treasury/Pension DAO address stability)
    #[runtime::pallet_index(39)]
    pub type Proxy = pallet_proxy;

    // Altan Network: Organization Roles (SBT) management.
    #[runtime::pallet_index(40)]
    pub type OrgRoles = pallet_org_roles;
}
