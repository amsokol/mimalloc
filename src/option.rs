//! Runtime options of mimalloc (`mi_option_*`).

use std::ffi::{c_int, c_long};

unsafe extern "C" {
    fn mi_option_get(option: c_int) -> c_long;
    fn mi_option_get_size(option: c_int) -> usize;
    fn mi_option_set(option: c_int, value: c_long);
    fn mi_option_is_enabled(option: c_int) -> bool;
    fn mi_option_set_enabled(option: c_int, enable: bool);
}

/// A mimalloc runtime option (`mi_option_t` in `mimalloc.h`).
///
/// Every option of mimalloc 3.6.0 is listed except the `deprecated_*` ones,
/// which mimalloc no longer reads. Each variant is named after the option in
/// `util/options.c`; [`Option::name`] gives that name, and `MIMALLOC_<NAME>` is the
/// environment variable that sets it.
///
/// This type shadows [`std::option::Option`] when imported, so refer to it as
/// `mimalloc::Option`:
///
/// ```rust
/// mimalloc::Option::PurgeDelay.set(0);
/// assert_eq!(mimalloc::Option::PurgeDelay.get(), 0);
/// ```
///
/// # When an option takes effect
///
/// mimalloc initializes itself before `main`: a C constructor (or the first
/// allocation, whichever comes first) reads every option from its
/// environment variable or its built-in default, sets up the OS layer and
/// the page map, and reserves the arenas that the Rust runtime's own
/// allocations need. A value set from Rust replaces the value from the
/// environment, but code that has already run with the old value is not
/// redone. Each variant says when mimalloc reads it:
///
/// - *on every use*: setting it changes all later operations of that kind;
/// - *per arena* or *per thread*: it applies only to arenas reserved or
///   threads started after the call;
/// - *at process start* or *at exit*: setting it from `main` has no effect,
///   or it applies at exit only.
///
/// For an option that only works from process start, use its environment
/// variable or a build-time default (the `no_thp` cargo feature sets
/// [`Option::AllowThp`] to 0 at build time).
///
/// Options are process-wide. mimalloc stores them without locking (reads and
/// writes are word-sized and tolerated as races by design), so set them early
/// in `main`, before spawning threads, to have every thread see the same
/// value from the start.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[repr(i32)]
#[non_exhaustive]
pub enum Option {
    /// Print error messages to stderr (default: on in debug builds of this
    /// crate, off in release builds). Read on every message.
    ShowErrors = 0,
    /// Print statistics to stderr at process exit (default: off). Read at
    /// exit, so setting it from `main` works.
    ShowStats = 1,
    /// Print verbose messages to stderr (default: off). Read on every
    /// message; the option dump that verbose mode prints at start-up only
    /// happens when it is set through the environment.
    Verbose = 2,
    /// Commit arena memory when the arena is reserved: 0 = no, 1 = yes,
    /// 2 = only on systems with overcommit, such as Linux (default: 2).
    /// Per arena: applies to arenas reserved after the call.
    ArenaEagerCommit = 4,
    /// Purge memory by decommitting it (1, default) instead of resetting it
    /// (0; `MADV_FREE` on Unix, `MEM_RESET` on Windows). Read on every purge.
    PurgeDecommits = 5,
    /// Use large OS pages (2 or 4 MiB, `MAP_HUGETLB`) when available; implies
    /// eager commit (default: off). Read when mimalloc maps OS memory, so it
    /// applies to arenas reserved after the call.
    AllowLargeOsPages = 6,
    /// Reserve this many 1 GiB huge OS pages at start-up (default: 0).
    /// At process start only: setting it from `main` has no effect.
    ReserveHugeOsPages = 7,
    /// NUMA node to reserve [`Option::ReserveHugeOsPages`] on; -1 spreads
    /// them over all nodes (default: -1). At process start only.
    ReserveHugeOsPagesAt = 8,
    /// Reserve this much OS memory, in KiB, in an arena at start-up
    /// (default: 0). At process start only. [`Option::get_size`] returns
    /// bytes.
    ReserveOsMemory = 9,
    /// Delay, in milliseconds, before freed memory is purged (returned to the
    /// OS): 0 purges at once, a negative value never purges (default: 1000).
    /// Memory inside arenas waits `purge_delay × arena_purge_mult` ms, so
    /// 4 s by default. Since mimalloc 3.6.0, a freed range of at least
    /// [`Option::ArenaPurgeImmediateSize`] (32 MiB by default) is purged at
    /// once whatever the delay, unless purging is off (a negative delay).
    ///
    /// Read on every free that schedules a purge and on every purge pass, so
    /// it applies to all memory freed after the call, including memory in
    /// arenas reserved before `main`. With 0, each range is purged as it is
    /// freed, in 64 KiB arena slices, without the 2 MiB minimum that
    /// [`Option::AllowThp`] 2 otherwise keeps. When it is changed from a
    /// positive value to 0 or less, ranges that were scheduled earlier with the old
    /// delay are not purged any more (mimalloc's periodic purge pass, and
    /// [`collect`](crate::collect), skip all work when the delay is ≤ 0)
    /// until those ranges are reused and freed again; set it at the start of
    /// `main` so that set is limited to what the runtime freed before `main`.
    PurgeDelay = 15,
    /// Use at most this many NUMA nodes; 0 detects them (default: 0).
    /// At process start only: the node count is fixed when the main thread
    /// is initialized.
    UseNumaNodes = 16,
    /// Do not allocate memory from the OS, only from arenas reserved
    /// up front (default: off). Read on every arena or OS allocation.
    DisallowOsAlloc = 17,
    /// Tag for OS memory accounting, macOS only (default: 100). Read on
    /// every OS mapping.
    OsTag = 18,
    /// Print at most this many error messages (default: 32). At process
    /// start only: the limit is copied when mimalloc initializes.
    MaxErrors = 19,
    /// Print at most this many warnings (default: 32). At process start
    /// only: the limit is copied when mimalloc initializes.
    MaxWarnings = 20,
    /// Release all OS memory at process exit (default: off). Unsafe if
    /// anything still frees after exit, for example in other atexit
    /// handlers. Read at exit.
    DestroyOnExit = 22,
    /// Size, in KiB, of a new arena (default: 1 GiB on 64-bit, 128 MiB on
    /// 32-bit; later arenas grow exponentially). Per arena: applies to arenas
    /// reserved after the call. [`Option::get_size`] returns bytes.
    ArenaReserve = 23,
    /// Multiplier on [`Option::PurgeDelay`] for memory inside arenas; 0
    /// purges at once, a negative value never purges (default: 4). Read on
    /// every free that schedules a purge and on every purge pass, like
    /// [`Option::PurgeDelay`], so it applies to all arenas, including those
    /// reserved before `main`. As with [`Option::PurgeDelay`], ranges of at
    /// least [`Option::ArenaPurgeImmediateSize`] are purged at once.
    ArenaPurgeMult = 24,
    /// Do not allocate from arenas; take every page straight from the OS
    /// (default: off). Read on every page allocation.
    DisallowArenaAlloc = 26,
    /// Windows only: retry an out-of-memory OS allocation for up to this many
    /// milliseconds (default: 400, at most 2000). Read on every failed OS
    /// allocation.
    RetryOnOom = 27,
    /// Minimal object size for guard pages. Has no effect in this crate,
    /// which does not build mimalloc with `MI_GUARDED`. Setting it above
    /// [`Option::GuardedMax`] raises that option too.
    GuardedMin = 29,
    /// Maximal object size for guard pages (default: 1 GiB). No effect in
    /// this crate (no `MI_GUARDED`). Setting it below [`Option::GuardedMin`]
    /// lowers that option too.
    GuardedMax = 30,
    /// Place guarded objects right before the guard page, ignoring their
    /// alignment (default: off). Named `guarded_precise` before mimalloc
    /// 3.6.0. No effect in this crate (no `MI_GUARDED`).
    UnsafeGuardedPrecise = 31,
    /// Guard one in this many objects in the guarded size range
    /// (default: 0). No effect in this crate (no `MI_GUARDED`).
    GuardedSampleRate = 32,
    /// Seed for picking guarded objects (default: 0). No effect in this
    /// crate (no `MI_GUARDED`).
    GuardedSampleSeed = 33,
    /// Collect a thread's heap every this many calls into the slow
    /// allocation path (default: 10000). Read on every slow-path allocation.
    GenericCollect = 34,
    /// Reclaim an abandoned page on free: -1 = never, 0 = only into the
    /// thread heap it came from, 1 = into any heap (default: 1; 0 before
    /// mimalloc 3.6.0).
    ///
    /// Since mimalloc 3.6.0 this is only the default for new heaps: each heap
    /// copies it when it is created. The heap behind the global allocator is
    /// created before `main`, so setting the option from `main` does not
    /// change it. To get the 3.5 behaviour, start the process with
    /// `MIMALLOC_PAGE_RECLAIM_ON_FREE=0`. Before 3.6.0, the 0/1 choice was
    /// read on every free.
    PageReclaimOnFree = 35,
    /// Number of full small pages a thread heap keeps per size class;
    /// -1 never abandons pages (default: 2). Per thread: applies to threads
    /// started after the call.
    PageFullRetain = 36,
    /// Number of candidate pages to look at when picking a page to allocate
    /// from (default: 4). Read on every page search.
    PageMaxCandidates = 37,
    /// Maximal user-space virtual address bits to assume; 0 detects them
    /// (default: 0). At process start only (it sizes the page map).
    MaxVabits = 38,
    /// Commit the whole page map at start-up (default: off; on in some macOS
    /// builds). At process start only.
    PagemapCommit = 39,
    /// Commit page memory on demand instead of all at once: 0 = no, 1 = yes,
    /// 2 = only on systems without overcommit (default: 0). Read on every
    /// fresh page allocation.
    PageCommitOnDemand = 40,
    /// Do not reclaim a page into the thread heap it came from when that heap
    /// already owns this many pages of its size class; -1 = no limit
    /// (default: -1). Read on every reclaim.
    PageMaxReclaim = 41,
    /// Do not reclaim a page across threads when the heap already owns this
    /// many pages of its size class (default: 32). Read on every reclaim.
    PageCrossThreadMaxReclaim = 42,
    /// Transparent huge pages (THP), Linux and Android: 0 = off, 1 = on,
    /// 2 = on and purge in whole huge pages only (default: 2; 0 on Android).
    ///
    /// mimalloc reads it in three places:
    ///
    /// - **At process start** (Linux/Android): when it is 0, mimalloc calls
    ///   `prctl(PR_SET_THP_DISABLE)`, which turns THP off for the whole
    ///   process, also for memory not allocated by mimalloc. Child processes
    ///   inherit that setting across `fork` and `execve`. Setting the option
    ///   from `main` does **not** do this.
    /// - **Per arena**: when it is not 0, mimalloc marks each new OS mapping
    ///   (each arena) with `madvise(MADV_HUGEPAGE)`, which is what
    ///   lets the kernel use huge pages there when
    ///   `/sys/kernel/mm/transparent_hugepage/enabled` is `madvise`. Setting
    ///   it to 0 from `main` only affects arenas reserved after the call; the
    ///   first arena (1 GiB), reserved before `main` for the runtime's own
    ///   allocations, stays marked.
    /// - **On every delayed purge pass**: with 2 (and
    ///   [`Option::ArenaPurgeMinSize`] at 0), mimalloc purges only whole 2 MiB
    ///   ranges, so a huge page goes back to the OS only when all of it is
    ///   free. With 0 or 1 it purges in OS pages (4 KiB). This part takes
    ///   effect at once from `main`. It does not apply when purging is
    ///   immediate ([`Option::PurgeDelay`] or [`Option::ArenaPurgeMult`] 0):
    ///   then each freed range (64 KiB slices) is purged as it is freed.
    ///
    /// Since mimalloc 3.6.0, when the kernel's huge page size (read from
    /// `hpage_pmd_size`) is over 8 MiB, as with 64 KiB pages on arm64,
    /// mimalloc treats the system as having no THP and marks its mappings
    /// `MADV_NOHUGEPAGE`.
    ///
    /// To ship a binary with THP off from the start, build this crate with
    /// the `no_thp` cargo feature (compiles in `MI_DEFAULT_ALLOW_THP=0`, as
    /// mimalloc's CMake option `MI_NO_THP` does), or set
    /// `MIMALLOC_ALLOW_THP=0` in the environment; the environment variable
    /// overrides the build-time default.
    AllowThp = 43,
    /// Minimal size, in KiB, of a purge (named `minimal_purge_size` before
    /// mimalloc 3.6.0); 0 means the OS page size, or 2 MiB
    /// when [`Option::AllowThp`] is 2 and the system has THP (default: 0).
    /// Read on every delayed purge pass; immediate purges
    /// ([`Option::PurgeDelay`] 0, or a range of at least
    /// [`Option::ArenaPurgeImmediateSize`]) do not apply it. [`Option::get_size`]
    /// returns bytes.
    ArenaPurgeMinSize = 44,
    /// Largest object, in KiB, allocated inside an arena; larger ones are
    /// mapped from the OS directly (default: 2 GiB on 64-bit, 256 MiB on
    /// 32-bit). Read on every large allocation. [`Option::get_size`]
    /// returns bytes.
    ArenaMaxObjectSize = 45,
    /// Experimental: tie a new arena to the NUMA node of the thread that
    /// reserves it (default: off). Per arena.
    ArenaIsNumaLocal = 46,
    /// Merge a thread heap's statistics into its parent heap on every
    /// collection (default: on). Read on every collection.
    StatsMergeOnCollect = 47,
    /// Bytes allocated between automatic snapshots of the heap profiler
    /// (default: 1 GiB); 0 disables them. The profiler runs only when
    /// `MIMALLOC_PROFILE` is set, and reads this option once at process
    /// start, so setting it from `main` has no effect: use
    /// `MIMALLOC_PROFILE_ALLOC_INTERVAL`. [`Option::get_size`] returns bytes.
    ProfileAllocInterval = 48,
    /// Take a heap-profiler snapshot whenever the in-use bytes have grown by
    /// this much (default: 0, off). Only used with `MIMALLOC_PROFILE`, and read
    /// once at process start, as for [`Option::ProfileAllocInterval`].
    ProfileInuseInterval = 49,
    /// Seconds between automatic heap-profiler snapshots (default: 0, off).
    /// Only used with `MIMALLOC_PROFILE`, and read once at process start, as
    /// for [`Option::ProfileAllocInterval`].
    ProfileTimeInterval = 50,
    /// Heap-profiler sampling rate, in bytes (default: 512 KiB). Only used
    /// with `MIMALLOC_PROFILE`, and read once at process start, as for
    /// [`Option::ProfileAllocInterval`].
    ProfileSampleRate = 51,
    /// Merge a thread heap's statistics into its parent heap once this many
    /// bytes were allocated or freed on a page; 0 disables (default: 512 KiB).
    /// Read on every page statistics update.
    StatsMergeThreshold = 52,
    /// Freed ranges of at least this size, in KiB, are purged as soon as they
    /// are freed instead of waiting for [`Option::PurgeDelay`] (default:
    /// 32 MiB on 64-bit). New in mimalloc 3.6.0. It does not override a
    /// negative delay, which never purges. Read on every free that schedules
    /// a purge. [`Option::get_size`]
    /// returns bytes.
    ArenaPurgeImmediateSize = 53,
}

