# mimalloc — Rust bindings

Rust wrapper for Microsoft's [mimalloc](https://github.com/microsoft/mimalloc) — a compact general purpose allocator with excellent performance.

## Requirements

- Rust 1.85+ (edition 2024)
- A C compiler (automatically invoked via the [`cc`](https://crates.io/crates/cc) crate)

## Updating the mimalloc submodule

To checkout a specific release tag of [microsoft/mimalloc](https://github.com/microsoft/mimalloc), use:

```bash
# Clone with submodules
git clone --recurse-submodules https://github.com/amsokol/mimalloc.git

# Or update existing submodule to a specific tag
cd c_src/mimalloc
git fetch origin
git checkout v3.6.0  # Replace with desired tag
cd ../..
git add c_src/mimalloc
git commit -m "Update mimalloc to v3.6.0"
```

Or checkout a specific commit:

```bash
cd c_src/mimalloc
git fetch origin
git checkout 152fbf2634aeafca3774df791b0a77683035f076
cd ../..
git add c_src/mimalloc
git commit -m "Pin mimalloc to specific commit"
```

## Usage

Add the dependency to your `Cargo.toml`:

```toml
[dependencies]
mimalloc = { git = "https://github.com/amsokol/mimalloc", tag = "v3.6.0" }
```

Set `MiMalloc` as the global allocator in your application:

```rust
use mimalloc::MiMalloc;

#[global_allocator]
static GLOBAL: MiMalloc = MiMalloc;

fn main() {
    let v: Vec<u32> = vec![1, 2, 3];
    println!("{v:?}");
}
```

## Features

| Feature    | Description                                                                                                                                           |
| ---------- | ----------------------------------------------------------------------------------------------------------------------------------------------------- |
| `secure`   | Builds mimalloc in secure mode — guard pages, encrypted free lists, randomized allocation                                                             |
| `override` | Overrides the standard C `malloc`/`free` interface so all C library allocations also use mimalloc (Unix/macOS only; ignored on Windows static builds) |
| `no_thp`   | Builds mimalloc with transparent huge pages off by default (`MI_DEFAULT_ALLOW_THP=0`, like mimalloc's CMake option `MI_NO_THP`); Linux/Android only, no effect elsewhere. `MIMALLOC_ALLOW_THP=1` still turns them back on |

Enable features in your `Cargo.toml`:

```toml
[dependencies]
mimalloc = { git = "https://github.com/amsokol/mimalloc", tag = "v3.6.0", features = ["secure"] }
```

## API

### `MiMalloc`

A unit struct implementing [`GlobalAlloc`](https://doc.rust-lang.org/std/alloc/trait.GlobalAlloc.html). Supports `alloc`, `dealloc`, `alloc_zeroed`, and `realloc`, with automatic use of aligned allocation variants when the requested alignment exceeds the platform default.

### `usable_size(ptr: *const u8) -> usize`

Returns the usable size of an allocated memory block. The returned size can be larger than the originally requested size. Returns `0` if `ptr` is null.

### `Option`

mimalloc's runtime options (`mi_option_t`), one variant per option of mimalloc 3.6.0 except the deprecated ones. Refer to it as `mimalloc::Option`, since importing it shadows `std::option::Option`.

- `Option::get(self) -> i64`, `Option::set(self, i64)`: read and set the value (`mi_option_get`, `mi_option_set`).
- `Option::is_enabled(self) -> bool`, `Option::set_enabled(self, bool)`: the same as non-zero / 1 or 0.
- `Option::get_size(self) -> usize`: the value in bytes for the options kept in KiB.
- `Option::name(self) -> &'static str`: the name in `util/options.c`; `MIMALLOC_<NAME>` is its environment variable.
- `Option::ALL`: every option.

### `process_info() -> ProcessInfo`

Process time and memory counters (`mi_process_info`): `elapsed_ms`, `user_ms`, `system_ms`, `current_rss`, `peak_rss`, `current_commit`, `peak_commit`, `page_faults`, all `u64`, in milliseconds and bytes. A value the platform does not provide is 0; the rustdoc of `ProcessInfo` has a table of where each one comes from. On Linux and Android `current_rss` is `VmRSS` from `/proc/self/status` (mimalloc itself reports its commit count there).

### `collect(force: bool)`

Returns free memory of the calling thread's heap to its arenas and runs a purge pass (`mi_collect`). With `force`, it also purges everything waiting for its purge delay.

## Tuning

mimalloc keeps freed memory for a while before returning it to the OS (purging), and on Linux it asks for transparent huge pages (THP). Both make allocation faster, and both can make a long-running process hold much more memory than it uses. One program using this crate allocated and freed about 14 GB in five minutes with a live heap under 70 MB; on Linux with THP set to `madvise`:

| Run | Anonymous RSS peak | RSS peak |
| - | - | - |
| defaults | 1294–1413 MB | 1469–1587 MB |
| `MIMALLOC_PURGE_DELAY=0` | 1096 MB | 1271 MB |
| `MIMALLOC_PURGE_DELAY=0 MIMALLOC_ALLOW_THP=0` | 622–648 MB | 797–821 MB |

With `allow_thp` on, 330 of 848 MB of anonymous memory sat in huge pages, and a huge page goes back to the OS only when its whole 2 MiB is free.

A program can get the same result without environment variables:

```toml
[dependencies]
mimalloc = { git = "https://github.com/amsokol/mimalloc", tag = "v3.6.0", features = ["no_thp"] }
```

```rust
fn main() {
    // First thing in main, before spawning threads.
    mimalloc::Option::PurgeDelay.set(0);
    // ...
}
```

`purge_delay` can be set from `main`, but `allow_thp` needs the `no_thp` feature. mimalloc initializes before `main` and reserves its first arena (1 GiB) then. Some options are read again on every use, others only at that point:

| Option | Default | What it does | Set from `main` |
| - | - | - | - |
| `PurgeDelay` | 1000 | Milliseconds before freed memory is purged; 0 = at once, < 0 = never. Arenas wait `purge_delay × arena_purge_mult` | Applies to everything freed afterwards, in every arena. Ranges already scheduled under a positive delay are not purged any more once it is 0 |
| `ArenaPurgeMult` | 4 | Multiplier on `purge_delay` inside arenas; 0 = at once | Applies to everything freed afterwards |
| `ArenaPurgeImmediateSize` | 32 MiB | Freed ranges at least this large (KiB) are purged at once, whatever `purge_delay` (new in mimalloc 3.6.0; a negative delay still never purges) | Applies to everything freed afterwards |
| `AllowThp` | 2 | THP on Linux/Android: 0 = off, 1 = on, 2 = on and purge whole 2 MiB only (0 on Android) | Partly. Purge granularity changes at once; arenas reserved later are not marked `MADV_HUGEPAGE`; but the first arena stays marked and the process-wide `prctl(PR_SET_THP_DISABLE)` is only done at start. Use the `no_thp` feature or `MIMALLOC_ALLOW_THP=0` |
| `ArenaPurgeMinSize` | 0 | Smallest purge in KiB; 0 = OS page, or 2 MiB with `allow_thp` 2. Not applied when purging is immediate, including ranges over `arena_purge_immediate_size` | Applies to later purge passes |
| `PurgeDecommits` | 1 | Purge by decommitting (1) or by resetting with `MADV_FREE` (0) | Applies to later purges |
| `ArenaReserve` | 1 GiB | Size of a new arena, in KiB | Arenas reserved afterwards |
| `ArenaEagerCommit` | 2 | Commit arenas when reserved: 0 no, 1 yes, 2 on overcommit systems | Arenas reserved afterwards |
| `ArenaMaxObjectSize` | 2 GiB | Larger objects are mapped from the OS directly | Later allocations |
| `PageReclaimOnFree` | 1 (0 before mimalloc 3.6.0) | Reclaim abandoned pages on free: -1 never, 0 only into the heap they came from, 1 into any heap | No: each heap copies it when it is created, and the global allocator's heap exists before `main`. Use `MIMALLOC_PAGE_RECLAIM_ON_FREE=0` for the 3.5 behaviour |
| `PageFullRetain` | 2 | Full small pages a thread keeps per size class | Threads started afterwards |
| `PageCommitOnDemand` | 0 | Commit page memory on demand: 0 no, 1 yes, 2 without overcommit | Later page allocations |
| `ShowStats`, `Verbose`, `ShowErrors` | off | Print statistics at exit, verbose messages, errors | Yes |
| `ReserveOsMemory`, `ReserveHugeOsPages`, `UseNumaNodes`, `MaxVabits`, `PagemapCommit`, `MaxErrors`, `MaxWarnings` | | Start-up reservations and limits | No: read at start only |

The rustdoc of each `Option` variant covers every option, including the reclaim, NUMA, guarded-page (no effect: this crate does not build with `MI_GUARDED`), and exit options. An environment variable `MIMALLOC_<NAME>` still overrides a build-time default, and `Option::set` overrides both.

When measuring, use `process_info().current_rss`: on Unix release builds mimalloc's commit counter does not drop when it purges, because it purges with `MADV_DONTNEED` and keeps the range committed in its books. The test `tests/purge.rs` allocates 400 MiB in pieces, frees it, and checks that the RSS goes back down with `purge_delay` 0; with the defaults it stays at the peak.

## How it works

The build script (`build.rs`) compiles mimalloc's `src/static.c` — a single-file build that includes the entire library — into a static archive using the `cc` crate. The Rust side declares FFI bindings to the core allocation functions and implements `GlobalAlloc`, making mimalloc a drop-in replacement for Rust's default allocator, and wraps the option and process-info functions in safe Rust.

## License

MIT — same as [mimalloc](https://github.com/microsoft/mimalloc/blob/master/LICENSE).
