//! Test suite — pallet-buryad-mongol-mixer
//!
//! ## Покрытие (23 теста)
//!
//! ### Экономика депозита (тесты 01–06)
//! 01. deposit_one_denomination_exact_fee_split
//! 02. deposit_three_denominations_succeeds
//! 03. deposit_massive_mul_fee_is_capped
//! 04. deposit_below_denomination_fails
//! 05. deposit_non_multiple_denomination_fails
//! 06. deposit_duplicate_commitment_fails
//!
//! ### Вывод (тесты 07–11)
//! 07. withdraw_recipient_gets_exact_amount
//! 08. withdraw_nonexistent_commitment_fails
//! 09. withdraw_duplicate_nullifier_fails
//! 10. pool_leaf_count_tracks_correctly
//! 11. withdraw_too_small_amount_fails
//!
//! ### Безопасность / MEV (тесты 12–15)
//! 12. withdraw_by_unauthorized_user_returns_bad_origin
//! 13. withdraw_by_authorized_relayer_succeeds
//! 14. recharge_attack_is_blocked
//! 15. deposit_emits_correct_event
//!
//! ### Governance — BankBoard (тесты 16–18)
//! 16. reveal_transaction_by_bank_board_succeeds
//! 17. reveal_transaction_by_unauthorized_fails
//! 18. reveal_transaction_stores_audit_log
//!
//! ### Governance — Khural (тесты 19–23)
//! 19. submit_quarterly_audit_by_khural_succeeds
//! 20. submit_quarterly_audit_by_unauthorized_fails
//! 21. submit_quarterly_audit_duplicate_fails
//! 22. submit_quarterly_audit_emits_event
//! 23. multiple_quarterly_audits_different_quarters

use crate::mock::*;
use crate::{AuditLogId, AuditLogs, QuarterlyAudits};
use crate::{CommitmentState, Commitments, Error, PoolLeafCount, SpentNullifiers};
use frame_support::{assert_noop, assert_ok};

// Удобные псевдонимы.
const DENOM: Balance = 1_000 * UNIT;

// =============================================================================
// Тест 01: deposit 1× denomination — точный fee split 36/10/54
// =============================================================================
#[test]
fn deposit_one_denomination_exact_fee_split() {
    new_test_ext().execute_with(|| {
        // DENOM = 1000 ALTAN.
        // Effective rate: BaseFee(300 ppm) + MixerFee(500 ppm) = 800 ppm = 0.08%
        // raw_fee = 800/1_000_000 * 1000 ALTAN = 0.8 ALTAN = 800_000_000_000 planck < cap(10) → OK
        let amount = DENOM;
        let total_fee = 800_000_000_000u128; // 0.8 ALTAN

        let before_inomad = Balances::free_balance(INOMAD_AG);
        let before_validator = Balances::free_balance(VALIDATORS_POOL);
        let before_bank = Balances::free_balance(BANK);
        let before_creator = Balances::free_balance(CREATOR);

        assert_ok!(Mixer::deposit(
            frame_system::RawOrigin::Signed(ALICE).into(),
            amount,
            commitment(1),
            empty_payload(),
        ));

        let inomad_got = Balances::free_balance(INOMAD_AG) - before_inomad;
        let validator_got = Balances::free_balance(VALIDATORS_POOL) - before_validator;
        let bank_got = Balances::free_balance(BANK) - before_bank;
        let creator_got = Balances::free_balance(CREATOR) - before_creator;

        // All 4 shares must sum exactly to total_fee (no planck lost).
        assert_eq!(
            inomad_got + validator_got + bank_got + creator_got,
            total_fee,
            "нет потерь planck"
        );
        // Validator: 10% of 0.8 ALTAN = 80_000_000_000
        assert_eq!(validator_got, 80_000_000_000, "Validators 10%");
        // Creator: 10% of 0.8 ALTAN = 80_000_000_000
        assert_eq!(creator_got, 80_000_000_000, "Creator 10%");
        // INOMAD AG: 26% of 0.8 ALTAN = 208_000_000_000
        assert_eq!(inomad_got, 208_000_000_000, "INOMAD AG 26%");
        // Khural/Bank: 54% + dust = remainder
        assert_eq!(
            bank_got,
            total_fee - inomad_got - validator_got - creator_got,
            "Bank 54%+dust"
        );
        assert_eq!(
            Commitments::<Test>::get(&commitment(1)),
            Some(CommitmentState::Active)
        );
        assert_eq!(PoolLeafCount::<Test>::get(), 1);
    });
}

