//! # INOMAD Judicial Courts Pallet — Altan Network
//!
//! **Sprint L1-07 Rev.2: The Sovereign Judicial Engine + Digital Guillotine**
//!
//! Implements the **Judicial Branch of Power** for the Altan Republic.
//!
//! ## Architecture
//!
//! ```text
//! open_case         → CourtCase { Open }  + Habeas Corpus timer registered
//!                     (defendant's assets frozen via Identity bridge)
//! issue_verdict     → CourtCase { Guilty | Acquitted }  + CrimeCategory stored
//!                     (Habeas Corpus timer resolved — courts control outcome)
//! execute_verdict   → CourtCase { Executed }
//!   Economic        →  penalty_amount transferred (20% → whistleblower, 80% → Treasury)
//!   Usurpation      →  same as Economic
//!   HateCrime/Fascsm→  100% balance confiscation + exile (CitizenStatus::Exiled)
//!                       + PERMANENT entry in RegistryOfShame
//!                       Land: blocked via pallet-land-registry exile check (already live)
//! declare_usurper   → MartialLawActive = true
//!                     (100% asset lock + freeze + demote target)
//! ```
//!
//! ## Constitutional Anchors
//!
//! - **Article III — Habeas Corpus**: every `open_case` registers a deadline timer.
//! - **Separation of Powers**: only the Judges College can call `issue_verdict`.
//! - **Anti-Tyranny Protocol**: `declare_usurper` requires `UsurpationOrigin`.
//! - **Digital Guillotine**: `HateCrimeAndFascism` triggers permanent exile and
//!   full asset confiscation — the gravest constitutional sentence.
//!
//! ## Dispatchables
//!
//! | Call | Origin | Description |
//! |---|---|---|
//! | `open_case` | Signed (Judicial Officer) | Open a new court case against a defendant |
//! | `issue_verdict` | Signed (Judge) | Issue a binding verdict in an open case |
//! | `execute_verdict` | Signed (Executor) | Execute a verdict (freeze, fine, exile) |
//! | `declare_usurper` | Signed (Constitutional Court) | Mark a citizen as a constitutional usurper |
//! | `defendant_has_verdict` | Any (Query) | Check if a defendant has an active verdict |
//! | `is_in_registry_of_shame` | Any (Query) | Check if an account is in the Registry of Shame |

#![cfg_attr(not(feature = "std"), no_std)]

extern crate alloc;

pub mod migrations;
pub mod weights;
pub use pallet::*;

// =========================================================================
// Cross-pallet: Constitution Interface (Habeas Corpus Bridge)
// =========================================================================

/// Interface that `pallet-judicial-courts` calls into `pallet-constitution`.
pub trait ConstitutionInterface<T: frame_system::Config> {
    /// Register a Habeas Corpus timer for the defendant at the given block deadline.
    fn register_lockup(
        citizen: &T::AccountId,
        max_lockup_block: frame_system::pallet_prelude::BlockNumberFor<T>,
        case_hash: [u8; 32],
    ) -> frame_support::dispatch::DispatchResult;

    /// Mark the Habeas Corpus timer as resolved — verdict has been entered.
    fn resolve_habeas_corpus(citizen: &T::AccountId) -> frame_support::dispatch::DispatchResult;
}

/// No-op — use `()` in unit tests / when constitution pallet is not wired.
impl<T: frame_system::Config> ConstitutionInterface<T> for () {
    fn register_lockup(
        _citizen: &T::AccountId,
        _max_lockup_block: frame_system::pallet_prelude::BlockNumberFor<T>,
        _case_hash: [u8; 32],
    ) -> frame_support::dispatch::DispatchResult {
        Ok(())
    }
    fn resolve_habeas_corpus(_citizen: &T::AccountId) -> frame_support::dispatch::DispatchResult {
        Ok(())
    }
}

#[cfg(feature = "runtime-benchmarks")]
mod benchmarking;
#[frame_support::pallet]
pub mod pallet {
    use crate::weights::WeightInfo as _;
    use frame_support::{
        pallet_prelude::*,
        sp_runtime::{traits::Saturating, Perbill},
        traits::{
            fungible::{Inspect, Mutate},
            tokens::Preservation,
        },
    };
    use frame_system::pallet_prelude::*;

