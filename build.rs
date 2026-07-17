use std::env;
use std::path::PathBuf;

fn main() {
    let manifest_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR"));
    let mimalloc_root = manifest_dir.join("c_src").join("mimalloc");
    let include_dir = mimalloc_root.join("include");
    let src_dir = mimalloc_root.join("src");
    let static_source = src_dir.join("static.c");

    println!("cargo:rerun-if-changed={}", static_source.display());
    println!("cargo:rerun-if-changed={}", include_dir.display());
    println!("cargo:rerun-if-changed={}", src_dir.display());

    let mut build = cc::Build::new();
    build
        .include(&include_dir)
        .include(&src_dir)
        .define("MI_STATIC_LIB", None);

    let target_os = env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();
    let target_family = env::var("CARGO_CFG_TARGET_FAMILY").unwrap_or_default();
    let target_env = env::var("CARGO_CFG_TARGET_ENV").unwrap_or_default();
    let target_vendor = env::var("CARGO_CFG_TARGET_VENDOR").unwrap_or_default();
    let target_arch = env::var("CARGO_CFG_TARGET_ARCH").unwrap_or_default();

    // Mimalloc expects MSVC/clang-cl builds to use the C++ atomics path.
    // `cpp(true)` alone is not enough: cc does not force C++ for `.c` files, so
    // pass `/TP` (MSVC) / `-xc++` (clang-cl) and compile static.c directly.
    // Avoid a OUT_DIR wrapper + #include: Path::canonicalize() on Windows can
    // produce a `\\?\` prefix that MSVC cannot open.
    if target_env == "msvc" {
        build.cpp(true);
        build.std("c++17");
        build.flag_if_supported("/Zc:__cplusplus");
        let compiler = build.get_compiler();
        if compiler.is_like_msvc() {
            build.flag("/TP");
        } else {
            build.flag_if_supported("-xc++");
        }
    }
    build.file(&static_source);

    if cfg!(feature = "secure") {
        build.define("MI_SECURE", "4");
    }

    // Static malloc override is not supported on Windows; skip it there.
    if cfg!(feature = "override") && target_family != "windows" {
        build.define("MI_MALLOC_OVERRIDE", None);
        if target_vendor == "apple" {
            build.define("MI_OSX_ZONE", "1");
            build.define("MI_OSX_INTERPOSE", "1");
        }
        build.flag_if_supported("-fno-builtin-malloc");
    }

    let profile = env::var("PROFILE").unwrap_or_default();
    if profile == "debug" {
        // Use level 2 (internal checks). Level 3 triggers false assertions in
        // mimalloc v2 secure mode (segment slice accounting).
        build.define("MI_DEBUG", "2");
        build.define("MI_SHOW_ERRORS", "1");
    } else {
        build.define("MI_DEBUG", "0");
    }

    if target_family != "windows" {
        build
            .flag_if_supported("-Wall")
            .flag_if_supported("-Wextra")
            .flag_if_supported("-Wno-unknown-pragmas")
            .flag_if_supported("-Wno-static-in-inline")
            .flag_if_supported("-fvisibility=hidden");
    }

    if target_family == "unix" && target_os != "haiku" {
        build.flag_if_supported("-ftls-model=initial-exec");
    }

    if target_family == "unix" {
        println!("cargo:rustc-link-lib=pthread");

        if target_os == "linux" || target_os == "android" {
            // Prefer DEP_ATOMIC on 32-bit ARM when a toolchain provides a custom name.
            if target_arch == "arm" {
                let atomic_name = env::var("DEP_ATOMIC").unwrap_or_else(|_| "atomic".to_owned());
                println!("cargo:rustc-link-lib={atomic_name}");
            } else {
                println!("cargo:rustc-link-lib=atomic");
            }
        }
    }

    if target_os == "windows" {
        for lib in ["psapi", "shell32", "user32", "advapi32", "bcrypt"] {
            println!("cargo:rustc-link-lib={lib}");
        }
    }

    build.compile("mimalloc");
}
