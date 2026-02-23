use std::env;

fn main() {
    let mut build = cc::Build::new();

    let mimalloc_src = "c_src/mimalloc";

    build
        .file(format!("{mimalloc_src}/src/static.c"))
        .include(format!("{mimalloc_src}/include"))
        .include(format!("{mimalloc_src}/src"))
        .define("MI_STATIC_LIB", None);

    let target_os = env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();
    let target_family = env::var("CARGO_CFG_TARGET_FAMILY").unwrap_or_default();

    if cfg!(feature = "secure") {
        build.define("MI_SECURE", "4");
    }

    if cfg!(feature = "override") {
        build.define("MI_MALLOC_OVERRIDE", None);
    }

    let profile = env::var("PROFILE").unwrap_or_default();
    if profile == "debug" {
        build.define("MI_DEBUG", "3");
    }

    build
        .flag_if_supported("-Wall")
        .flag_if_supported("-Wextra")
        .flag_if_supported("-Wpedantic")
        .flag_if_supported("-Wno-unknown-pragmas")
        .flag_if_supported("-fvisibility=hidden")
        .flag_if_supported("-ftls-model=initial-exec");

    if target_family == "unix" {
        println!("cargo:rustc-link-lib=pthread");

        if target_os == "linux" || target_os == "android" {
            println!("cargo:rustc-link-lib=atomic");
        }
    }

    build.compile("mimalloc");
}