    use crate::ConstitutionInterface as _;
    use pallet_inomad_identity::IdentityInterface;

    // =========================================================================
    // Balance type alias
    // =========================================================================

    pub type BalanceOf<T> =
        <<T as Config>::Currency as Inspect<<T as frame_system::Config>::AccountId>>::Balance;

    // =========================================================================
    // Habeas Corpus deadline: 7,200 blocks ≈ 12 hours at 6s/block
    // =========================================================================
    const HABEAS_CORPUS_DEADLINE_BLOCKS: u32 = 7_200;

    // =========================================================================
    // Pallet Struct
    // =========================================================================

    #[pallet::pallet]
    pub struct Pallet<T>(_);

    // =========================================================================
    // Config
    // =========================================================================

    #[pallet::config]
    pub trait Config: frame_system::Config<RuntimeEvent: From<Event<Self>>> {
        /// Weight information for extrinsics in this pallet.
        type WeightInfo: crate::weights::WeightInfo;
        // ─── Core dependencies ────────────────────────────────────────────────

        /// The fungible currency (pallet-balances) for penalty transfers.
        type Currency: Inspect<Self::AccountId> + Mutate<Self::AccountId>;

        /// Bridge to `pallet-inomad-identity` for citizen lookups and mutations.
        type Identity: IdentityInterface<Self::AccountId>;

        // ─── Constitutional bridges ───────────────────────────────────────────

        /// Cross-pallet Habeas Corpus bridge → `pallet-constitution`.
        type ConstitutionBridge: crate::ConstitutionInterface<Self>;

        // ─── Privileged origins ───────────────────────────────────────────────

        /// Origin that may call `issue_verdict`.
        /// Represents the **College of Judges**.
        type JudgesCollectiveOrigin: EnsureOrigin<Self::RuntimeOrigin>;

        /// Origin that may call `declare_usurper`.
        type UsurpationOrigin: EnsureOrigin<Self::RuntimeOrigin>;

        // ─── Treasury ─────────────────────────────────────────────────────────

        /// The State Treasury account — receives penalty remainder.
        #[pallet::constant]
        type Treasury: Get<Self::AccountId>;
    }

    // =========================================================================
    // Enums
    // =========================================================================

