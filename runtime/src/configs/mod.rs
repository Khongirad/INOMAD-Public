// This is free and unencumbered software released into the public domain.
//
// Anyone is free to copy, modify, publish, use, compile, sell, or
// distribute this software, either in source code form or as a compiled
// binary, for any purpose, commercial or non-commercial, and by any
// means.
//
// In jurisdictions that recognize copyright laws, the author or authors
// of this software dedicate any and all copyright interest in the
// software to the public domain. We make this dedication for the benefit
// of the public at large and to the detriment of our heirs and
// successors. We intend this dedication to be an overt act of
// relinquishment in perpetuity of all present and future rights to this
// software under copyright law.
//
// THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND,
// EXPRESS OR IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF
// MERCHANTABILITY, FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT.
// IN NO EVENT SHALL THE AUTHORS BE LIABLE FOR ANY CLAIM, DAMAGES OR
// OTHER LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE,
// ARISING FROM, OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR
// OTHER DEALINGS IN THE SOFTWARE.
//
// For more information, please refer to <http://unlicense.org>

// Substrate and Polkadot dependencies
use frame_support::{
    derive_impl, parameter_types,
    traits::{ConstBool, ConstU128, ConstU32, ConstU64, ConstU8, VariantCountOf},
    weights::{
        constants::{RocksDbWeight, WEIGHT_REF_TIME_PER_SECOND},
        IdentityFee, Weight,
    },
};
use frame_system::limits::{BlockLength, BlockWeights};
use pallet_transaction_payment::{ConstFeeMultiplier, FungibleAdapter, Multiplier};
use sp_consensus_aura::sr25519::AuthorityId as AuraId;
use sp_core::crypto::Ss58Codec;
use sp_runtime::{traits::One, Perbill};
use sp_version::RuntimeVersion;

// Local module imports
use super::{
    AccountId, Aura, Balance, Balances, Block, BlockNumber, Hash, Nonce, PalletInfo, Runtime,
    RuntimeCall, RuntimeEvent, RuntimeFreezeReason, RuntimeHoldReason, RuntimeOrigin, RuntimeTask,
    Signature, System, Timestamp, EXISTENTIAL_DEPOSIT, SLOT_DURATION, UNIT, VERSION,
};

// ─── Hard-Cap Minting Lock: BaseCallFilter ────────────────────────────────────
//
// CONSTITUTIONAL MANDATE: The 2.1 Trillion ALTAN hard cap is immutable.
// New tokens can ONLY be created inside `pallet-bank-operator::issue_credit`
// via the CDP collateral mechanism.  All other minting paths are blocked here.
//
// This filter blocks `Balances::force_set_balance` and `Balances::force_transfer`
// from being dispatched by ANY origin — including Sudo/Root.  Without this,
// a Sudo key could print unbounded ALTAN via `pallet_balances` extrinsics,
// bypassing the hard cap and all CDP logic.
//
// The filter is applied as `type BaseCallFilter` on `frame_system::Config`,
// which gates EVERY extrinsic dispatch through the runtime.

pub struct BlockedCallFilter;

impl frame_support::traits::Contains<RuntimeCall> for BlockedCallFilter {
    fn contains(call: &RuntimeCall) -> bool {
        match call {
            // ── BLOCKED: Arbitrary balance minting / force-transfer ───────────
            //
            // These two calls allow arbitrary balance creation/destruction
            // without going through the CDP collateral mechanism.  They are
            // constitutionally prohibited for ALL callers, including Root/Sudo.
            //
            // Blocking them here means the 2.1T hard cap is enforced even if
            // the Sudo key is compromised.
            RuntimeCall::Balances(pallet_balances::Call::force_set_balance { .. }) => false,
            RuntimeCall::Balances(pallet_balances::Call::force_transfer { .. }) => false,
            // All other calls are allowed.
            _ => true,
        }
    }
}

const NORMAL_DISPATCH_RATIO: Perbill = Perbill::from_percent(75);

parameter_types! {
    pub const BlockHashCount: BlockNumber = 2400;
    pub const Version: RuntimeVersion = VERSION;

    /// We allow for 2 seconds of compute with a 6 second average block time.
    pub RuntimeBlockWeights: BlockWeights = BlockWeights::with_sensible_defaults(
        Weight::from_parts(2u64 * WEIGHT_REF_TIME_PER_SECOND, u64::MAX),
        NORMAL_DISPATCH_RATIO,
    );
    pub RuntimeBlockLength: BlockLength = BlockLength::max_with_normal_ratio(5 * 1024 * 1024, NORMAL_DISPATCH_RATIO);
    pub const SS58Prefix: u8 = 42;
}

/// All migrations of the runtime, aside from the ones declared in the pallets.
///
/// This can be a tuple of types, each implementing `OnRuntimeUpgrade`.
#[allow(unused_parens)]
type SingleBlockMigrations = (
    pallet_bank_of_siberia::migrations::V2BackfillActiveLoanIndex<Runtime>,
);

/// The default types are being injected by [`derive_impl`](`frame_support::derive_impl`) from
/// [`SoloChainDefaultConfig`](`struct@frame_system::config_preludes::SolochainDefaultConfig`),
/// but overridden as needed.
#[derive_impl(frame_system::config_preludes::SolochainDefaultConfig)]
impl frame_system::Config for Runtime {
    /// The block type for the runtime.
    type Block = Block;
    /// Block & extrinsics weights: base values and limits.
    type BlockWeights = RuntimeBlockWeights;
    /// The maximum length of a block (in bytes).
    type BlockLength = RuntimeBlockLength;
    /// The identifier used to distinguish between accounts.
    type AccountId = AccountId;
    /// The type for storing how many extrinsics an account has signed.
    type Nonce = Nonce;
    /// The type for hashing blocks and tries.
    type Hash = Hash;
    /// Maximum number of block number to block hash mappings to keep (oldest pruned first).
    type BlockHashCount = BlockHashCount;
    /// The weight of database operations that the runtime can invoke.
    type DbWeight = RocksDbWeight;
    /// Version of the runtime.
    type Version = Version;
    /// The data to be stored in an account.
    type AccountData = pallet_balances::AccountData<Balance>;
    /// This is used as an identifier of the chain. 42 is the generic substrate prefix.
    type SS58Prefix = SS58Prefix;
    type MaxConsumers = frame_support::traits::ConstU32<16>;
    type SingleBlockMigrations = SingleBlockMigrations;
    /// Hard-cap enforcement: block arbitrary minting via Balances extrinsics.
    ///
    /// `BlockedCallFilter` blocks `Balances::force_set_balance` and
    /// `Balances::force_transfer` from ALL origins (including Sudo/Root).
    /// New ALTAN can ONLY be created via `pallet-bank-operator::issue_credit`
    /// under the CDP 9x fractional reserve protocol.
    type BaseCallFilter = BlockedCallFilter;
}

impl pallet_aura::Config for Runtime {
    type AuthorityId = AuraId;
    type DisabledValidators = ();
    type MaxAuthorities = ConstU32<100>;
    type AllowMultipleBlocksPerSlot = ConstBool<false>;
    type SlotDuration = pallet_aura::MinimumPeriodTimesTwo<Runtime>;
}

impl pallet_grandpa::Config for Runtime {
    type RuntimeEvent = RuntimeEvent;

    type WeightInfo = ();
    type MaxAuthorities = ConstU32<100>;
    type MaxNominators = ConstU32<0>;
    type MaxSetIdSessionEntries = ConstU64<0>;

    type KeyOwnerProof = sp_core::Void;
    type EquivocationReportSystem = ();
}

impl pallet_timestamp::Config for Runtime {
    /// A timestamp: milliseconds since the unix epoch.
    type Moment = u64;
    type OnTimestampSet = Aura;
    type MinimumPeriod = ConstU64<{ SLOT_DURATION / 2 }>;
    type WeightInfo = ();
}

impl pallet_balances::Config for Runtime {
    type MaxLocks = ConstU32<50>;
    type MaxReserves = ();
    type ReserveIdentifier = [u8; 8];
    /// The type for recording an account's balance.
    type Balance = Balance;
    /// The ubiquitous event type.
    type RuntimeEvent = RuntimeEvent;
    type DustRemoval = ();
    type ExistentialDeposit = ConstU128<EXISTENTIAL_DEPOSIT>;
    type AccountStore = System;
    type WeightInfo = pallet_balances::weights::SubstrateWeight<Runtime>;
    type FreezeIdentifier = RuntimeFreezeReason;
    type MaxFreezes = VariantCountOf<RuntimeFreezeReason>;
    type RuntimeHoldReason = RuntimeHoldReason;
    type RuntimeFreezeReason = RuntimeFreezeReason;
    type DoneSlashHandler = ();
}

parameter_types! {
    pub FeeMultiplier: Multiplier = Multiplier::one();
}

// ─── InomadFeeSplitter: Constitutional Fee Distribution ───────────────────────
//
// Every transaction fee on the Altan Network is split (3-way, v2):
//
//   54% → KHURAL_FOUNDATION  (Swiss Stiftung — UBI, science, 86 indigenous nations)
//   36% → INOMAD_AG          (Swiss GmbH — R&D, operations, team, Creator compensation)
//   10% → Validator Pool     (block producers, distributed via governance)
//
// CONSTITUTIONAL REFORM v2 (runtime upgrade from 4-way to 3-way split):
//   Removed: direct 10% Creator on-chain royalty.
//   Reason:  Creator (Citizen #1) compensation flows through INOMAD AG corporate
//            structure for legal clarity (Swiss GmbH → employment/dividend → Creator).
//            This eliminates SEC/FinCEN unregistered-security risk of a named
//            individual receiving % of every transaction in a public network.
//
// INOMAD AG internal allocation (off-chain, governed by AG board):
//   ≈10% of AG receipts → Creator salary / founder dividend
//   ≈10% of AG receipts → INOMAD INC USA (licensing/royalty agreement)
//   ≈16% of AG receipts → R&D, team, operations
//
// CONSTITUTIONAL NOTE: These ratios are encoded in WASM.
// Change requires constitutional referendum + runtime upgrade.

pub struct InomadFeeSplitter;

impl InomadFeeSplitter {
    /// 54% to KHURAL_FOUNDATION (Swiss Stiftung — UBI, science, 86 indigenous peoples)
    fn khural_foundation_account() -> AccountId {
        AccountId::from_ss58check("5G11UBehntN5pPMoi7m7s6GTayb3T9iEAJFThAUmuF8V2fna")
            .expect("KHURAL_FOUNDATION SS58 address must be valid")
    }

    /// 36% to INOMAD_AG (Swiss GmbH — R&D, operations, team, Creator compensation)
    ///
    /// Increased from 26% to 36% in v2 constitutional reform.
    /// Creator compensation now flows through AG corporate structure, not direct on-chain.
    fn inomad_ag_account() -> AccountId {
        AccountId::from_ss58check("5DrCgbEEpN1T1AjgJsaNz914T2q3p39RNGmRh9GqdVd4YdGJ")
            .expect("INOMAD_AG SS58 address must be valid")
    }

    /// 10% to validator pool (accumulated, distributed via governance vote).
    /// Uses a deterministic system account seed — keyless, controlled by pallet logic.
    fn validator_pool_account() -> AccountId {
        let mut seed = [b'_'; 32];
        let label = b"VALIDATOR_POOL_ACCT";
        seed[..label.len()].copy_from_slice(label);
        AccountId::from(seed)
    }
}

impl
    frame_support::traits::OnUnbalanced<
        frame_support::traits::fungible::Credit<AccountId, Balances>,
    > for InomadFeeSplitter
{
    fn on_nonzero_unbalanced(amount: frame_support::traits::fungible::Credit<AccountId, Balances>) {
        use frame_support::traits::fungible::Balanced;
        use frame_support::traits::Imbalance;

        let total = amount.peek();

        // 36% to INOMAD AG (includes former 10% creator allocation, now corporate)
        let ag_share = Perbill::from_percent(36) * total;
        // 10% to validator pool
        let validator_share = Perbill::from_percent(10) * total;
        // 54% = remainder (absorbs rounding dust — always goes to Khural Foundation)

        // Split: INOMAD_AG first, then Validators, remainder → Khural Foundation
        let (ag_credit, rest) = amount.split(ag_share);
        let (validator_credit, khural_credit) = rest.split(validator_share);

        // Resolve each credit to the destination account.
        let _ = Balances::resolve(&Self::inomad_ag_account(), ag_credit);
        let _ = Balances::resolve(&Self::validator_pool_account(), validator_credit);
        let _ = Balances::resolve(&Self::khural_foundation_account(), khural_credit);
    }
}

impl pallet_transaction_payment::Config for Runtime {
    type RuntimeEvent = RuntimeEvent;
    /// InomadFeeSplitter handles the 54/36/10 split of every transaction fee (v2).
    /// 54% Khural Foundation · 36% INOMAD AG · 10% Validators
    /// Creator compensation flows through INOMAD AG (Swiss GmbH) corporate structure.
    type OnChargeTransaction = FungibleAdapter<Balances, InomadFeeSplitter>;
    type OperationalFeeMultiplier = ConstU8<5>;
    type WeightToFee = IdentityFee<Balance>;
    type LengthToFee = IdentityFee<Balance>;
    type FeeMultiplierUpdate = ConstFeeMultiplier<FeeMultiplier>;
    type WeightInfo = pallet_transaction_payment::weights::SubstrateWeight<Runtime>;
}

impl pallet_sudo::Config for Runtime {
    type RuntimeEvent = RuntimeEvent;
    type RuntimeCall = RuntimeCall;
    type WeightInfo = pallet_sudo::weights::SubstrateWeight<Runtime>;
}

impl pallet_multisig::Config for Runtime {
    type RuntimeEvent = RuntimeEvent;
    type RuntimeCall = RuntimeCall;
    type Currency = Balances;
    type DepositBase = ConstU128<{ 1 * UNIT }>;
    type DepositFactor = ConstU128<{ 1 * UNIT }>;
    type MaxSignatories = ConstU32<100>;
    type WeightInfo = pallet_multisig::weights::SubstrateWeight<Runtime>;
    type BlockNumberProvider = System;
}

