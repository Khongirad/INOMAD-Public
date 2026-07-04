# Contributing to Altan Network

Thank you for your interest in contributing to the Altan Network — the sovereign L1 blockchain of INOMAD OS.

---

## ⚠️ Before You Contribute

This repository implements the **Altan Constitution** — a legally binding document governing a sovereign digital state. All contributions must be constitutionally compatible.

**Immutable constants** (cannot be changed by PRs):
- Genesis supply cap: 2,100,000,000,000 ALTAN
- Fee split: 54% Khural / 26% INOMAD AG / 10% Creator / 10% Validators
- Authentication: sr25519 only (no email/password per §10)

---

## Development Environment

### Prerequisites

```bash
# Install Rust
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
rustup target add wasm32-unknown-unknown

# Verify
cargo --version
rustc --version
```

### Build

```bash
# Clone
git clone https://github.com/KhongiradInomad/altan-network
cd altan-network

# Build
cargo build --release

# Test
cargo test --all

# Lint
cargo clippy -- -D warnings
cargo fmt --check
```

### Run Dev Node

```bash
./target/release/altan-node --dev --tmp
```

---

## Code Quality Standards

All PRs **must** pass:

```bash
cargo test --all              # All tests green
cargo clippy -- -D warnings   # Zero warnings
cargo fmt --check             # Formatted correctly
cargo doc --no-deps           # Docs build without errors
```

These are enforced in CI (`.github/workflows/ci.yml`).

---

## Pallet Development

### Structure

Each pallet follows the FRAME v2 pattern:

```
pallets/<pallet-name>/
├── Cargo.toml
└── src/
    ├── lib.rs          # Pallet implementation
    ├── mock.rs         # Test mock runtime
    └── tests.rs        # Unit tests
```

### Requirements for New Pallets

1. **Module-level docs** — `//!` block at top of `lib.rs`
2. **Unit tests** — minimum 5 tests covering happy path and error cases
3. **Constitutional compatibility** — any new fee logic must respect the 54/26/10/10 split
4. **Dispatchable docs** — every `#[pallet::call]` must have rustdoc
5. **Error docs** — every `#[pallet::error]` variant must have rustdoc
6. **Event docs** — every `#[pallet::event]` variant must have rustdoc

### Example Dispatchable Documentation

```rust
/// Transfer tokens with constitutional fee routing.
///
/// Deducts a 0.03% fee (capped at 1,000 ALTAN) from the sender and routes
/// it according to the 54/26/10/10 constitutional split.
///
/// # Parameters
/// - `origin`: Signed sender account
/// - `dest`: Recipient account
/// - `amount`: Transfer amount in planck (10^-12 ALTAN)
///
/// # Errors
/// - [`Error::TreasuriesNotInitialized`]: Treasury accounts not configured in genesis
/// - [`Error::InsufficientBalance`]: Sender has insufficient funds including fee
///
/// # Events
/// - [`Event::FeeRouted`]: Emitted when fee is successfully distributed
#[pallet::call_index(0)]
#[pallet::weight(T::WeightInfo::transfer_with_fee())]
pub fn transfer_with_fee(origin: OriginFor<T>, dest: T::AccountId, amount: BalanceOf<T>) -> DispatchResult {
    // ...
}
```

---

## Pull Request Process

1. Fork the repository
2. Create a feature branch: `git checkout -b feat/my-feature`
3. Write tests for your changes
4. Ensure all CI checks pass
5. Submit PR with a clear description referencing the constitutional article (if applicable)

### PR Title Format

```
feat(pallet-name): add constitutional X feature
fix(pallet-name): resolve Y error in Z dispatchable
docs(pallet-name): add rustdoc for public API
test(pallet-name): add coverage for edge case Z
```

---

## Questions

- **Substrate Stack Exchange**: https://substrate.stackexchange.com
- **Polkadot Discord**: https://discord.gg/polkadot
- **Security issues**: security@inomad.life (see [SECURITY.md](SECURITY.md))
