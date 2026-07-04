//! Mock runtime for pallet-buryad-mongol-mixer.
//!
//! Origins (Разделение Властей):
//! - RelayerOrigin  = EnsureBankOfSiberia  (AccountId = 13) → withdraw
//! - BankBoardOrigin = EnsureBankBoard     (AccountId = 20) → reveal_transaction
//! - KhuralOrigin   = EnsureKhural         (AccountId = 21) → submit_quarterly_audit

use crate as pallet_buryad_mongol_mixer;
use frame_support::{
    construct_runtime, derive_impl, parameter_types,
    traits::{ConstU128, EnsureOrigin},
    PalletId,
};
use sp_runtime::{traits::AccountIdConversion, BuildStorage, Permill};

pub type AccountId = u64;
pub type Balance = u128;
pub type Block = frame_system::mocking::MockBlock<Test>;

/// 1 ALTAN = 10^12 planck.
pub const UNIT: Balance = 1_000_000_000_000;

// ── Test Accounts ─────────────────────────────────────────────────────────────
pub const ALICE: AccountId = 1;
pub const BOB: AccountId = 2;
pub const CHARLIE: AccountId = 3;
pub const INOMAD_AG: AccountId = 10;
pub const VALIDATORS_POOL: AccountId = 11;
pub const BANK: AccountId = 12;
pub const BANK_OF_SIBERIA: AccountId = 13; // RelayerOrigin
pub const BANK_BOARD: AccountId = 20; // BankBoardOrigin
pub const KHURAL: AccountId = 21; // KhuralOrigin
pub const CREATOR: AccountId = 99; // Sovereign Creator / Citizen #1 (10% fee)