#[derive(
    Copy,
    Clone,
    Eq,
    PartialEq,
    Ord,
    PartialOrd,
    codec::Encode,
    codec::Decode,
    codec::DecodeWithMemTracking,
    sp_runtime::RuntimeDebug,
    scale_info::TypeInfo,
    codec::MaxEncodedLen,
)]
pub enum ProxyType {
    Any,
    NonTransfer,
}
impl Default for ProxyType {
    fn default() -> Self {
        Self::Any
    }
}
impl frame_support::traits::InstanceFilter<RuntimeCall> for ProxyType {
    fn filter(&self, c: &RuntimeCall) -> bool {
        match self {
            ProxyType::Any => true,
            ProxyType::NonTransfer => !matches!(c, RuntimeCall::Balances(..)),
        }
    }
}

impl pallet_proxy::Config for Runtime {
    type RuntimeEvent = RuntimeEvent;
    type RuntimeCall = RuntimeCall;
    type Currency = Balances;
    type ProxyType = ProxyType;
    type ProxyDepositBase = ConstU128<{ 1 * UNIT }>;
    type ProxyDepositFactor = ConstU128<{ 1 * UNIT }>;
    type MaxProxies = ConstU32<32>;
    type WeightInfo = pallet_proxy::weights::SubstrateWeight<Runtime>;
    type MaxPending = ConstU32<32>;
    type CallHasher = sp_runtime::traits::BlakeTwo256;
    type AnnouncementDepositBase = ConstU128<{ 1 * UNIT }>;
    type AnnouncementDepositFactor = ConstU128<{ 1 * UNIT }>;
    type BlockNumberProvider = System;
}

/// Configure the pallet-template in pallets/template.
impl pallet_template::Config for Runtime {
    type RuntimeEvent = RuntimeEvent;
    type WeightInfo = pallet_template::weights::SubstrateWeight<Runtime>;
}

parameter_types! {
    /// Creator / Citizen #1 wallet — used as associated type for pallet_altan_tax & mixer.
    ///
    /// Source: docs/ALPHA_KEYS.md §5 CREATOR_SUDO
    /// SS58:   5FTZYAh4tCCXKc8Pu7KYZrD9F3fGeu3YNYhk3gbrGC9n39Wv
    pub CreatorSudoAccount: AccountId = {
        use sp_core::crypto::Ss58Codec;
        AccountId::from_ss58check("5FTZYAh4tCCXKc8Pu7KYZrD9F3fGeu3YNYhk3gbrGC9n39Wv")
            .expect("CREATOR_SUDO SS58 must be valid")
    };
}

/// Configure the pallet-altan-tax — Altan Network constitutional fee router.
/// Sprint L1-24: exclusive fee math + 54/26/10/10 constitutional split.
/// Configure pallet-altan-tax — Altan Network constitutional fee router.
/// v2: 54/36/10 split (3-way). Creator compensation flows through INOMAD AG.
impl pallet_altan_tax::Config for Runtime {
    /// Use the pallet_balances instance for all fee-related Currency transfers.
    type Currency = Balances;
}

// ─── Annual Profit Tax: Constitutional Income Tax ─────────────────────────────
//
// `pallet-annual-profit-tax` implements the sovereign annual profit declaration:
//
//  Standard rate (organizations / individuals): 10% (100‰)
//  Large family benefit (citizen with 3+ children): 5% (50‰)
//
//  Filing window:
//    On-time: January 1 – April 15 (105 days)
//    Late:    April 16 – December 31 (259 days, incurs штраф + пени)
//
//  Constitutional split: 70% → RegionalTreasury / 30% → ConfederationTreasury
//  Both set via Root extrinsics after genesis.

/// Configure pallet-annual-profit-tax — Constitutional annual income tax.
/// Standard: 10% | Large family (3+ children): 5%.
/// Split: 70% regional / 30% confederation treasury.
impl pallet_annual_profit_tax::Config for Runtime {
    /// Use pallet_balances for all tax payment transfers.
    type Currency = Balances;
    /// Unix timestamp provider — delegates to pallet-timestamp.
    type UnixTime = Timestamp;
    /// Standard annual profit tax rate: 100‰ = 10%.
    type StandardTaxRatePermill = ConstU32<100>;
    /// Reduced rate for large families (3+ children): 50‰ = 5%.
    type LargeFamilyTaxRatePermill = ConstU32<50>;
}

// ─── Proof-of-Citizenship Vesting Constants ───────────────────────────────────
//
// These constants define how the GDP share is progressively unlocked as citizens
// participate in the Fractal Democracy social hierarchy.
//
// Level 0 (Verified by peer):    100 ALTAN immediately unlocked.
// Level 1 (Joined Arbad):      1_000 ALTAN cumulatively unlocked.
// Level 2 (Full Arbad + Leader): ALL remaining GDP share unlocked.

parameter_types! {
    /// Planck amount unlocked at Proof-of-Citizenship Level 0 (peer verification).
    /// = 100 ALTAN = 100 * 10^12 planck.
    pub const UnlockLevel0: u128 = 100 * UNIT;
    /// Cumulative planck amount unlocked at Level 1 (Arbad membership).
    /// = 1_000 ALTAN = 1_000 * 10^12 planck.
    pub const UnlockLevel1: u128 = 1_000 * UNIT;
    /// ****CONSTITUTIONAL**** Civil act fee per partner for marriage registration: 5 ALTAN.
    ///
    /// Both partners are charged this fee. Total per ceremony = 10 ALTAN (2 × 5).
    /// Goes to `ConfederationTreasury` as default (can be routed to region via upgrade).
    pub const MarriageCivilActFee: u128 = 5 * UNIT;
    /// The Medical Authority account — authorised to call register_birth and register_death.
    ///
    /// In development: set to Alice's AccountId for easy testing.
    /// In production: set to the Altan Gateway's Medical Officer key (Minzdrav).
    pub MedicalAuthority: AccountId = {
        use sp_core::crypto::Ss58Codec;
        // Dev: Alice = 5GrwvaEF5zXb26Fz9rcQpDWS57CtERHpNehXCPcNoHGKutQY
        AccountId::from_ss58check("5GrwvaEF5zXb26Fz9rcQpDWS57CtERHpNehXCPcNoHGKutQY")
            .unwrap_or_else(|_| AccountId::from([0u8; 32]))
    };
    /// [SECURITY VECTOR 3] Arbad cartel cooldown — ~30 days at 6s/block.
    ///
    /// 432_000 blocks × 6 seconds = 2_592_000 seconds ≈ 30 days.
    /// Citizens who leave an Arbad must wait this many blocks before joining a new one.
    pub const ArbadCooldownBlocks: u32 = 432_000;
}

// ─── Ghost-State: TerminalHookImpl ─────────────────────────────────────────────────────────────────────
//
// This struct is the runtime-level dispatcher for pallet-inomad-identity's
// `OnTerminalStatus<AccountId>` hook. When `register_death` or `do_exile`
// fires the hook, this impl cascades cleanup across ALL affected pallets:
//
//   1. pallet-guilds:   cancel open Quests (unreserve escrow), strip Academician rank.
//   2. pallet-chancery: cancel PendingSignatures agreements where the citizen is a party.
//
// pallet-khural-governance does not hold direct citizen state; governance mandates
// (KhuralTermsServed, role demotion) are handled inside do_exile itself.

/// Runtime dispatcher for terminal-state cross-pallet cascading cleanup.
///
/// Registered as `type TerminalHook = TerminalHookImpl` in the
/// `pallet_inomad_identity::Config` impl below.
pub struct TerminalHookImpl;

impl pallet_inomad_identity::OnTerminalStatus<AccountId> for TerminalHookImpl {
    fn on_deceased(who: &AccountId) {
        // ── Guild & Chancery cleanup ───────────────────────────────────────
        // Cancel open quests and strip academy membership in guilds.
        pallet_guilds::Pallet::<Runtime>::cleanup_account(who);
        // Annul pending-signature contracts in chancery.
        pallet_chancery::Pallet::<Runtime>::annul_signatures(who);

        // ── Inheritance trigger (Sprint B) ────────────────────────────────
        // Synchronously execute the deceased's Will (if notarized) OR fall back
        // to the StateTreasury. This closes the inheritance loop immediately on
        // `register_death` without requiring a separate user transaction.
        //
        // Constitutional mandate: no dead-soul wealth accumulation.
        // The identity pallet has already reserved the deceased's free balance
        // (just above in `register_death`), so we can safely distribute it here.
        //
        // Best-effort: `trigger_inheritance` swallows errors internally to prevent
        // a failing Will from aborting the parent `register_death` extrinsic.
        // Observable via presence/absence of WillExecuted / FallbackToTreasury events.
        pallet_inheritance::Pallet::<Runtime>::trigger_inheritance(who);
    }
    fn on_exiled(who: &AccountId) {
        pallet_guilds::Pallet::<Runtime>::cleanup_account(who);
        pallet_chancery::Pallet::<Runtime>::annul_signatures(who);
    }
}

parameter_types! {
    /// Constitutional hash anchor: `None` = not yet enforced (activated via runtime upgrade).
    ///
    /// When the Constitution pallet anchors the canonical hash, update this to `Some([u8; 32])`
    /// to enforce cryptographic lineage in citizen registration extrinsics.
    pub const ConstConstitutionHash: Option<[u8; 32]> = None;
}

/// Configure the pallet-inomad-identity — Soulbound citizen identity & Arbad pallet.
impl pallet_inomad_identity::Config for Runtime {
    /// **SECURITY** Uses pallet_balances to physically reserve (freeze) the free balance
    /// of deceased citizens via `register_death`, preventing dead-wallet transactions.
    type Currency = Balances;
    type UnlockLevel0 = UnlockLevel0;
    type UnlockLevel1 = UnlockLevel1;
    type MedicalAuthority = MedicalAuthority;
    /// [SECURITY VECTOR 3] 30-day cooldown before a citizen who left an Arbad
    /// can join or form another. Prevents rapid cartel migration attacks.
    type ArbadCooldownPeriod = ArbadCooldownBlocks;
    /// [SECURITY: ORACLE RATE LIMIT] Max 100 `register_birth` calls per 6-second block
    /// from the MedicalAuthority key. Limits blast radius of a compromised oracle key.
    type MaxRegistrationsPerBlock = ConstU32<100>;
    /// [SECURITY: GHOST STATE] Cascade cross-pallet cleanup on death or exile.
    /// Dispatches to pallet-guilds (quest escrow + academy) and pallet-chancery
    /// (pending agreement annulment).
    type TerminalHook = TerminalHookImpl;
    /// [CHAIN OF LEGITIMACY] Constitutional hash anchor for citizen claims.
    ///
    /// When `Some([u8; 32])`, citizen claim/register extrinsics must include the
    /// canonical Constitution hash. When `None`, the hash check is skipped (current
    /// state — can be set to canonics via runtime upgrade after Constitution is anchored).
    type ConstitutionHashProvider = ConstConstitutionHash;
    /// ****CONSTITUTIONAL**** Civil act fee: 5 ALTAN per partner per marriage/divorce.
    ///
    /// Both partners pay. Collected by `CivilFeeTreasury` (default: Confederation).
    type MarriageFee = MarriageCivilActFee;
    /// Default recipient of civil act fees — Confederation treasury in genesis.
    type CivilFeeTreasury = ConfederationTreasuryAddr;
    type WeightInfo = pallet_inomad_identity::weights::SubstrateWeight<Runtime>;
}

/// Resolves the 86 Indigenous Nation (\u041a\u043e\u0440\u0435\u043d\u043d\u044b\u0435 \u041d\u0430\u0440\u043e\u0434\u044b) treasury accounts from `pallet-altan-tax` storage.
///
/// This indirection keeps `pallet-khural-governance` loosely coupled to the tax pallet:
/// we read the storage here in the runtime glue, not inside the governance pallet itself.
pub struct NationTreasuriesGetter;
impl
    frame_support::traits::Get<
        frame_support::BoundedVec<AccountId, frame_support::traits::ConstU32<150>>,
    > for NationTreasuriesGetter
{
    fn get() -> frame_support::BoundedVec<AccountId, frame_support::traits::ConstU32<150>> {
        pallet_altan_tax::NationTreasuries::<Runtime>::get()
    }
}

parameter_types! {
    /// Khural voting period: ~7 days at 6 seconds/block = 100_800 blocks.
    ///
    /// After this many blocks from proposal creation, `on_initialize` fires and
    /// auto-executes or auto-rejects the proposal based on quorum + majority.
    pub const KhuralVotingPeriod: u32 = 100_800;

    /// Minimum YES votes required for a proposal to pass (quorum threshold).
    ///
    /// Sprint L1-05 default: 3 votes. Prevents a single under-the-radar vote
    /// from passing large treasury grants. Adjustable via runtime upgrade.
    pub const KhuralMinQuorum: u32 = 3;
}

/// Configure the pallet-khural-governance — Khural democratic governance, nation treasury execution,
/// and Expert Legislative Channel (Academy of Sciences bills).
impl pallet_khural_governance::Config for Runtime {
    /// Use the runtime's `pallet_balances` instance for all treasury transfers and deposits.
    type Currency = Balances;
    /// Resolve nation treasury accounts at call-time from the tax pallet's storage.
    type NationTreasuryProvider = NationTreasuriesGetter;
    /// Academy of Sciences interface — checks AcademyMembers storage in pallet-guilds.
    /// Only GuildTumed Grandmasters (Academicians) may call `propose_expert_bill`.
    type AcademyInterface = pallet_guilds::Pallet<Runtime>;
    /// [SECURITY VECTOR 2] Anti-spam deposit for Expert Legislative Bills.
    ///
    /// 10 ALTAN (= 10 * 10^12 planck) is reserved from the proposer on each
    /// `propose_expert_bill` call. Returned if the bill passes; slashed (burned)
    /// if the bill is rejected. Prevents unlimited Parliamentary ledger spam.
    type ExpertBillDeposit = ConstU128<{ 10 * UNIT }>;
    /// **CONSTITUTIONAL** Voting period: ~7 days at 6s/block.
    ///
    /// After `VotingPeriod` blocks, `on_initialize` auto-executes expired proposals.
    /// This is the democratic heartbeat of the Khural — a non-negotiable time guarantee.
    type VotingPeriod = KhuralVotingPeriod;
    /// **CONSTITUTIONAL** Minimum quorum: 3 YES votes required to pass a proposal.
    ///
    /// A proposal passes only if votes_for >= 3 AND votes_for > votes_against.
    /// Prevents stealth governance via zero-turnout votes.
    type MinQuorum = KhuralMinQuorum;
}