impl Option {
    /// All options, in the order of `mi_option_t`.
    pub const ALL: [Option; 45] = [
        Option::ShowErrors,
        Option::ShowStats,
        Option::Verbose,
        Option::ArenaEagerCommit,
        Option::PurgeDecommits,
        Option::AllowLargeOsPages,
        Option::ReserveHugeOsPages,
        Option::ReserveHugeOsPagesAt,
        Option::ReserveOsMemory,
        Option::PurgeDelay,
        Option::UseNumaNodes,
        Option::DisallowOsAlloc,
        Option::OsTag,
        Option::MaxErrors,
        Option::MaxWarnings,
        Option::DestroyOnExit,
        Option::ArenaReserve,
        Option::ArenaPurgeMult,
        Option::DisallowArenaAlloc,
        Option::RetryOnOom,
        Option::GuardedMin,
        Option::GuardedMax,
        Option::UnsafeGuardedPrecise,
        Option::GuardedSampleRate,
        Option::GuardedSampleSeed,
        Option::GenericCollect,
        Option::PageReclaimOnFree,
        Option::PageFullRetain,
        Option::PageMaxCandidates,
        Option::MaxVabits,
        Option::PagemapCommit,
        Option::PageCommitOnDemand,
        Option::PageMaxReclaim,
        Option::PageCrossThreadMaxReclaim,
        Option::AllowThp,
        Option::ArenaPurgeMinSize,
        Option::ArenaMaxObjectSize,
        Option::ArenaIsNumaLocal,
        Option::StatsMergeOnCollect,
        Option::ProfileAllocInterval,
        Option::ProfileInuseInterval,
        Option::ProfileTimeInterval,
        Option::ProfileSampleRate,
        Option::StatsMergeThreshold,
        Option::ArenaPurgeImmediateSize,
    ];

