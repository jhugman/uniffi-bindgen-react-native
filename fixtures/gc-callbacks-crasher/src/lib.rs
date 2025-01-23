/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/
 */

use std::sync::Arc;

#[uniffi::export(callback_interface)]
#[async_trait::async_trait]
pub trait AsyncDelegate: Sync + Send {
    async fn method(&self, user_id: String) -> String;
}

#[uniffi::export(callback_interface)]
pub trait BasicDelegate: Sync + Send {
    fn method(&self, user_id: String) -> String;
}

#[derive(Clone, uniffi::Object)]
pub struct Builder {
    async_delegate: Option<Arc<dyn AsyncDelegate>>,
    basic_delegate: Option<Arc<dyn BasicDelegate>>,
}

pub(crate) fn unwrap_or_clone_arc<T: Clone>(arc: Arc<T>) -> T {
    Arc::try_unwrap(arc).unwrap_or_else(|x| (*x).clone())
}

#[uniffi::export]
impl Builder {
    pub fn echo(self: Arc<Self>) -> String {
        "echo".to_string()
    }
    pub fn set_async_delegate(self: Arc<Self>, delegate: Box<dyn AsyncDelegate>) -> Arc<Self> {
        let mut builder = unwrap_or_clone_arc(self);
        builder.async_delegate = Some(delegate.into());
        Arc::new(builder)
    }
    pub fn set_basic_delegate(self: Arc<Self>, delegate: Box<dyn BasicDelegate>) -> Arc<Self> {
        let mut builder = unwrap_or_clone_arc(self);
        builder.basic_delegate = Some(delegate.into());
        Arc::new(builder)
    }
}

impl Builder {
    pub fn set_basic_delegate_simplistic(&mut self, delegate: Box<dyn BasicDelegate>) {
        self.basic_delegate = Some(delegate.into());
    }
}

#[uniffi::export]
pub fn get_builder() -> Arc<Builder> {
    Arc::new(new_builder())
}

pub fn new_builder() -> Builder {
    Builder {
        async_delegate: None,
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

uniffi::setup_scaffolding!();
