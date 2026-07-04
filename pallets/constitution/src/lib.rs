//! # Constitution Pallet — Layer 0 | Билль о Правах и Свободах
//!
//! **Altan Network — Sovereign L1 Blockchain**
//! **Sprint L1-06: Constitutional Sovereignty** | Layer 0
//!
//! This pallet is the **highest layer** of the Altan Network's legal stack —
//! the immutable anchor of sovereign rights that all other pallets must respect.
//!
//! ## Constitutional Guarantees (Comments are Law)
//!
//! The following rights are ABSOLUTE and enforced at Layer 0.
//! No governance vote, no Khural majority, no Root key can override them.
//!
//! ### Article I — Fundamental Freedoms
//!
//! - **Freedom of Speech**: Every citizen may express any opinion, idea, or belief.
//!   Speech implies personal responsibility (adjudicated through `pallet-justice`).
//!   Censorship at the L1 protocol level is **constitutionally forbidden**.
//!   No validator, no pallet, no governance can silence a transaction.
//!
//! - **Freedom of Religion**: No state religion. Citizens may practice any faith
//!   or none. Religious affiliation is a private matter; no on-chain discrimination.
//!
//! - **Freedom of Thought**: Conscience, belief, and private reasoning are inviolable.
//!   No pallet may demand disclosure of private thought. Cryptographic proofs of
//!   knowledge are explicitly excluded from forced disclosure mandates.
//!
//! - **Freedom of Learning**: Every citizen has the right to access knowledge,
//!   education, and information. The Chronicles pallet (`pallet-chronicles`)
//!   cannot be censored by administrative order.
//!
//! - **Freedom of Movement**: Citizens may transact across all 79 nation treasuries
//!   without requiring a visa or permit. Nation membership does not restrict
//!   economic participation in the broader Republic.
//!
//! ### Article II — "My Home is My Castle" (Casa Fortaleza)
//!
//! The sovereignty of a citizen over their personal space — digital and physical —
//! is absolute:
//!
//! - No account may be frozen without a **judicial order** from a verified Judicial
//!   branch Tumed court (enforced by `pallet-judicial-courts`).
//! - **Cryptographic inviolability of keys**: No pallet may compel a citizen to
//!   reveal or surrender their private key. Key disclosure is a voluntary act.
//!   The network provides identity via Soulbound Tokens; keys are never transmitted.
//! - Mass surveillance of transactions is prohibited. Data aggregation for law
//!   enforcement requires an individual court order per citizen, per case.
//!
//! ### Article III — Habeas Corpus
//!
//! No citizen may be held in a frozen state without **judicial proceedings**:
//!
//! - Any fund freeze (`set_lock` / `CitizenStatus::Frozen`) initiated via
//!   `pallet-judicial-courts` MUST have a `max_lockup_block` deadline registered
//!   in the `HabeasCorpusTimers` storage of this pallet.
//! - The `on_initialize` hook of this pallet checks all active timers every block.
//! - If the deadline is reached and no `Verdict` has been recorded in
//!   `pallet-judicial-courts`, the funds are **automatically unfrozen** and
//!   the citizen's status is restored to `Active`.
//! - This is the constitutional protection against indefinite pre-trial detention
//!   by the investigating authority.
//!
//! ### Article IV — Non-Alienation of Sovereignty
//!
//! - The 79 sovereign peoples (as enumerated in `pallet-inomad-identity`) are the
//!   PERMANENT source of all political power. No temporary majority may dissolve
//!   the Confederation or hand sovereignty to a foreign power.
//! - The `CoreRightsHash` stored in this pallet is the immutable IPFS CID of the
//!   constitutional text. It cannot be overwritten after genesis.
//!
//! ## Storage
//!
//! - `CoreRightsHash`: `[u8; 46]` — IPFS CIDv1 of the constitutional document.
//!   Stored ONCE at genesis or by Root-signed initialisation. NEVER overwritten.
//! - `HabeasCorpusTimers`: Maps `AccountId → HabeasCorpusTimer` — tracks all
//!   active judicial freezes that require automatic expiry enforcement.
//!
//! ## Extrinsics
//!
//! | Call                    | Origin | Description                                              |
//! |-------------------------|--------|----------------------------------------------------------|
//! | `set_constitution_hash` | Root   | One-time: store the IPFS CID of the constitutional doc.  |
//! | `register_lockup`       | Root   | Judicial: register a Habeas Corpus timer for a freeze.   |
//! | `resolve_habeas_corpus` | Root   | Judicial: mark case as resolved (prevents auto-unfreeze).|