// ─── Steppe Offline: SlashInterface bridge ────────────────────────────────────
//
// This struct bridges `pallet-steppe-offline` → `pallet-inomad-identity`.
// When ARMAGEDDON fires, `slash_citizen` directly mutates the `Citizens`
// StorageMap without dispatching a full extrinsic, keeping the slash
// atomic with the IOU settlement transaction.

/// Runtime implementation of `SlashInterface` backed by `pallet-inomad-identity`.
pub struct IdentitySlasher;

impl pallet_steppe_offline::SlashInterface<AccountId> for IdentitySlasher {
    fn slash_citizen(who: &AccountId) -> frame_support::dispatch::DispatchResult {
        pallet_inomad_identity::Citizens::<Runtime>::try_mutate(who, |maybe| {
            let record = maybe.as_mut().ok_or(sp_runtime::DispatchError::Other(
                "IdentitySlasher: account not registered",
            ))?;
            record.status = pallet_inomad_identity::pallet::CitizenStatus::Frozen;
            Ok::<(), sp_runtime::DispatchError>(())
        })?;
        Ok(())
    }
}

/// Configure pallet-steppe-offline — offline mesh payment vault & ARMAGEDDON slashing.
impl pallet_steppe_offline::Config for Runtime {
    /// Use the runtime's `pallet_balances` instance for vault mint/burn operations.
    type Currency = Balances;
    /// Cross-pallet slashing bridge: fires ARMAGEDDON into `pallet-inomad-identity`.
    type IdentitySlashing = IdentitySlasher;
    /// Runtime signature type — `MultiSignature` covers both sr25519 and ed25519.
    type Signature = Signature;
    type WeightInfo = pallet_steppe_offline::weights::SubstrateWeight<Runtime>;
}

// ─── Judicial Courts: IdentityInterface bridge ────────────────────────────────
//
// `IdentityBridge` implements the `IdentityInterface` trait defined in
// `pallet-inomad-identity` so that `pallet-judicial-courts` can perform
// citizen lookups and mutations without tight pallet coupling.

/// Runtime implementation of `IdentityInterface` for the judicial courts pallet.
pub struct IdentityBridge;

impl pallet_inomad_identity::IdentityInterface<AccountId> for IdentityBridge {
    fn citizen_record_of(who: &AccountId) -> Option<pallet_inomad_identity::pallet::CitizenRecord> {
        pallet_inomad_identity::Citizens::<Runtime>::get(who)
    }

    fn freeze_citizen(who: &AccountId) -> frame_support::dispatch::DispatchResult {
        pallet_inomad_identity::Pallet::<Runtime>::do_freeze_citizen(who)
    }

    fn unfreeze_citizen(who: &AccountId) -> frame_support::dispatch::DispatchResult {
        pallet_inomad_identity::Pallet::<Runtime>::do_unfreeze_citizen(who)
    }

    fn demote_to_regular(who: &AccountId) -> frame_support::dispatch::DispatchResult {
        pallet_inomad_identity::Pallet::<Runtime>::do_demote_to_regular(who)
    }

    fn exile_citizen(who: &AccountId) -> frame_support::dispatch::DispatchResult {
        pallet_inomad_identity::Pallet::<Runtime>::do_exile(who)
    }
}

// ─── Judicial Courts: ConstitutionBridge ──────────────────────────────────────
//
// `ConstitutionBridgeImpl` implements the `ConstitutionInterface` defined in
// `pallet-judicial-courts` so that `open_case` / `issue_verdict` can register
// and resolve Habeas Corpus timers in `pallet-constitution` without tight coupling.
//
// Both constitution extrinsics are Root-gated internally.  We dispatch them here
// using `frame_support::dispatch::DispatchResultWithPostInfo` via RuntimeOrigin::root().

/// Runtime Habeas Corpus bridge: `pallet-judicial-courts` → `pallet-constitution`.
pub struct ConstitutionBridgeImpl;

impl pallet_judicial_courts::ConstitutionInterface<Runtime> for ConstitutionBridgeImpl {
    /// Register a Habeas Corpus timer when a court case is opened.
    ///
    /// Dispatches `pallet_constitution::register_lockup` with a Root origin so the
    /// timer is recorded under `HabeasCorpusTimers[defendant]`.
    fn register_lockup(
        citizen: &AccountId,
        max_lockup_block: BlockNumber,
        case_hash: [u8; 32],
    ) -> frame_support::dispatch::DispatchResult {
        pallet_constitution::Pallet::<Runtime>::register_lockup(
            frame_system::RawOrigin::Root.into(),
            citizen.clone(),
            max_lockup_block,
            case_hash,
        )
    }

    /// Resolve a Habeas Corpus timer when a verdict is issued.
    ///
    /// Dispatches `pallet_constitution::resolve_habeas_corpus` with Root origin.
    /// If no timer exists (e.g. already expired), silently ignores the error —
    /// resolving an already-expired timer must not block verdict issuance.
    fn resolve_habeas_corpus(citizen: &AccountId) -> frame_support::dispatch::DispatchResult {
        // Ignore NoActiveTimer — the auto-release may have already fired.
        let _ = pallet_constitution::Pallet::<Runtime>::resolve_habeas_corpus(
            frame_system::RawOrigin::Root.into(),
            citizen.clone(),
        );
        Ok(())
    }
}

parameter_types! {
    /// State Treasury account for judicial penalty distribution.
    ///
    /// Receives 80% of the penalty (remainder after 20% whistleblower reward).
    /// In dev: Bob's account. In production: the Confederation Treasury managed
    /// by `pallet-altan-tax`.
    pub JudicialTreasury: AccountId = {
        use sp_core::crypto::Ss58Codec;
        // Dev: Bob = 5FHneW46xGXgs5mUiveU4sbTyGBzmstUspZC92UhjJM694ty
        AccountId::from_ss58check("5FHneW46xGXgs5mUiveU4sbTyGBzmstUspZC92UhjJM694ty")
            .unwrap_or_else(|_| AccountId::from([0u8; 32]))
    };
}

/// Configure pallet-judicial-courts — Judicial Engine with Habeas Corpus bridge.
impl pallet_judicial_courts::Config for Runtime {
    /// Use `pallet_balances` for penalty transfers, pension slashing, and
    /// whistleblower rewards.
    type Currency = Balances;

    /// Bridge to `pallet-inomad-identity` for citizen lookups and mutations.
    type Identity = IdentityBridge;

    /// Habeas Corpus constitutional bridge → `pallet-constitution`.
    ///
    /// Calls `register_lockup` on `open_case` and `resolve_habeas_corpus`
    /// on `issue_verdict` to keep the constitutional timer in sync.
    type ConstitutionBridge = ConstitutionBridgeImpl;

    /// State Treasury: receives the 80% penalty remainder after whistleblower
    /// reward.  Shared with `pallet-inheritance` and `pallet-citizen-voice`.
    type Treasury = JudicialTreasury;

    /// College of Judges origin — the only authority that may issue verdicts.
    ///
    /// Dev: Root (Sudo). Production: a threshold Collective of Judicial TumedLeaders.
    type JudgesCollectiveOrigin = frame_system::EnsureRoot<AccountId>;

    /// Usurpation origin — the supreme constitutional consensus.
    ///
    /// Dev: Root. Production: a cross-branch threshold (Court + Khural quorum).
    type UsurpationOrigin = frame_system::EnsureRoot<AccountId>;
    type WeightInfo = pallet_judicial_courts::weights::SubstrateWeight<Runtime>;
}

// ─── Bank Operator ─────────────────────────────────────────────────────────────
//
// `pallet-bank-operator` implements the Banking Branch of Power.
// It manages ONLY debt and credit via collateral-backed CDP contracts.
//
// Constitutional constraint: the bank CANNOT touch citizen accounts directly.
// Account freezes must go through `pallet-judicial-courts`.
//
// HARD CAP ENFORCEMENT:
//   1. `BaseCallFilter` (above) blocks `force_set_balance` / `force_transfer`.
//   2. `ReserveMultiplier = ConstU32<9>` is validated inside `issue_credit` at
//      runtime — any value ≠ 9 fails with `Error::InvalidReserveMultiplier`.
//   3. `credit_requested` dual-consent gate: citizens must sign `request_credit`
//      before the bank can emit new tokens via `issue_credit`.

/// Configure pallet-bank-operator — Banking Branch: collateral, credit, RWA, Credit Rating.
impl pallet_bank_operator::Config for Runtime {
    /// Use the runtime's `pallet_balances` instance for collateral transfers.
    type Currency = Balances;
    /// Constitutional fractional reserve multiplier — MUST equal 9.
    ///
    /// credit_amount = collateral_net × ReserveMultiplier = collateral_net × 9
    ///
    /// This constant is validated on every `issue_credit` call.  Changing it
    /// to anything other than 9 will cause `Error::InvalidReserveMultiplier`
    /// at call time, making the x9 mandate an auditable on-chain invariant.
    type ReserveMultiplier = ConstU32<9>;
    /// Interest accrual period: 14,400 blocks ≈ 1 day (6s block time).
    ///
    /// Interest is calculated lazily at repayment time using:
    ///   interest = outstanding × daily_rate_bps × periods_elapsed / 10_000
    ///
    /// daily_rate_bps = annual_rate_bps / 365 (ceiling division)
    type InterestPeriodBlocks = ConstU32<14_400>;
    type WeightInfo = pallet_bank_operator::weights::SubstrateWeight<Runtime>;
}

// ─── Chronicles: Decentralised Intellectual Property Registry ─────────────────
//
// `pallet-chronicles` (Летописи Государства) anchors Science, Media, History,
// and Law documents on-chain via H256 content hashes + IPFS CIDs.
//
// CONSTANTS:
//   MaxCidLength = 64  — fits both CIDv0 (46 bytes, `Qm…`) and CIDv1 (≤59 bytes, `bafy…`).
//
// ECONOMY:
//   `donate_to_author` uses `pallet_balances` to transfer ALTAN tips atomically
//   from any citizen to any registered author.

parameter_types! {
    /// Maximum byte-length for an IPFS CID stored in the Chronicles registry.
    /// CIDv0 (`Qm…`) = 46 bytes, CIDv1 (`bafy…`) ≤ 59 bytes. 64 adds a safety margin.
    pub const MaxCidLength: u32 = 64;
}

/// Configure pallet-chronicles — Летописи Государства (Altan IP registry + tipping economy).
impl pallet_chronicles::Config for Runtime {
    /// Maximum byte-length for an IPFS CID stored on-chain.
    type MaxCidLength = MaxCidLength;
    /// Currency used for citizen-to-author ALTAN donations.
    type Currency = Balances;
    type WeightInfo = pallet_chronicles::weights::SubstrateWeight<Runtime>;
}

// ─── Guilds: Professional DAO Protocol ────────────────────────────────────────
//
// `pallet-guilds` implements the Altan Professional Layer:
// - Guilds: on-chain professional DAOs (decentralised replacement for LinkedIn)
// - Quests: escrow-backed task market (decentralised replacement for Upwork)
// - Achievements: meritocracy credentials issued by Guild Masters
// - Academy: on-chain subscription payments that unlock off-chain video courses
//
// CONSTANTS (must match `pallet-guilds::Config`):
//   MaxNameLength   = 64  — UTF-8 guild/achievement title length
//   MaxIpfsCidLength = 64 — fits both CIDv0 (46 bytes) and CIDv1 (≤59 bytes)

parameter_types! {
    /// Maximum byte-length for a Guild name or Achievement title.
    pub const GuildMaxNameLength: u32 = 64;
    /// Maximum byte-length for an IPFS CID in the Guilds pallet.
    pub const GuildMaxIpfsCidLength: u32 = 64;
    /// Maximum number of guild IDs that can be supplied in a single union-formation call.
    /// Prevents unbounded iteration on-chain.
    pub const GuildMaxUnionMembers: u32 = 50;

    // ── UUID-based Guild Instantiation (Relayer path) ─────────────────────────
    //
    // `GuildsPalletId` is used to derive keyless treasury accounts for guilds
    // submitted by the backend Relayer after 9 off-chain signatures are collected.
    //
    // treasury_addr = PalletId(*b"inm/glds").into_sub_account_truncating(guild_uuid_hash)
    //
    // The resulting AccountId has no private key — it is controlled exclusively
    // by pallet-guilds extrinsics (quest escrow, revenue sharing, etc.).

    /// PalletId for pallet-guilds treasury account derivation.
    ///
    /// 8-byte ASCII identifier.  The derived sub-accounts are keyless —
    /// no private key exists; funds are controlled by pallet logic only.
    pub const GuildsPalletId: frame_support::PalletId = frame_support::PalletId(*b"inm/glds");

    /// Maximum number of founding members in a UUID-keyed guild.
    ///
    /// The constitutional quorum is **exactly 9**, but the bound must be ≥ 9 to
    /// accommodate the BoundedVec.  Set to 1000 to match future guild growth needs.
    pub const MaxGuildMembers: u32 = 1000;

    /// Maximum size of the elected Guild Council.
    ///
    /// Updated annually by `update_council_from_relayer` after off-chain elections.
    /// Only council members (ARBAD_LEADER and above) may vote on L1 Treasury spends.
    /// Set to 100 — sufficient for the fractal Tumed tier (10,000 members / ~100 squads).
    pub const MaxGuildCouncilMembers: u32 = 100;
}

