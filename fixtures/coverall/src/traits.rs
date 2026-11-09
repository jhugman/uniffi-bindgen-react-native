/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/
 */

use std::sync::{Arc, Mutex};

// namespace functions.
pub fn get_traits() -> Vec<Arc<dyn NodeTrait>> {
    vec![Arc::new(Trait1::default()), Arc::new(Trait2::default())]
}

#[uniffi::trait_interface]
pub trait NodeTrait: Send + Sync + std::fmt::Debug {
    fn name(&self) -> String;

    fn set_parent(&self, parent: Option<Arc<dyn NodeTrait>>);

    fn get_parent(&self) -> Option<Arc<dyn NodeTrait>>;

    fn strong_count(self: Arc<Self>) -> u64 {
        Arc::strong_count(&self) as u64
    }
}

pub fn ancestor_names(node: Arc<dyn NodeTrait>) -> Vec<String> {
    let mut names = vec![];
    let mut parent = node.get_parent();
    while let Some(node) = parent {
        names.push(node.name());
        parent = node.get_parent();
    }
    names
}

#[derive(Debug, Default)]
pub(crate) struct Trait1 {
    // A reference to another trait.
    parent: Mutex<Option<Arc<dyn NodeTrait>>>,
}

impl NodeTrait for Trait1 {
    fn name(&self) -> String {
        "node-1".to_string()
    }

    fn set_parent(&self, parent: Option<Arc<dyn NodeTrait>>) {
        *self.parent.lock().unwrap() = parent.map(|arc| Arc::clone(&arc))
    }

    fn get_parent(&self) -> Option<Arc<dyn NodeTrait>> {
        (*self.parent.lock().unwrap()).clone()
    }
}

#[derive(Debug, Default)]
pub(crate) struct Trait2 {
    parent: Mutex<Option<Arc<dyn NodeTrait>>>,
}
impl NodeTrait for Trait2 {
    fn name(&self) -> String {
        "node-2".to_string()
    }

    fn set_parent(&self, parent: Option<Arc<dyn NodeTrait>>) {
        *self.parent.lock().unwrap() = parent.map(|arc| Arc::clone(&arc))
    }

    fn get_parent(&self) -> Option<Arc<dyn NodeTrait>> {
        (*self.parent.lock().unwrap()).clone()
    }
}

#[uniffi::trait_interface]
pub trait StringUtil: Send + Sync {
    fn concat(&self, a: &str, b: &str) -> String;
}

pub struct StringUtilImpl1;
pub struct StringUtilImpl2;

pub fn get_string_util_traits() -> Vec<Arc<dyn StringUtil>> {
    vec![Arc::new(StringUtilImpl1), Arc::new(StringUtilImpl2)]
}

impl StringUtil for StringUtilImpl1 {
    fn concat(&self, a: &str, b: &str) -> String {
        format!("{a}{b}")
    }
}

impl StringUtil for StringUtilImpl2 {
    fn concat(&self, a: &str, b: &str) -> String {
        format!("{a}{b}")
    }
}
