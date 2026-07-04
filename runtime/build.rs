#[cfg(all(feature = "std", feature = "metadata-hash"))]
fn main() {
    substrate_wasm_builder::WasmBuilder::init_with_defaults()
        .append_to_rust_flags("-C")
        .append_to_rust_flags("link-arg=--allow-undefined")
        .append_to_rust_flags("-C")
        .append_to_rust_flags("link-arg=--import-memory")
        .enable_metadata_hash("UNIT", 12)
        .build();
}

#[cfg(all(feature = "std", not(feature = "metadata-hash")))]
fn main() {
    substrate_wasm_builder::WasmBuilder::init_with_defaults()
        .append_to_rust_flags("-C")
        .append_to_rust_flags("link-arg=--allow-undefined")
        .append_to_rust_flags("-C")
        .append_to_rust_flags("link-arg=--import-memory")
        .build();
}

/// The wasm builder is deactivated when compiling
/// this crate for wasm to speed up the compilation.
#[cfg(not(feature = "std"))]
fn main() {}