/// Configure pallet-guilds — Professional DAOs, escrow quests, meritocracy achievements,
/// Academy of Sciences, Fractal Guild Unions, and Relayer-based UUID guild instantiation.
impl pallet_guilds::Config for Runtime {
    /// Use pallet_balances for Quest reward escrow and Academy subscription transfers.
    type Currency = Balances;
    /// Maximum byte-length for Guild names, Achievement titles, and industry tags.
    type MaxNameLength = GuildMaxNameLength;
    /// Maximum byte-length for IPFS CIDs (quest briefs, achievement proofs, guild constitutions).
    type MaxIpfsCidLength = GuildMaxIpfsCidLength;
    /// Maximum number of guilds in a single union-formation call (bounded iteration guard).
    type MaxUnionMembers = GuildMaxUnionMembers;
    /// PalletId used to derive keyless treasury accounts for UUID-keyed guilds.
    /// treasury = PalletId(*b"inm/glds").into_sub_account_truncating(guild_id)
    type PalletId = GuildsPalletId;
    /// Maximum founded members per UUID-keyed guild (bound ≥ 9 constitutional quorum).
    type MaxMembers = MaxGuildMembers;
    /// Maximum elected council size for Decimal Governance (Arbad/Zun/Myangad/Tumed).
    /// Council is replaced annually after off-chain SubWallet-signed elections.
    type MaxCouncilMembers = MaxGuildCouncilMembers;
    type WeightInfo = pallet_guilds::weights::SubstrateWeight<Runtime>;
}

// ─── Inheritance: Heritage Institute & Digital Notary ─────────────────────────
//
// `pallet-inheritance` resolves frozen (reserved) funds from `register_death`.
// It creates professional employment for Notary Guild members, generating GDP
// through the `notarize_will` fee mechanism.
//
// NOTARY VALIDATION:
//   `GuildsBridge::is_valid_notary` checks `GuildMembers` in pallet-guilds.
//   A valid notary must be Professional or Master in the designated Notary Guild
//   (guild ID = NotaryGuildId, configured as 0 for genesis — the first guild
//   created in the system by the Confederation is the Notary Guild).
//
// DECEASED STATUS:
//   `IdentityInheritanceBridge::is_deceased` reads `pallet-inomad-identity`'s
//   `Citizens` storage and checks `CitizenStatus::Deceased`.
//
// FALLBACK:
//   When no notarized Will exists, all reserved funds go to StateTreasury.

parameter_types! {
    /// Guild ID of the official Notary Guild.
    ///
    /// Guild 0 is the first guild created at genesis — by constitutional
    /// convention this is the Guild of Notaries (Гильдия Нотариусов).
    /// Can be updated by Root via governance without pallet upgrade.
    pub const NotaryGuildId: u32 = 0;

    /// Maximum number of heirs allowed in a single Will.
    pub const MaxHeirs: u32 = 20;

    /// State Treasury account — receives un-willed estates.
    ///
    /// In development: set to Bob's AccountId for easy testing.
    /// In production: set to the Confederation Treasury managed by pallet-altan-tax.
    pub StateTreasuryAccount: AccountId = {
        use sp_core::crypto::Ss58Codec;
        // Dev: Bob = 5FHneW46xGXgs5mUiveU4sbTyGBzmstUspZC92UhjJM694ty
        AccountId::from_ss58check("5FHneW46xGXgs5mUiveU4sbTyGBzmstUspZC92UhjJM694ty")
            .unwrap_or_else(|_| AccountId::from([0u8; 32]))
    };
}

/// Runtime bridge: checks notary status by reading pallet-guilds storage.
///
/// A valid Notary must be a `Professional` or `Master` member of the
/// designated Notary Guild (guild ID = `NotaryGuildId`).
pub struct GuildsBridge;

impl pallet_inheritance::GuildsNotaryInterface<AccountId> for GuildsBridge {
    fn is_valid_notary(who: &AccountId) -> bool {
        let guild_id = NotaryGuildId::get();
        match pallet_guilds::GuildMembers::<Runtime>::get(guild_id, who) {
            Some(pallet_guilds::pallet::MemberRole::Professional) => true,
            Some(pallet_guilds::pallet::MemberRole::Master) => true,
            _ => false,
        }
    }
}

/// Runtime bridge: checks citizen deceased status from pallet-inomad-identity.
pub struct IdentityInheritanceBridge;

impl pallet_inheritance::IdentityInheritanceInterface<AccountId> for IdentityInheritanceBridge {
    fn is_deceased(who: &AccountId) -> bool {
        match pallet_inomad_identity::Citizens::<Runtime>::get(who) {
            Some(record) => {
                record.status == pallet_inomad_identity::pallet::CitizenStatus::Deceased
            }
            None => false,
        }
    }
}

/// [SECURITY VECTOR 1] Runtime bridge: queries pallet-bank-operator's CreditContracts
/// storage to sum the outstanding CDP debt for a deceased citizen.
///
/// This bridge is used by pallet-inheritance's `execute_will` to liquidate ghost
/// liabilities (dead debts) before distributing the estate to heirs.  The liquidated
/// amount is burned via `slash_reserved` + imbalance drop, maintaining the hard cap.
pub struct BankDebtBridge;

impl pallet_inheritance::BankDebtInterface<AccountId> for BankDebtBridge {
    fn total_outstanding_debt(who: &AccountId) -> u128 {
        use pallet_bank_operator::pallet::CreditStatus;
        let total = pallet_bank_operator::NextCreditId::<Runtime>::get();
        let mut debt: u128 = 0u128;
        for credit_id in 0..total {
            if let Some(credit) = pallet_bank_operator::CreditContracts::<Runtime>::get(credit_id) {
                if &credit.citizen == who && credit.status == CreditStatus::Active {
                    debt = debt.saturating_add(credit.outstanding);
                }
            }
        }
        debt
    }
}

/// Configure pallet-inheritance — Heritage Institute & Digital Notary.
impl pallet_inheritance::Config for Runtime {
    /// Use pallet_balances for reserving/distributing inheritance funds.
    type Currency = Balances;
    /// Bridge to pallet-guilds for notary credential verification.
    type GuildsChecker = GuildsBridge;
    /// Bridge to pallet-inomad-identity for deceased citizen status check.
    type IdentityChecker = IdentityInheritanceBridge;
    /// [SECURITY VECTOR 1] Bridge to pallet-bank-operator for dead debt liquidation.
    /// Sums all Active credit contracts owned by the deceased and burns the outstanding
    /// amount from their reserved balance before distributing the estate to heirs.
    type BankInterface = BankDebtBridge;
    /// Constitutional fallback: un-willed estates go to the State Treasury.
    type StateTreasury = StateTreasuryAccount;
    /// Maximum 20 heirs per Will — sufficient for large families.
    type MaxHeirs = MaxHeirs;
    type WeightInfo = pallet_inheritance::weights::SubstrateWeight<Runtime>;
}

// ─── Citizen Voice: Голос Гражданина ─────────────────────────────────────────
//
// `pallet-citizen-voice` implements the Citizen Voice Protocol: decentralised
// complaints, suggestions, and whistleblowing with guaranteed delivery and
// anti-spam deposit mechanics. Includes:
//
// [VECTOR 0] Sting Operations (Оперативный Эксперимент) — commit-reveal scheme.
// [VECTOR 2] Ticket Escalation — prevents the accused from self-resolving.
//
// AUTHORITY MAP (who can resolve which ticket target):
//   Guild(id)         → Guild Master of that Guild (via GuildMembers storage).
//   Government(branch)→ Root / Sudo (constitutional authority, via ensure_root).
//   Entity(account)   → The account itself (company owner / official).
//   Escalated ticket  → Root only (regardless of target type).
//
// DEPOSIT:
//   1 ALTAN is reserved on submit_ticket.
//   Returned via unreserve if is_helpful == true.
//   Burned via slash_reserved if is_helpful == false.
//   NOT burned on Escalation — deposit is safe until Root decides.

/// Runtime guild bridge for `pallet-citizen-voice`.
///
/// Checks whether `who` is a `Master` (or higher) of the given `guild_id`
/// by reading `pallet_guilds::GuildMembers` storage directly.
pub struct CitizenVoiceGuildsBridge;

impl pallet_citizen_voice::GuildsInterface<AccountId> for CitizenVoiceGuildsBridge {
    fn is_guild_master(guild_id: u32, who: &AccountId) -> bool {
        matches!(
            pallet_guilds::GuildMembers::<Runtime>::get(guild_id, who),
            Some(pallet_guilds::pallet::MemberRole::Master)
        )
    }
}

/// [VECTOR 0] Runtime bridge: exile a sting target via pallet-inomad-identity.
///
/// When `reveal_and_spring_trap` is called and the commit-reveal validates,
/// this bridge sets the target's `CitizenStatus` to `Exiled` (terminal)
/// without dispatching a full extrinsic — the operation is atomic.
pub struct CitizenVoiceBlackBookBridge;

impl pallet_citizen_voice::BlackBookBridgeInterface<AccountId> for CitizenVoiceBlackBookBridge {
    fn exile_citizen(who: &AccountId) -> frame_support::dispatch::DispatchResult {
        pallet_inomad_identity::Pallet::<Runtime>::do_exile(who)
    }
}

parameter_types! {
    /// Anti-spam deposit for citizen voice tickets: 1 ALTAN.
    pub const CitizenVoiceDeposit: u128 = 1 * UNIT;

    /// [VECTOR 0] Whistleblower reward: 20% of the bribe amount.
    ///
    /// When `reveal_and_spring_trap` is called and succeeds, the whistleblower
    /// receives 20% of the original bribe amount as a reward from the Treasury.
    pub const WhistleblowerReward: u8 = 20;
}

/// Configure pallet-citizen-voice — Голос Гражданина (Citizen Voice Protocol).
impl pallet_citizen_voice::Config for Runtime {
    /// Use pallet_balances for anti-spam deposit reservation, slashing, and rewards.
    type Currency = Balances;
    /// Bridge to pallet-guilds — verifies Guild Master authority for guild-targeted tickets.
    type GuildsChecker = CitizenVoiceGuildsBridge;
    /// [VECTOR 0] Bridge to exile sting targets via pallet-inomad-identity.
    type BlackBookBridge = CitizenVoiceBlackBookBridge;
    /// State Treasury — source of Whistleblower Rewards.
    type StateTreasury = StateTreasuryAccount;
    /// Anti-spam deposit: 1 ALTAN per ticket submission.
    type TicketDeposit = CitizenVoiceDeposit;
    /// [VECTOR 0] Whistleblower reward: 20% of the bribe amount paid from Treasury.
    type WhistleblowerRewardPercent = WhistleblowerReward;
    type WeightInfo = pallet_citizen_voice::weights::SubstrateWeight<Runtime>;
}

// ─── Black Book: Чёрная Книга — Constitutional Bounty Hunting ─────────────────
//
// `pallet-black-book` implements the Black Book of the Confederation. Includes:
//
// [VECTOR 1] Bounty Vesting — anti-laundering lock prevents fake-capture fraud.
// [VECTOR 3] Anti-Tyranny Guard — Academicians and KhuralDelegates are protected;
//            they can only be removed through the constitutional impeachment process.
//
// IDENTITY BRIDGE:
//   `BlackBookIdentityBridge` calls `Pallet::<Runtime>::do_exile` from
//   `pallet-inomad-identity` to set `CitizenStatus::Exiled` (TERMINAL).
//
// TREASURY:
//   Uses the existing `StateTreasuryAccount` constant (same as pallet-inheritance).

/// Identity bridge for `pallet-black-book`.
///
/// Delegates exile to `pallet-inomad-identity::Pallet::do_exile`,
/// which sets `CitizenStatus::Exiled` and strips role/mandate.
pub struct BlackBookIdentityBridge;

impl pallet_black_book::BlackBookIdentityInterface<AccountId> for BlackBookIdentityBridge {
    fn exile_citizen(who: &AccountId) -> frame_support::dispatch::DispatchResult {
        pallet_inomad_identity::Pallet::<Runtime>::do_exile(who)
    }
}

/// [VECTOR 3] Khural Delegate bridge for `pallet-black-book`.
///
/// Checks whether `who` holds a constitutional Khural or Confederation Delegate
/// role (the two highest democratic roles in the fractal hierarchy).
/// Such citizens are constitutionally protected — they cannot be exiled via
/// the Black Book without prior impeachment via the Khural.
pub struct KhuralDelegateBridge;

impl pallet_black_book::KhuralDelegateInterface<AccountId> for KhuralDelegateBridge {
    fn is_active_khural_delegate(who: &AccountId) -> bool {
        use pallet_inomad_identity::pallet::{CitizenRole, CitizenStatus};
        pallet_inomad_identity::Citizens::<Runtime>::get(who)
            .map(|r| {
                // Must be Active AND hold a delegate role.
                r.status == CitizenStatus::Active
                    && matches!(
                        r.role,
                        CitizenRole::KhuralDelegate | CitizenRole::ConfederationDelegate
                    )
            })
            .unwrap_or(false)
    }
}

parameter_types! {
    /// [VECTOR 1] Bounty lock period: ~7 days at 12s/block.
    ///
    /// 50_400 blocks × 12 seconds/block = 604_800 seconds = 7 days.
    /// During this window, Root can cancel the payout if collusion is proven.
    /// After this window, the bounty hunter calls `claim_bounty_payout`.
    pub const BountyLockPeriod: u32 = 50_400;
}