// =============================================================================
// Тест 02: deposit 3× denomination — сумма комиссий без потерь
// =============================================================================
#[test]
fn deposit_three_denominations_succeeds() {
    new_test_ext().execute_with(|| {
        let amount = 3 * DENOM;
        // Effective rate: 800 ppm. fee = 800/1_000_000 * 3000 ALTAN = 2.4 ALTAN < cap
        let total_fee = 2_400_000_000_000u128;

        let bi = Balances::free_balance(INOMAD_AG);
        let bv = Balances::free_balance(VALIDATORS_POOL);
        let bb = Balances::free_balance(BANK);
        let bc = Balances::free_balance(CREATOR);

        assert_ok!(Mixer::deposit(
            frame_system::RawOrigin::Signed(ALICE).into(),
            amount,
            commitment(2),
            empty_payload(),
        ));

        let gi = Balances::free_balance(INOMAD_AG) - bi;
        let gv = Balances::free_balance(VALIDATORS_POOL) - bv;
        let gb = Balances::free_balance(BANK) - bb;
        let gc = Balances::free_balance(CREATOR) - bc;

        assert_eq!(gi + gv + gb + gc, total_fee, "fee без потерь");
        assert_eq!(
            Commitments::<Test>::get(&commitment(2)),
            Some(CommitmentState::Active)
        );
        assert_eq!(PoolLeafCount::<Test>::get(), 1);
    });
}

// =============================================================================
// Тест 03: огромный депозит — fee-cap в действии
// =============================================================================
#[test]
fn deposit_massive_mul_fee_is_capped() {
    new_test_ext().execute_with(|| {
        // 25_000 ALTAN: raw = 0.05% * 25000 = 12.5 ALTAN > cap(10) → total_fee = 10 ALTAN
        let amount = 25 * DENOM;
        let cap: Balance = 10 * UNIT;

        let bi = Balances::free_balance(INOMAD_AG);
        let bv = Balances::free_balance(VALIDATORS_POOL);
        let bb = Balances::free_balance(BANK);
        let bc = Balances::free_balance(CREATOR);

        assert_ok!(Mixer::deposit(
            frame_system::RawOrigin::Signed(ALICE).into(),
            amount,
            commitment(3),
            empty_payload(),
        ));

        let total_got = (Balances::free_balance(INOMAD_AG) - bi)
            + (Balances::free_balance(VALIDATORS_POOL) - bv)
            + (Balances::free_balance(BANK) - bb)
            + (Balances::free_balance(CREATOR) - bc);
        assert_eq!(total_got, cap, "fee capped at 10 ALTAN");
    });
}

// =============================================================================
// Тест 04: amount < MixerDenomination → DepositTooSmall
// =============================================================================
#[test]
fn deposit_below_denomination_fails() {
    new_test_ext().execute_with(|| {
        assert_noop!(
            Mixer::deposit(
                frame_system::RawOrigin::Signed(ALICE).into(),
                500 * UNIT,
                commitment(4),
                empty_payload(),
            ),
            Error::<Test>::DepositTooSmall
        );
    });
}

