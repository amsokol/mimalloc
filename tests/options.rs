//! `mimalloc::Option`: names and discriminants match mimalloc, and `set`
//! then `get` returns the value. Options are process-wide, so this file holds
//! a single test.

use mimalloc::MiMalloc;
use std::ffi::{CStr, c_char, c_void};

#[global_allocator]
static GLOBAL: MiMalloc = MiMalloc;

type OutputFun = unsafe extern "C" fn(msg: *const c_char, arg: *mut c_void);

unsafe extern "C" {
    fn mi_options_print_out(out: OutputFun, arg: *mut c_void);
}

unsafe extern "C" fn append(msg: *const c_char, arg: *mut c_void) {
    // SAFETY: mimalloc passes a NUL-terminated message, and `arg` is the
    // `&mut String` given to `mi_options_print_out` below.
    unsafe {
        let out = &mut *(arg as *mut String);
        out.push_str(&CStr::from_ptr(msg).to_string_lossy());
    }
}

/// `(name, value)` of each `option '<name>': <value>` line mimalloc prints,
/// in `mi_option_t` order.
fn printed_options() -> Vec<(String, i64)> {
    let mut out = String::new();
    // SAFETY: `append` matches `mi_output_fun`, and `out` outlives the call.
    unsafe { mi_options_print_out(append, &mut out as *mut String as *mut c_void) };
    out.lines()
        .filter_map(|l| l.strip_prefix("option '"))
        .map(|l| {
            let (name, rest) = l.split_once("': ").unwrap();
            let value = rest.split_whitespace().next().unwrap().parse().unwrap();
            (name.to_owned(), value)
        })
        .collect()
}

#[test]
fn options() {
    // Names and discriminants: mimalloc prints every option by index, so
    // `Option::ALL[i]` must sit at its index among the non-deprecated ones,
    // and print the same value `get` returns.
    let printed = printed_options();
    assert_eq!(printed.len(), 54, "mimalloc 3.6.0 has 54 options");
    let live: Vec<(usize, &(String, i64))> = printed
        .iter()
        .enumerate()
        .filter(|(_, (name, _))| !name.starts_with("deprecated_"))
        .collect();
    assert_eq!(live.len(), mimalloc::Option::ALL.len());
    for (option, (index, (name, value))) in mimalloc::Option::ALL.iter().zip(live) {
        assert_eq!(option.name(), name);
        assert_eq!(*option as usize, index, "{name}");
        assert_eq!(option.get(), *value, "{name}");
    }

    // `set` then `get`, restoring each option right away; nothing in
    // between allocates, so options such as `disallow_os_alloc` are safe to
    // flip.
    for option in mimalloc::Option::ALL {
        let original = option.get();
        let value = if original == 7 { 8 } else { 7 };
        option.set(value);
        let got = option.get();
        option.set(original);
        assert_eq!(got, value, "{}", option.name());
        assert_eq!(option.get(), original, "{}", option.name());
    }

    // `set_enabled` and `is_enabled`.
    for option in mimalloc::Option::ALL {
        let original = option.get();
        option.set_enabled(true);
        let (on, on_value) = (option.is_enabled(), option.get());
        option.set_enabled(false);
        let (off, off_value) = (option.is_enabled(), option.get());
        option.set(original);
        assert!(on && on_value == 1, "{}", option.name());
        assert!(!off && off_value == 0, "{}", option.name());
    }

    // `get_size` is in bytes for the KiB options.
    let reserve = mimalloc::Option::ArenaReserve;
    assert_eq!(reserve.get_size() as i64, reserve.get() * 1024);
    let delay = mimalloc::Option::PurgeDelay;
    assert_eq!(delay.get_size() as i64, delay.get());
}