construct_runtime!(
    pub enum Test {
        System:   frame_system,
        Balances: pallet_balances,
        Mixer:    pallet_buryad_mongol_mixer,
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
    /// Existential Deposit: 0.001 ALTAN (конституция §3.2).
    type ExistentialDeposit = ConstU128<1_000_000_000>;
    type AccountStore = System;
}

// ── EnsureBankOfSiberia — RelayerOrigin ──────────────────────────────────────
pub struct EnsureBankOfSiberia;
impl EnsureOrigin<RuntimeOrigin> for EnsureBankOfSiberia {
    type Success = AccountId;
    fn try_origin(o: RuntimeOrigin) -> Result<Self::Success, RuntimeOrigin> {
        match o.clone().into() {
            Ok(frame_system::RawOrigin::Signed(who)) if who == BANK_OF_SIBERIA => Ok(who),
            Ok(frame_system::RawOrigin::Signed(who)) => Err(RuntimeOrigin::signed(who)),
            _ => Err(o),
        }
    }
    #[cfg(feature = "runtime-benchmarks")]
    fn try_successful_origin() -> Result<RuntimeOrigin, ()> {
        Ok(RuntimeOrigin::signed(BANK_OF_SIBERIA))
    }
}

// ── EnsureBankBoard — BankBoardOrigin ────────────────────────────────────────
pub struct EnsureBankBoard;
impl EnsureOrigin<RuntimeOrigin> for EnsureBankBoard {
    type Success = AccountId;
    fn try_origin(o: RuntimeOrigin) -> Result<Self::Success, RuntimeOrigin> {
        match o.clone().into() {
            Ok(frame_system::RawOrigin::Signed(who)) if who == BANK_BOARD => Ok(who),
            Ok(frame_system::RawOrigin::Signed(who)) => Err(RuntimeOrigin::signed(who)),
            _ => Err(o),
        }
    }
    #[cfg(feature = "runtime-benchmarks")]
    fn try_successful_origin() -> Result<RuntimeOrigin, ()> {
        Ok(RuntimeOrigin::signed(BANK_BOARD))
    }
}

// ── EnsureKhural — KhuralOrigin ──────────────────────────────────────────────
pub struct EnsureKhural;
impl EnsureOrigin<RuntimeOrigin> for EnsureKhural {
    type Success = ();
    fn try_origin(o: RuntimeOrigin) -> Result<Self::Success, RuntimeOrigin> {
        match o.clone().into() {
            Ok(frame_system::RawOrigin::Signed(who)) if who == KHURAL => Ok(()),
            Ok(frame_system::RawOrigin::Signed(who)) => Err(RuntimeOrigin::signed(who)),
            _ => Err(o),
        }
    }
    #[cfg(feature = "runtime-benchmarks")]
    fn try_successful_origin() -> Result<RuntimeOrigin, ()> {
        Ok(RuntimeOrigin::signed(KHURAL))
    }
}

// ── Mixer Config ──────────────────────────────────────────────────────────────
parameter_types! {
    pub const MixerPalletId: PalletId = PalletId(*b"bmm/pool");
    /// 1,000 ALTAN. Допустимо: 1000, 2000. Недопустимо: 1500, 1560.
    pub const MixerDenomination: Balance = 1_000 * UNIT;
    /// 0.05% = 500 ppm.
    pub const MixerFeePermill: Permill = Permill::from_parts(500);
    /// MaxMixerFee = 10 ALTAN (малый cap — fee-cap тест: 25000 ALTAN deposit).
    pub const MixerMaxFee: Balance = 10 * UNIT;
    pub const InomadAgAcc:            AccountId = INOMAD_AG;
    pub const ValidatorsPoolAcc:       AccountId = VALIDATORS_POOL;
    pub const KhuralFoundationAcc:     AccountId = BANK;    // test placeholder for Khural treasury
    pub const CreatorAcc:              AccountId = CREATOR; // Sovereign Creator (10% fee)
    pub const BaseFeePermillConst:     sp_runtime::Permill = sp_runtime::Permill::from_parts(300);
}

impl pallet_buryad_mongol_mixer::Config for Test {
    type Currency = Balances;
    type PalletId = MixerPalletId;
    type MixerDenomination = MixerDenomination;
    type MixerFeePermill = MixerFeePermill;
    type BaseFeePermill = BaseFeePermillConst;
    type MaxMixerFee = MixerMaxFee;
    type KhuralFoundationAccount = KhuralFoundationAcc;
    type InomadAgAccount = InomadAgAcc;
    type CreatorAccount = CreatorAcc;
    type ValidatorsPoolAccount = ValidatorsPoolAcc;
    type RelayerOrigin = EnsureBankOfSiberia;
    type BankBoardOrigin = EnsureBankBoard;
    type KhuralOrigin = EnsureKhural;
    type WeightInfo = ();
}

// ── Fixtures ──────────────────────────────────────────────────────────────────
pub fn commitment(seed: u8) -> [u8; 32] {
    [seed; 32]
}
pub fn nullifier(seed: u8) -> [u8; 32] {
    [seed | 0x80; 32]
}
pub fn empty_payload() -> frame_support::BoundedVec<u8, frame_support::traits::ConstU32<64>> {
    Default::default()
}
pub fn warrant(id: u8) -> frame_support::BoundedVec<u8, frame_support::traits::ConstU32<256>> {
    let v = alloc::vec![id; 8];
    frame_support::BoundedVec::try_from(v).unwrap()
}

// ── Genesis ───────────────────────────────────────────────────────────────────
pub fn new_test_ext() -> sp_io::TestExternalities {
    let pool: AccountId = PalletId(*b"bmm/pool").into_account_truncating();
    let mut t = frame_system::GenesisConfig::<Test>::default()
        .build_storage()
        .unwrap();
    pallet_balances::GenesisConfig::<Test> {
        balances: vec![
            (ALICE, 1_000_000 * UNIT),
            (BOB, 10_000 * UNIT),
            (CHARLIE, 5_000 * UNIT),
            (INOMAD_AG, UNIT),
            (VALIDATORS_POOL, UNIT),
            (BANK, UNIT),
            (BANK_OF_SIBERIA, UNIT),
            (BANK_BOARD, UNIT),
            (KHURAL, UNIT),
            (CREATOR, UNIT), // Sovereign Creator treasury
            (pool, UNIT),
        ],
        dev_accounts: None,
    }
    .assimilate_storage(&mut t)
    .unwrap();
    let mut ext = sp_io::TestExternalities::new(t);
    ext.execute_with(|| System::set_block_number(1));
    ext
}

#[test]
fn mock_genesis_seeded_correctly() {
    new_test_ext().execute_with(|| {
        let pool: AccountId = PalletId(*b"bmm/pool").into_account_truncating();
        assert_eq!(Balances::free_balance(ALICE), 1_000_000 * UNIT);
        assert!(Balances::free_balance(&pool) > 0);
    });
}