// =============================================================================
// Тест 05: amount % MixerDenomination != 0 → InvalidDenomination
// =============================================================================
#[test]
fn deposit_non_multiple_denomination_fails() {
    new_test_ext().execute_with(|| {
        assert_noop!(
            Mixer::deposit(
                frame_system::RawOrigin::Signed(ALICE).into(),
                1_500 * UNIT,
                commitment(5),
                empty_payload(),
            ),
            Error::<Test>::InvalidDenomination
        );
    });
}

// =============================================================================
// Тест 06: повторный commitment → CommitmentAlreadyExists
// =============================================================================
#[test]
fn deposit_duplicate_commitment_fails() {
    new_test_ext().execute_with(|| {
        assert_ok!(Mixer::deposit(
            frame_system::RawOrigin::Signed(ALICE).into(),
            DENOM,
            commitment(6),
            empty_payload(),
        ));
        assert_noop!(
            Mixer::deposit(
                frame_system::RawOrigin::Signed(ALICE).into(),
                DENOM,
                commitment(6),
                empty_payload(),
            ),
            Error::<Test>::CommitmentAlreadyExists
        );
    });
}

// =============================================================================
// Тест 07: BOB получает ровно amount
// =============================================================================
#[test]
fn withdraw_recipient_gets_exact_amount() {
    new_test_ext().execute_with(|| {
        let c = commitment(7);
        let n = nullifier(7);
        assert_ok!(Mixer::deposit(
            frame_system::RawOrigin::Signed(ALICE).into(),
            DENOM,
            c,
            empty_payload(),
        ));

        let before = Balances::free_balance(BOB);
        assert_ok!(Mixer::withdraw(
            frame_system::RawOrigin::Signed(BANK_OF_SIBERIA).into(),
            n,
            c,
            BOB,
            DENOM,
        ));
        assert_eq!(Balances::free_balance(BOB) - before, DENOM);
        assert_eq!(Commitments::<Test>::get(&c), Some(CommitmentState::Spent));
        assert!(SpentNullifiers::<Test>::get(&n).is_some());
        assert_eq!(PoolLeafCount::<Test>::get(), 0);
    });
}

// =============================================================================
// Тест 08: несуществующий commitment → CommitmentNotFound
// =============================================================================
#[test]
fn withdraw_nonexistent_commitment_fails() {
    new_test_ext().execute_with(|| {
        assert_noop!(
            Mixer::withdraw(
                frame_system::RawOrigin::Signed(BANK_OF_SIBERIA).into(),
                nullifier(8),
                commitment(8),
                BOB,
                DENOM,
            ),
            Error::<Test>::CommitmentNotFound
        );
    });
}

// =============================================================================
// Тест 09: двойной вывод → NullifierAlreadySpent
// =============================================================================
#[test]
fn withdraw_duplicate_nullifier_fails() {
    new_test_ext().execute_with(|| {
        let c = commitment(9);
        let n = nullifier(9);
        let c2 = commitment(90);
        assert_ok!(Mixer::deposit(
            frame_system::RawOrigin::Signed(ALICE).into(),
            DENOM,
            c,
            empty_payload(),
        ));
        assert_ok!(Mixer::deposit(
            frame_system::RawOrigin::Signed(ALICE).into(),
            DENOM,
            c2,
            empty_payload(),
        ));
        assert_ok!(Mixer::withdraw(
            frame_system::RawOrigin::Signed(BANK_OF_SIBERIA).into(),
            n,
            c,
            BOB,
            DENOM,
        ));
        assert_noop!(
            Mixer::withdraw(
                frame_system::RawOrigin::Signed(BANK_OF_SIBERIA).into(),
                n,
                c2,
                BOB,
                DENOM,
            ),
            Error::<Test>::NullifierAlreadySpent
        );
    });
}

