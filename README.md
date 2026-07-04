# INOMAD OS — Altan Network

<p align="center">
  <img alt="Built on Substrate" src="https://img.shields.io/badge/Built%20on-Substrate-green?logo=parity&logoColor=white"/>
  <img alt="Polkadot Ecosystem" src="https://img.shields.io/badge/Polkadot-Ecosystem-E6007A?logo=polkadot&logoColor=white"/>
  <img alt="License: Apache 2.0" src="https://img.shields.io/badge/License-Apache%202.0-blue"/>
  <img alt="Rust" src="https://img.shields.io/badge/Rust-1.82%2B-orange?logo=rust"/>
  <img alt="Web3 Foundation Grant" src="https://img.shields.io/badge/Web3%20Foundation-Grant%20Applicant-00B5A3"/>
</p>

> **INOMAD OS** is a sovereign digital-state infrastructure built on Substrate, enabling decentralized governance, collateral-backed credit lines, and cross-chain liquidity through the **Cash Advance Bridge** — all governed on-chain by constitutional rules encoded directly in runtime pallets.

---

## Table of Contents

1. [Overview](#overview)
2. [Technical Architecture](#technical-architecture)
3. [Cash Advance Bridge](#cash-advance-bridge)
4. [Substrate Module Structure](#substrate-module-structure)
5. [Altan Token Economics](#altan-token-economics)
6. [Local Development Setup](#local-development-setup)
7. [Running Tests](#running-tests)
8. [Security Model](#security-model)
9. [Contributing](#contributing)
10. [License](#license)

---

## Overview

INOMAD OS implements a **layer-1 sovereign state** on Substrate with:

- **Decentralized credit lines** — collateral-backed loans issued and enforced by on-chain pallets, without external oracles or centralized risk engines.
- **Cash Advance Bridge** — a trustless mechanism allowing citizens to draw short-term liquidity against locked collateral, with automatic repayment via enforced on-chain schedules.
- **Constitutional governance** — all economic parameters (interest rates, collateral ratios, liquidation thresholds) are governed by the Khural DAO and encoded in the `pallet-constitution`.
- **Keyless sub-account architecture** — derived accounts via `Blake2_256(master ++ type_byte ++ nonce)` eliminate seed proliferation while preserving social recovery compatibility.

---

## Technical Architecture

```
┌──────────────────────────────────────────────────────────────────┐
│                        ALTAN NETWORK L1                          │
│                    (Substrate FRAME Runtime)                      │
│                                                                   │
│  ┌─────────────┐  ┌──────────────┐  ┌────────────────────────┐  │
│  │ Central Bank │  │Bank of Siberia│  │   Cash Advance Bridge  │  │
│  │  (Emission)  │  │(Credit Ops)  │  │  (Liquidity Gateway)   │  │
│  └──────┬───────┘  └──────┬───────┘  └──────────┬─────────────┘  │
│         │                 │                       │               │
│         └─────────────────┴───────────────────────┘               │
│                           │                                       │
│              ┌────────────▼───────────┐                          │
│              │    Altan Vault (MPC)   │                          │
│              │  + Social Recovery     │                          │
│              └────────────┬───────────┘                          │
│                           │                                       │
│  ┌────────────────────────▼────────────────────────────────────┐ │
│  │              Governance & Constitution Layer                 │ │
│  │  Khural DAO · InomadElections · CitizenVoice · Chancery    │ │
│  └─────────────────────────────────────────────────────────────┘ │
└──────────────────────────────────────────────────────────────────┘
         │                                          │
         ▼                                          ▼
  ┌─────────────┐                         ┌─────────────────┐
  │  Next.js    │                         │   NestJS API    │
  │  Frontend   │◄───── Polkadot.js ─────►│   (Off-chain    │
  │  (UI Layer) │                         │   Indexer/Auth) │
  └─────────────┘                         └─────────────────┘
```

### Key Design Principles

| Principle | Implementation |
|---|---|
| **Separation of Monetary Powers** | `pallet-central-bank` is sole ALTAN emitter; `pallet-bank-of-siberia` never calls `deposit_creating` directly |
| **Keyless Sub-Accounts** | Deterministic Blake2_256 derivation — no new seed phrases per account type |
| **Social Recovery Compatibility** | All ownership checks use `ensure_signed(origin)` — recovered identity inherits all sub-accounts |
| **Constitutional Enforcement** | Economic parameters are dispatchables of `pallet-constitution`, requiring supermajority Khural vote to change |
| **ZKP Privacy** | `pallet-shielded-vaults` uses Groth16 proofs for private balance operations |

---

## Cash Advance Bridge

The **Cash Advance Bridge** (CAB) is the core innovation of this grant proposal. It enables short-term, trustless liquidity draw-downs against on-chain collateral, bridging the gap between illiquid digital-state assets and immediate spending needs.

### Protocol Flow

```
  Citizen                Bank of Siberia            Central Bank
     │                        │                          │
     │── lock_collateral() ──►│                          │
     │                        │── verify_collateral() ──►│
     │                        │◄── collateral_ok ────────│
     │◄── advance_issued() ───│                          │
     │                        │                          │
     │    (uses advance)      │                          │
     │                        │                          │
     │── repay_advance() ─────►│                         │
     │                        │── unlock_collateral() ──►│
     │◄── collateral_released │◄── unlocked ─────────────│
```

### CAB Parameters (Governance-Controlled)

| Parameter | Default | Governance |
|---|---|---|
| `MaxAdvanceRatio` | 70% of collateral | Khural vote |
| `AdvanceInterestRate` | 2% / 30 days | Central Bank epoch |
| `MaxAdvanceDuration` | 90 days | Khural vote |
| `LiquidationThreshold` | 110% health ratio | Constitution pallet |
| `FreezeGracePeriod` | 72 hours | Constitution pallet |

### Liquidation Guard

If a collateral position's health ratio (`collateral_value / outstanding_advance`) drops below `LiquidationThreshold`:

1. **Freeze phase** (0–72h): Account frozen; citizen notified via on-chain event.
2. **Grace repayment** (72h window): Citizen can top-up collateral or repay advance.
3. **Forced liquidation** (>72h): `pallet-bank-of-siberia` transfers collateral to State Treasury; position closed; `AdvanceLiquidated` event emitted.

All three phases are executed by on-chain dispatchables — **no off-chain keeper bots required**.

---

## Substrate Module Structure

### Core Financial Pallets

| Pallet | Path | Description |
|---|---|---|
| `pallet-central-bank` | `pallets/central-bank/` | Primary ALTAN issuance, monetary policy epochs |
| `pallet-bank-of-siberia` | `pallets/bank-of-siberia/` | Credit lines, loans, escrow, time deposits |
| `pallet-altan-vault` | `pallets/altan-vault/` | MPC custody, keyless sub-accounts, social recovery |
| `pallet-altan-tax` | `pallets/altan-tax/` | On-chain transaction tax collection |
| `pallet-annual-profit-tax` | `pallets/annual-profit-tax/` | Annual profit tax assessment |
| `pallet-shielded-vaults` | `pallets/shielded-vaults/` | Groth16 ZKP private balance operations |

### Governance Pallets

| Pallet | Path | Description |
|---|---|---|
| `pallet-constitution` | `pallets/constitution/` | Constitutional rule storage and enforcement |
| `pallet-khural-governance` | `pallets/khural-governance/` | Parliamentary voting (Khural DAO) |
| `pallet-inomad-elections` | `pallets/inomad-elections/` | Ranked-choice election module |
| `pallet-citizen-voice` | `pallets/citizen-voice/` | Direct citizen referendum |
| `pallet-decimal-dao` | `pallets/decimal-dao/` | Conviction-weighted voting |
| `pallet-judicial-courts` | `pallets/judicial-courts/` | Dispute resolution, fine enforcement |

### Identity & Civil Registry Pallets

| Pallet | Path | Description |
|---|---|---|
| `pallet-inomad-identity` | `pallets/inomad-identity/` | Citizen identity, KYC hooks |
| `pallet-land-registry` | `pallets/land-registry/` | On-chain land title management |
| `pallet-inheritance` | `pallets/inheritance/` | Digital asset inheritance |
| `pallet-migration-center` | `pallets/migration-center/` | Cross-jurisdiction migration |
| `pallet-chancery` | `pallets/chancery/` | Official document issuance |
| `pallet-chronicles` | `pallets/chronicles/` | Immutable state history |

### Economic Activity Pallets

| Pallet | Path | Description |
|---|---|---|
| `pallet-organization` | `pallets/organization/` | Corporate entity registration |
| `pallet-guilds` | `pallets/guilds/` | Trade guild membership and fees |
| `pallet-licensing` | `pallets/licensing/` | Business license issuance |
| `pallet-bank-operator` | `pallets/bank-operator/` | Licensed bank operator registration |
| `pallet-foreign-affairs` | `pallets/foreign-affairs/` | Cross-chain treaty management |
| `pallet-forums` | `pallets/forums/` | Decentralized deliberation layer |
| `pallet-black-book` | `pallets/black-book/` | Sanctions and compliance registry |
| `pallet-steppe-offline` | `pallets/steppe-offline/` | Offline-first signed transaction queue |

### NFT & Access Pallets

| Pallet | Path | Description |
|---|---|---|
| `pallet-access-nft` | `pallets/access-nft/` | Role-based access NFTs |
| `pallet-achievement-nft` | `pallets/achievement-nft/` | Citizen achievement badges |
| `pallet-recovery-nft` | `pallets/recovery-nft/` | Social recovery guardianship tokens |

---

## Altan Token Economics

**ALTAN** is the native utility token of the Altan Network, issued exclusively by `pallet-central-bank`.

```
Total Initial Supply:    Governed by Constitution (no hard cap — epoch-based issuance)
Issuance Model:          Credit-epoch-based (Central Bank proposes, Khural approves)
Transaction Tax:         Configurable via pallet-altan-tax (default: 0.5%)
Collateral Types:        ALTAN (primary), future: XCM-bridged DOT/KSM
Interest on Time Deposits: Set per epoch by Central Bank
```

### Token Flow Diagram

```
  Central Bank (Epoch Issuance)
         │
         ▼
  Bank of Siberia Treasury
         │
    ┌────┴─────────────────────┐
    │                          │
    ▼                          ▼
Loan Disbursements        Time Deposit Reserves
    │                          │
    ▼                          ▼
 Citizens ←── Repayments ──► Citizens
    │                          │
    └──── Tax ──► State Treasury
```

---

## Local Development Setup

### Prerequisites

| Tool | Version |
|---|---|
| Rust | `stable` (see `rust-toolchain.toml`) |
| Cargo | Bundled with Rust |
| Node.js | `≥ 20.x` |
| npm | `≥ 10.x` |
| PostgreSQL | `≥ 15.x` |

### 1. Clone and Setup Environment

```bash
git clone https://github.com/YOUR_ORG/altan-network.git
cd altan-network

# Copy environment template — fill in your own values
cp backend/.env.example backend/.env
```

### 2. Build the Substrate Node

```bash
# Install Rust wasm target
rustup target add wasm32-unknown-unknown

# Build in release mode (takes ~10–20 min first time)
cd altan-network
cargo build --release

# Verify build
./target/release/altan-node --version
```

### 3. Start a Local Development Chain

```bash
# Start a single-node local chain with Alice as validator
./target/release/altan-node \
  --dev \
  --tmp \
  --rpc-external \
  --rpc-cors=all \
  --rpc-port=9944
```

You can now connect to the chain via [Polkadot.js Apps](https://polkadot.js.org/apps/?rpc=ws://localhost:9944).

### 4. Start the Backend (NestJS)

```bash
cd backend
npm install

# Run database migrations
npx prisma migrate dev

# Start backend in development mode
npm run start:dev
```

Backend API will be available at `http://localhost:3001`.

### 5. Start the Frontend (Next.js)

```bash
cd ..   # project root
npm install
npm run dev
```

Frontend will be available at `http://localhost:3000`.

---

## Running Tests

### Substrate Pallet Unit Tests

```bash
cd altan-network

# Run all pallet tests
cargo test --workspace

# Run a specific pallet
cargo test -p pallet-bank-of-siberia

# Run with output (for debugging)
cargo test -p pallet-bank-of-siberia -- --nocapture
```

### Backend Unit & Integration Tests

```bash
cd backend

# Unit tests
npm run test

# Integration tests (requires running PostgreSQL)
npm run test:e2e

# Coverage report
npm run test:cov
```

### Frontend Tests (Playwright E2E)

```bash
# From project root
npx playwright test
```

---

## Security Model

The security of INOMAD OS relies on three pillars:

1. **On-chain enforcement** — Liquidation, fine collection, and collateral management are dispatchables; they require no off-chain keepers and are auditable by any network participant.

2. **Constitutional constraints** — Critical parameters (collateral ratios, interest rates, taxation) can only be changed via supermajority Khural vote (`pallet-constitution`), preventing unilateral admin manipulation.

3. **Keyless account derivation** — Citizen sub-accounts have no private key; they are controlled exclusively by the master account. This eliminates a class of key-theft attacks against secondary wallets.

For responsible disclosure of security vulnerabilities, see [SECURITY.md](./SECURITY.md).

---

## Contributing

We welcome contributions from the Polkadot/Substrate community. Please read [CONTRIBUTING.md](./CONTRIBUTING.md) before opening a pull request.

**Code of Conduct**: We follow the [Rust Code of Conduct](https://www.rust-lang.org/policies/code-of-conduct).

---

## License

Copyright 2024–2026 INOMAD OS Contributors

Licensed under the Apache License, Version 2.0 (the "License");
you may not use this file except in compliance with the License.
You may obtain a copy of the License at

http://www.apache.org/licenses/LICENSE-2.0

Unless required by applicable law or agreed to in writing, software distributed under the License is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied. See the [LICENSE](./LICENSE) file for the full license text.
