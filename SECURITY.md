# Security Policy — Altan Network

## Supported Versions

| Version | Supported          |
| ------- | ------------------ |
| `main`  | ✅ Yes              |
| Other   | ❌ No               |

## Reporting a Vulnerability

**Do NOT create a public GitHub issue for security vulnerabilities.**

### Responsible Disclosure

Please report vulnerabilities via email to:

**ceo@inomad.life**

Include:
1. Description of the vulnerability
2. Steps to reproduce
3. Affected pallet(s) or component(s)
4. Potential impact assessment
5. Suggested fix (if any)

We will acknowledge receipt within **48 hours** and aim to provide a fix within **14 days** for critical issues.

---

## Scope

### In-Scope

- All Substrate pallets in `/pallets/`
- Runtime configuration in `/runtime/`
- Node implementation in `/node/`
- ZK proof circuits (snarkjs/circom)
- Cryptographic operations (sr25519, blake2)

### Out-of-Scope

- Backend NestJS service (separate repository)
- Frontend Next.js (separate repository)
- Third-party dependencies (report upstream)

---

## Security Architecture

### Constitutional Invariants (Immutable)

The following constants are **baked into WASM** and cannot be changed by governance or sudo (the Foundation Fund is managed by the AG multisig):

| Constant | Value | Location |
|---|---|---|
| Genesis Supply | 2,100,000,000,000 ALTAN | `pallet-central-bank` |
| Fee Split (Khural Treasury) | 54% | `pallet-altan-tax` |
| Fee Split (Foundation Fund) | 36% | `pallet-altan-tax` |
| Fee Split (Validators) | 10% | `pallet-altan-tax` → `pallet-staking` |
| Fee Rate | 0.03% (max 1,000 ALTAN) | `pallet-altan-tax` |
| CDP Collateral Ratio | 9× | `pallet-bank-operator` |

**Fee Split Notes:**

- **Khural Treasury (54%)** — On-chain account controlled exclusively by `pallet-khural-governance`. Spending requires a passed Khural proposal. No individual can access these funds unilaterally.
- **Foundation Fund (36%)** — Governed by **INOMAD AG** (registered company) and its **Board of Directors**. Funds are held in a corporate multisig account — not a personal wallet of any individual. All expenditures (ecosystem development, infrastructure, R&D, grants) require Board authorization and are subject to corporate accounting and audits. The 36% ratio is encoded in WASM and **cannot be altered by INOMAD AG or the Board** — only a runtime upgrade passed by the Khural can change it.
- **Validators (10%)** — Distributed **directly on-chain** by `pallet-staking` proportionally to each validator's stake and performance. INOMAD AG has **no role** in this distribution path — it flows automatically per-era without any off-chain intermediary.

### Authentication


- **Only sr25519** key pairs are accepted (per §10 of the Altan Constitution)
- Email/password authentication is constitutionally prohibited
- All citizen interactions require cryptographic signature

### ZK Privacy

- `pallet-shielded-vaults` uses Groth16 proofs (snarkjs/circom)
- Denomination constraint: deposits must be multiples of 10 ALTAN (anonymity set protection)
- Double-spend prevention via nullifier registry

---

## Bug Bounty

We are working toward a formal bug bounty program. Until then, we recognize and credit responsible disclosures in our release notes.