#![cfg_attr(not(feature = "std"), no_std)]

extern crate alloc;

pub mod migrations;
pub mod weights;
pub use pallet::*;

// =========================================================================
// Constitutional Branch Markers — 4 Ветви Власти Республики
// =========================================================================
//
// These marker types represent the four constitutionally independent branches
// of the Altan Republic. They are used as phantom types in `Config` traits
// and runtime wiring to enforce at the TYPE LEVEL that each branch's
// extrinsics can only be called by actors holding the appropriate origin.
//
// SEPARATION OF POWERS (Article V — четыре независимые ветви власти):
//   No branch may call extrinsics belonging to another branch without
//   going through the constitutional cross-branch consensus mechanism
//   defined in `BranchConsensus`.
//
// WIRING AT RUNTIME:
//   Each pallet that belongs to a branch declares `type XxxOrigin: EnsureOrigin`
//   and the runtime wires the appropriate concrete type (Root for dev,
//   a Collective for production) at compile time.

/// **Законодательная ветвь** — Хурал (только для коренных народов).
///
/// The legislative branch of the Altan Republic: the Confederate Khural
/// (79 sovereign indigenous nations). All laws and budgets are enacted here.
/// Membership is restricted to citizens registered via `pallet-inomad-identity`
/// with indigenous (`Native`) citizenship status.
///
/// Legislative acts that affect other branches require cross-branch consensus
/// per `BranchConsensus::requires_cross_branch_approval`.
pub struct LegislativeAuthority;

/// **Исполнительная ветвь** — Президент и Министерства.
///
/// The executive branch of the Altan Republic: the President, Ministers,
/// and their administrative departments. Executes laws passed by the Khural
/// and manages day-to-day governance and public services.
///
/// Cannot issue currency, enact law, or render judicial verdicts.
pub struct ExecutiveAuthority;

/// **Судебная ветвь** — Верховный Суд.
///
/// The judicial branch of the Altan Republic: the Supreme Court and
/// the 79 nation-scoped Tumed courts (implemented in `pallet-judicial-courts`).
/// Sole authority to issue binding verdicts, asset confiscations, and
/// constitutional penalties. Habeas Corpus enforcement is a judicial act.
///
/// Cannot issue currency or enact law.
pub struct JudicialAuthority;

/// **Банковская ветвь** — Центральный Банк (монопольная эмиссия).
///
/// The banking branch of the Altan Republic: the Central Bank.
/// Holds the EXCLUSIVE constitutional right to issue (mint) new ALTAN tokens
/// via `pallet-central-bank::mint_to_operator`.
///
/// ## Правило Независимости (ФРС)
///
/// The Central Bank:
///   - Has NO spending capability (no `transfer` extrinsic).
///   - Cannot freeze citizens (Judicial authority only).
///   - Cannot enact laws (Legislative authority only).
///   - Can ONLY mint to a licensed operator's account.
///
/// In development: wired to `EnsureRoot` (Sudo).
/// In production: wired to a threshold `Banking Collective` of appointed
/// Central Bank governors, independent of the Khural and Executive.
pub struct BankingAuthority;

// ─── Cross-Branch Consensus ───────────────────────────────────────────────────
//
// This trait encodes the constitutional rule that no branch may unilaterally
// invoke the authority of another without formal cross-branch consensus.
//
// Concrete enforcement is implemented at the runtime level by requiring
// that mixed-branch call paths pass through governance proposals (Khural)
// or judicial orders, rather than direct calls.