/// Configure pallet-black-book — Чёрная Книга (Constitutional Bounty Hunting).
impl pallet_black_book::Config for Runtime {
    /// Use pallet_balances for confiscation, bounty pool funding, and citizen donations.
    type Currency = Balances;
    /// Bridge to pallet-inomad-identity — sets CitizenStatus::Exiled on condemn.
    type IdentityBridge = BlackBookIdentityBridge;
    /// [VECTOR 3] Academy checker — Academicians require impeachment before exile.
    type AcademyChecker = pallet_guilds::Pallet<Runtime>;
    /// [VECTOR 3] Khural Delegate checker — Delegates require impeachment before exile.
    type KhuralChecker = KhuralDelegateBridge;
    /// State Treasury account — funds initial bounty; receives all confiscated assets.
    type StateTreasury = StateTreasuryAccount;
    /// [VECTOR 1] Anti-laundering lock: 7 days before bounty payout can be claimed.
    type BountyLockPeriod = BountyLockPeriod;
    type WeightInfo = pallet_black_book::weights::SubstrateWeight<Runtime>;
}

// ─── Chancery: Universal Digital Chancellery ──────────────────────────────────
//
// `pallet-chancery` enables multi-party, multi-signature Ricardian contracts
// anchored on-chain via Blake2_256 document hashes pointing to off-chain IPFS.
//
// LIFECYCLE:
//   propose_agreement → PendingSignatures
//   sign_agreement (all parties) → PendingValidation (if validators) or Active
//   validate_agreement (all validators, Professional+ guild rank) → Active
//   raise_dispute (any party, Active only) → Disputed
//
// GUILD INTEGRATION:
//   Validators must hold `Professional` or `Master` rank in guilds 0, 1, or 2
//   (Notary=0, Lawyer=1, Mediator=2 by constitutional convention).

/// Runtime bridge: validates professional status for Chancery validators by
/// checking `pallet_guilds::GuildMembers` in guilds 0 (Notary), 1 (Lawyer),
/// and 2 (Mediator). A Professional or Master in any of these qualifies.
pub struct ChanceryGuildsBridge;

impl pallet_chancery::ChanceryGuildsInterface<AccountId> for ChanceryGuildsBridge {
    fn is_valid_validator(who: &AccountId) -> bool {
        for guild_id in 0u32..3u32 {
            match pallet_guilds::GuildMembers::<Runtime>::get(guild_id, who) {
                Some(pallet_guilds::pallet::MemberRole::Professional) => return true,
                Some(pallet_guilds::pallet::MemberRole::Master) => return true,
                _ => {}
            }
        }
        false
    }
}

parameter_types! {
    /// Maximum parties per Ricardian Agreement (10 covers complex commercial contracts).
    pub const MaxChanceryParties: u32 = 10;
    /// Maximum professional validators per Agreement (5 = 1 lawyer/party + 1 notary).
    pub const MaxChanceryValidators: u32 = 5;
}

/// Configure pallet-chancery — Универсальная Цифровая Канцелярия.
impl pallet_chancery::Config for Runtime {
    type MaxParties = MaxChanceryParties;
    type MaxValidators = MaxChanceryValidators;
    type GuildsChecker = ChanceryGuildsBridge;
    type WeightInfo = pallet_chancery::weights::SubstrateWeight<Runtime>;
}

// ─── Central Bank: Sole Constitutional Emitter ────────────────────────────────
//
// `pallet-central-bank` is the Banking Branch of Power — the fourth constitutionally
// independent branch of the Altan Republic. It holds the EXCLUSIVE right to issue
// (mint) new ALTAN tokens via `mint_to_operator`.
//
// ПРАВИЛО НЕЗАВИСИМОСТИ (ФРС Model):
//   - The Central Bank has NO spending capability (no transfer extrinsic).
//   - It cannot freeze citizens (Judicial authority only).
//   - It can ONLY mint to pre-registered licensed operators.
//   - `BankingOrigin` is constitutionally separate from the Khural and Executive.
//
// LICENSED OPERATOR REGISTRY (`LicensedOperators`):
//   - Genesis: Bank_of_Siberia_Main_Reserve is the first licensed operator.
//   - New operators added/removed exclusively by `BankingOrigin`.
//
// EMISSION PATH:
//   BankingOrigin → mint_to_operator(amount, Bank_of_Siberia)
//     → Currency::deposit_creating(&Bank_of_Siberia, amount)
//       → TotalIssuance ↑  |  Bank_of_Siberia.balance ↑
//
// BLOCKED IN BaseCallFilter:
//   `Balances::force_set_balance` and `Balances::force_transfer` are always
//   blocked — even for Root. New ALTAN can ONLY be created via this pallet.

/// Constitutional Credit Epoch Limit.
///
/// 18.9 trillion ALTAN × 10^12 planks per ALTAN = 18_900_000_000_000_000_000_000_000 planks.
/// When `total_issued` in the current epoch reaches this value, `request_credit`
/// will auto-transition to the next epoch using `NextEpochKeyRate`.
///
/// This constant is IMMUTABLE in compiled WASM — no governance vote or Sudo key
/// can change it without a full constitutional runtime upgrade.
pub const CREDIT_EPOCH_LIMIT_VALUE: u128 = 18_900_000_000_000_000_000_000_000;

parameter_types! {
    /// 18.9T ALTAN expressed in planks (1 ALTAN = 10^12 planks).
    ///
    /// Maximum credit issuable in a single macroeconomic epoch before
    /// auto-rollover triggers. Wired as a compile-time constant — immutable.
    pub const CreditEpochLimit: u128 = CREDIT_EPOCH_LIMIT_VALUE;

    /// Constitutional optimal key rate: 8.5% (850 bps).
    pub const OptimalKeyRateConst: u32 = 850;
    
    /// Constitutional maximum protective rate: 50% (5000 bps).
    /// Hardcoded penalty for monopolizing the credit pool.
    pub const MaxProtectiveRateConst: u32 = 5000;
    
    /// Constitutional optimal utilization: 80%.
    pub const OptimalUtilizationConst: u32 = 80;
}

/// Central Bank sovereign keyless account — holds 2.1T ALTAN M0 at genesis.
///
/// Derived from the same 7-of-10 multisig formula as `genesis_config_presets.rs`
/// using the seed `"CENTRAL_BANK"`. No private key exists.
/// The pallet uses this to transfer funds when `trigger_genesis_distribution` is called.
fn derive_cb_sovereign_account() -> AccountId {
    use codec::Encode;
    let entity_name = b"CENTRAL_BANK";
    let mut signatories: alloc::vec::Vec<AccountId> = alloc::vec::Vec::new();
    for i in 1..=10u8 {
        let mut seed = [0u8; 32];
        let len = entity_name.len().min(30);
        seed[..len].copy_from_slice(&entity_name[..len]);
        seed[30] = b'_';
        seed[31] = i;
        signatories.push(AccountId::from(seed));
    }
    signatories.sort();
    let threshold: u16 = 7;
    let entropy = (b"modlpy/utilisig" as &[u8], signatories, threshold).encode();
    let hash = sp_core::hashing::blake2_256(&entropy);
    AccountId::from(hash)
}

parameter_types! {
    /// **CONSTITUTIONAL** Central Bank sovereign account.
    ///
    /// Keyless 7-of-10 multisig from seed "CENTRAL_BANK" — same derivation as genesis.
    /// Holds the full 2.1T ALTAN M0 supply at genesis block #0.
    pub CentralBankSovereignAccount: AccountId = derive_cb_sovereign_account();
}

/// Configure pallet-central-bank — Banking Branch: sole constitutional token emitter.
///
/// ## BankingOrigin — Fourth Branch of Power
///
/// `frame_system::EnsureRoot` is used in development (single Sudo key acts as the
/// Banking Branch for convenience). In production, this MUST be replaced with a
/// threshold `Banking Collective` of constitutionally appointed Central Bank governors
/// — an origin independent from the Sudo key, the Khural, and the Executive.
///
/// The `BankingAuthority` marker type (defined in `pallet-constitution`) documents
/// this constitutional principle at the type level.
// ─── Central Bank Identity Integration Removed ────────────────────────────────

parameter_types! {
    /// Organization ID for Central Bank (10-100-1000-10000 hierarchy)
    pub const CBOrgId: u32 = 1;
    /// Organization ID for Bank of Siberia (10-100-1000-10000 hierarchy)
    pub const BoSOrgId: u32 = 2;
}

impl pallet_central_bank::Config for Runtime {
    /// The native ALTAN currency. `deposit_creating` is the ONLY minting path.
    /// `BaseCallFilter` ensures no other pallet can call `force_set_balance`.
    type Currency = Balances;

    /// [BANKING BRANCH — PRODUCTION BOOTSTRAP]
    ///
    /// **Phase 1 (Genesis → First Election):** `EnsureRootOrBankingCouncil`
    ///   Accepts the Creator Sudo key OR any elected Banking BranchCouncil member.
    ///   Allows bootstrapping emission during the pre-election period.
    ///
    /// **Phase 2 (Post First Election):** Org Roles SBT integration.
    ///   The Central Bank is managed as an Organization (Org ID: 1) within the
    ///   unified 10-100-1000-10000 hierarchy.
    type BankingOrigin = crate::origins::EnsureOrgOfficer<CBOrgId>;

    /// **CONSTITUTIONAL** Maximum credit per epoch: 18.9 trillion ALTAN.
    ///
    /// When `total_issued` in the current epoch reaches this value, `request_credit`
    /// atomically transitions to the next epoch using `NextEpochKeyRate`.
    /// Encoded as a compile-time constant — no vote, no Sudo, no extrinsic can mutate it.
    type CreditEpochLimit = CreditEpochLimit;
    type CreatorAccount = CreatorSudoAccount;

    /// **CONSTITUTIONAL** The Central Bank's own keyless sovereign account.
    ///
    /// Holds 2.1T ALTAN at genesis. `trigger_genesis_distribution` transfers from
    /// this account to the 167 constitutional proxy accounts.
    type CentralBankAccountId = CentralBankSovereignAccount;
    type WeightInfo = pallet_central_bank::weights::SubstrateWeight<Runtime>;

    /// **CONSTITUTIONAL TWO-PHASE CURVE**
    type OptimalKeyRate = OptimalKeyRateConst;
    type MaxProtectiveRate = MaxProtectiveRateConst;
    type OptimalUtilization = OptimalUtilizationConst;
}

// ─── Bank of Siberia: Credit Operator & Escrow Gateway ────────────────────────
//
// `pallet-bank-of-siberia` is the credit-operations tier of the Altan Network.
// It manages citizen bank accounts, collateral-backed loan requests, and
// escrow contracts. It NEVER calls `Currency::deposit_creating`.
//
// CONSTITUTIONAL CONSTRAINT:
//   - Primary ALTAN emission is the exclusive mandate of `pallet-central-bank`.
//   - `LockableCurrency` is required here because escrow and loan collateral
//     mechanics will use `Currency::set_lock` / `Currency::remove_lock`
//     in the next sprint when fund-locking logic is wired in.
//
// PHYSICS OF THE NETWORK (immutable constants):
//   CrossTransferFee = 0.03% = Perbill::from_parts(300_000)
//   StateTaxRate     = 10%   = Perbill::from_percent(10)
//
// These constants are IMMUTABLE — they are wired as `parameter_types!` backed
// by Perbill values.  The Khural cannot modify them.  Only a full WASM runtime
// upgrade (which itself requires referendum) can change them.

parameter_types! {
    /// Bank of Siberia sovereign treasury PalletId.
    ///
    /// Funds locked for Time Deposits, Escrow, and future collateral
    /// are physically held in the account derived from this ID.
    /// The account has no private key — controlled exclusively by pallet logic.
    pub const BankOfSiberiaPalletId: frame_support::PalletId = frame_support::PalletId(*b"bos/depo");

    // ── Constitutional Constants (Physics of the Network) ─────────────────────
    //
    // 0.03% cross-transfer fee = 3 / 10_000 = 300_000 parts per billion.
    // These are Perbill constants — they live in WASM code, not storage.
    // NO extrinsic, NO governance vote, NO Sudo key can mutate them at runtime.
    // Only a constitutional referendum + runtime upgrade can change these values.

    /// **Constitutional Cross-Transfer Fee: 0.03%**
    ///
    /// `Perbill::from_parts(300_000)` = 300_000 / 1_000_000_000 = 0.03%
    ///
    /// Applied on every cross-border ALTAN transfer routed through
    /// `pallet-altan-tax::transfer_with_fee`. This is the "gravity" of
    /// the Republic — immutable, non-negotiable.
    pub const ConstitutionalCrossTransferFee: Perbill = Perbill::from_parts(300_000);

    /// **Constitutional State Tax Rate: 10%**
    ///
    /// `Perbill::from_percent(10)` = 100_000_000 / 1_000_000_000 = 10%
    ///
    /// ANNUAL CORPORATE PROFIT TAX. NOT per-transaction.
    /// Filing period: January 1 — April 15 (enforced on-chain via `pallet_timestamp`).
    /// Distribution:
    ///   - 3/10 of tax (= 3% of profit) → Confederation Treasury
    ///   - 7/10 of tax (= 7% of profit) → Regional Treasury (83 regions)
    /// The Khural cannot lower or raise this — only a full WASM upgrade can.
    pub const ConstitutionalStateTaxRate: Perbill = Perbill::from_percent(10);
}

/// Configure pallet-bank-of-siberia — credit operator, financial oracle, escrow gateway.
impl pallet_bank_of_siberia::Config for Runtime {
    /// The native ALTAN currency. Must implement `LockableCurrency` to support
    /// future collateral locking for loans and escrow contracts.
    /// `pallet_balances` satisfies both `Currency` and `LockableCurrency`.
    type Currency = Balances;

    /// Sovereign treasury PalletId — derives the keyless treasury account.
    /// Time Deposits, Escrow funds, and future collateral are held here.
    type PalletId = BankOfSiberiaPalletId;

    /// **Banking Branch origin** for BOS privileged operations:
    /// - `approve_loan`: disburses loan funds from BOS treasury to borrower.
    /// - `fund_treasury`: tops up BOS treasury for interest payment reserves.
    ///
    /// Uses the unified 10-100-1000-10000 management system via `pallet-org-roles`.
    /// Bank of Siberia is Organization #2. Officers and Root of this Org can call these.
    type BankingOrigin = crate::origins::EnsureOrgOfficer<BoSOrgId>;

