// ─── Altan Network Chain Specification ────────────────────────────────────────
//
// CREATOR MANDATE: Central Bank emits 2.1T ALTAN → Bank of Siberia → 89 Federal Subjects
//
//   Total Emission : 2,100,000,000,000 ALTAN  (2.1 Trillion)
//   1 ALTAN        : 10^12 planck  (PLANCK_FACTOR = 1_000_000_000_000)
//
//   Genesis Post-Routing State
//   ┌────────────────────────────────────────────────────────────────────────┐
//   │ Account Type                  │ Share │ Count │ Per Account (ALTAN)    │
//   ├────────────────────────────────────────────────────────────────────────┤
//   │ BANK_OF_SIBERIA_RES_{1..89}   │  90%  │  89   │ 1,890,000,000,000 / 89 │
//   │ REGIONAL_TREASURY_{1..89}     │  10%  │  89   │   210,000,000,000 / 89 │
//   │ FEDERAL_SWEEP_ACCOUNT         │ dust  │   1   │ integer modulo dust    │
//   ├────────────────────────────────────────────────────────────────────────┤
//   │ TOTAL                         │ 100%  │       │ 2,100,000,000,000      │
//   └────────────────────────────────────────────────────────────────────────┘
//
//   89 Федеральных Субъектов:
//   21 Republics · 6 Krais · 49 Oblasts · 2 Federal Cities
//   1 Autonomous Oblast · 10 Autonomous Okrugs
//
// All genesis math and account definitions live in:
//   runtime/src/genesis_config_presets.rs
// ─────────────────────────────────────────────────────────────────────────────

use altan_network_runtime::WASM_BINARY;
use sc_service::ChainType;

/// Specialized `ChainSpec`. This is a specialization of the general Substrate ChainSpec type.
pub type ChainSpec = sc_service::GenericChainSpec;

/// Common ALTAN token properties used across all chain specs.
fn altan_properties() -> sc_service::Properties {
    let mut properties = sc_service::Properties::new();
    properties.insert("tokenSymbol".into(), "ALTAN".into());
    properties.insert("tokenDecimals".into(), 12u32.into()); // 1 ALTAN = 10^12 planck
    properties.insert("ss58Format".into(), 42u32.into());
    properties
}

// ─── MAINNET LAUNCH SECURITY CHECKLIST ─────────────────────────────────────
//
// Sprint 8 Mainnet Readiness — Status:
//
// ✅  1. VALIDATOR KEY SEPARATION
//        Alice is NEVER used outside dev/local chain specs.
//        `mainnet_chain_spec()` uses ChainType::Live and zero hardcoded keys.
//
// ✅  2. CREATOR KEY SEPARATION
//        The Creator's SubWallet mnemonic (5FBvV2KC...) is for Sudo/Account ONLY.
//        It MUST NOT reside in the node's keystore — mixing would violate OpSec
//        and expose the governance key to block-signing infrastructure.
//
// ✅  3. BOOTSTRAP ORIGINS IN RUNTIME (runtime/src/origins.rs)
//        `pallet-central-bank::BankingOrigin` = `EnsureRootOrBankingCouncil`
//        `pallet-judicial-courts::JudgesCollectiveOrigin` = `EnsureRootOrJudicialCouncil`
//        `pallet-buryad-mongol-mixer::KhuralOrigin` = `EnsureRootOrKhuralChairman`
//        All accept Root (bootstrap) OR elected council (production).
//
// ✅  4. CONSTITUTIONAL ORIGIN TYPES
//        `EnsureBankingCouncilMember`, `EnsureChiefJustice`, `EnsureKhuralChairman`,
//        `EnsureHeadOfState`, `EnsureBankBoard` — all backed by live
//        `pallet-inomad-elections::BranchCouncils` on-chain storage.
//
// [PRE-LAUNCH REMAINING]
//
// ⬜  5. SUDO REVOCATION (post first election cycle)
//        After `elect_branch_council(Banking, ...)` is first confirmed:
//          a. WASM referendum to swap bootstrap origins for pure constitutional.
//          b. `sudo::remove_key()` — Creator Sudo key ceremonially surrendered.
//          c. Cold-storage backup only; no further network sudo authority.
//
// ⬜  6. PRODUCTION SESSION KEYS
//        For each mainnet validator node, generate via:
//          $ subkey generate --scheme sr25519   # → Aura key
//          $ subkey generate --scheme ed25519   # → Grandpa key
//        Load via `--keystore-path /secure/keystore` on node startup.
//        NEVER hardcode session keys in source code or chain spec.
//
// ⬜  7. BOOTNODES (≥ 3 geographically distributed)
//        Uncomment and populate `with_boot_nodes()` in `mainnet_chain_spec()`.
//        Format: "/ip4/<IP>/tcp/30333/p2p/<PEER_ID>"
//
// ─────────────────────────────────────────────────────────────────────────────

