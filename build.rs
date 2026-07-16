use std::env;
use std::fs;
use std::path::{Path, PathBuf};

fn main() {
    let mimalloc_root = Path::new("c_src/mimalloc");
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
    if target_env == "msvc" {
        build.cpp(true);
        build.std("c++17");
        build.flag_if_supported("/Zc:__cplusplus");

        let wrapper = PathBuf::from(env::var("OUT_DIR").expect("OUT_DIR not set")).join("mimalloc-static.cc");
        let include = static_source.to_string_lossy().replace('\\', "/");
        fs::write(&wrapper, format!("#include \"{include}\"\n"))
            .expect("failed to write mimalloc C++ wrapper");
        build.file(wrapper);
    } else {
        build.file(&static_source);
    }

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
        build.define("MI_DEBUG", "3");
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