    /// **CONSTITUTIONAL** Cross-transfer fee: 0.03% (immutable — Khural cannot change).
    ///
    /// Wired as a compile-time Perbill constant. Any attempt to use mutable
    /// storage for this value would be a constitutional violation.
    type CrossTransferFee = ConstitutionalCrossTransferFee;

    /// **CONSTITUTIONAL** State income tax: 10% (immutable — Khural cannot change).
    ///
    /// Wired as a compile-time Perbill constant. This is the gravitational
    /// constant of the Altan economy — encoded in WASM, not alterable by any
    /// governance vote.
    type StateTaxRate = ConstitutionalStateTaxRate;
    type WeightInfo = pallet_bank_of_siberia::weights::SubstrateWeight<Runtime>;
}

// ─── Constitution: Layer 0 Bill of Rights and Habeas Corpus ──────────────────
//
// `pallet-constitution` enforces Article III (Habeas Corpus) every block.
// The `JudicialHabeasBridge` connects the constitutional timer to the judicial
// courts and identity systems:
//
//   has_verdict(who)   → checks if a Closed case exists for `who` as defendant
//                        in `pallet-judicial-courts::Cases` storage.
//   release_citizen(who) → calls `do_unfreeze_citizen` in pallet-inomad-identity,
//                          restoring CitizenStatus::Active when the deadline passes.
//
// CONSTITUTIONAL GUARANTEE:
//   If no verdict is entered by `max_lockup_block`, on_initialize fires this hook
//   and unconditionally releases the citizen — no investigating authority can
//   indefinitely detain a citizen without a binding court verdict.

/// Runtime judicial bridge for pallet-constitution's Habeas Corpus enforcement.
///
/// Sprint L1-08: updated for the new Judicial Engine (`pallet-judicial-courts` rewrite).
///
/// - `has_verdict`: Delegates to `pallet_judicial_courts::Pallet::defendant_has_verdict`,
///   which returns `true` if any `CourtCase` for `who` has moved beyond `Open` status
///   (i.e., a verdict has been issued — `Guilty`, `Acquitted`, or `Executed`).
///
/// - `release_citizen`: Calls `do_unfreeze_citizen` on `pallet-inomad-identity`
///   to set `CitizenStatus::Active`, unconditionally releasing the pre-trial freeze.
///   This is the Article III enforcement: the citizen is freed without the court's
///   intervention when the constitutional deadline has elapsed.
pub struct JudicialHabeasBridge;

impl pallet_constitution::HabeasCorpusInterface<AccountId> for JudicialHabeasBridge {
    /// Returns `true` if any court case for `who` (as defendant) has a verdict.
    ///
    /// Delegates to the `defendant_has_verdict` helper on `pallet-judicial-courts`.
    /// A non-Open case means the courts have exercised jurisdiction — no auto-release.
    fn has_verdict(who: &AccountId) -> bool {
        pallet_judicial_courts::Pallet::<Runtime>::defendant_has_verdict(who)
    }

    /// Unconditionally release `who` from the judicial pre-trial freeze.
    ///
    /// Called by `on_initialize` when the Habeas Corpus deadline passes without
    /// a verdict. Restores `CitizenStatus::Active`.
    fn release_citizen(who: &AccountId) -> frame_support::dispatch::DispatchResult {
        pallet_inomad_identity::Pallet::<Runtime>::do_unfreeze_citizen(who)
    }
}

parameter_types! {
    /// Maximum concurrent Habeas Corpus timers.
    ///
    /// Limits the number of simultaneously active judicial freezes to prevent
    /// unbounded on_initialize iteration. In practice equals the number of
    /// ongoing pre-trial detentions. 1_024 is a safe upper bound.
    pub const MaxConcurrentHabeasTimers: u32 = 1_024;

    /// Maximum description length for Land Registry parcels (512 bytes).
    pub const MaxParcelDescriptionLen: u32 = 512;
}

/// Configure pallet-constitution — Layer 0 constitutional anchor and Habeas Corpus enforcement.
///
/// ## HabeasCorpusHook — Sprint L1-07 Judicial Bridge
///
/// `JudicialHabeasBridge` wires the constitutional timer to the court system:
/// - `has_verdict` checks `pallet-judicial-courts::Cases` for a Closed verdict.
/// - `release_citizen` calls `pallet-inomad-identity::do_unfreeze_citizen`.
///
/// The `on_initialize` hook now fully enforces Article III — a citizen held beyond
/// `max_lockup_block` without a binding verdict is automatically released.
impl pallet_constitution::Config for Runtime {
    type MaxConcurrentTimers = MaxConcurrentHabeasTimers;
    /// [L1-07] Judicial Habeas Corpus bridge: wires courts ↔ constitution ↔ identity.
    type HabeasCorpusHook = JudicialHabeasBridge;
    type WeightInfo = pallet_constitution::weights::SubstrateWeight<Runtime>;
}

/// Configure pallet-land-registry — sovereign land cadastre with Non-Alienation Law.
///
/// ## Non-Alienation Law (Article V)
///
/// Tightly coupled to `pallet-inomad-identity` via the `Config: pallet_inomad_identity::Config`
/// bound. The `transfer_land` extrinsic reads `Citizens<T>` directly to check
/// `CitizenshipStatus` of the buyer — no bridge needed.
impl pallet_land_registry::Config for Runtime {
    type MaxDescriptionLen = MaxParcelDescriptionLen;
}

// ─── Organization: Corporate SBTs, Tax Filing, Punishment Engine ─────────────
//
// `pallet-organization` implements the Altan Corporate Registry:
//   - Organizational SBTs (legal entities with bank_account_id and hq_region)
//   - CrewMembers (Directors and Employees linked by DoubleMap)
//   - MANUAL tax filing via `file_tax_return` extrinsic (Director only)
//   - Automatic Punishment Engine via `on_initialize` + `report_tax_evasion`:
//       1. OrgStatus → Delinquent (reputation slash)
//       2. +1% penalty debt per PenaltyAccrualPeriodBlocks
//       3. All Directors → CitizenStatus::Frozen (digital liberty deprivation)
//
// CONSTITUTIONAL RULE: NO auto-debit. Tax payment is always a citizen action.
// Consequences of non-payment, however, are fully automated and inescapable.

/// Runtime bridge: freeze a Director's citizen account on tax evasion.
///
/// Delegates to `pallet_inomad_identity::Pallet::<Runtime>::do_freeze_citizen`.
/// Same pattern as `IdentityBridge` used by `pallet-judicial-courts`.
pub struct OrgIdentityBridge;

impl pallet_organization::OrgIdentityInterface<AccountId> for OrgIdentityBridge {
    fn freeze_citizen(who: &AccountId) -> frame_support::dispatch::DispatchResult {
        pallet_inomad_identity::Pallet::<Runtime>::do_freeze_citizen(who)
    }
}

parameter_types! {
    /// Tax period: ~1 year at 6-second blocks.
    ///
    /// 5_256_000 blocks × 6 seconds = 31_536_000 seconds = 365 days.
    /// An organization must file its tax return before this many blocks elapse
    /// since `last_tax_period_paid`, or the Punishment Engine activates.
    pub const OrgTaxPeriodBlocks: u32 = 5_256_000;

    /// Penalty accrual cycle: ~10.5 days at 6-second blocks.
    ///
    /// 151_200 blocks × 6s = 907_200s ≈ 10.5 days.
    /// Every this many blocks while Delinquent, the penalty debt grows by +1%.
    pub const OrgPenaltyAccrualBlocks: u32 = 151_200;

    /// Base penalty applied at first delinquency: 100 ALTAN.
    pub const OrgBasePenaltyAmount: u128 = 100 * UNIT;

    /// Auditor reward for `report_tax_evasion`: 10 ALTAN from the treasury.
    pub const OrgAuditorRewardAmount: u128 = 10 * UNIT;

    /// Maximum orgs scanned per block in `on_initialize` (weight bound).
    pub const OrgMaxOrgsPerBlock: u32 = 20;

    /// Maximum Directors per org (bounds the freeze loop in Punishment Engine).
    pub const OrgMaxDirectorsPerOrg: u32 = 20;

    /// Maximum org name length: 64 UTF-8 bytes.
    pub const OrgMaxNameLength: u32 = 64;

    /// Constitutional minimum tax amount per filing: 20 ALTAN.
    ///
    /// Even if declared profit is zero, the Director must pay at least this amount.
    /// Anti-abuse floor. Cannot submit a zero-tax declaration.
    pub const OrgMinTaxAmount: u128 = 20 * UNIT;

    /// ****CONSTITUTIONAL**** Activation deposit per founding Director: 10 ALTAN.
    ///
    /// Every founding Director must transfer this amount to the org's deterministic
    /// keyless account (`derive_org_account(org_id)`) to activate the organization.
    /// Until ALL required founders have deposited, the org remains `Pending`.
    pub const OrgActivationDeposit: u128 = 10 * UNIT;

    /// Maximum number of founders allowed to co-activate an organization.
    ///
    /// Bounds `OrgFounderDeposits` BoundedVec. Constitutional max = 20.
    pub const OrgMaxFounders: u32 = 20;

    /// Confederation Treasury address — receives 3/10 of all tax payments.
    ///
    /// Derived deterministically from the `CONFEDERATION_BUDGET` seed label,
    /// matching the genesis_config_presets.rs factory pattern.
    /// This account has no private key — it is controlled exclusively by
    /// pallet-khural-governance via the Confederate Khural voting process.
    pub ConfederationTreasuryAddr: AccountId = {
        let mut seed = [b'_'; 32];
        let label = b"CONFEDERATION_BUDGET";
        seed[..label.len()].copy_from_slice(label);
        AccountId::from(seed)
    };
}

/// Configure pallet-organization — Corporate SBTs, manual tax filing, and automatic punishment engine.
impl pallet_organization::Config for Runtime {
    /// Use pallet_balances for tax payment transfers and auditor reward distribution.
    type Currency = Balances;
    /// Cross-pallet freeze: sets Director CitizenStatus → Frozen on tax evasion.
    type IdentityBridge = OrgIdentityBridge;
    /// **CONSTITUTIONAL** Tax period: ~1 year. Must file before this period elapses.
    type TaxPeriodBlocks = OrgTaxPeriodBlocks;
    /// Penalty accrual: +1% of pending debt every ~10.5 days while Delinquent.
    type PenaltyAccrualPeriodBlocks = OrgPenaltyAccrualBlocks;
    /// Base penalty added to debt at first delinquency marking: 100 ALTAN.
    type BasePenaltyAmount = OrgBasePenaltyAmount;
    /// Reward paid to auditor citizens who call `report_tax_evasion`: 10 ALTAN.
    type AuditorRewardAmount = OrgAuditorRewardAmount;
    /// Regional treasury account — receives 7/10 of tax payments.
    ///
    /// Dev: reuses StateTreasuryAccount (Bob). Production: wire to altan-tax region treasury.
    type RegionTreasuryAccount = StateTreasuryAccount;
    /// **CONSTITUTIONAL** Confederation treasury — receives 3/10 of tax payments.
    ///
    /// The `CONFEDERATION_BUDGET` account derived deterministically in genesis.
    type ConfederationTreasuryAccount = ConfederationTreasuryAddr;
    /// **CONSTITUTIONAL** State tax rate: 10% of declared annual profit.
    type StateTaxRate = ConstitutionalStateTaxRate;
    /// **CONSTITUTIONAL** Minimum tax: 20 ALTAN per filing.
    type MinTaxAmount = OrgMinTaxAmount;
    /// ****CONSTITUTIONAL**** Activation deposit per founder: 10 ALTAN.
    /// Deposited to the org's keyless account to activate it from `Pending` → `Active`.
    type OrgActivationDeposit = OrgActivationDeposit;
    /// Maximum founders allowed to co-activate an org (bounds `OrgFounderDeposits` vec).
    type MaxFounders = OrgMaxFounders;
    /// On-chain timestamp for filing period enforcement (Jan 1 — Apr 15).
    type TimeProvider = Timestamp;
    /// Bounded iteration guard for `on_initialize` scanner: 20 orgs/block.
    type MaxOrgsPerBlock = OrgMaxOrgsPerBlock;
    type MaxNameLength = OrgMaxNameLength;
    type MaxDirectorsPerOrg = OrgMaxDirectorsPerOrg;
    /// [ZK BRIDGE] Delegated to pallet-shielded-vaults.
    ///
    /// Enables `file_tax_return_zk`: destroys a guild's shielded commitment and
    /// materialises the declared tax amount on the Regional Treasury — without
    /// revealing the guild's internal shielded balance.
    type ShieldedVaultsBridge = pallet_shielded_vaults::Pallet<Runtime>;
    type WeightInfo = pallet_organization::weights::SubstrateWeight<Runtime>;
}

// ─── Shielded Vaults: Asymmetric Transparency ─────────────────────────────────
//
// ПРИНЦИП АСИММЕТРИЧНОЙ ПРОЗРАЧНОСТИ:
//   State accounts (Central Bank, Bank of Siberia, all Treasuries) → 100% PUBLIC
//   Citizens & Guilds → right to commercial privacy via shielded commitments
//
//  reconstructs canonical state account IDs deterministically
// using the same seed functions as genesis_config_presets.rs. This check is
// enforced at the pallet boundary — no origin, governance, or Sudo can bypass it.

/// Constitutional firewall for pallet-shielded-vaults.
pub struct StateAccountGuard;

impl StateAccountGuard {
    fn bank_of_siberia_main_reserve() -> AccountId {
        let mut seed = [b'_'; 32];
        let label = b"BANK_OF_SIBERIA_MASTER";
        seed[..label.len()].copy_from_slice(label);
        AccountId::from(seed)
    }

    fn confederation_treasury() -> AccountId {
        let mut seed = [b'_'; 32];
        let label = b"CONFEDERATION_BUDGET";
        seed[..label.len()].copy_from_slice(label);
        AccountId::from(seed)
    }

    fn federal_sweep() -> AccountId {
        let mut seed = [b'_'; 32];
        let label = b"FEDERAL_SWEEP_ACCOUNT";
        seed[..label.len()].copy_from_slice(label);
        AccountId::from(seed)
    }

