# INOMAD OS — Audit Readiness Document

**Version:** 1.0.0  
**Date:** May 2026  
**Prepared for:** Commercial Security Audit / Parity Protocol Builders Program  
**Chain:** Altan Network (Substrate FRAME, Polkadot-compatible)

---

## Executive Summary

INOMAD OS is a sovereign state operating system implemented as **28 native Substrate FRAME pallets** encoding the constitutional law of the Mongol Republic on-chain. The system has no EVM/Cosmos SDK components — it is 100% Rust/Substrate-native.

| Metric | Value |
|---|---|
| Total Rust source lines | ~91,000 |
| FRAME pallets | 28 |
| Dispatchable extrinsics | 197 |
| Storage items | 112 |
| On-chain events | 249+ |
| Test suites (`tests.rs`) | 28 |
| Benchmarking suites | 28 |
| Migration modules | 28 |

---

## 1. Architecture Overview

```
┌─────────────────────────────────────────────────────────┐
│                    INOMAD OS Runtime                     │
│              (Substrate FRAME, Polkadot-SDK)             │
├─────────────┬───────────────┬──────────┬────────────────┤
│  Identity   │  Governance   │ Finance  │  Civil Society │
├─────────────┼───────────────┼──────────┼────────────────┤
│ inomad-     │ constitution  │ central- │ guilds         │
│ identity    │ decimal-dao   │ bank     │ chronicles     │
│ inomad-     │ khural-       │ bank-of- │ citizen-voice  │
│ elections   │ governance    │ siberia  │ chancery       │
│ recovery-   │ licensing     │ bank-    │ foreign-       │
│ nft         │ inomad-       │ operator │ affairs        │
│ access-nft  │ elections     │ altan-   │ forums         │
│ achievement │ judicial-     │ vault    │ inheritance    │
│ -nft        │ courts        │ shielded │ land-registry  │
│             │ black-book    │ -vaults  │ organization   │
│             │               │ steppe-  │                │
│             │               │ offline  │                │
│             │               │ buryad-  │                │
│             │               │ mongol-  │                │
│             │               │ mixer    │                │
│             │               │ altan-   │                │
│             │               │ tax      │                │
└─────────────┴───────────────┴──────────┴────────────────┘
```

### Fractal Governance Hierarchy
```
Arbad (10)  →  Zun (100)  →  Myangad (1,000)  →  Tumed (10,000)
    ↓                                                    ↓
Arbad Leader                                    Branch Council (9)
                                                        ↓
                                               Supreme Leader / Khural Chairman
```

---

## 2. Codebase Integrity

### 2.1 No Legacy EVM/Cosmos Artifacts

```bash
# Verification commands — all return 0 results
grep -r "ethers\|web3js\|0x[0-9a-fA-F]\{40\}" altan-network/ # EVM addresses
grep -r "CosmWasm\|cosmwasm\|cosmos-sdk"        altan-network/ # Cosmos SDK
grep -r "solidity\|\.sol\b"                     altan-network/ # Solidity
grep -r "ERC20\|ERC721\|IERC"                   altan-network/ # EVM standards
```

### 2.2 No Unsafe Code

```bash
grep -r "unsafe" altan-network/pallets/*/src/lib.rs  # 0 results
```

### 2.3 Build Integrity

```bash
cargo check --workspace                          # ✅ 0 errors
cargo fmt --all -- --check                       # ✅ 0 formatting issues
cargo clippy --workspace                         # ✅ 0 lint errors
cargo check --workspace --features try-runtime   # ✅ 0 errors
cargo check --workspace --features runtime-benchmarks  # ✅ 0 errors
```

---

## 3. Constitutional Invariants

All invariants are enforced via `try_state` hooks in `migrations.rs` and verified by `try-runtime` before every upgrade:

| Pallet | Invariant | Hook |
|---|---|---|
| `central-bank` | `TotalEmitted ≥ TotalBurned` | `try_state` |
| `central-bank` | `CurrentEpochId > 0` after genesis | `try_state` |
| `inomad-identity` | No zero-address citizen keys | `try_state` |
| `bank-of-siberia` | All escrow amounts > 0 | `try_state` |
| `constitution` | Amendment version consistency | `try_state` |
| All pallets | Pre/post upgrade snapshot verification | `pre_upgrade` / `post_upgrade` |