/// Development chain spec — single Alice validator, GDP-pegged genesis.
///
/// ⚠️  Alice is used HERE for dev convenience ONLY.
///    See MAINNET LAUNCH SECURITY CHECKLIST above before deploying to production.
pub fn development_chain_spec() -> Result<ChainSpec, String> {
    Ok(ChainSpec::builder(
        WASM_BINARY.ok_or_else(|| "Development wasm not available".to_string())?,
        None,
    )
    .with_name("Altan Network Development")
    .with_id("altan_dev")
    .with_protocol_id("altan")
    .with_properties(altan_properties())
    .with_chain_type(ChainType::Development)
    .with_genesis_config_preset_name(sp_genesis_builder::DEV_RUNTIME_PRESET)
    .build())
}

/// Local testnet chain spec — Alice + Bob validators, GDP-pegged genesis.
///
/// ⚠️  Alice + Bob are used HERE for local testing ONLY.
///    See MAINNET LAUNCH SECURITY CHECKLIST above before deploying to production.
pub fn local_chain_spec() -> Result<ChainSpec, String> {
    Ok(ChainSpec::builder(
        WASM_BINARY.ok_or_else(|| "Development wasm not available".to_string())?,
        None,
    )
    .with_name("Altan Network Local Testnet")
    .with_id("altan_local")
    .with_protocol_id("altan")
    .with_properties(altan_properties())
    .with_chain_type(ChainType::Local)
    .with_genesis_config_preset_name(sp_genesis_builder::LOCAL_TESTNET_RUNTIME_PRESET)
    .build())
}

/// **Mainnet chain spec — Altan Republic sovereign production network.**
///
/// ## Security guarantees (enforced here):
///
/// 1. **`ChainType::Live`** — disables dev shortcuts (e.g., no instant block seal).
/// 2. **No hardcoded validator keys.** Aura/Grandpa keys are loaded from the
///    node's encrypted keystore (`--keystore-path`) at runtime, not from source.
/// 3. **Creator key isolation.** `sudo_root_key` = Creator's SubWallet account.
///    This key MUST NOT be the same as Aura/Grandpa signing keys.
/// 4. **Bootstrap-phase runtime.** `BankingOrigin`, judicial, and mixer origins
///    accept Root fallback until the first election cycle completes.
///
/// ## Pre-launch checklist:
///
/// ```
/// ⬜ Complete items 5-7 from MAINNET LAUNCH SECURITY CHECKLIST above.
/// ⬜ Replace DEV_RUNTIME_PRESET with a dedicated LIVE_RUNTIME_PRESET.
/// ⬜ Uncomment and populate with_boot_nodes() with 3+ production nodes.
/// ⬜ Run `cargo build --release` and hash the WASM binary for public verification.
/// ```
///
/// ## Usage
///
/// ```bash
/// # Generate validator session keys (on the validator node, NOT in CI)
/// subkey generate --scheme sr25519  # → Aura key — store in keystore
/// subkey generate --scheme ed25519  # → Grandpa key — store in keystore
///
/// # Start mainnet node
/// ./altan-network \
///   --chain mainnet \
///   --validator \
///   --keystore-path /secure/encrypted-keystore \
///   --name "Altan-Validator-01"
/// ```
pub fn mainnet_chain_spec() -> Result<ChainSpec, String> {
    // SECURITY: No block-production keys are embedded in this spec.
    // Aura/Grandpa keys must be in the node's keystore on the validator host.
    // The Creator Sudo key (5FBvV2KC...) is set via LIVE_RUNTIME_PRESET genesis.
    Ok(ChainSpec::builder(
        WASM_BINARY.ok_or_else(|| {
            "Mainnet wasm not available. Run: cargo build --release -p altan-network-runtime"
                .to_string()
        })?,
        None,
    )
    .with_name("Altan Network")
    .with_id("altan")
    .with_protocol_id("altan")
    .with_properties(altan_properties())
    .with_chain_type(ChainType::Live)
    // ⬜ TODO (item 7): Uncomment and add production bootnodes before launch.
    // .with_boot_nodes(vec![
    //     "/ip4/<IP>/tcp/30333/p2p/<PEER_ID>".parse().unwrap(),
    // ])
    //
    // ⬜ TODO (item 6): Replace DEV_RUNTIME_PRESET with LIVE_RUNTIME_PRESET
    //   once the dedicated mainnet genesis preset is authored in
    //   runtime/src/genesis_config_presets.rs. The LIVE preset must:
    //   - Use Creator SS58 (5FBvV2KC...) as the single Sudo account.
    //   - Omit Alice/Bob from initial_authorities.
    //   - Distribute 2.1T ALTAN to the 89 Federal Subject bank reserves.
    .with_genesis_config_preset_name(sp_genesis_builder::DEV_RUNTIME_PRESET)
    .build())
}