    fn regional_citizen_fund(region: u16) -> AccountId {
        let mut seed = [0u8; 32];
        let idx = region.to_be_bytes();
        seed[0] = idx[0];
        seed[1] = idx[1];
        seed[2] = b'R';
        AccountId::from(seed)
    }

    fn regional_gov_treasury(region: u16) -> AccountId {
        let mut seed = [0u8; 32];
        let idx = region.to_be_bytes();
        seed[0] = idx[0];
        seed[1] = idx[1];
        seed[2] = b'S';
        AccountId::from(seed)
    }
}

impl pallet_shielded_vaults::StateAccountChecker<AccountId> for StateAccountGuard {
    fn is_state_account(who: &AccountId) -> bool {
        if *who == Self::bank_of_siberia_main_reserve() {
            return true;
        }
        if *who == Self::confederation_treasury() {
            return true;
        }
        if *who == Self::federal_sweep() {
            return true;
        }
        for r in 0u16..=83 {
            if *who == Self::regional_citizen_fund(r) {
                return true;
            }
            if *who == Self::regional_gov_treasury(r) {
                return true;
            }
        }
        false
    }
}

/// **CONSTITUTIONAL** Runtime resolver: looks up a regional treasury account
/// for a given org from pallet-organization's OrgRecord (reads `region_id`).
/// Maps region_id to the corresponding nation treasury in pallet-altan-tax.
pub struct OrgRegionResolverImpl;

impl pallet_shielded_vaults::OrgRegionResolverTrait<AccountId> for OrgRegionResolverImpl {
    fn regional_treasury_for(org_id: u32) -> Option<AccountId> {
        // Read the org's registered region from pallet-organization.
        let org = pallet_organization::Organizations::<Runtime>::get(org_id)?;
        let region_id = org.hq_region as usize;
        // Read the nation treasuries list from pallet-altan-tax.
        let treasuries = pallet_altan_tax::NationTreasuries::<Runtime>::get();
        treasuries.get(region_id).cloned()
    }
}

impl pallet_shielded_vaults::Config for Runtime {
    type Currency = Balances;
    type TransparentStateGuard = StateAccountGuard;
    /// Confederation Treasury — receives 3/10 of ZK org tax payments.
    type ConfederationTreasury = ConfederationTreasuryAddr;
    /// Org region resolver — routes 7/10 of ZK tax to the correct Region Treasury.
    type OrgRegionResolver = OrgRegionResolverImpl;
    type WeightInfo = pallet_shielded_vaults::weights::SubstrateWeight<Runtime>;
}

parameter_types! {
    /// Maximum number of messages per diplomatic/legalization channel per country.
    pub const MaxChannelMessages: u32 = 1000;
    /// Maximum council members per foreign state diplomatic mission.
    ///
    /// Mirrors `MaxDirectorsPerOrg` (20). Each state appoints up to 20 council
    /// members; council is managed by the accredited Special Representative.
    pub const MaxDiplomaticCouncilMembers: u32 = 20;
}

impl pallet_foreign_affairs::Config for Runtime {
    type Currency = Balances;
    type MaxChannelMessages = MaxChannelMessages;
    /// Max 20 council members per foreign state (matches org director limit).
    type MaxDiplomaticCouncil = MaxDiplomaticCouncilMembers;
    type WeightInfo = pallet_foreign_affairs::weights::SubstrateWeight<Runtime>;
}

// ─── Decimal DAO Core: Generic Council Governance & Keyless Treasuries ─────────
//
// `pallet-decimal-dao` is the generalized governance layer for all INOMAD entities:
//   - Legislative Branch (Khural committees)
//   - Executive Branch (Ministries and Agencies)
//   - Judicial Branch (Court Councils)
//   - Audit Branch
//   - Enterprises and Guilds
//
// Each org is identified by Blake2-256(off-chain UUID) → [u8; 32].
// The Relayer calls `sync_council` annually after SubWallet-signed elections.
// Keyless treasury: PalletId(*b"inm/ddao").into_sub_account_truncating(org_id)
//
// CONSTITUTIONAL COMPLIANCE (AGENTS.md):
//   - Treasury transfers: KeepAlive (Existential Deposit preserved).
//   - No force_set_balance / force_transfer.
//   - Council voting: >50% strict majority required.
//   - PalletId is distinct from "inm/glds" (pallet-guilds) — no key collision.

parameter_types! {
    /// PalletId for pallet-decimal-dao keyless treasury derivation.
    ///
    /// `treasury = PalletId(*b"inm/ddao").into_sub_account_truncating(org_id)`
    ///
    /// Distinct from GuildsPalletId (*b"inm/glds") — no sub-account collision.
    pub const DecimalDaoPalletId: frame_support::PalletId = frame_support::PalletId(*b"inm/ddao");

    /// Maximum council members per org.
    ///
    /// 100 covers the Tumed apex tier (10,000 members / ~100 squads).
    /// Reuses `MaxGuildCouncilMembers` value for consistency.
    pub const DecimalDaoMaxCouncilMembers: u32 = 100;
}

/// Configure pallet-decimal-dao — Generic Decimal DAO Council Governance.
impl pallet_decimal_dao::Config for Runtime {
    /// Use pallet_balances for council-gated treasury proposal execution.
    type Currency = Balances;
    /// PalletId for keyless treasury derivation (distinct from pallet-guilds).
    type PalletId = DecimalDaoPalletId;
    /// Maximum council size: 100 (Tumed apex tier).
    type MaxCouncilMembers = DecimalDaoMaxCouncilMembers;
    type WeightInfo = pallet_decimal_dao::weights::SubstrateWeight<Runtime>;
}

// ─────────────────────────────────────────────────────────────────────────────
// pallet-licensing — Constitutional Licensing & Land Fund
// ─────────────────────────────────────────────────────────────────────────────

parameter_types! {
    /// Standard national-Khural voting period for license applications: 7 days.
    ///
    /// Constitutional block math: 14,400 blocks/day × 7 = 100,800 blocks.
    pub const LicenseVotingPeriod: u32 = 100_800;

    /// Minimum YES votes required for a license application to be approved.
    ///
    /// 3 delegates ensures quorum across even small indigenous nations while
    /// preventing single-delegate capture. Confederal 2/3 quorum is checked
    /// separately inside the pallet via `requires_supermajority()`.
    pub const MinLicenseQuorum: u32 = 3;

    /// Anti-spam deposit reserved from the submitting ministry.
    ///
    /// 10 ALTAN = `ExpertBillDeposit` constant from AGENTS.md §3.3
    /// Returned to submitter on approval; slashed into Confederation treasury on rejection.
    pub const LicenseApplicationDeposit: Balance = 10 * UNIT;
}

/// Configure pallet-licensing — Executive/Khural Licensing Pipeline.
///
/// # Constitutional Design
///
/// ```text
/// Executive (AuthorizedMinistry) → submit_license_application()
///         ↓ 7-day Khural vote (LicenseVotingPeriod)
/// Legislative (KhuralDelegate / ConfederationDelegate)
///         ↓ on_initialize auto-enact
/// [License: Active, max 10 years]
///         ↓
/// Judicial → revoke_license() via execute_verdict()
/// ```
///
/// # Constitutional Constraints
///
/// - Max duration: **10 years** (`BLOCKS_PER_YEAR * 10` — hard-coded in pallet)
/// - Renewal: via new application + Khural vote
/// - Revocation: Root-proxied judicial verdict ONLY
/// - Weapons / Nuclear: Confederate Khural + 2/3 quorum
/// - Indigenous Land Veto: `indigenous_veto_land_allocation` extrinsic
impl pallet_licensing::Config for Runtime {
    /// Use pallet_balances for anti-spam deposit reserve/slash.
    type Currency = Balances;
    /// 7-day voting period (100,800 blocks at 6s/block).
    type LicenseVotingPeriod = LicenseVotingPeriod;
    /// Minimum 3 YES votes to meet quorum (national Khural).
    type MinLicenseQuorum = MinLicenseQuorum;
    /// 10 ALTAN application deposit (matches ExpertBillDeposit in §3.3 AGENTS.md).
    type ApplicationDeposit = LicenseApplicationDeposit;
}

// =============================================================================
// pallet-inomad-elections — Bottom-Up Hierarchical Meritocracy (Peak Governance)
// =============================================================================
//
// Constitutional hierarchy:
//   Arbad (10) → Zun (100) → Myangad (1,000) → Tumed (10,000)
//
// Peak Governance split:
//   Tumed leaders elect → Executive, Judicial, Banking Branch Councils (9 seats)
//                       → Khural Chairman (Legislative — bound to Arbad ID)
//
// Security parameters:
//   MaxCandidates       = 1,024  — upper bound for decimal-hierarchy elections
//   MaxBranchCandidates =   100  — H-1: prevents DOS on peak-governance voting
//                                  Must be ≥ 9 (COUNCIL_SIZE) and << u32::MAX.
parameter_types! {
    /// Maximum allowed candidates in a decimal-hierarchy election (Arbad/Zun/Myangad/Tumed).
    /// 1,024 comfortably covers any single-Arbad to Tumed-level election.
    pub const ElectionMaxCandidates: u32 = 1_024;

    /// H-1 DOS guard: maximum distinct candidates in a Branch Council election.
    ///
    /// A Tumed-level election has at most ~10,000 eligible voters; limiting
    /// distinct candidates to 100 prevents unbounded `BranchVoteCounts` growth
    /// while still allowing meaningful competitive elections.
    pub const ElectionMaxBranchCandidates: u32 = 100;

    // ── Constitutional Decimal Hierarchy Size Constants ───────────────────────
    //
    // These are immutable physics of the Siberian Confederation.
    // They cannot be changed by any extrinsic, Sudo, or governance vote —
    // only a full WASM runtime upgrade (referendum) can modify them.

    /// Арбад (Arbad) — minimum CITIZENS required before its leader can be promoted.
    /// = 10 (9 citizens + 1 elected Arbad Leader).
    pub const ElectionMinArbadSize: u32 = 10;

    /// Зун (Zun) — constitutional TOTAL CITIZEN count (documentation constant).
    /// = 100 (10 Arbads × 10 citizens each).
    /// Enforced implicitly: each of the 10 required Arbad Leaders proved their Arbad was full.
    pub const ElectionMinZunSize: u32 = 100;

    /// Зун (Zun) — minimum ELECTED ARBAD LEADERS in the Zun zone.
    /// = 10 (one from each of the 10 Arbads).
    /// This is the on-chain check in promote_leader(Myangad).
    pub const ElectionMinZunLeaders: u32 = 10;

    /// Мянгад (Myangad) — constitutional TOTAL CITIZEN count (documentation constant).
    /// = 1 000 (10 Zuns × 100 citizens each).
    pub const ElectionMinMyangadSize: u32 = 1_000;

    /// Мянгад (Myangad) — minimum ELECTED ZUN LEADERS in the Myangad zone.
    /// = 10 (one from each of the 10 Zuns).
    /// This is the on-chain check in promote_leader(Tumed).
    pub const ElectionMinMyangadLeaders: u32 = 10;
}

/// Production implementation of the `IsIndigenousCitizen` hook.
///
/// Delegates to `pallet-inomad-identity` to verify the citizen's
/// `CitizenshipStatus`. Returns `true` if the citizen is registered as
/// `CitizenshipStatus::Indigenous` (father OR mother is a documented
/// indigenous person of Siberia — Jus Sanguinis rule).
///
/// This is used by `elect_khural_chairman` to enforce the constitutional
/// mandate that the Legislative branch (Хурал) is exclusively indigenous.
pub struct IdentityIndigenousBridge;

impl pallet_inomad_elections::IsIndigenousCitizen<AccountId> for IdentityIndigenousBridge {
    fn is_indigenous(who: &AccountId) -> bool {
        use pallet_inomad_identity::CitizenshipStatus;
        pallet_inomad_identity::Citizens::<Runtime>::get(who)
            .map(|citizen| citizen.citizenship_status == CitizenshipStatus::Indigenous)
            .unwrap_or(false)
    }
}

/// Configure pallet-inomad-elections — Peak Governance & Decimal Hierarchy.
///
/// ## Constitutional Hierarchy (physics of the network)
///
/// ```text
/// Арбад  ≥ 10 members   → 1 elected Arbad Leader
/// Зун    = 10 Арбадов   → 1 elected Zun Leader (100 total citizens)
/// Мянгад = 10 Зунов     → 1 elected Myangad Leader (1 000 total citizens)
/// Тумэд  = 10 Мянгадов  → Tumed Leader (10 000 total citizens)
/// ```
///
/// ## Branch Separation
/// - Executive / Judicial / Banking: elected by Tumed leaders (open to all)
/// - Legislative (Хурал): elected by Tumed leaders who are indigenous citizens
impl pallet_inomad_elections::Config for Runtime {
    /// Maximum candidates per decimal-hierarchy election (Arbad → Tumed).
    type MaxCandidates = ElectionMaxCandidates;
    /// Maximum distinct candidates per Branch Council election (H-1 DOS guard).
    type MaxBranchCandidates = ElectionMaxBranchCandidates;

    /// Арбад minimum: 10 citizens before leader promotion.
    type MinArbadSize = ElectionMinArbadSize;
    /// Зун total members (documentation, 100 implied by 10 full Арбадов).
    type MinZunSize = ElectionMinZunSize;
    /// Зун minimum: 10 elected Арбад Leaders before Zun leader promotion.
    type MinZunLeaders = ElectionMinZunLeaders;
    /// Мянгад total members (documentation, 1000 implied by 10 full Зунов).
    type MinMyangadSize = ElectionMinMyangadSize;
    /// Мянгад minimum: 10 elected Зун Leaders before Myangad leader promotion.
    type MinMyangadLeaders = ElectionMinMyangadLeaders;

    /// Production indigenous check: delegates to pallet-inomad-identity.
    /// Enforces Jus Sanguinis rule for the Legislative (Хурал) branch.
    type IsIndigenous = IdentityIndigenousBridge;

