// Forwards to UniFFI's bindgen-main so `cargo run --bin uniffi-bindgen
// --features cli` works without a separate top-level install. Matches
// the pattern UniFFI recommends for in-crate bindgen tools.
fn main() {
    uniffi::uniffi_bindgen_main()
}
