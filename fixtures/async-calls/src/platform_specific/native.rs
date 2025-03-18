/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/
 */
use std::time::Duration;

use crate::{AsyncError, SharedResourceOptions};

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