    /// Full lifecycle status of a `CourtCase`.
    #[derive(
        Encode,
        Decode,
        DecodeWithMemTracking,
        Clone,
        PartialEq,
        Eq,
        RuntimeDebug,
        TypeInfo,
        MaxEncodedLen,
    )]
    pub enum CaseStatus {
        /// Case filed; defendant frozen; awaiting Judges College verdict.
        Open,
        /// Verdict has been issued; awaiting execution (penalty transfer).
        Deliberating,
        /// Defendant found guilty — `execute_verdict` will enforce the penalty.
        Guilty,
        /// Defendant acquitted — frozen assets released; case closed.
        Acquitted,
        /// Penalty has been executed; case is permanently closed.
        Executed,
    }

    /// Classification of the offence — determines the execution algorithm.
    ///
    /// ## Digital Guillotine Protocol
    ///
    /// | Category           | Confiscation | Exile | RegistryOfShame | Land Block |
    /// |--------------------|-------------|-------|-----------------|------------|
    /// | `Economic`         | Partial (penalty_amount) | ❌ | ❌ | ❌ |
    /// | `Usurpation`       | Partial (penalty_amount) | ❌ | ❌ | ❌ |
    /// | `HateCrimeAndFascism` | **100% balance** | ✅ Terminal | ✅ Permanent | ✅ (via Exiled status) |
    ///
    /// For `HateCrimeAndFascism`, execution calls:
    /// 1. `Currency::transfer(defendant → Treasury, 100% balance, Expendable)`
    /// 2. `Identity::exile_citizen(defendant)` → `CitizenStatus::Exiled` (TERMINAL)
    /// 3. `RegistryOfShame::insert(defendant, ShameRecord { ... })` (PERMANENT)
    ///
    /// Land acquisitions by exiled citizens are already blocked by
    /// `pallet-land-registry::transfer_land` which requires `CitizenStatus::Active`.
    #[derive(
        Encode,
        Decode,
        DecodeWithMemTracking,
        Clone,
        PartialEq,
        Eq,
        RuntimeDebug,
        TypeInfo,
        MaxEncodedLen,
    )]
    pub enum CrimeCategory {
        /// Standard economic offence — fraud, tax evasion, contract breach.
        /// Penalty_amount transferred; defendant unfrozen after execution.
        Economic,
        /// Attempt to seize illegitimate constitutional power.
        /// Penalty_amount transferred; reserved for future graduated punishment.
        Usurpation,
        /// Nazism, fascism, genocide propaganda, or systematic oppression of
        /// indigenous peoples and their rights.
        ///
        /// **The gravest sentence in the Republic's legal code.**
        /// Triggers the **Digital Guillotine**: total confiscation + permanent exile.
        HateCrimeAndFascism,
    }

    // =========================================================================
    // Structs
    // =========================================================================

    /// An on-chain court case in the Altan Judicial Engine.
    #[derive(
        Encode,
        Decode,
        DecodeWithMemTracking,
        Clone,
        PartialEq,
        Eq,
        RuntimeDebug,
        TypeInfo,
        MaxEncodedLen,
    )]
    #[scale_info(skip_type_params(T))]
    pub struct CourtCase<T: Config> {
        /// The accusing party — any registered citizen.
        pub plaintiff: T::AccountId,
        /// The accused — must be a registered citizen at case-filing time.
        pub defendant: T::AccountId,
        /// Optional whistleblower who provided inside evidence.
        ///
        /// If `Some`, receives 20% of confiscated proceeds on verdict execution.
        pub whistleblower: Option<T::AccountId>,
        /// Blake2_256 hash of the evidence bundle (IPFS CID anchor).
        pub evidence_hash: [u8; 32],
        /// Blake2_256 hash of the formal verdict document (filled in by judges).
        pub verdict_hash: Option<[u8; 32]>,
        /// The penalty amount determined by the Judges College in `issue_verdict`.
        /// For `HateCrimeAndFascism`, this is overridden to 100% of the
        /// defendant's balance at execution time.
        pub penalty_amount: BalanceOf<T>,
        /// Current lifecycle stage.
        pub status: CaseStatus,
        /// Category of the crime — determines the execution algorithm.
        /// `None` until `issue_verdict` is called.
        pub crime_category: Option<CrimeCategory>,
    }

    /// A permanent entry in the Registry of Shame (Доска Позора).
    ///
    /// Once written, this record is immutable and cannot be removed —
    /// not by Root, not by any governance body. It is the Republic's
    /// permanent constitutional ledger of those convicted of
    /// fascism and oppression of indigenous peoples.
    #[derive(
        Encode,
        Decode,
        DecodeWithMemTracking,
        Clone,
        PartialEq,
        Eq,
        RuntimeDebug,
        TypeInfo,
        MaxEncodedLen,
    )]
    pub struct ShameRecord {
        /// The case whose verdict triggered this entry.
        pub case_id: u32,
        /// Blake2_256 hash of the evidence and verdict documents.
        pub evidence_hash: [u8; 32],
        /// Block number when the Digital Guillotine was executed.
        pub executed_at_block: u32,
    }

    // =========================================================================
    // Storage
    // =========================================================================

    /// All court cases, keyed by sequential case ID.
    #[pallet::storage]
    #[pallet::getter(fn court_cases)]
    pub type CourtCases<T: Config> =
        StorageMap<_, Blake2_128Concat, u32, CourtCase<T>, OptionQuery>;

    /// Monotonically increasing case ID counter.
    #[pallet::storage]
    #[pallet::getter(fn next_case_id)]
    pub type NextCaseId<T: Config> = StorageValue<_, u32, ValueQuery>;

    /// Martial Law flag — set to `true` by `declare_usurper`.
    #[pallet::storage]
    #[pallet::getter(fn martial_law_active)]
    pub type MartialLawActive<T: Config> = StorageValue<_, bool, ValueQuery>;

    /// The Registry of Shame (Доска Позора) — permanent, immutable, publicly readable.
    ///
    /// Maps an `AccountId` to their `ShameRecord`.  Once an account is recorded here,
    /// it **cannot be removed** — not by Root, governance, nor any future extrinsic.
    ///
    /// This is the Republic's permanent constitutional ledger of those convicted
    /// of fascism, nazism, and oppression of indigenous peoples.
    ///
    /// ## Immutability Guarantee
    ///
    /// The only write path is `Pallet::execute_verdict` when
    /// `crime_category == CrimeCategory::HateCrimeAndFascism`.
    /// There is intentionally NO extrinsic or sudo call that removes entries.
    #[pallet::storage]
    #[pallet::getter(fn registry_of_shame)]
    pub type RegistryOfShame<T: Config> =
        StorageMap<_, Blake2_128Concat, T::AccountId, ShameRecord, OptionQuery>;

    // =========================================================================
    // Events
    // =========================================================================

    #[pallet::event]
    #[pallet::generate_deposit(pub(super) fn deposit_event)]
    pub enum Event<T: Config> {
        /// A new court case has been filed and the defendant's assets frozen.
        CaseOpened {
            case_id: u32,
            plaintiff: T::AccountId,
            defendant: T::AccountId,
            has_whistleblower: bool,
        },
        /// The Judges College issued a verdict.
        VerdictIssued {
            case_id: u32,
            is_guilty: bool,
            crime_category: CrimeCategory,
            penalty_amount: BalanceOf<T>,
        },
        /// A guilty verdict has been executed — penalties enforced.
        VerdictExecuted {
            case_id: u32,
            crime_category: CrimeCategory,
            penalty_amount: BalanceOf<T>,
            whistleblower_reward: BalanceOf<T>,
        },
        /// The defendant was acquitted — frozen assets released.
        DefendantAcquitted {
            case_id: u32,
            defendant: T::AccountId,
        },
        /// A citizen was permanently exiled via the Digital Guillotine.
        ///
        /// Triggered when `execute_verdict` processes a `HateCrimeAndFascism` case.
        /// All assets confiscated; `CitizenStatus::Exiled` is TERMINAL.
        CitizenExiled {
            case_id: u32,
            defendant: T::AccountId,
        },
        /// An account has been permanently entered in the Registry of Shame.
        ///
        /// This event is immutable and permanent.  The account will be publicly
        /// visible in the Registry for the lifetime of the chain.
        AddedToRegistryOfShame {
            case_id: u32,
            defendant: T::AccountId,
        },
        /// A citizen has been declared a usurper — Martial Law is now active.
        UsurperDeclared { target: T::AccountId },
        /// Martial Law has been lifted (future governance call).
        MartialLawLifted,
    }

    // =========================================================================
    // Errors
    // =========================================================================

    #[pallet::error]
    pub enum Error<T> {
        /// The referenced case does not exist.
        CaseNotFound,
        /// Case is not in the `Guilty` state — cannot execute verdict yet.
        CaseNotGuilty,
        /// Case is already closed (Acquitted or Executed) — no further actions.
        CaseAlreadyClosed,
        /// Case is not `Open` — verdict cannot be issued in current state.
        CaseNotOpen,
        /// The defendant is not a registered citizen.
        DefendantNotRegistered,
        /// Penalty amount overflows.
        PenaltyOverflow,
        /// Treasury transfer failed.
        TreasuryTransferFailed,
        /// Whistleblower reward transfer failed.
        WhistleblowerTransferFailed,
        /// Cannot transfer — insufficient defendant balance.
        InsufficientBalance,
        /// The defendant is already listed in the Registry of Shame.
        ///
        /// This is a safeguard against duplicate entries — once added,
        /// the record is immutable and permanent.
        AlreadyInRegistryOfShame,
    }

    // =========================================================================
    // Extrinsics
    // =========================================================================

    #[pallet::call]
    impl<T: Config> Pallet<T> {
        // ─── open_case ────────────────────────────────────────────────────────

        /// File a new court case against a registered defendant.
        ///
        /// 1. Freezes the defendant's identity record via `Identity::freeze_citizen`.
        /// 2. Registers a Habeas Corpus timer in `pallet-constitution`.
        /// 3. Stores the `CourtCase` on-chain.
        ///
        /// # Origin: Signed (any citizen)
        #[pallet::call_index(0)]
        #[pallet::weight(T::WeightInfo::open_case())]
        pub fn open_case(
            origin: OriginFor<T>,
            defendant: T::AccountId,
            evidence_hash: [u8; 32],
            whistleblower: Option<T::AccountId>,
        ) -> DispatchResult {
            let plaintiff = ensure_signed(origin)?;

            ensure!(
                T::Identity::citizen_record_of(&defendant).is_some(),
                Error::<T>::DefendantNotRegistered
            );

            let _ = T::Identity::freeze_citizen(&defendant);

            let now = frame_system::Pallet::<T>::block_number();
            let offset: BlockNumberFor<T> = HABEAS_CORPUS_DEADLINE_BLOCKS.into();
            let deadline = now.saturating_add(offset);
            T::ConstitutionBridge::register_lockup(&defendant, deadline, evidence_hash)?;

            let case_id = NextCaseId::<T>::get();
            let has_wb = whistleblower.is_some();

            CourtCases::<T>::insert(
                case_id,
                CourtCase {
                    plaintiff: plaintiff.clone(),
                    defendant: defendant.clone(),
                    whistleblower,
                    evidence_hash,
                    verdict_hash: None,
                    penalty_amount: BalanceOf::<T>::from(0u32),
                    status: CaseStatus::Open,
                    crime_category: None,
                },
            );
            NextCaseId::<T>::put(case_id.saturating_add(1));

            Self::deposit_event(Event::CaseOpened {
                case_id,
                plaintiff,
                defendant,
                has_whistleblower: has_wb,
            });
            Ok(())
        }

        // ─── issue_verdict ────────────────────────────────────────────────────

        /// Issue a binding verdict from the Judges College.
        ///
        /// ## Access control
        ///
        /// Only `T::JudgesCollectiveOrigin` may call this.
        ///
        /// ## Crime Category
        ///
        /// The `crime_category` parameter classifies the offence and determines
        /// the execution algorithm used by `execute_verdict`:
        ///
        /// - `Economic`           → standard penalty transfer
        /// - `Usurpation`         → standard penalty transfer
        /// - `HateCrimeAndFascism`→ Digital Guillotine (100% confiscation + permanent exile)
        ///
        /// # Origin: JudgesCollectiveOrigin
        #[pallet::call_index(1)]
        #[pallet::weight(T::WeightInfo::issue_verdict())]
        pub fn issue_verdict(
            origin: OriginFor<T>,
            case_id: u32,
            is_guilty: bool,
            verdict_hash: [u8; 32],
            penalty_amount: BalanceOf<T>,
            crime_category: CrimeCategory,
        ) -> DispatchResult {
            T::JudgesCollectiveOrigin::ensure_origin(origin)?;

            let category_clone = crime_category.clone();

            CourtCases::<T>::try_mutate(case_id, |maybe| -> DispatchResult {
                let case = maybe.as_mut().ok_or(Error::<T>::CaseNotFound)?;

                ensure!(case.status == CaseStatus::Open, Error::<T>::CaseNotOpen);

                T::ConstitutionBridge::resolve_habeas_corpus(&case.defendant)?;

                case.verdict_hash = Some(verdict_hash);
                case.penalty_amount = penalty_amount;
                case.crime_category = Some(crime_category);

                if is_guilty {
                    case.status = CaseStatus::Guilty;
                } else {
                    case.status = CaseStatus::Acquitted;
                    let _ = T::Identity::unfreeze_citizen(&case.defendant);
                    Self::deposit_event(Event::DefendantAcquitted {
                        case_id,
                        defendant: case.defendant.clone(),
                    });
                }

                Ok(())
            })?;

            Self::deposit_event(Event::VerdictIssued {
                case_id,
                is_guilty,
                crime_category: category_clone,
                penalty_amount,
            });
            Ok(())
        }

        // ─── execute_verdict ──────────────────────────────────────────────────

        /// Execute an issued guilty verdict — enforce the financial penalty.
        ///
        /// ## Access control
        ///
        /// Open to **any signed account** once the case is in `Guilty` status.
        ///
        /// ## Execution Algorithms by Crime Category
        ///
        /// ### Economic / Usurpation
        ///
        /// ```text
        /// If whistleblower:
        ///     20% of penalty_amount → whistleblower account
        ///     80% of penalty_amount → Treasury
        /// Else:
        ///     100% of penalty_amount → Treasury
        /// Unfreeze defendant (judicial hold complete).
        /// ```
        ///
        /// ### HateCrimeAndFascism — The Digital Guillotine
        ///
        /// ```text
        /// 1. Confiscate 100% of defendant balance → Treasury (Expendable)
        /// 2. Re-route 20% of that amount Treasury → whistleblower (if present)
        /// 3. Call Identity::exile_citizen(defendant) → CitizenStatus::Exiled (TERMINAL)
        ///    - Land acquisitions blocked by pallet-land-registry (CitizenStatus::Active required)
        /// 4. Insert defendant → RegistryOfShame { case_id, evidence_hash, block } (PERMANENT)
        /// 5. Emit CitizenExiled + AddedToRegistryOfShame events
        /// ```
        ///
        /// # Origin: Signed (any citizen)
        #[pallet::call_index(2)]
        #[pallet::weight(T::WeightInfo::execute_verdict())]
        pub fn execute_verdict(origin: OriginFor<T>, case_id: u32) -> DispatchResult {
            ensure_signed(origin)?;

            let case = CourtCases::<T>::get(case_id).ok_or(Error::<T>::CaseNotFound)?;

            ensure!(case.status == CaseStatus::Guilty, Error::<T>::CaseNotGuilty);

            let treasury = T::Treasury::get();
            let category = case
                .crime_category
                .clone()
                .unwrap_or(CrimeCategory::Economic);

            let (actual_penalty, wb_reward) = match &category {
                // ── Digital Guillotine: HateCrimeAndFascism ───────────────────
                //
                // 100% total confiscation — existential protections removed.
                // The penalty amount in the case record is IGNORED; we confiscate
                // the defendant's entire balance at execution time.
                CrimeCategory::HateCrimeAndFascism => {
                    let full_balance = T::Currency::balance(&case.defendant);

                    // Compute whistleblower share (20%) from the full balance.
                    let reward = if case.whistleblower.is_some() {
                        Perbill::from_percent(20) * full_balance
                    } else {
                        BalanceOf::<T>::from(0u32)
                    };

                    // Transfer everything to Treasury — Expendable removes the
                    // existential deposit floor (constitutional punishment, not tx).
                    if full_balance > BalanceOf::<T>::from(0u32) {
                        T::Currency::transfer(
                            &case.defendant,
                            &treasury,
                            full_balance,
                            Preservation::Expendable,
                        )
                        .map_err(|_| Error::<T>::InsufficientBalance)?;
                    }

                    // Re-route informant share: Treasury → Whistleblower.
                    if reward > BalanceOf::<T>::from(0u32) {
                        if let Some(ref wb) = case.whistleblower {
                            T::Currency::transfer(&treasury, wb, reward, Preservation::Preserve)
                                .map_err(|_| Error::<T>::WhistleblowerTransferFailed)?;
                        }
                    }

                    // ── Permanent Exile ────────────────────────────────────────
                    //
                    // Terminal status: CitizenStatus::Exiled.
                    // Calls OnTerminalStatus::on_exiled hook (cascades through guilds,
                    // khural-governance, etc. via the runtime's TerminalHookImpl).
                    // pallet-land-registry already blocks Exiled citizens from
                    // acquiring land (CitizenStatus::Active required in transfer_land).
                    let _ = T::Identity::exile_citizen(&case.defendant);

                    Self::deposit_event(Event::CitizenExiled {
                        case_id,
                        defendant: case.defendant.clone(),
                    });

                    // ── Registry of Shame ──────────────────────────────────────
                    //
                    // Immutable, permanent public entry. No removal path exists.
                    // Returns AlreadyInRegistryOfShame if the defendant was somehow
                    // previously recorded (shouldn't happen — defensive guard).
                    ensure!(
                        !RegistryOfShame::<T>::contains_key(&case.defendant),
                        Error::<T>::AlreadyInRegistryOfShame
                    );

                    let now_block: u32 = frame_system::Pallet::<T>::block_number()
                        .try_into()
                        .unwrap_or(u32::MAX);

                    RegistryOfShame::<T>::insert(
                        &case.defendant,
                        ShameRecord {
                            case_id,
                            evidence_hash: case.evidence_hash,
                            executed_at_block: now_block,
                        },
                    );

                    Self::deposit_event(Event::AddedToRegistryOfShame {
                        case_id,
                        defendant: case.defendant.clone(),
                    });

                    (full_balance, reward)
                }

                // ── Standard Sentence: Economic / Usurpation ──────────────────
                CrimeCategory::Economic | CrimeCategory::Usurpation => {
                    let penalty = case.penalty_amount;
                    let reward = if case.whistleblower.is_some() {
                        Perbill::from_percent(20) * penalty
                    } else {
                        BalanceOf::<T>::from(0u32)
                    };

                    let is_official = T::Identity::citizen_record_of(&case.defendant)
                        .map(|r| r.branch.is_some())
                        .unwrap_or(false);

                    if penalty > BalanceOf::<T>::from(0u32) {
                        if is_official {
                            T::Currency::transfer(
                                &case.defendant,
                                &treasury,
                                penalty,
                                Preservation::Expendable,
                            )
                            .map_err(|_| Error::<T>::InsufficientBalance)?;
                        } else {
                            T::Currency::transfer(
                                &case.defendant,
                                &treasury,
                                penalty,
                                Preservation::Preserve,
                            )
                            .map_err(|_| Error::<T>::InsufficientBalance)?;
                        }

                        if reward > BalanceOf::<T>::from(0u32) {
                            if let Some(ref wb) = case.whistleblower {
                                T::Currency::transfer(
                                    &treasury,
                                    wb,
                                    reward,
                                    Preservation::Preserve,
                                )
                                .map_err(|_| Error::<T>::WhistleblowerTransferFailed)?;
                            }
                        }
                    }

                    // Unfreeze defendant — judicial hold is complete.
                    let _ = T::Identity::unfreeze_citizen(&case.defendant);

                    (penalty, reward)
                }
            };

            // ── Close the case ─────────────────────────────────────────────────
            CourtCases::<T>::mutate(case_id, |maybe| {
                if let Some(ref mut c) = maybe {
                    c.status = CaseStatus::Executed;
                }
            });

            Self::deposit_event(Event::VerdictExecuted {
                case_id,
                crime_category: category,
                penalty_amount: actual_penalty,
                whistleblower_reward: wb_reward,
            });
            Ok(())
        }

        // ─── declare_usurper ──────────────────────────────────────────────────

        /// Invoke the Anti-Tyranny Protocol — declare a citizen a usurper.
        ///
        /// ## What this does
        ///
        /// 1. Sets `MartialLawActive` to `true`.
        /// 2. Burns/transfers 100% of the target's free balance to the Treasury.
        /// 3. Freezes the target's identity record.
        /// 4. Strips all governmental roles via `Identity::demote_to_regular`.
        ///
        /// # Origin: UsurpationOrigin
        #[pallet::call_index(3)]
        #[pallet::weight(T::WeightInfo::declare_usurper())]
        pub fn declare_usurper(origin: OriginFor<T>, target: T::AccountId) -> DispatchResult {
            T::UsurpationOrigin::ensure_origin(origin)?;

            let treasury = T::Treasury::get();

            MartialLawActive::<T>::put(true);

            let balance = T::Currency::balance(&target);
            if balance > BalanceOf::<T>::from(0u32) {
                let _ =
                    T::Currency::transfer(&target, &treasury, balance, Preservation::Expendable);
            }

            let _ = T::Identity::freeze_citizen(&target);
            let _ = T::Identity::demote_to_regular(&target);

            Self::deposit_event(Event::UsurperDeclared { target });
            Ok(())
        }
    }

    // =========================================================================
    // Helper: BlockNumber conversion
    // =========================================================================

    impl<T: Config> Pallet<T> {}
}

// =========================================================================
// HabeasCorpusInterface for pallet-constitution
// =========================================================================

impl<T: Config> Pallet<T> {
    /// Returns `true` if any court case for `who` has a verdict (non-Open status).
    ///
    /// Used by `pallet-constitution::JudicialHabeasBridge::has_verdict`.
    pub fn defendant_has_verdict(who: &T::AccountId) -> bool {
        let total = NextCaseId::<T>::get();
        for case_id in 0..total {
            if let Some(case) = CourtCases::<T>::get(case_id) {
                if &case.defendant == who && case.status != CaseStatus::Open {
                    return true;
                }
            }
        }
        false
    }

    /// Returns `true` if `who` is listed in the Registry of Shame.
    ///
    /// Public query — can be called by any pallet or RPC client for checks.
    pub fn is_in_registry_of_shame(who: &T::AccountId) -> bool {
        RegistryOfShame::<T>::contains_key(who)
    }
}

// =========================================================================
// Unit Tests
// =========================================================================

#[cfg(test)]
mod mock;

#[cfg(test)]
mod tests;
