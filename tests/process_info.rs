//! `mimalloc::process_info`: the counters are consistent and grow after a
//! large allocation that is touched.

use mimalloc::MiMalloc;

#[global_allocator]
static GLOBAL: MiMalloc = MiMalloc;

const MIB: u64 = 1 << 20;

#[test]
fn process_info() {
    let small = vec![1u8; 1 << 20];
    let before = mimalloc::process_info();
    assert!(before.current_rss > 0, "{before:?}");
    assert!(before.peak_rss >= before.current_rss, "{before:?}");
    assert!(before.current_commit > 0, "{before:?}");
    assert!(before.peak_commit >= before.current_commit, "{before:?}");

    // 256 MiB, every page written.
    let mut big = vec![0u8; 256 << 20];
    for page in big.chunks_mut(4096) {
        page[0] = 1;
    }
    std::hint::black_box(&big);
    let after = mimalloc::process_info();
    assert!(
        after.current_rss >= before.current_rss + 200 * MIB,
        "{before:?}\n{after:?}"
    );
    assert!(after.peak_rss >= after.current_rss, "{after:?}");
    assert!(
        after.current_commit >= before.current_commit + 200 * MIB,
        "{before:?}\n{after:?}"
    );
    assert!(after.peak_commit >= after.current_commit, "{after:?}");
    assert!(
        after.elapsed_ms >= before.elapsed_ms,
        "{before:?}\n{after:?}"
    );
    drop((small, big));
}
