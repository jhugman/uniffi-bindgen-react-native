/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/
 */
use std::sync::Arc;

// Object with a primary constructor, a plain method, and all five traits.
#[derive(Debug, PartialEq, Eq, Hash, PartialOrd, Ord, uniffi::Object)]
#[uniffi::export(Debug, Display, Eq, Hash, Ord)]
pub struct Widget {
    val: String,
}

#[uniffi::export]
impl Widget {
    #[uniffi::constructor]
    pub fn new(val: String) -> Arc<Self> {
        Arc::new(Self { val })
    }

    pub fn label(&self) -> String {
        self.val.clone()
    }
}

impl std::fmt::Display for Widget {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Widget({})", self.val)
    }
}

// Debug without Display: exercises the synthesized asyncToString delegator.
#[derive(Debug, uniffi::Object)]
#[uniffi::export(Debug)]
pub struct DebugOnly {
    // Read only through the derived `Debug` impl, which the dead-code pass
    // cannot see through.
    #[allow(dead_code)]
    n: i32,
}

#[uniffi::export]
impl DebugOnly {
    #[uniffi::constructor]
    pub fn new(n: i32) -> Arc<Self> {
        Arc::new(Self { n })
    }
}

// Record with all five traits (namespace-function trait surface).
#[derive(Debug, PartialEq, Eq, Hash, PartialOrd, Ord, uniffi::Record)]
#[uniffi::export(Debug, Display, Eq, Hash, Ord)]
pub struct WidgetRecord {
    pub name: String,
    pub value: i32,
}

impl std::fmt::Display for WidgetRecord {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "WidgetRecord({}, {})", self.name, self.value)
    }
}

// Tagged enum with traits + a value method.
#[derive(Debug, PartialEq, Eq, Hash, PartialOrd, Ord, uniffi::Enum)]
#[uniffi::export(Debug, Display, Eq, Hash, Ord)]
pub enum WidgetEnum {
    Alpha,
    Beta { val: String },
}

#[uniffi::export]
impl WidgetEnum {
    pub fn describe(&self) -> String {
        match self {
            WidgetEnum::Alpha => "alpha".to_string(),
            WidgetEnum::Beta { val } => format!("beta:{val}"),
        }
    }
}

impl std::fmt::Display for WidgetEnum {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            WidgetEnum::Alpha => write!(f, "Alpha"),
            WidgetEnum::Beta { val } => write!(f, "Beta({val})"),
        }
    }
}

// Flat enum with traits + a value method + a top-level factory function.
#[derive(Debug, PartialEq, Eq, Hash, PartialOrd, Ord, uniffi::Enum)]
#[uniffi::export(Debug, Display, Eq, Hash, Ord)]
pub enum FlatWidget {
    One,
    Two,
    Three,
}

#[uniffi::export]
impl FlatWidget {
    pub fn ordinal(&self) -> u8 {
        match self {
            FlatWidget::One => 1,
            FlatWidget::Two => 2,
            FlatWidget::Three => 3,
        }
    }
}

impl std::fmt::Display for FlatWidget {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            FlatWidget::One => write!(f, "one"),
            FlatWidget::Two => write!(f, "two"),
            FlatWidget::Three => write!(f, "three"),
        }
    }
}

#[uniffi::export]
pub fn make_flat_widget(v: u8) -> FlatWidget {
    match v {
        1 => FlatWidget::One,
        2 => FlatWidget::Two,
        _ => FlatWidget::Three,
    }
}

uniffi::setup_scaffolding!();
