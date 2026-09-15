/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/
 */

//! Rendering tests for the JS-to-Rust call bodies emitted by `macros.cpp`.

use super::*;

fn render(udl: &str) -> String {
    let mut ci = ComponentInterface::from_webidl(udl, "uniffi_test").expect("parse udl");
    ci.derive_ffi_funcs().expect("derive ffi funcs");
    let module = ModuleMetadata::new(ci.namespace());
    generate_cpp(&ci, &Config::default(), &module).expect("generate cpp")
}

/// Slice the generated source from a function's definition to its closing brace.
fn fn_body<'a>(rendered: &'a str, marker: &str) -> &'a str {
    let start = rendered
        .find(marker)
        .unwrap_or_else(|| panic!("function {marker} not found"));
    let body = &rendered[start..];
    let end = body.find("\n}\n").expect("function end not found");
    &body[..end]
}

const BORROWED_UDL: &str = r#"
namespace uniffi_test {
    void with_bytes([ByRef] bytes data, u32 value);
};
"#;

const TWO_BORROWED_UDL: &str = r#"
namespace uniffi_test {
    void concat_borrowed_bytes([ByRef] bytes first, [ByRef] bytes second);
};
"#;

const OWNED_UDL: &str = r#"
namespace uniffi_test {
    void without_bytes(u32 value, i32 other);
};
"#;

/// Assert that phase 1 (`fromJs` for every argument) precedes phase 2 (`capture`
/// for each borrowed argument), and that phase 2 runs straight into the call.
fn assert_two_phase(body: &str, call_name: &str, captures: usize) {
    let last_fromjs = body.rfind("fromJs(").expect("fromJs statements missing");
    let first_capture = body[last_fromjs..]
        .find(".capture(rt);")
        .map(|i| last_fromjs + i)
        .expect("capture statement missing");
    assert!(
        last_fromjs < first_capture,
        "every fromJs must precede every capture:\n{body}"
    );

    let call = first_capture
        + body[first_capture..]
            .find(call_name)
            .expect("call statement missing");
    let between = &body[first_capture..call];
    assert_eq!(
        between.matches(".capture(rt);").count(),
        captures,
        "unexpected phase-2 statements:\n{body}"
    );
    assert!(
        !between.contains("fromJs") && !between.contains("Bridging"),
        "no conversion may sit between the first capture and the call:\n{body}"
    );
}

#[test]
fn borrowed_bytes_are_captured_after_all_conversions() {
    let rendered = render(BORROWED_UDL);
    let body = fn_body(
        &rendered,
        "jsi::Value NativeUniffiTest::cpp_uniffi_uniffi_test_fn_func_with_bytes(",
    );

    assert_two_phase(body, "uniffi_uniffi_test_fn_func_with_bytes(", 1);

    // The call still passes the captured values in argument order.
    assert!(
        body.contains(
            "uniffi_uniffi_test_fn_func_with_bytes(\n            arg0, \n            arg1, \n            &status\n        );"
        ),
        "call must use the captured variables in order:\n{body}"
    );
}

#[test]
fn two_borrowed_bytes_are_captured_after_all_conversions() {
    let rendered = render(TWO_BORROWED_UDL);
    let body = fn_body(
        &rendered,
        "jsi::Value NativeUniffiTest::cpp_uniffi_uniffi_test_fn_func_concat_borrowed_bytes(",
    );

    assert_two_phase(body, "uniffi_uniffi_test_fn_func_concat_borrowed_bytes(", 2);

    // Both holders are converted for the call, in argument order.
    assert!(
        body.contains(
            "uniffi_uniffi_test_fn_func_concat_borrowed_bytes(\n            arg0, \n            arg1, \n            &status\n        );"
        ),
        "call must use the captured variables in order:\n{body}"
    );
}

#[test]
fn call_without_foreign_bytes_is_unchanged() {
    let rendered = render(OWNED_UDL);
    let body = fn_body(
        &rendered,
        "jsi::Value NativeUniffiTest::cpp_uniffi_uniffi_test_fn_func_without_bytes(",
    );

    // No temporaries: conversions stay interpolated directly into the call.
    assert!(
        !body.contains("auto arg"),
        "unexpected temporaries:\n{body}"
    );
    assert!(
        body.contains(
            "uniffi_uniffi_test_fn_func_without_bytes(uniffi_jsi::Bridging<uint32_t>::fromJs(rt, callInvoker, args[0]), uniffi_jsi::Bridging<int32_t>::fromJs(rt, callInvoker, args[1]), \n            &status\n        );"
        ),
        "inline conversion form changed:\n{body}"
    );
}
