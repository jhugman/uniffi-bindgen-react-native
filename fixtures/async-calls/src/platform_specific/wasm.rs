/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/
 */
use std::time::Duration;

use gloo_timers::future::TimeoutFuture;

use super::{TimerFuture, TimerService};
use crate::{AsyncError, SharedResourceOptions};

impl TimerService for TimerFuture {
    type Future = TimeoutFuture;
    fn sleep(duration: Duration) -> Self::Future {
        let millis = duration.as_millis();
        let millis = millis.try_into().unwrap();
        TimeoutFuture::new(millis)
    }
}

/// This simulates a shared resource, without using a Mutex, which
/// in a single-threaded environment deadlocks.
///
/// The BUSY boolean indicates if the resource is busy or not.
///
/// This function will check if the resource is BUSY. If not, make it
/// busy for the number of millis required.
///
/// If it is already BUSY, then this function will wait for the timeout
/// before checking again. If still busy, then an error is returned.
///
/// The final implementation detail is around cancellation. If the future
/// is dropped while the resource is in use, then BUSY is marked as false.
///
/// This is the crux of the simulation, and enough to test the FFI.
pub(crate) async fn acquire_with_timeout(options: SharedResourceOptions) -> Result<(), AsyncError> {
    use once_cell::sync::Lazy;
    use std::sync::atomic::{AtomicBool, Ordering};

    static BUSY: Lazy<AtomicBool> = Lazy::new(|| AtomicBool::new(false));

    struct StateGuard;
    impl Drop for StateGuard {
        fn drop(&mut self) {
            // Reset the state when guard is dropped
            BUSY.store(false, Ordering::Release);
        }
    }

    let mut attempts = 0;
    while attempts < 2 {
        match BUSY.compare_exchange(
            false, // expected
            true,  // new
            Ordering::Acquire,
            Ordering::Relaxed,
        ) {
            Ok(_) => {
                // Create guard that will reset state on drop
                let _guard = StateGuard;

                // Even if this future is dropped, the guard will clean up
                TimerFuture::sleep(Duration::from_millis(options.release_after_ms.into())).await;
                return Ok(());
            }
            Err(_) => {
                TimerFuture::sleep(Duration::from_millis(options.timeout_ms.into())).await;
                attempts += 1;
            }
        }
    }

    println!("Timeout error in use_shared_resource(). The unit test may hang after this");
    Err(AsyncError::Timeout)
}
