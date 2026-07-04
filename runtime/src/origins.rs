//! # Constitutional Origin Types — Altan Republic Production Guards
//!
//! This module replaces the development-only `EnsureRoot` / `EnsureSigned` origin
//! stubs with **constitutionally correct, on-chain verifiable** origin checks.
//!
//! Each origin type reads live on-chain state from `pallet-inomad-elections`:
//! - `BranchCouncils<T>` — 9-member council per `GovernmentBranch`
//! - `SupremeLeaders<T>` — single supreme leader per branch
//!
//! ## Bootstrap Strategy
//!
//! During genesis → first election period, `EnsureRootOrXxx` variants accept
//! EITHER the Creator Sudo key OR a council member. After the first election
//! cycle completes, upgrade via WASM referendum to remove Root fallback.

use frame_support::traits::EnsureOrigin;
use frame_system::RawOrigin;
use pallet_inomad_elections::GovernmentBranch;

use crate::Runtime;

/// Convenience alias for the Altan Network's AccountId type.
pub type AccountId = <Runtime as frame_system::Config>::AccountId;

/// Type alias for the RuntimeOrigin used in this runtime.
pub type RuntimeOrigin = <Runtime as frame_system::Config>::RuntimeOrigin;

// ─────────────────────────────────────────────────────────────────────────────
// Internal helpers
// ─────────────────────────────────────────────────────────────────────────────

/// Returns `true` if `who` is a member of the elected `branch` council.
#[inline]
fn is_branch_council_member(branch: GovernmentBranch, who: &AccountId) -> bool {
    pallet_inomad_elections::BranchCouncils::<Runtime>::get(branch)
        .map(|council| council.contains(who))
        .unwrap_or(false)
}

/// Returns `true` if `who` is the elected SupremeLeader of the given `branch`.
#[inline]
fn is_supreme_leader(branch: GovernmentBranch, who: &AccountId) -> bool {
    pallet_inomad_elections::SupremeLeaders::<Runtime>::get(branch)
        .map(|leader| &leader == who)
        .unwrap_or(false)
}

/// Root sentinel AccountId: 32 zero-bytes.
///
/// Returned when a bootstrap `EnsureRootOrXxx` accepts a Root origin.
/// Not a valid sr25519 public key — will never clash with a real citizen account.
#[inline]
fn root_sentinel() -> AccountId {
    AccountId::from([0u8; 32])
}

