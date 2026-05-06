/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/
 */

#[derive(uniffi::Enum, Debug, PartialEq, Eq, Clone)]
pub enum Color {
    Red,
    Green,
    Blue,
}

// Function-arg defaults across the literal value forms.
#[uniffi::export(default(value = 42))]
pub fn echo_i32(value: i32) -> i32 {
    value
}

#[uniffi::export(default(value = "hello"))]
pub fn echo_string(value: String) -> String {
    value
}

#[uniffi::export(default(value = true))]
pub fn echo_bool(value: bool) -> bool {
    value
}

#[uniffi::export(default(value = None))]
pub fn echo_option_none(value: Option<i32>) -> Option<i32> {
    value
}

#[uniffi::export(default(value = Some(7)))]
pub fn echo_option_some(value: Option<i32>) -> Option<i32> {
    value
}

// Enum-variant literals as a default value (e.g. `default = Color::Green`)
// are not accepted by uniffi-rs' DefaultValue parser, so this function is
// exported without a default; the test confirms round-trip codegen instead.
#[uniffi::export]
pub fn echo_color(value: Color) -> Color {
    value
}

// Constructor and method arg defaults.
#[derive(uniffi::Object)]
pub struct Greeter {
    prefix: String,
}

#[uniffi::export]
impl Greeter {
    #[uniffi::constructor(default(prefix = "hi", _version = 1))]
    pub fn new(prefix: String, _version: i32) -> std::sync::Arc<Self> {
        std::sync::Arc::new(Self { prefix })
    }

    #[uniffi::method(default(name = "world"))]
    pub fn greet(&self, name: String) -> String {
        format!("{} {}", self.prefix, name)
    }
}

// Record field defaults.
#[derive(uniffi::Record, Debug, PartialEq, Eq, Clone)]
pub struct Settings {
    #[uniffi(default = 10)]
    pub retries: i32,
    #[uniffi(default = None)]
    pub label: Option<String>,
    pub required: String,
}

#[uniffi::export]
pub fn echo_settings(value: Settings) -> Settings {
    value
}

// Defaults on enum-variant fields, both named-variant and tuple-variant.
#[derive(uniffi::Enum, Debug, PartialEq, Eq, Clone)]
pub enum TestCase {
    One {
        #[uniffi(default = 100)]
        num_value: i32,
    },
    Two {
        #[uniffi(default = None)]
        maybe_label: Option<String>,
        required: String,
    },
    Three(#[uniffi(default = 5)] i32),
}

#[uniffi::export]
pub fn echo_test_case(value: TestCase) -> TestCase {
    value
}

// Defaulted arg after a non-defaulted arg.
#[uniffi::export(default(suffix = "!"))]
pub fn join(prefix: String, suffix: String) -> String {
    format!("{prefix}{suffix}")
}

// Trait method with a default arg, called from foreign code.
#[uniffi::export(with_foreign)]
pub trait Formatter: Send + Sync {
    #[uniffi::method(default(width = 2))]
    fn format(&self, value: i32, width: i32) -> String;
}

#[uniffi::export]
pub fn use_formatter(f: std::sync::Arc<dyn Formatter>) -> String {
    f.format(7, 4)
}

// Bare-keyword `default` form: uniffi-rs maps `#[uniffi(default)]` on a field
// and `default(arg)` (no `= value`) on a function arg to the type's default
// value (i32 → 0, String → "", Option → None, etc).
#[derive(uniffi::Record, Debug, PartialEq, Eq, Clone)]
pub struct BareDefaults {
    #[uniffi(default)]
    pub n: i32,
    pub required: String,
}

#[uniffi::export]
pub fn echo_bare_defaults(value: BareDefaults) -> BareDefaults {
    value
}

#[uniffi::export(default(value))]
pub fn echo_bare_arg(value: i32) -> i32 {
    value
}

// Bare default on a Record-typed arg: uniffi-rs requires every field of the
// record to carry a default; codegen renders `AllDefaults.create({})`.
#[derive(uniffi::Record, Debug, PartialEq, Eq, Clone)]
pub struct AllDefaults {
    #[uniffi(default = 1)]
    pub a: i32,
    #[uniffi(default = "x")]
    pub b: String,
}

#[uniffi::export(default(value))]
pub fn echo_bare_record_arg(value: AllDefaults) -> AllDefaults {
    value
}

// Bare default on an Object-typed arg: uniffi-rs requires a 0-arg primary
// constructor; codegen renders `new ZeroArgObj()`.
#[derive(uniffi::Object)]
pub struct ZeroArgObj {}

#[uniffi::export]
impl ZeroArgObj {
    #[uniffi::constructor]
    pub fn new() -> std::sync::Arc<Self> {
        std::sync::Arc::new(Self {})
    }

    #[uniffi::method]
    pub fn label(&self) -> String {
        "z".into()
    }
}

#[uniffi::export(default(value))]
pub fn echo_bare_obj_arg(value: std::sync::Arc<ZeroArgObj>) -> std::sync::Arc<ZeroArgObj> {
    value
}

uniffi::setup_scaffolding!();
