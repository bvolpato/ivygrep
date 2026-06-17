fn main() {
    let mut build = cxx_build::bridge("rust/lib.rs");

    build
        .file("rust/lib.cpp")
        .flag_if_supported("-Wno-unknown-pragmas")
        .warnings(false)
        .include("include")
        .include("rust")
        .include("fp16/include");

    // Check for optional features
    if cfg!(feature = "openmp") {
        build.define("USEARCH_USE_OPENMP", "1");
    } else {
        build.define("USEARCH_USE_OPENMP", "0");
    }

    if cfg!(feature = "fp16lib") {
        build.define("USEARCH_USE_FP16LIB", "1");
    } else {
        build.define("USEARCH_USE_FP16LIB", "0");
    }

    build.define("USEARCH_USE_SIMSIMD", "0");

    let target_os = std::env::var("CARGO_CFG_TARGET_OS").unwrap();
    // Conditional compilation depending on the target operating system.
    if target_os == "linux" || target_os == "android" {
        build
            .flag_if_supported("-std=c++17")
            .flag_if_supported("-O3")
            .flag_if_supported("-ffast-math")
            .flag_if_supported("-fdiagnostics-color=always")
            .flag_if_supported("-g1"); // Simplify debugging
    } else if target_os == "macos" {
        build
            .flag_if_supported("-mmacosx-version-min=10.15")
            .flag_if_supported("-std=c++17")
            .flag_if_supported("-O3")
            .flag_if_supported("-ffast-math")
            .flag_if_supported("-fcolor-diagnostics")
            .flag_if_supported("-g1"); // Simplify debugging
    } else if target_os == "windows" {
        // Let cc select /MT or /MD from Cargo's crt-static target feature.
        build
            .flag_if_supported("/std:c++17")
            .flag_if_supported("/O2")
            .flag_if_supported("/fp:fast")
            .flag_if_supported("/W1") // Reduce warnings verbosity
            .flag_if_supported("/EHsc")
            .flag_if_supported("/permissive-")
            .flag_if_supported("/sdl-")
            .define("_ALLOW_RUNTIME_LIBRARY_MISMATCH", None)
            .define("_ALLOW_POINTER_TO_CONST_MISMATCH", None);
    }

    build.try_compile("usearch").unwrap();

    println!("cargo:rerun-if-changed=rust/lib.rs");
    println!("cargo:rerun-if-changed=rust/lib.cpp");
    println!("cargo:rerun-if-changed=rust/lib.hpp");
    println!("cargo:rerun-if-changed=include/index_plugins.hpp");
    println!("cargo:rerun-if-changed=include/index_dense.hpp");
    println!("cargo:rerun-if-changed=include/usearch/index.hpp");
}