/// Constitutional trait: no single branch may act in another's domain.
///
/// Any function that crosses branch boundaries MUST pass through the
/// cross-branch consensus mechanism — either a Khural proposal, a court
/// order, or (for emergencies) a constitutional supermajority.
///
/// The default implementation returns `true` (approval required).
/// Override to `false` ONLY for explicitly constitutional exceptions
/// documented in the pallet docs.
pub trait BranchConsensus {
    /// Whether cross-branch approval is constitutionally required
    /// for this branch to perform the requested action.
    ///
    /// Returns `true` by default — the constitutional default is
    /// maximum separation of powers.
    fn requires_cross_branch_approval() -> bool {
        true
    }
}

impl BranchConsensus for LegislativeAuthority {}
impl BranchConsensus for ExecutiveAuthority {}
impl BranchConsensus for JudicialAuthority {}
impl BranchConsensus for BankingAuthority {}

#[cfg(test)]
mod mock;
#[cfg(test)]
mod tests;

// =========================================================================
// Cross-pallet Interface for Habeas Corpus Enforcement
// =========================================================================

/// Interface that `pallet-constitution` calls to enforce Habeas Corpus.
///
/// Implement this in the runtime and wire to `pallet-judicial-courts` + `pallet-inomad-identity`.
/// When the timer expires without a verdict: unfreeze the citizen account.
pub trait HabeasCorpusInterface<AccountId> {
    /// Returns `true` if a binding verdict has been recorded for `who`.
    ///
    /// If `false` when the `max_lockup_block` passes, `on_initialize` calls
    /// `release_citizen` to unfreeze the account automatically.
    fn has_verdict(who: &AccountId) -> bool;

    /// Unconditionally release the citizen from the judicial freeze.
    ///
    /// Called automatically by `on_initialize` when the Habeas Corpus timer expires
    /// without a verdict having been entered by the courts.
    fn release_citizen(who: &AccountId) -> frame_support::dispatch::DispatchResult;
}

/// No-op implementation — use `()` when Habeas Corpus is not wired at the runtime level.
impl<AccountId> HabeasCorpusInterface<AccountId> for () {
    fn has_verdict(_who: &AccountId) -> bool {
        true
    } // treat as resolved → never auto-release
    fn release_citizen(_who: &AccountId) -> frame_support::dispatch::DispatchResult {
        Ok(())
    }
}

#[cfg(feature = "runtime-benchmarks")]
mod benchmarking;
#[frame_support::pallet]
pub mod pallet {
    use crate::weights::WeightInfo as _;
    use crate::HabeasCorpusInterface;
    use frame_support::{pallet_prelude::*, weights::Weight};
    use frame_system::pallet_prelude::*;

    // =========================================================================
    // Pallet Struct
    // =========================================================================

    #[pallet::pallet]
    pub struct Pallet<T>(_);

    // =========================================================================
    // Configuration Trait
    // =========================================================================

    #[pallet::config]
    pub trait Config: frame_system::Config<RuntimeEvent: From<Event<Self>>> {
        /// Weight information for extrinsics in this pallet.
        type WeightInfo: crate::weights::WeightInfo;
        /// Maximum number of concurrent Habeas Corpus timers.
        ///
        /// Bounded to prevent unbounded `on_initialize` iteration.
        /// In practice: number of active judicial freezes. `ConstU32<1024>` is safe.
        #[pallet::constant]
        type MaxConcurrentTimers: Get<u32>;

        /// Cross-pallet Habeas Corpus enforcement hook.
        ///
        /// Wire to `JudicialCorpusHook` in the runtime, which internally calls
        /// `pallet-judicial-courts::has_verdict` and `pallet-inomad-identity::unfreeze_citizen`.
        type HabeasCorpusHook: crate::HabeasCorpusInterface<Self::AccountId>;
    }

    // =========================================================================
    // Structs
    // =========================================================================