### try-runtime Command
```bash
cargo try-runtime \
  --runtime ./target/release/wbuild/altan-network-runtime/*.wasm \
  on-runtime-upgrade --checks=all \
  live --uri wss://rpc.altan.network
```

---

## 4. Security Boundaries

### 4.1 Origin Guards

| Origin Type | Used For | Implementation |
|---|---|---|
| `EnsureRoot` | Genesis bootstrap, emergency | `frame_system::EnsureRoot` |
| `BankingOrigin` | CB monetary policy | `pallet_inomad_elections::BranchCouncil` |
| `JudicialOrigin` | Court verdicts, exile | `pallet_judicial_courts::JudicialOrigin` |
| `MedicalAuthority` | Birth/death registration | `T::MedicalAuthority` Config type |
| `EnsureSigned` | Citizen actions | Standard FRAME |

### 4.2 Constitutional Guards

```rust
// Non-Alienation Law (pallet-land-registry)
ensure!(
    !is_foreign_state_account::<T>(&buyer),
    Error::<T>::ForeignOwnershipForbidden
);

// Sole emission path (pallet-central-bank)  
ensure!(
    LicensedOperators::<T>::get(&operator),
    Error::<T>::UnlicensedOperator
);

// Habeas Corpus (pallet-constitution)
// Auto-releases frozen citizens after MaxLockupPeriod blocks
fn on_initialize(n: BlockNumberFor<T>) -> Weight { ... }
```

### 4.3 Rate Limiting

```rust
// pallet-inomad-identity
type MaxRegistrationsPerBlock = ConstU32<100>;
// Limits blast radius of compromised MedicalAuthority oracle key
```

### 4.4 Existential Deposit Protection

All treasury transfers use `KeepAlive` preservation mode — no treasury can be wiped to zero.

---

## 5. Weight System

### 5.1 WeightInfo Architecture

Every pallet follows the standard FRAME weight pattern:

```
pallet-X/
├── src/
│   ├── lib.rs          — type WeightInfo: crate::weights::WeightInfo
│   ├── weights.rs      — WeightInfo trait + SubstrateWeight<T> impl
│   └── benchmarking.rs — #[benchmark] functions for each extrinsic
```

### 5.2 Current Status

| Category | Status |
|---|---|
| `WeightInfo` trait | ✅ Defined for all 197 dispatchables |
| `SubstrateWeight<T>` | ✅ Static analysis placeholder (safe upper bounds) |
| `benchmarking.rs` | ✅ `#[benchmark]` functions for all pallets |
| Real `cargo benchmark` | 📋 Requires reference hardware (x86_64, 3.1GHz) |

### 5.3 Weight Classification

```
Heavy ops  (register/create/form): 80M ref_time, 3R + 3W DbWeight
Standard   (transfer/vote/update): 25M ref_time, 1R + 1W DbWeight  
Queries    (is_/validate_):         5M ref_time, 1R + 0W DbWeight
```

### 5.4 Benchmark Regeneration

```bash
cargo benchmark \
  --chain=dev \
  --execution=wasm \
  --pallet="*" \
  --extrinsic="*" \
  --steps=50 \
  --repeat=20 \
  --output=pallets/{pallet}/src/weights.rs
```

---

## 6. Storage Layout

### 6.1 Storage Item Types

| Type | Count | Pattern |
|---|---|---|
| `StorageValue` | 48 | Single global state |
| `StorageMap` | 45 | Per-entity records |
| `StorageDoubleMap` | 19 | Bidirectional relationships |
| **Total** | **112** | |

### 6.2 Key Storage Items by Pallet

| Pallet | Key Storage | Risk Notes |
|---|---|---|
| `inomad-identity` | `Citizens`, `Arbads`, `UsedEmailHashes` (15 items) | Central identity — highest risk |
| `central-bank` | `TotalEmitted`, `CitizenDebt`, `Epochs` | Economic invariants |
| `bank-of-siberia` | `SubAccounts`, `Escrows`, `TimeDeposits` | Financial safety |
| `inomad-elections` | `Votes`, `BranchCouncils`, `SupremeLeader` | Governance integrity |
| `guilds` | `Guilds`, `Quests`, `GuildUnions` (11 items) | Largest pallet by storage |

