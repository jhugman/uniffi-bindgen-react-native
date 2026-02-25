/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/
 */

use std::sync::Arc;

#[derive(Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct TraitMethods {
    val: String,
}

impl TraitMethods {
    fn new(val: String) -> Self {
        Self { val }
    }
}

impl std::fmt::Display for TraitMethods {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "TraitMethods({})", self.val)
    }
}

#[derive(Debug, PartialEq, Eq, Hash, PartialOrd, Ord, uniffi::Object)]
#[uniffi::export(Debug, Display, Eq, Hash, Ord)]
pub struct ProcTraitMethods {
    val: String,
}

#[uniffi::export]
impl ProcTraitMethods {
    #[uniffi::constructor]
    fn new(val: String) -> Arc<Self> {
        Arc::new(Self { val })
    }
}

impl std::fmt::Display for ProcTraitMethods {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "ProcTraitMethods({})", self.val)
    }
}

// Tagged enum with traits
#[derive(Debug, PartialEq, Eq, Hash, PartialOrd, Ord, uniffi::Enum)]
#[uniffi::export(Debug, Display, Eq, Hash, Ord)]
pub enum TraitEnum {
    Alpha,
    Beta { val: String },
}

impl std::fmt::Display for TraitEnum {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TraitEnum::Alpha => write!(f, "Alpha"),
            TraitEnum::Beta { val } => write!(f, "Beta({})", val),
        }
    }
}

#[derive(Debug, PartialEq, Eq, Hash, PartialOrd, Ord, uniffi::Record)]
#[uniffi::export(Debug, Display, Eq, Hash, Ord)]
pub struct TraitRecord {
    pub name: String,
    pub value: i32,
}

impl std::fmt::Display for TraitRecord {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "TraitRecord({}, {})", self.name, self.value)
    }
}

// Flat enum (all variants have no data) with uniffi traits exported.
// Before the fix, the flat template would silently drop the trait methods.
#[derive(Debug, PartialEq, Eq, Hash, PartialOrd, Ord, uniffi::Enum)]
#[uniffi::export(Debug, Display, Eq, Hash, Ord)]
pub enum FlatTraitEnum {
    Alpha,
    Beta,
    Gamma,
}

impl std::fmt::Display for FlatTraitEnum {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            FlatTraitEnum::Alpha => write!(f, "alpha"),
            FlatTraitEnum::Beta => write!(f, "beta"),
            FlatTraitEnum::Gamma => write!(f, "gamma"),
        }
    }
}

#[uniffi::export]
pub fn make_flat_trait_enum(v: u8) -> FlatTraitEnum {
    match v {
        1 => FlatTraitEnum::Alpha,
        2 => FlatTraitEnum::Beta,
        _ => FlatTraitEnum::Gamma,
    }
}

uniffi::include_scaffolding!("trait_methods");