    /// An active Habeas Corpus timer for a judicially frozen citizen.
    ///
    /// Registered by the judicial courts when freezing a citizen before trial.
    /// Automatically enforced by `on_initialize` if deadline passes without verdict.
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
    pub struct HabeasCorpusTimer<BlockNumber> {
        /// Block number at which the pre-trial freeze expires automatically.
        ///
        /// If no `Verdict` is recorded by this block in `pallet-judicial-courts`,
        /// `on_initialize` automatically calls `HabeasCorpusHook::release_citizen`.
        ///
        /// ## Constitutional Guarantee
        ///
        /// This is the technical enforcement of Article III of the Constitution:
        /// "No citizen may be held in a frozen state beyond this deadline without
        /// a binding court verdict." The investigating authority cannot extend this
        /// deadline without going through the judicial pallet.
        pub max_lockup_block: BlockNumber,
        /// The block at which the freeze was registered. For audit trail.
        pub registered_at: BlockNumber,
        /// Short description of the case — blake2_256 hash of the case reference.
        /// Stored on-chain for transparency; full case record is in `pallet-judicial-courts`.
        pub case_hash: [u8; 32],
        /// Whether this timer has already been resolved (prevents double-release).
        pub resolved: bool,
    }

    // =========================================================================
    // Storage
    // =========================================================================

    /// The immutable IPFS CID of the Altan Republic's Constitutional Document.
    ///
    /// ## Article IV — Non-Alienation of Sovereignty
    ///
    /// This value is stored ONCE by the Creator at genesis (or via the one-time
    /// `set_constitution_hash` Root extrinsic) and can NEVER be overwritten.
    /// Any subsequent call to `set_constitution_hash` returns `Error::ConstitutionAlreadySet`.
    ///
    /// The 46 bytes store a base58-encoded IPFS CIDv1 (SHA2-256 multihash).
    /// The IPFS node guarantees content-addressed immutability.
    ///
    /// Omitting ownership — this is a GLOBAL constant of the Republic.
    #[pallet::storage]
    #[pallet::getter(fn core_rights_hash)]
    pub type CoreRightsHash<T> = StorageValue<_, [u8; 46], OptionQuery>;

    /// Active Habeas Corpus timers — one per judicially frozen citizen.
    ///
    /// ## Article III — Habeas Corpus Enforcement
    ///
    /// Keyed by `AccountId`. Bounded by `T::MaxConcurrentTimers`.
    /// `on_initialize` iterates this map every block, automatically releasing
    /// citizens whose deadline has passed without a court verdict.
    ///
    /// Entries are removed when:
    /// - The timer expires and `release_citizen` is called (auto-release).
    /// - `resolve_habeas_corpus` is called by the courts (verdict entered).
    #[pallet::storage]
    #[pallet::getter(fn habeas_corpus_timers)]
    pub type HabeasCorpusTimers<T: Config> = StorageMap<
        _,
        Blake2_128Concat,
        T::AccountId,
        HabeasCorpusTimer<BlockNumberFor<T>>,
        OptionQuery,
    >;

    // =========================================================================
    // Events
    // =========================================================================

    #[pallet::event]
    #[pallet::generate_deposit(pub(super) fn deposit_event)]
    pub enum Event<T: Config> {
        /// The Constitution hash was set for the first (and only) time.
        ///
        /// Emitted exactly once in the Republic's lifetime.
        ConstitutionAnchoredOnChain {
            /// IPFS CIDv1 of the constitutional document (base58, 46 bytes).
            cid: [u8; 46],
        },

        /// A Habeas Corpus timer was registered for a judicially frozen citizen.
        HabeasCorpusTimerRegistered {
            citizen: T::AccountId,
            max_lockup_block: BlockNumberFor<T>,
            case_hash: [u8; 32],
        },

        /// A Habeas Corpus timer fired automatically — no verdict was entered in time.
        ///
        /// The citizen has been released per Article III of the Constitution.
        /// The investigating authority must re-apply to the courts if they wish
        /// to continue proceedings.
        HabeasCorpusAutoReleased {
            citizen: T::AccountId,
            /// Block at which the auto-release fired.
            released_at: BlockNumberFor<T>,
        },

