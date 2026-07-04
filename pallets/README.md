# Altan Network — Open-Source Pallets

> **Altan Network** is the L1 blockchain of INOMAD OS — a constitutional state governance infrastructure for the peoples of the Russian Federation, built on Substrate/FRAME.

This directory contains the **5 pallets open-sourced** for the Polkadot ecosystem as part of the Web3 Foundation grant program.

[![License: Apache 2.0](https://img.shields.io/badge/License-Apache%202.0-blue.svg)](LICENSE)
[![Substrate](https://img.shields.io/badge/Built%20with-Substrate-brightgreen)](https://substrate.dev)
[![Node](https://img.shields.io/badge/L1%20Node-wss%3A%2F%2Fnode.inomad.life%2Fws-orange)](wss://node.inomad.life/ws)

---

## Pallets

| Pallet | Description | Status |
|--------|-------------|--------|
| [`pallet_inomad_identity`](./pallet_inomad_identity/) | Constitutional Arbad→Zun→Myangad→Tumed hierarchy | ✅ Open |
| [`pallet_khural_governance`](./pallet_khural_governance/) | 4-branch parliamentary governance | ✅ Open |
| [`pallet_inomad_elections`](./pallet_inomad_elections/) | Multi-tier sovereign elections | ✅ Open |
| [`pallet_decimal_dao`](./pallet_decimal_dao/) | Chinggis-scale decimal DAO primitive | ✅ Open |
| [`pallet_steppe_offline`](./pallet_steppe_offline/) | Offline consensus for nomadic networks | 🔬 Research |

---

## The Chinggis Principle

These pallets encode an 800-year-old governance model as runtime-enforced invariants:

```
Arbad   =     10 citizens  →  family unit
Zun     =    100 citizens  →  10 Arbads  (clan)
Myangad =  1,000 citizens  →  10 Zuns   (district)
Tumed   = 10,000 citizens  →  10 Myangads (region)
```

**Every limit is a WASM runtime invariant** — no amount of off-chain action can bypass it.

---

## pallet_inomad_identity

**The core identity and hierarchy management pallet.**

### Extrinsics

```rust
/// Create a new family Arbad unit (max 10 members)
pub fn create_arbad(origin: OriginFor<T>) -> DispatchResult

/// Join an existing Arbad
pub fn join_arbad(origin: OriginFor<T>, arbad_id: u32) -> DispatchResult

/// Form a Zun from 10 Arbads
pub fn form_zun(
    origin: OriginFor<T>,
    arbad_ids: BoundedVec<u32, ConstU32<10>>,
) -> DispatchResult

/// Form a Myangad from 10 Zuns
pub fn form_myangad(
    origin: OriginFor<T>,
    zun_ids: BoundedVec<u32, ConstU32<10>>,
) -> DispatchResult

/// Form a Tumed from 10 Myangads
pub fn form_tumed(
    origin: OriginFor<T>,
    myangad_ids: BoundedVec<u32, ConstU32<10>>,
) -> DispatchResult

/// Elect a leader for any hierarchy unit
pub fn elect_leader(
    origin: OriginFor<T>,
    unit_id: u32,
    unit_type: HierarchyLevel,
    candidate: T::AccountId,
) -> DispatchResult
```

### Storage

```rust
pub Arbads<T>: map hasher(blake2_128_concat) u32 => ArbadInfo<T>;
pub Zuns<T>: map hasher(blake2_128_concat) u32 => ZunInfo<T>;
pub Myangads<T>: map hasher(blake2_128_concat) u32 => MyangadInfo<T>;
pub Tumeds<T>: map hasher(blake2_128_concat) u32 => TumedInfo<T>;
pub CitizenUnit<T>: map hasher(blake2_128_concat) T::AccountId => UnitMembership<T>;
```

### Errors

```rust
pub enum Error<T> {
    ArbadFull,              // > 10 members
    ZunCapacityExceeded,    // > 10 Arbads
    MyangadCapacityExceeded,// > 10 Zuns
    TumedCapacityExceeded,  // > 10 Myangads
    AlreadyMember,
    NotAMember,
    NotLeader,
    UnitNotFound,
}
```

---

## pallet_khural_governance

**Four-branch parliamentary governance pallet.**

Implements the constitutional separation of powers:
- **Legislative** (Supreme Khural) — laws, budget approval
- **Executive** (Government) — implementation, spending orders  
- **Judicial** (Supreme Court) — constitutional review, verdicts
- **Banking** (Bank of Siberia) — monetary policy, treasury

```rust
pub fn propose_law(origin, title: Vec<u8>, content: Vec<u8>) -> DispatchResult
pub fn vote_on_proposal(origin, proposal_id: u32, vote: Vote) -> DispatchResult
pub fn approve_spending(origin, order_id: u32, branch: Branch) -> DispatchResult
pub fn veto_proposal(origin, proposal_id: u32, branch: Branch) -> DispatchResult
pub fn ratify_constitutional_amendment(origin, amendment_id: u32) -> DispatchResult
```

---

## pallet_inomad_elections

**Sovereign multi-tier elections pallet.**

Supports:
- **Direct democracy** — 1 citizen = 1 vote within Arbad
- **Representative** — elected delegates vote at higher tiers
- **Ranked choice** — preference voting for Tumed leadership
- **Emergency recall** — 66% supermajority to recall any leader

```rust
pub fn open_election(origin, unit_id: u32, election_type: ElectionType) -> DispatchResult
pub fn nominate_candidate(origin, election_id: u32) -> DispatchResult
pub fn cast_vote(origin, election_id: u32, candidate: T::AccountId) -> DispatchResult
pub fn finalize_election(origin, election_id: u32) -> DispatchResult
pub fn recall_leader(origin, unit_id: u32, petition_signatures: u32) -> DispatchResult
```

---

## pallet_decimal_dao

**Chinggis-scale DAO primitive — reusable by any Substrate chain.**

A general-purpose DAO implementation that enforces decimal capacity constraints at each tier. **Not specific to INOMAD** — any community can deploy it with their own `MaxMembersPerUnit` config.

```rust
// Runtime config
pub trait Config: frame_system::Config {
    type MaxMembersPerUnit: Get<u32>;  // = 10 for Chinggis, any value works
    type MaxUnitsPerTier: Get<u32>;    // = 10 for Chinggis
    type MaxTiers: Get<u32>;           // = 4 for INOMAD (Arbad/Zun/Myangad/Tumed)
}
```

This pallet is the **reusable contribution** to the Polkadot ecosystem — any community that wants a structured, capacity-enforced governance hierarchy can use it without any INOMAD-specific dependencies.

---

## pallet_steppe_offline

**Research pallet: Offline consensus for nomadic/low-connectivity networks.**

> ⚠️ Status: Architecture complete, implementation in progress. Seeking Parity R&D collaboration.

**Problem**: Nomadic populations in Siberia, Mongolia, and Central Asia have intermittent internet connectivity. Standard Substrate consensus requires constant connectivity for transaction submission.

**Solution**: An offline transaction queue with deterministic replay and conflict resolution:

```
┌─────────────────────────────────────────────────────────┐
│  Offline Mode                                           │
│                                                         │
│  Citizen signs txs locally  →  SteppeQueue (device)    │
│  Connectivity restored      →  Batch sync to node       │
│  Conflict resolution        →  Hierarchy-priority rules │
│  Finality                   →  GRANDPA inclusion        │
└─────────────────────────────────────────────────────────┘
```

**Conflict resolution algorithm** (Chinggis Priority):
1. Transactions from higher hierarchy tiers take priority
2. Within same tier: timestamp ordering
3. Mutually exclusive txs: the one with more endorsers wins

---

## Integration Guide

### Add to your runtime

```toml
# Cargo.toml
[dependencies]
pallet-inomad-identity = { git = "https://github.com/Khongirad/altan-network-pallets", branch = "main" }
pallet-decimal-dao = { git = "https://github.com/Khongirad/altan-network-pallets", branch = "main" }
```

```rust
// runtime/src/lib.rs
#[runtime::pallet_index(X)]
pub type InomadIdentity = pallet_inomad_identity;

impl pallet_inomad_identity::Config for Runtime {
    type RuntimeEvent = RuntimeEvent;
    type MaxArbadSize = ConstU32<10>;
    type MaxZunsPerMyangad = ConstU32<10>;
    type MaxMyangadsPerTumed = ConstU32<10>;
    type WeightInfo = pallet_inomad_identity::weights::SubstrateWeight<Runtime>;
}
```

### Run tests

```bash
cd pallets/pallet_inomad_identity
cargo test

cd pallets/pallet_decimal_dao
cargo test
```

### Start a local node

```bash
docker-compose -f docker/node.yml up
# Node starts at ws://localhost:9944
```

---

## License

All pallets in this directory are licensed under [Apache 2.0](LICENSE).

The full INOMAD OS platform (backend, frontend, remaining pallets) is available under NDA for Web3 Foundation / Parity technical review. Contact: `contact@inomad.life`.

---

## Contributing

We welcome contributions from the Polkadot ecosystem, especially:
- Performance improvements to `pallet_steppe_offline` sync algorithm
- XCM integration for `pallet_inomad_identity` cross-chain verification
- Additional `pallet_decimal_dao` unit tests and benchmarks

Open an issue or PR on this repository.

---

*INOMAD OS — State Governance Digital Sovereignty*  
*Node: `wss://node.inomad.life/ws` · Platform: `https://inomad.life`*
