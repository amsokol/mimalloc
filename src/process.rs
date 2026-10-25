//! Process memory information and collection (`mi_process_info`, `mi_collect`).

unsafe extern "C" {
    fn mi_process_info(
        elapsed_msecs: *mut usize,
        user_msecs: *mut usize,
        system_msecs: *mut usize,
        current_rss: *mut usize,
        peak_rss: *mut usize,
        current_commit: *mut usize,
        peak_commit: *mut usize,
        page_faults: *mut usize,
    );
    fn mi_collect(force: bool);
}

/// Process time and memory counters, as returned by [`process_info`].
///
/// Times are in milliseconds, sizes in bytes. A field mimalloc cannot get on
/// the current platform is 0. Where each value comes from:
///
/// | Field | Linux, Android | macOS, iOS | Windows | other Unix (BSD, ...) | WASI, Emscripten |
/// | - | - | - | - | - | - |
/// | `elapsed_ms` | mimalloc's clock since process start | same | same | same | same |
/// | `user_ms`, `system_ms` | `getrusage` | `getrusage` | `GetProcessTimes` | `getrusage` | 0 |
/// | `current_rss` | `VmRSS` in `/proc/self/status` | `task_info` resident size | working set | 0 | 0 |
/// | `peak_rss` | `getrusage` max RSS | `getrusage` max RSS | peak working set | `getrusage` max RSS | 0 |
/// | `current_commit`, `peak_commit` | mimalloc's committed bytes | mimalloc's committed bytes | process private bytes (`PagefileUsage`) | mimalloc's committed bytes | mimalloc's committed bytes |
/// | `page_faults` | major faults | major faults | all page faults | major faults (0 on Haiku) | 0 |
///
/// The RSS values cover the whole process. "mimalloc's committed bytes" is
/// memory mimalloc counts as committed from the OS, for its own allocations
/// only. On Linux, where arenas are mapped with overcommit, a range counts
/// once mimalloc first hands it out, possibly before the program touches it,
/// so it can exceed the RSS. On Unix, purging memory (`MADV_DONTNEED`, or
/// `MADV_FREE_REUSABLE` on macOS) lowers the RSS but, in release builds of
/// this crate, does not lower the commit count, because mimalloc keeps the
/// range committed in its books; it does lower it in debug builds and with
/// the `secure` feature. Use `current_rss` to see memory going back to the
/// OS. On Windows, the commit values are the OS's private-bytes count for
/// the whole process and drop when mimalloc decommits.
///
/// mimalloc itself reports its committed bytes as the current RSS on Linux,
/// Android and the BSDs; this crate replaces that with the OS's value on
/// Linux and Android and with 0 elsewhere, so `current_rss` is never a
/// commit count.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ProcessInfo {
    /// Wall-clock time since the process started (since mimalloc
    /// initialized, which is before `main`).
    pub elapsed_ms: u64,
    /// CPU time spent in user mode.
    pub user_ms: u64,
    /// CPU time spent in the kernel.
    pub system_ms: u64,
    /// Resident set size now.
    pub current_rss: u64,
    /// Largest resident set size so far.
    pub peak_rss: u64,
    /// Committed memory now.
    pub current_commit: u64,
    /// Largest committed memory so far.
    pub peak_commit: u64,
    /// Page faults so far.
    pub page_faults: u64,
}

/// Returns the process's time and memory counters (`mi_process_info`).
///
/// See [`ProcessInfo`] for where each value comes from on each platform.
/// On Linux and Android this reads `/proc/self/status`, so it allocates and
/// does a system call or two; do not call it in a hot loop.
pub fn process_info() -> ProcessInfo {
    let (mut elapsed, mut user, mut system, mut faults) = (0usize, 0usize, 0usize, 0usize);
    let (mut current_rss, mut peak_rss) = (0usize, 0usize);
    let (mut current_commit, mut peak_commit) = (0usize, 0usize);
    // SAFETY: each pointer is to a distinct, live, writable `usize` that
    // `mi_process_info` only writes; the call has no other preconditions.
    unsafe {
        mi_process_info(
            &mut elapsed,
            &mut user,
            &mut system,
            &mut current_rss,
            &mut peak_rss,
            &mut current_commit,
            &mut peak_commit,
            &mut faults,
        )
    };
    let current_rss = os_current_rss().unwrap_or(current_rss as u64);
    // The peak comes from a different call made slightly earlier (and, on
    // Linux, from counters the kernel updates lazily), so it can trail the
    // current value by a few pages.
    let peak_rss = if cfg!(target_family = "wasm") {
        0
    } else {
        (peak_rss as u64).max(current_rss)
    };
    ProcessInfo {
        elapsed_ms: elapsed as u64,
        user_ms: user as u64,
        system_ms: system as u64,
        current_rss,
        peak_rss,
        current_commit: current_commit as u64,
        peak_commit: peak_commit as u64,
        page_faults: faults as u64,
    }
}

/// Where mimalloc's `current_rss` is a real RSS, `None` (keep mimalloc's
/// value); elsewhere the OS's value, or `Some(0)` when it is not known.
fn os_current_rss() -> std::option::Option<u64> {
    if cfg!(target_family = "wasm") {
        Some(0)
    } else if cfg!(any(windows, target_vendor = "apple")) {
        None
    } else if cfg!(any(target_os = "linux", target_os = "android")) {
        Some(linux_vm_rss().unwrap_or(0))
    } else {
        Some(0)
    }
}

/// `VmRSS` from `/proc/self/status`, in bytes.
fn linux_vm_rss() -> std::option::Option<u64> {
    let status = std::fs::read_to_string("/proc/self/status").ok()?;
    let line = status.lines().find_map(|l| l.strip_prefix("VmRSS:"))?;
    let kib: u64 = line.trim().strip_suffix("kB")?.trim().parse().ok()?;
    kib.checked_mul(1024)
}

/// Collects free memory of the calling thread's heap (`mi_collect`).
///
/// This returns the thread's empty pages to their arenas and runs a purge
/// pass over the arenas whose purge delay has expired. With `force`, it also
/// visits full pages, takes in blocks other threads freed into them, returns
/// retired pages at once, and purges every range that is waiting for its
/// purge delay without waiting for it. Purge passes only run while the delay
/// is positive: with [`Option::PurgeDelay`](crate::Option::PurgeDelay) at 0
/// memory is purged as it is freed, and with a negative delay never. Since
/// mimalloc 3.6.0 a freed range of at least
/// [`Option::ArenaPurgeImmediateSize`](crate::Option::ArenaPurgeImmediateSize)
/// (32 MiB by default) is also purged as it is freed, so it never waits for a
/// pass.
///
/// It only collects the calling thread's heap; other threads collect their
/// own heaps as they allocate.
#[inline]
pub fn collect(force: bool) {
    // SAFETY: `mi_collect` takes a plain `bool` and has no preconditions;
    // on a thread whose heap is not initialized it does nothing.
    unsafe { mi_collect(force) }
}