---

## 7. Migration Safety

### 7.1 Migration Framework

Every pallet has a `migrations.rs` module with:
- `NoopMigration<T>` — placeholder (current state, no schema change)
- `try_state<T>()` — constitutional invariant checks
- `pre_upgrade()` / `post_upgrade()` — state snapshot and verification

### 7.2 Adding Real Migrations

When a storage schema changes:

1. Create `pub mod v2 { pub struct Migration<T>(...); }` in `migrations.rs`
2. Implement `OnRuntimeUpgrade` with `pre_upgrade` / `post_upgrade`
3. Replace `NoopMigration<Runtime>` in `runtime/src/lib.rs` `Migrations` tuple
4. Bump `spec_version` in `runtime/src/lib.rs`
5. Run `cargo try-runtime on-runtime-upgrade --checks=all`

### 7.3 Current Runtime Migrations Tuple

```rust
pub type Migrations = (
    pallet_inomad_identity::migrations::NoopMigration<Runtime>,
    pallet_central_bank::migrations::NoopMigration<Runtime>,
    // ... 26 more pallets
);
```

---

## 8. CI/CD Pipeline

### 8.1 Rust L1 CI (`.github/workflows/rust-l1-ci.yml`)

| Job | Check | Gate |
|---|---|---|
| `fmt` | `cargo fmt --check` | ❌ blocks merge |
| `clippy` | `cargo clippy -D warnings` | ❌ blocks merge |
| `check` | `cargo check` (3 feature sets) | ❌ blocks merge |
| `test` | 28 pallet test suites (parallel) | ❌ blocks merge |
| `benchmark-check` | `--features runtime-benchmarks` | ❌ blocks merge |
| `try-runtime-check` | `--features try-runtime` | ❌ blocks merge |
| `rustdoc` | `RUSTDOCFLAGS=-D warnings cargo doc` | ❌ blocks merge |
| `audit` | `cargo audit` (CVE check) | ⚠️ advisory |
| `l1-readiness` | Final gate summary | ❌ blocks merge |

---

## 9. Known Limitations & TODO Before Mainnet

| Item | Priority | Effort |
|---|---|---|
| Run `cargo benchmark` on reference HW → replace placeholder weights | 🔴 Critical | 2-4h CI time |
| `try-runtime` against live state (`--uri wss://...`) | 🔴 Critical | Needs live RPC |
| ZKP circuit security review (`pallet-shielded-vaults`, `pallet-buryad-mongol-mixer`) | 🔴 Critical | Specialist audit |
| 4 pallets with inherited `Config` use hardcoded weights | 🟡 Medium | 1 day |
| Real governance `BranchCouncilOrigin` (currently `EnsureRoot` in dev) | 🟡 Medium | 3 days |
| Multi-validator testnet (≥4 nodes) | 🟡 Medium | 1 week |

---

## 10. Audit Scope Recommendation

### Priority 1 — Economic Security
- `pallet-central-bank` — sole emission authority, epoch key rate
- `pallet-bank-of-siberia` — credit, escrow, time deposits
- `pallet-altan-tax` — 13% constitutional tax routing

### Priority 2 — Identity & Governance
- `pallet-inomad-identity` — CitizenCredential lifecycle (15 storage items)
- `pallet-inomad-elections` — 4-tier election cascade
- `pallet-constitution` — Habeas Corpus timer

### Priority 3 — Privacy
- `pallet-shielded-vaults` — commitment-based shielding
- `pallet-buryad-mongol-mixer` — commit-reveal mixer

### Out of Scope (for this audit)
- Frontend (Next.js) and Backend (NestJS) — separate audit track
- ZKP circuits — dedicated cryptographic review required

---

## 11. Contact & Repository

- **Repository:** Private GitHub (`INOMAD-Core_Private`)
- **Runtime spec_version:** 103
- **Substrate SDK:** polkadot-sdk (Substrate FRAME v53+)
- **Rust edition:** 2021
- **WASM target:** `wasm32-unknown-unknown`

---

*This document is auto-generated from codebase analysis and manually reviewed.*  
*Last updated: May 2026.*
