/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/
 */

use std::sync::Arc;

#[uniffi::export(callback_interface)]
pub trait BasicDelegate: Sync + Send {
    fn method(&self, user_id: String) -> String;
}

#[derive(Clone, uniffi::Object)]
pub struct Builder {
    basic_delegate: Option<Arc<dyn BasicDelegate>>,
}

fn unwrap_or_clone_arc<T: Clone>(arc: Arc<T>) -> T {
    Arc::try_unwrap(arc).unwrap_or_else(|x| (*x).clone())
}

#[uniffi::export]
impl Builder {
    pub fn set_basic_delegate(self: Arc<Self>, delegate: Box<dyn BasicDelegate>) -> Arc<Self> {
        let mut builder = unwrap_or_clone_arc(self);
        builder.basic_delegate = Some(delegate.into());
        Arc::new(builder)
    }
}

impl Builder {
    fn set_basic_delegate_simplistic(&mut self, delegate: Box<dyn BasicDelegate>) {
        self.basic_delegate = Some(delegate.into());
    }
}

#[uniffi::export]
pub fn get_builder() -> Arc<Builder> {
    Arc::new(new_builder())
}

fn new_builder() -> Builder {
    Builder {
        basic_delegate: None,
    }
}

#[uniffi::export]
pub fn create_arc_dropping_builder(delegate: Box<dyn BasicDelegate>) {
    let builder = get_builder();
    builder.set_basic_delegate(delegate);
}

// Not crashing
#[uniffi::export]
pub fn create_dropping_builder(delegate: Box<dyn BasicDelegate>) {
    let mut builder = new_builder();
    builder.set_basic_delegate_simplistic(delegate);
    drop(builder);
}