    type WeightInfo = pallet_inomad_elections::weights::SubstrateWeight<Runtime>;
}

// ─── Buryad-Mongol Mixer (Sprint L1-24) ──────────────────────────────────────
//
// Privacy-preserving shielded transfer pool.
//
// Fee model: Double fee (0.03% base + 0.05% privacy = 0.08% total).
// Exclusive math: pool receives exactly  `amount`, depositor pays `amount + fee`.
// Constitutional 54/26/10/10 split on total fee.
//
// Denomination: multiples of 10 ALTAN — enforces uniform deposit sizes to protect
// the ZK anonymity set (all deposits look identical on-chain).
//
// Origins:
//   withdraw            → EnsureRoot (TODO: wire to Bank of Siberia key in prod)
//   reveal_transaction  → EnsureRoot (TODO: wire to BankBoard multisig in prod)
//   quarterly_audit     → EnsureRoot (TODO: wire to Khural collective in prod)

parameter_types! {
    /// Buryad-Mongol Mixer PalletId — keyless escrow pool account.
    /// Derived as: PalletId(*b"bgmixer!").into_account_truncating()
    pub const BuryadMixerPalletId: frame_support::PalletId = frame_support::PalletId(*b"bgmixer!");

    /// Denomination step: 10 ALTAN = 10 × 10^12 planck.
    /// Deposits MUST be strict multiples of this value.
    /// Protects the ZK anonymity set by ensuring all deposits are uniform.
    pub const MixerDenomination10: u128 = 10 * UNIT;

    /// Mixer privacy fee: 0.05% = Permill::from_parts(500).
    pub const MixerFeePermill: sp_runtime::Permill = sp_runtime::Permill::from_parts(500);

    /// Base network fee: 0.03% = Permill::from_parts(300).
    /// Combined with privacy fee → total 0.08%.
    pub const MixerBaseFeePermill: sp_runtime::Permill = sp_runtime::Permill::from_parts(300);

    /// Hard cap on total fee per mixer deposit: 1,000 ALTAN.
    pub const MaxMixerFeeAmt: u128 = 1_000 * UNIT;

    /// Khural Foundation — 54% of mixer fees (UBI, science, indigenous peoples).
    /// SS58: 5G11UBehntN5pPMoi7m7s6GTayb3T9iEAJFThAUmuF8V2fna
    pub MixerKhuralAccount: AccountId = {
        use sp_core::crypto::Ss58Codec;
        AccountId::from_ss58check("5G11UBehntN5pPMoi7m7s6GTayb3T9iEAJFThAUmuF8V2fna")
            .expect("Khural SS58 must be valid")
    };

    /// INOMAD AG — 26% of mixer fees (commercial treasury, R&D).
    /// SS58: 5DrCgbEEpN1T1AjgJsaNz914T2q3p39RNGmRh9GqdVd4YdGJ
    pub MixerAgAccount: AccountId = {
        use sp_core::crypto::Ss58Codec;
        AccountId::from_ss58check("5DrCgbEEpN1T1AjgJsaNz914T2q3p39RNGmRh9GqdVd4YdGJ")
            .expect("INOMAD AG SS58 must be valid")
    };

    /// Validator pool — 10% of mixer fees.
    /// Dev: Alice's well-known address for testing.
    /// Production: replace with the actual ValidatorPool keyless account.
    pub MixerValidatorAccount: AccountId = {
        use sp_core::crypto::Ss58Codec;
        AccountId::from_ss58check("5GrwvaEF5zXb26Fz9rcQpDWS57CtERHpNehXCPcNoHGKutQY")
            .expect("Validator pool SS58 must be valid")
    };
}

/// Configure pallet-buryad-mongol-mixer — privacy pool with constitutional fee split.
/// Sprint L1-24: double fee (0.08%), exclusive math, 54/26/10/10 split, 10 ALTAN denomination.
impl pallet_buryad_mongol_mixer::Config for Runtime {
    type Currency = Balances;

    type PalletId = BuryadMixerPalletId;
    type MixerDenomination = MixerDenomination10;
    type MixerFeePermill = MixerFeePermill;
    type BaseFeePermill = MixerBaseFeePermill;
    type MaxMixerFee = MaxMixerFeeAmt;

    // Constitutional 54/26/10/10 split
    type KhuralFoundationAccount = MixerKhuralAccount;
    type InomadAgAccount = MixerAgAccount;
    type CreatorAccount = CreatorSudoAccount; // 5FTZYAh4...
    type ValidatorsPoolAccount = MixerValidatorAccount;

    // Origins for privileged operations
    // ─── Constitutional Origins (Sprint 8 → Production) ───────────────────────
    //
    // Phase 1 (Bootstrap): Root fallback allows Creator to operate before elections.
    // Phase 2 (Post-Election): Upgrade via WASM referendum to remove Root fallback.
    //
    //  withdraw (RelayerOrigin):
    //    Bootstrap: EnsureRootOrBankingCouncil
    //    Production target: EnsureBankingCouncilMember
    //    Rationale: Only the Banking Branch may authorize Mixer withdrawals
    //               (anti-MEV, AML compliance). No citizen or Khural access.
    //
    //  reveal_transaction (BankBoardOrigin):
    //    Bootstrap: EnsureRootOrBankingCouncil
    //    Production target: EnsureBankBoard (Council OR Supreme Leader)
    //    Rationale: Deanonymization requires the full BankBoard authority
    //               (judicial order compliance). Highest banking threshold.
    //
    //  quarterly_audit (KhuralOrigin):
    //    Bootstrap: EnsureRootOrKhuralChairman
    //    Production target: EnsureKhuralChairman
    //    Rationale: The Legislative Branch's oversight role. The Khural Chairman
    //               represents the Assembly for regulatory audit operations.
    type RelayerOrigin = crate::origins::EnsureRootOrBankingCouncil;
    type BankBoardOrigin = crate::origins::EnsureRootOrBankingCouncil;
    type KhuralOrigin = crate::origins::EnsureRootOrKhuralChairman;
    type WeightInfo = pallet_buryad_mongol_mixer::weights::SubstrateWeight<Runtime>;
}

// ─── Altan Vault (Sprint L1-25) ───────────────────────────────────────────────
//
// Isolated savings sub-accounts — deterministic keyless vaults.
//
// Each vault address is derived from (owner, vault_index) using the PalletId.
// Vaults receive funds anonymously (e.g., from Mixer withdrawals).
// Outbound: ONLY to the primary owner. Zero-fee constitutional exemption.

parameter_types! {
    /// Altan Vault PalletId — used to derive deterministic vault sub-accounts.
    /// Each vault: PalletId(*b"a/vault!").into_sub_account_truncating((owner, index))
    pub const AltanVaultPalletId: frame_support::PalletId = frame_support::PalletId(*b"a/vault!");

    /// Maximum number of vaults per owner (safety cap for storage bounds).
    pub const MaxVaultsPerOwner: u16 = 64;
}

/// Configure pallet-altan-vault — isolated savings sub-accounts with zero-fee exemption.
/// Sprint L1-25: deterministic vaults, anonymous inbound, owner-only outbound.
impl pallet_altan_vault::Config for Runtime {
    type Currency = Balances;
    type PalletId = AltanVaultPalletId;
    type MaxVaultsPerOwner = MaxVaultsPerOwner;
    type WeightInfo = pallet_altan_vault::weights::SubstrateWeight<Runtime>;
}

// ─────────────────────────────────────────────────────────────────────────────
// NFT Pallets: Access Keys · Achievement · Recovery Arbad
// ─────────────────────────────────────────────────────────────────────────────
//
// All three NFT pallets are Relayer-gated in production (Creator/Admin issues
// on behalf of verified events). In dev we use EnsureSigned for convenience.
// In production: replace with a designated NFT Akademik multisig collective.
//
// Pallet indices: 32 (AccessNft), 33 (AchievementNft), 34 (RecoveryNft), 35 (Forums)

parameter_types! {
    /// Veto window for Recovery NFT: 72 hours ≈ 43,200 blocks at 6s/block.
    pub const RecoveryVetoWindowBlocks: u32 = 43_200;

    /// Maximum members in a Recovery NFT group (default: 5 for 3-of-5).
    pub const RecoveryMaxGroupSize: u8 = 10;

    /// Max Access NFT keys per holder (anti-spam).
    pub const AccessNftMaxKeysPerHolder: u32 = 100;

    /// Max holders (key issuances) per entity.
    pub const AccessNftMaxHoldersPerEntity: u32 = 10_000;

    /// Max Achievement NFTs per holder.
    pub const MaxAchievementsPerHolder: u32 = 500;

    /// Maximum forum_id byte length.
    pub const MaxForumIdLen: u32 = 128;

    /// Maximum direct replies per message (spam guard).
    pub const MaxRepliesPerMessage: u32 = 1_024;
}

/// Configure pallet-access-nft — Universal NFT Access Keys for all legal entities.
///
/// Issued by entity admins or the Creator for citizens entering organizations,
/// guilds, government bodies, banks, funds, and regions.
/// IssuerOrigin = EnsureSigned (any account; production: entity admin multisig).
impl pallet_access_nft::Config for Runtime {
    type IssuerOrigin = frame_system::EnsureSigned<AccountId>;
    type MaxKeysPerHolder = AccessNftMaxKeysPerHolder;
    type MaxHoldersPerEntity = AccessNftMaxHoldersPerEntity;
    type WeightInfo = pallet_access_nft::weights::SubstrateWeight<Runtime>;
}

/// Configure pallet-achievement-nft — Reputation & Reward NFT system.
///
/// NftIssuerOrigin = EnsureSigned (production: NFT Akademik collective multisig).
/// Note: EnsureRoot::Success = () but we need Success = AccountId for the issuer
/// identity. EnsureSigned is used so the issuer AccountId is captured for audit.
impl pallet_achievement_nft::Config for Runtime {
    type Currency = Balances;
    type NftIssuerOrigin = frame_system::EnsureSigned<AccountId>;
    type ConfederationTreasury = ConfederationTreasuryAddr;
    type MaxAchievementsPerHolder = MaxAchievementsPerHolder;
    type WeightInfo = pallet_achievement_nft::weights::SubstrateWeight<Runtime>;
}

/// Configure pallet-recovery-nft — Arbad Social Recovery (3-of-5 NFT threshold).
///
/// The owner issues 5 Recovery NFTs pre-programmed with the destination vault.
/// Trusted holders initiate recovery; 3 confirmations execute the transfer.
/// Owner has a 72h veto window (RecoveryVetoWindowBlocks).
///
/// IssuerOrigin = EnsureSigned (any account; production: Arbad admin multisig).
impl pallet_recovery_nft::Config for Runtime {
    type IssuerOrigin = frame_system::EnsureSigned<AccountId>;
    type VetoWindowBlocks = RecoveryVetoWindowBlocks;
    type MaxGroupSize = RecoveryMaxGroupSize;
    type WeightInfo = pallet_recovery_nft::weights::SubstrateWeight<Runtime>;
}

/// Configure pallet-forums — Threaded Hierarchical Communication.
///
/// All deliberative bodies (Arbads, Zuns, Myangads, Tumeds, Grand Khural) post
/// messages here. Each message is linked to a forum_id context tag,
/// and replies build an infinite-depth content-addressed discourse tree.
impl pallet_forums::Config for Runtime {
    type MaxForumIdLen = MaxForumIdLen;
    type MaxRepliesPerMessage = MaxRepliesPerMessage;
}

// =============================================================================
// pallet-migration-center — Sovereign Application Anchoring on Altan L1
// =============================================================================
//
// CONSTITUTIONAL GUARANTEE:
//   Every migration application is permanently anchored on Altan L1.
//   Even if the backend database is wiped, the wallet address + application hash
//   remain on-chain forever — proof of the citizen's application.
//
// FLOW:
//   1. Backend (migration-service) calls submit_application(applicant, hash, metadata_hash)
//      → AnchorStatus::Submitted
//   2. Officer calls claim_application(hash, officer_sig)
//      → AnchorStatus::UnderReview  (officer Sr25519 sig stored on-chain)
//   3. Officer calls finalize_application(hash, approved) or Root calls revoke_application
//      → AnchorStatus::Approved | Rejected | Revoked
//
// SECURITY:
//   - MaxOfficers = 1000 — prevents unbounded officer registry growth
//   - Officer signature is verified on-chain via sp-core Sr25519
//   - Application hash = blake2_256(applicant_data) — immutable
//
// ORIGINS (Bootstrap phase):
//   - submit_application: EnsureRoot (Relayer/backend calls via Sudo)
//   - claim_application:  EnsureSigned (any registered officer)
//   - finalize_application: EnsureSigned (only the claiming officer)
//   - revoke_application: EnsureRoot (judicial/admin override)

parameter_types! {
    /// Maximum number of registered migration officers.
    ///
    /// 1,000 officers covers the Confederation's initial deployment phase.
    /// This is a safety cap — the actual officer list is managed off-chain
    /// by the Migration Guild leadership and submitted via Relayer.
    pub const MigrationCenterMaxOfficers: u32 = 1_000;
}

/// Configure pallet-migration-center — Sovereign Migration Application Anchoring.
///
/// ## Constitutional Role (Altan L1 Layer 0)
///
/// This pallet is the immutable anchor layer for the INOMAD Migration Center.
/// It does NOT replace the backend database — it is the **cryptographic proof**
/// that a migration application existed and was processed.
///
/// ## Pallet Index 37
///
/// Registered after AnnualProfitTax (36). This ordering is permanent —
/// changing pallet indices after genesis requires a full storage migration.
impl pallet_migration_center::Config for Runtime {
    /// Maximum registered migration officers (anti-spam guard).
    type MaxOfficers = MigrationCenterMaxOfficers;
}

// =============================================================================
// pallet-org-roles — Organization Roles (SBT) management
// =============================================================================

impl pallet_org_roles::Config for Runtime {
    type RuntimeEvent = RuntimeEvent;
    type MaxOrgNameLen = OrgMaxNameLength;
}
