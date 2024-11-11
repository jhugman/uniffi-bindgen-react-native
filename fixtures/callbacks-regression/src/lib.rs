/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/
 */

use std::{sync::Arc, thread::spawn};

uniffi::setup_scaffolding!();

#[uniffi::export(with_foreign)]
pub trait EventListener: Send + Sync {
    fn on_event(&self, message: String, number: i32);
}

#[derive(uniffi::Object)]
struct EventSource {
    listener: Arc<dyn EventListener>,
}

#[uniffi::export]
impl EventSource {
    #[uniffi::constructor]
    fn new(listener: Arc<dyn EventListener>) -> Arc<Self> {
        Arc::new(Self { listener })
    }
    fn start(self: Arc<Self>, message: String, until: i32) {
        let listener = self.listener.clone();
        spawn(move || {
            for i in 0..until {
                listener.on_event(message.clone(), i);
            }
        });
    }
}
