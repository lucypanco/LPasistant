//! 生成 UniFFI 绑定的命令行工具:
//!   cargo run --bin uniffi-bindgen -- generate --library <libassistant_core.so> \
//!       --language kotlin --out-dir <android app src dir>
fn main() {
    uniffi::uniffi_bindgen_main()
}
