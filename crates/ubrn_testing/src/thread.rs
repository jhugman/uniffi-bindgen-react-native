/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/
 */

mod platform {
    #[cfg(target_arch = "wasm32")]
    mod wasm {
        use wasm_bindgen_futures::spawn_local;

        pub fn spawn_task<F>(f: F)
        where
            F: FnOnce() -> (),
            F: Send + 'static,
        {
            spawn_local(async move {
                f();
            });
        }
    }

    #[cfg(not(target_arch = "wasm32"))]
    mod native {
        use std::thread;

        pub type JoinHandle<T> = thread::JoinHandle<T>;

        // pub fn spawn_task<F>(f: impl FnOnce() + Send + 'static) -> JoinHandle<()> {
        pub fn spawn_task<F, T>(f: F) -> JoinHandle<T>
        where
            F: FnOnce() -> T,
            F: Send + 'static,
            T: Send + 'static,
        {
            thread::spawn(f)
        }
    }

    #[cfg(not(target_arch = "wasm32"))]
    pub use native::*;
    #[cfg(target_arch = "wasm32")]
    pub use wasm::*;
}

pub use self::platform::*;
