# mimalloc — Rust bindings

Rust wrapper for Microsoft's [mimalloc](https://github.com/microsoft/mimalloc) — a compact general purpose allocator with excellent performance.

## Requirements

- Rust 1.85+ (edition 2024)
- A C compiler (automatically invoked via the [`cc`](https://crates.io/crates/cc) crate)

## Usage

Add the dependency to your `Cargo.toml`:

```toml
[dependencies]
mimalloc = { git = "https://github.com/amsokol/mimalloc", tag = "v2.4.1" }
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

| Feature    | Description                                                                                       |
| ---------- | ------------------------------------------------------------------------------------------------- |
| `secure`   | Builds mimalloc in secure mode — guard pages, encrypted free lists, randomized allocation         |
| `override` | Overrides the standard C `malloc`/`free` interface so all C library allocations also use mimalloc (Unix/macOS only; ignored on Windows static builds) |

Enable features in your `Cargo.toml`:

```toml
[dependencies]
mimalloc = { git = "https://github.com/amsokol/mimalloc", tag = "v2.4.1", features = ["secure"] }
```

## API

### `MiMalloc`

A unit struct implementing [`GlobalAlloc`](https://doc.rust-lang.org/std/alloc/trait.GlobalAlloc.html). Supports `alloc`, `dealloc`, `alloc_zeroed`, and `realloc`, with automatic use of aligned allocation variants when the requested alignment exceeds the platform default.

### `usable_size(ptr: *const u8) -> usize`

Returns the usable size of an allocated memory block. The returned size can be larger than the originally requested size. Returns `0` if `ptr` is null.

## How it works

The build script (`build.rs`) compiles mimalloc's `src/static.c` — a single-file build that includes the entire library — into a static archive using the `cc` crate. The Rust side declares FFI bindings to the core allocation functions and implements `GlobalAlloc`, making mimalloc a drop-in replacement for Rust's default allocator.

## License

MIT — same as [mimalloc](https://github.com/microsoft/mimalloc/blob/master/LICENSE).