// =============================================================================
// Тест 10: PoolLeafCount корректно инкрементируется и декрементируется
// =============================================================================
#[test]
fn pool_leaf_count_tracks_correctly() {
    new_test_ext().execute_with(|| {
        assert_eq!(PoolLeafCount::<Test>::get(), 0);
        assert_ok!(Mixer::deposit(
            frame_system::RawOrigin::Signed(ALICE).into(),
            DENOM,
            commitment(10),
            empty_payload(),
        ));
        assert_eq!(PoolLeafCount::<Test>::get(), 1);
        assert_ok!(Mixer::deposit(
            frame_system::RawOrigin::Signed(ALICE).into(),
            DENOM,
            commitment(101),
            empty_payload(),
        ));
        assert_eq!(PoolLeafCount::<Test>::get(), 2);
        assert_ok!(Mixer::withdraw(
            frame_system::RawOrigin::Signed(BANK_OF_SIBERIA).into(),
            nullifier(10),
            commitment(10),
            BOB,
            DENOM,
        ));
        assert_eq!(PoolLeafCount::<Test>::get(), 1);
    });
}

// =============================================================================
// Тест 11: withdraw с amount < denomination → DepositTooSmall; некратное → InvalidDenomination
// =============================================================================
#[test]
fn withdraw_too_small_amount_fails() {
    new_test_ext().execute_with(|| {
        let c = commitment(11);
        assert_ok!(Mixer::deposit(
            frame_system::RawOrigin::Signed(ALICE).into(),
            DENOM,
            c,
            empty_payload(),
        ));
        assert_noop!(
            Mixer::withdraw(
                frame_system::RawOrigin::Signed(BANK_OF_SIBERIA).into(),
                nullifier(11),
                c,
                BOB,
                500 * UNIT,
            ),
            Error::<Test>::DepositTooSmall
        );
        assert_noop!(
            Mixer::withdraw(
                frame_system::RawOrigin::Signed(BANK_OF_SIBERIA).into(),
                nullifier(11),
                c,
                BOB,
                1_500 * UNIT,
            ),
            Error::<Test>::InvalidDenomination
        );
    });
}

// =============================================================================
// Тест 12: неавторизованный withdraw → BadOrigin
// =============================================================================
#[test]
fn withdraw_by_unauthorized_user_returns_bad_origin() {
    new_test_ext().execute_with(|| {
        let c = commitment(12);
        assert_ok!(Mixer::deposit(
            frame_system::RawOrigin::Signed(ALICE).into(),
            DENOM,
            c,
            empty_payload(),
        ));
        assert_noop!(
            Mixer::withdraw(
                frame_system::RawOrigin::Signed(ALICE).into(),
                nullifier(12),
                c,
                BOB,
                DENOM,
            ),
            frame_support::error::BadOrigin
        );
    });
}

// =============================================================================
// Тест 13: авторизованный RelayerOrigin → Success
// =============================================================================
#[test]
fn withdraw_by_authorized_relayer_succeeds() {
    new_test_ext().execute_with(|| {
        let c = commitment(13);
        let n = nullifier(13);
        assert_ok!(Mixer::deposit(
            frame_system::RawOrigin::Signed(ALICE).into(),
            DENOM,
            c,
            empty_payload(),
        ));
        assert_ok!(Mixer::withdraw(
            frame_system::RawOrigin::Signed(BANK_OF_SIBERIA).into(),
            n,
            c,
            BOB,
            DENOM,
        ));
        assert_eq!(Commitments::<Test>::get(&c), Some(CommitmentState::Spent));
        assert!(SpentNullifiers::<Test>::get(&n).is_some());
    });
}