    /// The option's name in `util/options.c`, such as `"purge_delay"`. The
    /// environment variable `MIMALLOC_<NAME>` (any case) sets it at start-up.
    pub const fn name(self) -> &'static str {
        match self {
            Option::ShowErrors => "show_errors",
            Option::ShowStats => "show_stats",
            Option::Verbose => "verbose",
            Option::ArenaEagerCommit => "arena_eager_commit",
            Option::PurgeDecommits => "purge_decommits",
            Option::AllowLargeOsPages => "allow_large_os_pages",
            Option::ReserveHugeOsPages => "reserve_huge_os_pages",
            Option::ReserveHugeOsPagesAt => "reserve_huge_os_pages_at",
            Option::ReserveOsMemory => "reserve_os_memory",
            Option::PurgeDelay => "purge_delay",
            Option::UseNumaNodes => "use_numa_nodes",
            Option::DisallowOsAlloc => "disallow_os_alloc",
            Option::OsTag => "os_tag",
            Option::MaxErrors => "max_errors",
            Option::MaxWarnings => "max_warnings",
            Option::DestroyOnExit => "destroy_on_exit",
            Option::ArenaReserve => "arena_reserve",
            Option::ArenaPurgeMult => "arena_purge_mult",
            Option::DisallowArenaAlloc => "disallow_arena_alloc",
            Option::RetryOnOom => "retry_on_oom",
            Option::GuardedMin => "guarded_min",
            Option::GuardedMax => "guarded_max",
            Option::UnsafeGuardedPrecise => "unsafe_guarded_precise",
            Option::GuardedSampleRate => "guarded_sample_rate",
            Option::GuardedSampleSeed => "guarded_sample_seed",
            Option::GenericCollect => "generic_collect",
            Option::PageReclaimOnFree => "page_reclaim_on_free",
            Option::PageFullRetain => "page_full_retain",
            Option::PageMaxCandidates => "page_max_candidates",
            Option::MaxVabits => "max_vabits",
            Option::PagemapCommit => "pagemap_commit",
            Option::PageCommitOnDemand => "page_commit_on_demand",
            Option::PageMaxReclaim => "page_max_reclaim",
            Option::PageCrossThreadMaxReclaim => "page_cross_thread_max_reclaim",
            Option::AllowThp => "allow_thp",
            Option::ArenaPurgeMinSize => "arena_purge_min_size",
            Option::ArenaMaxObjectSize => "arena_max_object_size",
            Option::ArenaIsNumaLocal => "arena_is_numa_local",
            Option::StatsMergeOnCollect => "stats_merge_on_collect",
            Option::ProfileAllocInterval => "profile_alloc_interval",
            Option::ProfileInuseInterval => "profile_inuse_interval",
            Option::ProfileTimeInterval => "profile_time_interval",
            Option::ProfileSampleRate => "profile_sample_rate",
            Option::StatsMergeThreshold => "stats_merge_threshold",
            Option::ArenaPurgeImmediateSize => "arena_purge_immediate_size",
        }
    }

    /// Returns the option's current value (`mi_option_get`).
    ///
    /// Size options ([`Option::ReserveOsMemory`], [`Option::ArenaReserve`],
    /// [`Option::ArenaPurgeMinSize`], [`Option::ArenaPurgeImmediateSize`],
    /// [`Option::ArenaMaxObjectSize`]) are in KiB; [`Option::get_size`] returns them in bytes.
    #[inline]
    #[allow(clippy::useless_conversion)] // C `long` is `i32` on Windows
    pub fn get(self) -> i64 {
        // SAFETY: `self as c_int` is a valid `mi_option_t` value (the
        // discriminants match `mimalloc.h`, checked by a test), and
        // `mi_option_get` has no other preconditions.
        i64::from(unsafe { mi_option_get(self as c_int) })
    }

    /// Returns the option's value as a size in bytes (`mi_option_get_size`):
    /// KiB options are multiplied by 1024, negative values read as 0.
    #[inline]
    pub fn get_size(self) -> usize {
        // SAFETY: as in `get`, the argument is a valid `mi_option_t`.
        unsafe { mi_option_get_size(self as c_int) }
    }

    /// Sets the option (`mi_option_set`), replacing the value from the
    /// environment or the default. See the variant's documentation for when
    /// the new value takes effect.
    ///
    /// Where C `long` is 32 bits (Windows, 32-bit targets), `value` is
    /// clamped to the `i32` range.
    #[inline]
    #[allow(clippy::useless_conversion)] // C `long` is `i32` on Windows
    pub fn set(self, value: i64) {
        let value = value.clamp(i64::from(c_long::MIN), i64::from(c_long::MAX)) as c_long;
        // SAFETY: the argument is a valid `mi_option_t`; mimalloc allows
        // options to be set at any time and from any thread.
        unsafe { mi_option_set(self as c_int, value) }
    }

    /// Returns whether the option is non-zero (`mi_option_is_enabled`).
    #[inline]
    pub fn is_enabled(self) -> bool {
        // SAFETY: as in `get`, the argument is a valid `mi_option_t`.
        unsafe { mi_option_is_enabled(self as c_int) }
    }

    /// Sets the option to 1 or 0 (`mi_option_set_enabled`). Same as
    /// [`Option::set`] with `enable as i64`.
    #[inline]
    pub fn set_enabled(self, enable: bool) {
        // SAFETY: as in `set`, the argument is a valid `mi_option_t`.
        unsafe { mi_option_set_enabled(self as c_int, enable) }
    }
}