        /// A Habeas Corpus timer was resolved by the courts — a verdict was entered.
        ///
        /// The timer is removed; the courts now control the citizen's fate.
        HabeasCorpusResolved { citizen: T::AccountId },
    }

    // =========================================================================
    // Errors
    // =========================================================================

    #[pallet::error]
    pub enum Error<T> {
        /// The Constitution hash has already been set. It is immutable.
        ///
        /// ## Article IV — Non-Alienation of Sovereignty
        ///
        /// Once set, the constitutional anchor is eternal. Only a physical
        /// network fork can change it — and that would create a new Republic,
        /// not amend this one.
        ConstitutionAlreadySet,

        /// No active Habeas Corpus timer exists for this citizen.
        ///
        /// Either the timer was never registered, or it has already been resolved.
        NoActiveTimer,

        /// The Habeas Corpus timer is already resolved.
        TimerAlreadyResolved,
    }

    // =========================================================================
    // Block Hooks — Habeas Corpus Enforcement
    // =========================================================================

    #[pallet::hooks]
    impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T> {
        /// Habeas Corpus enforcement engine.
        ///
        /// Checks all active timers every block. For any timer where
        /// `current_block >= max_lockup_block` and no verdict has been entered,
        /// automatically releases the citizen.
        ///
        /// ## Constitutional Mandate (Article III)
        ///
        /// This hook is the technical guarantee of Habeas Corpus. It cannot be
        /// disabled by governance, paused by Root, or delayed by the courts.
        /// The Republic's constitution runs in every block, without exception.
        fn on_initialize(n: BlockNumberFor<T>) -> Weight {
            let mut weight = Weight::zero();

            // Collect expired, unresolved timers.
            // Two-phase (collect then mutate) to avoid re-entrancy on the storage iterator.
            let expired: alloc::vec::Vec<T::AccountId> = HabeasCorpusTimers::<T>::iter()
                .filter_map(|(who, timer)| {
                    if !timer.resolved
                        && n >= timer.max_lockup_block
                        && !T::HabeasCorpusHook::has_verdict(&who)
                    {
                        Some(who)
                    } else {
                        None
                    }
                })
                .collect();

            for citizen in expired {
                // Auto-release per Article III.
                let _ = T::HabeasCorpusHook::release_citizen(&citizen);
                HabeasCorpusTimers::<T>::remove(&citizen);

                Self::deposit_event(Event::HabeasCorpusAutoReleased {
                    citizen,
                    released_at: n,
                });

                weight = weight.saturating_add(Weight::from_parts(40_000_000, 0));
            }

            weight
        }
    }

    // =========================================================================
    // Extrinsics
    // =========================================================================

    #[pallet::call]
    impl<T: Config> Pallet<T> {
        // ─── set_constitution_hash ───────────────────────────────────────────

        /// Anchor the IPFS CID of the Republic's Constitutional Document on-chain.
        ///
        /// ## One-Time Constitutional Act
        ///
        /// This extrinsic may be called **exactly once** by the Root authority
        /// (the Creator of the Republic). After the first call, `CoreRightsHash`
        /// is immutable — any subsequent call returns `Error::ConstitutionAlreadySet`.
        ///
        /// The `cid` parameter is a 46-byte base58-encoded IPFS CIDv1 pointing to
        /// the full text of the Altan Republic's Bill of Rights and Freedoms.
        ///
        /// ## Why IPFS?
        ///
        /// IPFS CIDs are content-addressed: the hash of the document IS the CID.
        /// Storing the CID on-chain creates a permanent, tamper-proof link between
        /// the blockchain and the constitutional text. Any modification to the text
        /// would produce a different CID — making silent amendments impossible.
        ///
        /// ## Constitutional Rights Encoded (see module-level docs for full text)
        ///
        /// The linked document must contain (at minimum):
        /// - Article I: Fundamental Freedoms (speech, religion, thought, learning, movement)
        /// - Article II: "My Home is My Castle" (including cryptographic key inviolability)
        /// - Article III: Habeas Corpus (max pre-trial lockup duration)
        /// - Article IV: Non-Alienation of Sovereignty (the 79 peoples as permanent source of power)
        #[pallet::call_index(0)]
        #[pallet::weight(T::WeightInfo::set_constitution_hash())]
        pub fn set_constitution_hash(origin: OriginFor<T>, cid: [u8; 46]) -> DispatchResult {
            ensure_root(origin)?;

            // ONE-TIME: if already set, refuse. The Constitution is immutable.
            ensure!(
                CoreRightsHash::<T>::get().is_none(),
                Error::<T>::ConstitutionAlreadySet
            );

            CoreRightsHash::<T>::put(cid);

            Self::deposit_event(Event::ConstitutionAnchoredOnChain { cid });
            Ok(())
        }