// =============================================================================
// Тест 14: Recharge Attack заблокирован (BUG-01)
// =============================================================================
#[test]
fn recharge_attack_is_blocked() {
    new_test_ext().execute_with(|| {
        let c = commitment(14);
        let n = nullifier(14);

        assert_ok!(Mixer::deposit(
            frame_system::RawOrigin::Signed(ALICE).into(),
            DENOM,
            c,
            empty_payload(),
        ));
        assert_eq!(Commitments::<Test>::get(&c), Some(CommitmentState::Active));

        assert_ok!(Mixer::withdraw(
            frame_system::RawOrigin::Signed(BANK_OF_SIBERIA).into(),
            n,
            c,
            BOB,
            DENOM,
        ));
        assert_eq!(Commitments::<Test>::get(&c), Some(CommitmentState::Spent));

        // Попытка повторного deposit с тем же хэшем → ДОЛЖНА БЫТЬ ОТКЛОНЕНА.
        assert_noop!(
            Mixer::deposit(
                frame_system::RawOrigin::Signed(ALICE).into(),
                DENOM,
                c,
                empty_payload(),
            ),
            Error::<Test>::CommitmentAlreadyExists
        );
        assert_eq!(Commitments::<Test>::get(&c), Some(CommitmentState::Spent));
    });
}

// =============================================================================
// Тест 15: deposit эмитирует корректное событие
// =============================================================================
#[test]
fn deposit_emits_correct_event() {
    new_test_ext().execute_with(|| {
        let c = commitment(15);
        assert_ok!(Mixer::deposit(
            frame_system::RawOrigin::Signed(ALICE).into(),
            DENOM,
            c,
            empty_payload(),
        ));
        let events = System::events();
        let found = events.iter().any(|r| {
            matches!(
                r.event,
                RuntimeEvent::Mixer(crate::Event::Deposited { commitment, amount, .. })
                    if commitment == c && amount == DENOM
            )
        });
        assert!(found, "Deposited event не найден");
    });
}

// =============================================================================
// Тест 16: reveal_transaction BankBoard → Success + AuditLog
// =============================================================================
#[test]
fn reveal_transaction_by_bank_board_succeeds() {
    new_test_ext().execute_with(|| {
        let c = commitment(16);
        let w = warrant(1);
        assert_ok!(Mixer::reveal_transaction(
            frame_system::RawOrigin::Signed(BANK_BOARD).into(),
            c,
            w.clone(),
        ));
        // AuditLogId должен увеличиться.
        assert_eq!(AuditLogId::<Test>::get(), 1);

        // Запись в AuditLogs.
        let record = AuditLogs::<Test>::get(0).expect("AuditLog запись должна существовать");
        assert_eq!(record.target_commitment, c);
        assert_eq!(record.authorized_by, BANK_BOARD);
        assert_eq!(record.warrant_id, w);
    });
}

// =============================================================================
// Тест 17: reveal_transaction неавторизованным → BadOrigin
// =============================================================================
#[test]
fn reveal_transaction_by_unauthorized_fails() {
    new_test_ext().execute_with(|| {
        assert_noop!(
            Mixer::reveal_transaction(
                frame_system::RawOrigin::Signed(ALICE).into(),
                commitment(17),
                warrant(2),
            ),
            frame_support::error::BadOrigin
        );
        // RelayerOrigin тоже не имеет права.
        assert_noop!(
            Mixer::reveal_transaction(
                frame_system::RawOrigin::Signed(BANK_OF_SIBERIA).into(),
                commitment(17),
                warrant(2),
            ),
            frame_support::error::BadOrigin
        );
    });
}

// =============================================================================
// Тест 18: reveal_transaction хранит несколько записей с монотонным ID
// =============================================================================
#[test]
fn reveal_transaction_stores_audit_log() {
    new_test_ext().execute_with(|| {
        assert_ok!(Mixer::reveal_transaction(
            frame_system::RawOrigin::Signed(BANK_BOARD).into(),
            commitment(18),
            warrant(10),
        ));
        assert_ok!(Mixer::reveal_transaction(
            frame_system::RawOrigin::Signed(BANK_BOARD).into(),
            commitment(19),
            warrant(11),
        ));
        assert_eq!(AuditLogId::<Test>::get(), 2);
        let r0 = AuditLogs::<Test>::get(0).unwrap();
        let r1 = AuditLogs::<Test>::get(1).unwrap();
        assert_eq!(r0.target_commitment, commitment(18));
        assert_eq!(r1.target_commitment, commitment(19));
    });
}

