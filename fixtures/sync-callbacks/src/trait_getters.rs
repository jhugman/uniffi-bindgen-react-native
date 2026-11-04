/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/
 */

use std::sync::Arc;

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum GettersError {
    #[error("TooManyHoles")]
    TooManyHoles,
}

#[derive(Clone, Debug, thiserror::Error, PartialEq, Eq)]
pub enum GettersComplexError {
    #[error("OsError: {code} ({extended_code})")]
    OsError { code: i16, extended_code: i16 },
    #[error("PermissionDenied: {reason}")]
    PermissionDenied { reason: String },
    #[error("Unknown error")]
    UnknownError,
}

pub struct Payload {
    name: String,
}

impl Payload {
    pub fn new(name: String) -> Self {
        Self { name }
    }

    pub fn get_name(&self) -> String {
        self.name.clone()
    }
}

/// Test trait
///
/// The goal here is to test all possible arg, return, and error types.
#[uniffi::trait_interface]
pub trait Getters: Send + Sync {
    fn get_bool(&self, v: bool, arg2: bool) -> bool;
    fn get_string(&self, v: String, arg2: bool) -> Result<String, GettersError>;
    fn get_option(&self, v: String, arg2: bool) -> Result<Option<String>, GettersComplexError>;
    fn get_list(&self, v: Vec<i32>, arg2: bool) -> Vec<i32>;
    fn get_nothing(&self, v: String);
    fn round_trip_object(&self, payload: Arc<Payload>) -> Arc<Payload>;
}

pub fn test_round_trip_through_rust(getters: Arc<dyn Getters>) -> Arc<dyn Getters> {
    getters
}

pub fn test_round_trip_through_foreign(getters: Arc<dyn Getters>) {
    let payload = getters.round_trip_object(Arc::new(Payload::new("round-trip".to_owned())));
    assert_eq!(payload.get_name(), "round-trip");
}

struct RustTraitGetters;

impl Getters for RustTraitGetters {
    fn get_bool(&self, v: bool, arg2: bool) -> bool {
        v ^ arg2
    }

    fn get_string(&self, v: String, arg2: bool) -> Result<String, GettersError> {
        if v == "too-many-holes" {
            Err(GettersError::TooManyHoles)
        } else if v == "unexpected-error" {
            panic!("unexpected error")
        } else if arg2 {
            Ok(v.to_uppercase())
        } else {
            Ok(v)
        }
    }

    fn get_option(&self, v: String, arg2: bool) -> Result<Option<String>, GettersComplexError> {
        if v == "os-error" {
            Err(GettersComplexError::OsError {
                code: 100,
                extended_code: 200,
            })
        } else if v == "unknown-error" {
            Err(GettersComplexError::UnknownError)
        } else if arg2 {
            if !v.is_empty() {
                Ok(Some(v.to_uppercase()))
            } else {
                Ok(None)
            }
        } else {
            Ok(Some(v))
        }
    }

    fn get_list(&self, v: Vec<i32>, arg2: bool) -> Vec<i32> {
        if arg2 {
            v
        } else {
            vec![]
        }
    }

    fn get_nothing(&self, _v: String) {}

    fn round_trip_object(&self, payload: Arc<Payload>) -> Arc<Payload> {
        payload
    }
}

pub fn make_rust_getters() -> Arc<dyn Getters> {
    Arc::new(RustTraitGetters)
}

pub fn test_getters(getters: Arc<dyn Getters>) {
    assert!(!getters.get_bool(true, true));
    assert!(getters.get_bool(true, false));
    assert!(getters.get_bool(false, true));
    assert!(!getters.get_bool(false, false));

    assert_eq!(
        getters.get_string("hello".to_owned(), false).unwrap(),
        "hello"
    );
    assert_eq!(
        getters.get_string("hello".to_owned(), true).unwrap(),
        "HELLO"
    );

    assert_eq!(
        getters.get_option("hello".to_owned(), true).unwrap(),
        Some("HELLO".to_owned())
    );
    assert_eq!(
        getters.get_option("hello".to_owned(), false).unwrap(),
        Some("hello".to_owned())
    );
    assert_eq!(getters.get_option("".to_owned(), true).unwrap(), None);

    assert_eq!(getters.get_list(vec![1, 2, 3], true), vec![1, 2, 3]);
    assert_eq!(getters.get_list(vec![1, 2, 3], false), Vec::<i32>::new());

    // Call get_nothing to make sure it doesn't panic.  There's no point in checking the output
    // though
    getters.get_nothing("hello".to_owned());

    assert_eq!(
        getters.get_string("too-many-holes".to_owned(), true),
        Err(GettersError::TooManyHoles)
    );
    assert_eq!(
        getters.get_option("os-error".to_owned(), true),
        Err(GettersComplexError::OsError {
            code: 100,
            extended_code: 200
        })
    );
    assert_eq!(
        getters.get_option("unknown-error".to_owned(), true),
        Err(GettersComplexError::UnknownError)
    );
    // catch_unwind doesn't work in WASM (panic = "abort"); skip this assertion there.
    #[cfg(not(target_arch = "wasm32"))]
    {
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            getters.get_string("unexpected-error".to_owned(), true)
        }));
        assert!(result.is_err());
    }
}