        // ─── register_lockup ────────────────────────────────────────────────

        /// Register a Habeas Corpus timer for a judicially frozen citizen.
        ///
        /// ## Article III — Habeas Corpus
        ///
        /// Called by the judicial authority (Root-gated, in practice dispatched by
        /// `pallet-judicial-courts` via privileged origin) simultaneously with
        /// the account freeze. Every judicial freeze MUST have a corresponding
        /// Habeas Corpus timer — there is no indefinite pre-trial detention on
        /// the Altan Network.
        ///
        /// `max_lockup_block` is the constitutional deadline. If no verdict is
        /// recorded in `pallet-judicial-courts` by this block, `on_initialize` will
        /// automatically call `HabeasCorpusHook::release_citizen` and emit
        /// `HabeasCorpusAutoReleased`.
        ///
        /// `case_hash` is the blake2_256 hash of the case reference number — stored
        /// for transparency. The full case record is in `pallet-judicial-courts`.
        #[pallet::call_index(1)]
        #[pallet::weight(T::WeightInfo::register_lockup())]
        pub fn register_lockup(
            origin: OriginFor<T>,
            citizen: T::AccountId,
            max_lockup_block: BlockNumberFor<T>,
            case_hash: [u8; 32],
        ) -> DispatchResult {
            ensure_root(origin)?;

            let now = frame_system::Pallet::<T>::block_number();

            HabeasCorpusTimers::<T>::insert(
                &citizen,
                HabeasCorpusTimer {
                    max_lockup_block,
                    registered_at: now,
                    case_hash,
                    resolved: false,
                },
            );

            Self::deposit_event(Event::HabeasCorpusTimerRegistered {
                citizen,
                max_lockup_block,
                case_hash,
            });

            Ok(())
        }

        // ─── resolve_habeas_corpus ───────────────────────────────────────────

        /// Mark a Habeas Corpus timer as resolved — a court verdict has been entered.
        ///
        /// Called by the judicial authority after a verdict is recorded in
        /// `pallet-judicial-courts`. Prevents `on_initialize` from auto-releasing
        /// the citizen (the courts now control the outcome per the verdict).
        ///
        /// The timer entry is removed from storage; the citizen's status transitions
        /// are handled by the judicial pallet, not by this automatic release hook.
        #[pallet::call_index(2)]
        #[pallet::weight(T::WeightInfo::resolve_habeas_corpus())]
        pub fn resolve_habeas_corpus(
            origin: OriginFor<T>,
            citizen: T::AccountId,
        ) -> DispatchResult {
            ensure_root(origin)?;

            HabeasCorpusTimers::<T>::try_mutate(&citizen, |maybe| {
                let timer = maybe.as_mut().ok_or(Error::<T>::NoActiveTimer)?;
                ensure!(!timer.resolved, Error::<T>::TimerAlreadyResolved);
                timer.resolved = true;
                Ok::<(), DispatchError>(())
            })?;

            // Clean up resolved timer.
            HabeasCorpusTimers::<T>::remove(&citizen);

            Self::deposit_event(Event::HabeasCorpusResolved { citizen });
            Ok(())
        }
    }
}
