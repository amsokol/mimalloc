//! With `purge_delay` 0 and `allow_thp` off, set at the start of the test,
//! memory freed in pieces goes back to the OS at once: the resident set grows
//! by what is allocated and shrinks again when it is freed.

use mimalloc::MiMalloc;

#[global_allocator]
static GLOBAL: MiMalloc = MiMalloc;

const MIB: u64 = 1 << 20;

#[test]
fn freed_memory_goes_back_to_the_os() {
    mimalloc::Option::PurgeDelay.set(0);
    mimalloc::Option::AllowThp.set_enabled(false);

    let start = mimalloc::process_info();

    // 400 MiB in pieces of 4 KiB to 1 MiB, every page written.
    let total = 400 * MIB;
    let mut pieces: Vec<Vec<u8>> = Vec::new();
    let (mut allocated, mut seed) = (0u64, 1u64);
    while allocated < total {
        seed = seed
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        let size = 4096 + (seed >> 33) as usize % (1 << 20);
        pieces.push(vec![1u8; size]);
        allocated += size as u64;
    }
    let full = mimalloc::process_info();

    drop(pieces);
    let freed = mimalloc::process_info();
    eprintln!("start {start:?}\nfull  {full:?}\nfreed {freed:?}");

    assert!(
        full.current_rss >= start.current_rss + total * 3 / 4,
        "{start:?}\n{full:?}"
    );
    assert!(
        freed.current_rss <= start.current_rss + total / 4,
        "less than 3/4 of {} MiB went back to the OS:\nstart {start:?}\nfull  {full:?}\nfreed {freed:?}",
        total / MIB
    );
}
