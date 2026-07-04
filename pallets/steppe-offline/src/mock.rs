//! Mock runtime for pallet-steppe-offline unit tests.
//!
//! Uses a `MockSignature` that always passes verification — standard Substrate
//! pattern for testing signature-gated extrinsics without real key material.

use codec::{Decode, DecodeWithMemTracking, Encode, MaxEncodedLen};
use frame_support::{construct_runtime, derive_impl, traits::ConstU128};
use scale_info::TypeInfo;
use sp_runtime::traits::{IdentifyAccount, Verify};
use sp_runtime::BuildStorage;
use std::cell::RefCell;

// ── Types ─────────────────────────────────────────────────────────────────

pub type AccountId = u64;
pub type Balance = u128;
pub type Block = frame_system::mocking::MockBlock<Test>;

pub const UNIT: Balance = 1_000_000_000_000; // 1 ALTAN

// ── Thread-local slash tracker ────────────────────────────────────────────

thread_local! {
    pub static SLASHED: RefCell<Vec<AccountId>> = RefCell::new(Vec::new());
}

pub fn was_slashed(who: AccountId) -> bool {
    SLASHED.with(|s| s.borrow().contains(&who))
}

pub fn clear_slashed() {
    SLASHED.with(|s| s.borrow_mut().clear());
}

// ── Mock SlashInterface ───────────────────────────────────────────────────

pub struct MockSlash;

impl crate::SlashInterface<AccountId> for MockSlash {
    fn slash_citizen(who: &AccountId) -> frame_support::dispatch::DispatchResult {
        SLASHED.with(|s| s.borrow_mut().push(*who));
        Ok(())
    }
}

// ── MockSignature: always valid ───────────────────────────────────────────
//
// A real sr25519 signature would require secret keys.  Unit tests would be
// flaky and slow.  Instead we inject a no-op signature type that passes
// `verify()` unconditionally.  This isolates the *economic logic* from
// cryptography — the crypto path is covered by integration tests.

#[derive(
    Clone,
    PartialEq,
    Eq,
    Encode,
    Decode,
    DecodeWithMemTracking,
    TypeInfo,
    MaxEncodedLen,
    Debug,
    Default,
)]
pub struct MockSignature;

/// MockPublic resolves to the raw u64 AccountId.
#[derive(
    Clone,
    PartialEq,
    Eq,
    Encode,
    Decode,
    DecodeWithMemTracking,
    TypeInfo,
    MaxEncodedLen,
    Debug,
    Default,
)]
pub struct MockPublic(pub AccountId);

impl IdentifyAccount for MockPublic {
    type AccountId = AccountId;
    fn into_account(self) -> AccountId {
        self.0
    }
}

impl Verify for MockSignature {
    type Signer = MockPublic;

    fn verify<L: sp_runtime::traits::Lazy<[u8]>>(&self, _msg: L, _signer: &AccountId) -> bool {
        // Always passes — unit tests focus on economic logic, not cryptography.
        true
    }
}

// `Parameter` (Codec + Clone + Eq + Debug + TypeInfo + Send + Sync + 'static)
// is satisfied automatically by the derives above via Substrate blanket impls.

// ── construct_runtime ─────────────────────────────────────────────────────

construct_runtime!(
    pub enum Test {
        System:   frame_system,
        Balances: pallet_balances,
        Steppe:   crate,
    }
);

#[derive_impl(frame_system::config_preludes::TestDefaultConfig)]
impl frame_system::Config for Test {
    type Block = Block;
    type AccountData = pallet_balances::AccountData<Balance>;
}

#[derive_impl(pallet_balances::config_preludes::TestDefaultConfig)]
impl pallet_balances::Config for Test {
    type Balance = Balance;
    type ExistentialDeposit = ConstU128<1_000_000_000>; // 0.001 ALTAN
    type AccountStore = System;
}

// ── Steppe Config ─────────────────────────────────────────────────────────

impl crate::Config for Test {
    type Currency = Balances;
    type IdentitySlashing = MockSlash;
    type Signature = MockSignature;
    type WeightInfo = ();
}

// ── Genesis ───────────────────────────────────────────────────────────────

pub const ALICE: AccountId = 1;
pub const BOB: AccountId = 2;

pub fn new_test_ext() -> sp_io::TestExternalities {
    let mut t = frame_system::GenesisConfig::<Test>::default()
        .build_storage()
        .unwrap();

    pallet_balances::GenesisConfig::<Test> {
        balances: vec![(ALICE, 1_000 * UNIT), (BOB, 1_000 * UNIT)],
        dev_accounts: None,
    }
    .assimilate_storage(&mut t)
    .unwrap();

    let mut ext = sp_io::TestExternalities::new(t);
    ext.execute_with(|| System::set_block_number(1));
    ext
}

/// Build an IOU payload (the data Alice signs offline).
pub fn iou(amount: Balance, nonce: u64) -> crate::IouPayload<Balance> {
    crate::IouPayload { amount, nonce }
}