/// Internal helper: extract AccountId from a signed RuntimeOrigin or return Err.
#[inline]
fn signed_account(o: RuntimeOrigin) -> Result<AccountId, RuntimeOrigin> {
    match o.clone().into() {
        Ok(RawOrigin::Signed(who)) => Ok(who),
        _ => Err(o),
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Banking Branch Origins
// ─────────────────────────────────────────────────────────────────────────────

/// [BANKING BRANCH] Any of the 9 elected Banking BranchCouncil members.
pub struct EnsureBankingCouncilMember;

impl EnsureOrigin<RuntimeOrigin> for EnsureBankingCouncilMember {
    type Success = AccountId;
    fn try_origin(o: RuntimeOrigin) -> Result<Self::Success, RuntimeOrigin> {
        let who = signed_account(o)?;
        if is_branch_council_member(GovernmentBranch::Banking, &who) {
            Ok(who)
        } else {
            Err(RawOrigin::Signed(who).into())
        }
    }
    #[cfg(feature = "runtime-benchmarks")]
    fn try_successful_origin() -> Result<RuntimeOrigin, ()> {
        Ok(RawOrigin::None.into())
    }
}

/// [BANKING BRANCH] The elected Central Banker (SupremeLeader of Banking).
pub struct EnsureBankingSupremeLeader;

impl EnsureOrigin<RuntimeOrigin> for EnsureBankingSupremeLeader {
    type Success = AccountId;
    fn try_origin(o: RuntimeOrigin) -> Result<Self::Success, RuntimeOrigin> {
        let who = signed_account(o)?;
        if is_supreme_leader(GovernmentBranch::Banking, &who) {
            Ok(who)
        } else {
            Err(RawOrigin::Signed(who).into())
        }
    }
    #[cfg(feature = "runtime-benchmarks")]
    fn try_successful_origin() -> Result<RuntimeOrigin, ()> {
        Ok(RawOrigin::None.into())
    }
}

/// [BANK BOARD] Any Banking council member OR the Central Banker.
pub struct EnsureBankBoard;

impl EnsureOrigin<RuntimeOrigin> for EnsureBankBoard {
    type Success = AccountId;
    fn try_origin(o: RuntimeOrigin) -> Result<Self::Success, RuntimeOrigin> {
        let who = signed_account(o)?;
        if is_branch_council_member(GovernmentBranch::Banking, &who)
            || is_supreme_leader(GovernmentBranch::Banking, &who)
        {
            Ok(who)
        } else {
            Err(RawOrigin::Signed(who).into())
        }
    }
    #[cfg(feature = "runtime-benchmarks")]
    fn try_successful_origin() -> Result<RuntimeOrigin, ()> {
        Ok(RawOrigin::None.into())
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Judicial Branch Origins
// ─────────────────────────────────────────────────────────────────────────────

/// [JUDICIAL BRANCH] Any of the 9 elected Judicial BranchCouncil members.
pub struct EnsureJudicialCouncilMember;

impl EnsureOrigin<RuntimeOrigin> for EnsureJudicialCouncilMember {
    type Success = AccountId;
    fn try_origin(o: RuntimeOrigin) -> Result<Self::Success, RuntimeOrigin> {
        let who = signed_account(o)?;
        if is_branch_council_member(GovernmentBranch::Judicial, &who) {
            Ok(who)
        } else {
            Err(RawOrigin::Signed(who).into())
        }
    }
    #[cfg(feature = "runtime-benchmarks")]
    fn try_successful_origin() -> Result<RuntimeOrigin, ()> {
        Ok(RawOrigin::None.into())
    }
}

/// [JUDICIAL BRANCH] The elected Chief Justice (SupremeLeader of Judicial).
pub struct EnsureChiefJustice;

impl EnsureOrigin<RuntimeOrigin> for EnsureChiefJustice {
    type Success = AccountId;
    fn try_origin(o: RuntimeOrigin) -> Result<Self::Success, RuntimeOrigin> {
        let who = signed_account(o)?;
        if is_supreme_leader(GovernmentBranch::Judicial, &who) {
            Ok(who)
        } else {
            Err(RawOrigin::Signed(who).into())
        }
    }
    #[cfg(feature = "runtime-benchmarks")]
    fn try_successful_origin() -> Result<RuntimeOrigin, ()> {
        Ok(RawOrigin::None.into())
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Legislative Branch Origins
// ─────────────────────────────────────────────────────────────────────────────

/// [LEGISLATIVE BRANCH] The constitutionally elected Khural Chairman.
pub struct EnsureKhuralChairman;

impl EnsureOrigin<RuntimeOrigin> for EnsureKhuralChairman {
    type Success = AccountId;
    fn try_origin(o: RuntimeOrigin) -> Result<Self::Success, RuntimeOrigin> {
        let who = signed_account(o)?;
        if is_supreme_leader(GovernmentBranch::Legislative, &who) {
            Ok(who)
        } else {
            Err(RawOrigin::Signed(who).into())
        }
    }
    #[cfg(feature = "runtime-benchmarks")]
    fn try_successful_origin() -> Result<RuntimeOrigin, ()> {
        Ok(RawOrigin::None.into())
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Executive Branch Origins
// ─────────────────────────────────────────────────────────────────────────────

/// [EXECUTIVE BRANCH] Any of the 9 elected Executive BranchCouncil members.
pub struct EnsureExecutiveCouncilMember;

impl EnsureOrigin<RuntimeOrigin> for EnsureExecutiveCouncilMember {
    type Success = AccountId;
    fn try_origin(o: RuntimeOrigin) -> Result<Self::Success, RuntimeOrigin> {
        let who = signed_account(o)?;
        if is_branch_council_member(GovernmentBranch::Executive, &who) {
            Ok(who)
        } else {
            Err(RawOrigin::Signed(who).into())
        }
    }
    #[cfg(feature = "runtime-benchmarks")]
    fn try_successful_origin() -> Result<RuntimeOrigin, ()> {
        Ok(RawOrigin::None.into())
    }
}

/// [EXECUTIVE BRANCH] The elected Head of State (SupremeLeader of Executive).
pub struct EnsureHeadOfState;

impl EnsureOrigin<RuntimeOrigin> for EnsureHeadOfState {
    type Success = AccountId;
    fn try_origin(o: RuntimeOrigin) -> Result<Self::Success, RuntimeOrigin> {
        let who = signed_account(o)?;
        if is_supreme_leader(GovernmentBranch::Executive, &who) {
            Ok(who)
        } else {
            Err(RawOrigin::Signed(who).into())
        }
    }
    #[cfg(feature = "runtime-benchmarks")]
    fn try_successful_origin() -> Result<RuntimeOrigin, ()> {
        Ok(RawOrigin::None.into())
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Bootstrap Origins: Root fallback for pre-election period (Success = AccountId)
// ─────────────────────────────────────────────────────────────────────────────
//
// All bootstrap origins return `Success = AccountId`:
//   Root     → root_sentinel() = AccountId::from([0u8; 32])
//   Signed   → the actual council member AccountId
//
// This matches the `Success = AccountId` contract required by pallet configs
// for RelayerOrigin, BankBoardOrigin, KhuralOrigin, BankingOrigin, etc.

/// **BOOTSTRAP** Root (Sudo) OR any Banking BranchCouncil member.
///
/// Used for: `pallet-central-bank::BankingOrigin`,
///           `pallet-buryad-mongol-mixer::RelayerOrigin`,
///           `pallet-buryad-mongol-mixer::BankBoardOrigin`.
pub struct EnsureRootOrBankingCouncil;

impl EnsureOrigin<RuntimeOrigin> for EnsureRootOrBankingCouncil {
    type Success = AccountId;

    fn try_origin(o: RuntimeOrigin) -> Result<Self::Success, RuntimeOrigin> {
        match o.clone().into() {
            Ok(RawOrigin::Root) => return Ok(root_sentinel()),
            Ok(RawOrigin::Signed(who)) => {
                if is_branch_council_member(GovernmentBranch::Banking, &who) {
                    return Ok(who);
                }
            }
            _ => {}
        }
        Err(o)
    }

    #[cfg(feature = "runtime-benchmarks")]
    fn try_successful_origin() -> Result<RuntimeOrigin, ()> {
        Ok(RawOrigin::Root.into())
    }
}

/// **BOOTSTRAP** Root (Sudo) OR the elected Khural Chairman.
///
/// Used for: `pallet-buryad-mongol-mixer::KhuralOrigin`.
pub struct EnsureRootOrKhuralChairman;

impl EnsureOrigin<RuntimeOrigin> for EnsureRootOrKhuralChairman {
    type Success = AccountId;

    fn try_origin(o: RuntimeOrigin) -> Result<Self::Success, RuntimeOrigin> {
        match o.clone().into() {
            Ok(RawOrigin::Root) => return Ok(root_sentinel()),
            Ok(RawOrigin::Signed(who)) => {
                if is_supreme_leader(GovernmentBranch::Legislative, &who) {
                    return Ok(who);
                }
            }
            _ => {}
        }
        Err(o)
    }

    #[cfg(feature = "runtime-benchmarks")]
    fn try_successful_origin() -> Result<RuntimeOrigin, ()> {
        Ok(RawOrigin::Root.into())
    }
}

/// **BOOTSTRAP** Root (Sudo) OR any Judicial BranchCouncil member.
///
/// Used for: `pallet-judicial-courts::JudgesCollectiveOrigin`,
///           `pallet-judicial-courts::UsurpationOrigin`.
pub struct EnsureRootOrJudicialCouncil;

impl EnsureOrigin<RuntimeOrigin> for EnsureRootOrJudicialCouncil {
    type Success = AccountId;

    fn try_origin(o: RuntimeOrigin) -> Result<Self::Success, RuntimeOrigin> {
        match o.clone().into() {
            Ok(RawOrigin::Root) => return Ok(root_sentinel()),
            Ok(RawOrigin::Signed(who)) => {
                if is_branch_council_member(GovernmentBranch::Judicial, &who) {
                    return Ok(who);
                }
            }
            _ => {}
        }
        Err(o)
    }

    #[cfg(feature = "runtime-benchmarks")]
    fn try_successful_origin() -> Result<RuntimeOrigin, ()> {
        Ok(RawOrigin::Root.into())
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Organization Origins (10-100-1000-10000 System)
// ─────────────────────────────────────────────────────────────────────────────

use frame_support::traits::Get;
use core::marker::PhantomData;

/// Origin for any Organization Officer or Root.
/// Reads live role data from `pallet-org-roles`.
pub struct EnsureOrgOfficer<OrgId: Get<u32>>(PhantomData<OrgId>);

impl<OrgId: Get<u32>> EnsureOrigin<RuntimeOrigin> for EnsureOrgOfficer<OrgId> {
    type Success = AccountId;

    fn try_origin(o: RuntimeOrigin) -> Result<Self::Success, RuntimeOrigin> {
        match o.clone().into() {
            Ok(RawOrigin::Root) => return Ok(root_sentinel()),
            Ok(RawOrigin::Signed(who)) => {
                let org_id = OrgId::get();
                if let Some(tier) = pallet_org_roles::IssuedKeys::<Runtime>::get(org_id, &who) {
                    if tier == pallet_org_roles::RoleTier::Root || tier == pallet_org_roles::RoleTier::Officer {
                        return Ok(who);
                    }
                }
            }
            _ => {}
        }
        Err(o)
    }

    #[cfg(feature = "runtime-benchmarks")]
    fn try_successful_origin() -> Result<RuntimeOrigin, ()> {
        Ok(RawOrigin::Root.into())
    }
}