// =============================================================================
// Тест 19: submit_quarterly_audit Хурал → Success
// =============================================================================
#[test]
fn submit_quarterly_audit_by_khural_succeeds() {
    new_test_ext().execute_with(|| {
        let qid = 20261u32; // Q1 2026
        let hash = [0xAAu8; 32];

        assert_ok!(Mixer::submit_quarterly_audit(
            frame_system::RawOrigin::Signed(KHURAL).into(),
            qid,
            hash,
        ));

        let stored = QuarterlyAudits::<Test>::get(qid).expect("аудит должен быть сохранён");
        assert_eq!(stored, hash);
    });
}

// =============================================================================
// Тест 20: submit_quarterly_audit неавторизованным → BadOrigin
// =============================================================================
#[test]
fn submit_quarterly_audit_by_unauthorized_fails() {
    new_test_ext().execute_with(|| {
        let hash = [0xBBu8; 32];
        assert_noop!(
            Mixer::submit_quarterly_audit(
                frame_system::RawOrigin::Signed(ALICE).into(),
                20262,
                hash,
            ),
            frame_support::error::BadOrigin
        );
        // BankBoard не имеет права на подписание аудита.
        assert_noop!(
            Mixer::submit_quarterly_audit(
                frame_system::RawOrigin::Signed(BANK_BOARD).into(),
                20262,
                hash,
            ),
            frame_support::error::BadOrigin
        );
    });
}

// =============================================================================
// Тест 21: повторный submit того же quarter_id → AuditAlreadySigned
// =============================================================================
#[test]
fn submit_quarterly_audit_duplicate_fails() {
    new_test_ext().execute_with(|| {
        let qid = 20263u32;
        let hash = [0xCCu8; 32];

        assert_ok!(Mixer::submit_quarterly_audit(
            frame_system::RawOrigin::Signed(KHURAL).into(),
            qid,
            hash,
        ));
        assert_noop!(
            Mixer::submit_quarterly_audit(
                frame_system::RawOrigin::Signed(KHURAL).into(),
                qid,
                [0xDDu8; 32],
            ),
            Error::<Test>::AuditAlreadySigned
        );
        // Старый hash не перезаписан.
        assert_eq!(QuarterlyAudits::<Test>::get(qid).unwrap(), hash);
    });
}

// =============================================================================
// Тест 22: submit_quarterly_audit эмитирует QuarterlyAuditSigned
// =============================================================================
#[test]
fn submit_quarterly_audit_emits_event() {
    new_test_ext().execute_with(|| {
        let qid = 20264u32;
        let hash = [0xEEu8; 32];

        assert_ok!(Mixer::submit_quarterly_audit(
            frame_system::RawOrigin::Signed(KHURAL).into(),
            qid,
            hash,
        ));
        let found = System::events().iter().any(|r| {
            matches!(
                r.event,
                RuntimeEvent::Mixer(crate::Event::QuarterlyAuditSigned { quarter_id, report_hash })
                    if quarter_id == qid && report_hash == hash
            )
        });
        assert!(found, "QuarterlyAuditSigned event не найден");
    });
}

// =============================================================================
// Тест 23: несколько кварталов — хранятся независимо
// =============================================================================
#[test]
fn multiple_quarterly_audits_different_quarters() {
    new_test_ext().execute_with(|| {
        let quarters: &[(u32, [u8; 32])] = &[
            (20261, [0x01u8; 32]),
            (20262, [0x02u8; 32]),
            (20263, [0x03u8; 32]),
            (20264, [0x04u8; 32]),
        ];
        for &(qid, hash) in quarters {
            assert_ok!(Mixer::submit_quarterly_audit(
                frame_system::RawOrigin::Signed(KHURAL).into(),
                qid,
                hash,
            ));
        }
        for &(qid, hash) in quarters {
            assert_eq!(QuarterlyAudits::<Test>::get(qid).unwrap(), hash);
        }
    });
}
