//! Mock runtime for pallet-bank-of-siberia unit tests.

use crate as pallet_bank_of_siberia;
use frame_support::{derive_impl, parameter_types, PalletId};
use sp_runtime::{BuildStorage, Perbill};

type Block = frame_system::mocking::MockBlock<Test>;

// ─────────────────────────────────────────────────────────────────────────────
// Runtime definition
// ─────────────────────────────────────────────────────────────────────────────

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
    pub struct Test;

    #[runtime::pallet_index(0)]
    pub type System = frame_system::Pallet<Test>;

    #[runtime::pallet_index(1)]
    pub type Balances = pallet_balances::Pallet<Test>;

    #[runtime::pallet_index(2)]
    pub type BankOfSiberia = pallet_bank_of_siberia::Pallet<Test>;

    #[runtime::pallet_index(3)]
    pub type MigrationCenter = pallet_migration_center::Pallet<Test>;

    #[runtime::pallet_index(4)]
    pub type CentralBank = pallet_central_bank::Pallet<Test>;

    #[runtime::pallet_index(5)]
    pub type Proxy = pallet_proxy::Pallet<Test>;
}

// ─────────────────────────────────────────────────────────────────────────────
// frame-system
// ─────────────────────────────────────────────────────────────────────────────

#[derive_impl(frame_system::config_preludes::TestDefaultConfig)]
impl frame_system::Config for Test {
    type Block = Block;
    type AccountData = pallet_balances::AccountData<u64>;
}

// ─────────────────────────────────────────────────────────────────────────────
// pallet-balances
// ─────────────────────────────────────────────────────────────────────────────

#[derive_impl(pallet_balances::config_preludes::TestDefaultConfig)]
impl pallet_balances::Config for Test {
    type AccountStore = System;
}

// ─────────────────────────────────────────────────────────────────────────────
// pallet-bank-of-siberia
// ─────────────────────────────────────────────────────────────────────────────

parameter_types! {
    pub const MaxOfficers: u32 = 100;
}

impl pallet_migration_center::Config for Test {
    type MaxOfficers = MaxOfficers;
}

impl pallet_proxy::Config for Test {
    type RuntimeEvent = RuntimeEvent;
    type RuntimeCall = RuntimeCall;
    type Currency = Balances;
    type ProxyType = ();
    type ProxyDepositBase = frame_support::traits::ConstU64<1>;
    type ProxyDepositFactor = frame_support::traits::ConstU64<1>;
    type MaxProxies = frame_support::traits::ConstU32<32>;
    type WeightInfo = ();
    type MaxPending = frame_support::traits::ConstU32<32>;
    type CallHasher = sp_runtime::traits::BlakeTwo256;
    type AnnouncementDepositBase = frame_support::traits::ConstU64<1>;
    type AnnouncementDepositFactor = frame_support::traits::ConstU64<1>;
    type BlockNumberProvider = System;
}

/// Mock CitizenIdentity for bank-of-siberia tests.
/// Returns virtually unlimited credit (no identity check needed for BOS tests).
pub struct MockCitizenIdentity;
impl pallet_central_bank::CitizenIdentity<u64, u64> for MockCitizenIdentity {
    fn get_credit_limit(_who: &u64) -> Option<u64> {
        Some(u64::MAX / 2) // Virtually unlimited
    }
}

impl pallet_central_bank::Config for Test {
    type Currency = Balances;
    type BankingOrigin = frame_system::EnsureRoot<u64>;
    type CitizenIdentity = MockCitizenIdentity;
    type CreditEpochLimit = frame_support::traits::ConstU64<18_900_000_000_000>;
    type CreatorAccount = frame_support::traits::ConstU64<1>;
    type CentralBankAccountId = frame_support::traits::ConstU64<999>;
    type WeightInfo = ();
    // Two-phase curve constants (mirrors production values)
    type OptimalKeyRate    = frame_support::traits::ConstU32<850>;  // 8.5%
    type MaxProtectiveRate = frame_support::traits::ConstU32<5000>; // 50%
    type OptimalUtilization = frame_support::traits::ConstU32<80>;  // 80%
}

parameter_types! {
    /// Bank of Siberia treasury PalletId.
    pub const BankOfSiberiaPalletId: PalletId = PalletId(*b"bos/depo");
    /// Constitutional cross-transfer fee: 0.03% = 3/10_000 = 300_000 ppm.
    pub const TestCrossTransferFee: Perbill = Perbill::from_parts(300_000);
    /// Constitutional state income tax: 10%.
    pub const TestStateTaxRate: Perbill = Perbill::from_percent(10);
}

impl pallet_bank_of_siberia::Config for Test {
    type Currency = Balances;
    type PalletId = BankOfSiberiaPalletId;
    /// BankingOrigin = Root in tests (mirrors EnsureRootOrBankingCouncil in prod).
    type BankingOrigin = frame_system::EnsureRoot<u64>;
    type CrossTransferFee = TestCrossTransferFee;
    type StateTaxRate = TestStateTaxRate;
    type WeightInfo = ();
}

// ─────────────────────────────────────────────────────────────────────────────
// Test externalities
// ─────────────────────────────────────────────────────────────────────────────

/// Initial balances for test accounts.
pub const ALICE_BALANCE: u64 = 1_000_000_000_000_000;
pub const BOB_BALANCE: u64 = 1_000_000_000_000_000;
/// Treasury seed — covers interest payouts in Time Deposit tests.
pub const TREASURY_SEED: u64 = 10_000_000_000_000_000;

pub fn new_test_ext() -> sp_io::TestExternalities {
    let mut t = frame_system::GenesisConfig::<Test>::default()
        .build_storage()
        .unwrap();

    let treasury = pallet_bank_of_siberia::Pallet::<Test>::treasury_account();

    pallet_balances::GenesisConfig::<Test> {
        balances: vec![
            (1u64, ALICE_BALANCE), // ALICE
            (2u64, BOB_BALANCE),   // BOB
            (3u64, 1_000_000_000_000_000), // EVE
            (4u64, 1_000_000_000_000_000), // Pension Fund for Region 1
            (treasury, TREASURY_SEED),
        ],
        dev_accounts: None,
    }
    .assimilate_storage(&mut t)
    .unwrap();

    let mut ext: sp_io::TestExternalities = t.into();
    ext.execute_with(|| {
        let anchor = pallet_migration_center::ApplicationAnchor::<Test> {
            application_id_hash: sp_core::H256::zero(),
            data_hash: sp_core::H256::zero(),
            submitted_at_block: 1,
            status: pallet_migration_center::AnchorStatus::Approved,
            officer: None,
            officer_claim_sig: None,
            claimed_at_block: None,
            outcome_hash: None,
            finalized_at_block: None,
        };
        pallet_migration_center::ApplicationAnchors::<Test>::insert(1u64, anchor.clone());
        pallet_migration_center::ApplicationAnchors::<Test>::insert(2u64, anchor.clone());
        pallet_migration_center::ApplicationAnchors::<Test>::insert(3u64, anchor);
        
        // Mock Pension Fund for OKATO region code 1 (Республика Адыгея)
        // Key is u8 OKATO code, not sequential u16 index
        pallet_central_bank::PensionFunds::<Test>::insert(1u8, 4u64);
    });
    ext
}
