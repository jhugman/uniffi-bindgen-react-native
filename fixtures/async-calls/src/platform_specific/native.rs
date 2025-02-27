/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/
 */
use std::{
    future::Future,
    pin::Pin,
    sync::{Arc, Mutex, MutexGuard},
    task::{Context, Poll, Waker},
    thread,
    time::Duration,
};

use super::{TimerFuture, TimerService};
use crate::{AsyncError, SharedResourceOptions};
/// Non-blocking timer future.
pub struct TokioTimerFuture {
    shared_state: Arc<Mutex<SharedState>>,
}

struct SharedState {
    completed: bool,
    waker: Option<Waker>,
}

impl Future for TokioTimerFuture {
    type Output = ();

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let mut shared_state = self.shared_state.lock().unwrap();

        if shared_state.completed {
            Poll::Ready(())
        } else {
            shared_state.waker = Some(cx.waker().clone());
            Poll::Pending
        }
    }
}

impl TimerService for TimerFuture {
    type Future = TokioTimerFuture;
    fn sleep(duration: Duration) -> Self::Future {
        let shared_state = Arc::new(Mutex::new(SharedState {
            completed: false,
            waker: None,
        }));

        let thread_shared_state = shared_state.clone();

        // Let's mimic an event coming from somewhere else, like the system.
        thread::spawn(move || {
            thread::sleep(duration);

            let mut shared_state: MutexGuard<_> = thread_shared_state.lock().unwrap();
            shared_state.completed = true;

            if let Some(waker) = shared_state.waker.take() {
                waker.wake();
            }
        });

        Self::Future { shared_state }
    }
}

pub(crate) async fn acquire_with_timeout(options: SharedResourceOptions) -> Result<(), AsyncError> {
    use once_cell::sync::Lazy;
    use tokio::{
        sync::Mutex,
        time::{sleep, timeout},
    };

    static MUTEX: Lazy<Mutex<()>> = Lazy::new(|| Mutex::new(()));

    let _guard = timeout(
        Duration::from_millis(options.timeout_ms.into()),
        MUTEX.lock(),
    )
    .await
    .map_err(|_| {
        println!("Timeout error in use_shared_resource().  The unit test may hang after this");
        AsyncError::Timeout
    })?;

    sleep(Duration::from_millis(options.release_after_ms.into())).await;
    Ok(())
}
