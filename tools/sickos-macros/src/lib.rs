//! Kernel-side test attribute. The real `#[test]` is a hook into libtest,
//! which needs std and can never run in the kernel; our in-QEMU harness uses
//! `custom_test_frameworks` and its `#[test_case]` attribute instead.
//!
//! This shim lets tests keep the familiar spelling: `use sickos_macros::test;`
//! shadows the built-in attribute (imports outrank built-ins, same mechanism
//! as `#[tokio::test]`), and expands to `#[test_case]` on the same function.

use proc_macro::TokenStream;
use std::str::FromStr;

#[proc_macro_attribute]
pub fn test(attr: TokenStream, item: TokenStream) -> TokenStream {
    if !attr.is_empty() {
        return TokenStream::from_str(
            r##"compile_error!("#[test] takes no arguments in the kernel");"##,
        )
        .unwrap();
    }
    let mut out = TokenStream::from_str("#[test_case]").unwrap();
    out.extend(item);
    out
}
