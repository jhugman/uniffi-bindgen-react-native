/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/
 */

// A trivial guid, declared as `[Custom]` in the UDL.
pub struct Guid(pub String);
uniffi::custom_newtype!(Guid, String);

// Ditto, using a proc-macro.
pub struct Ouid(pub String);
uniffi::custom_newtype!(Ouid, String);

pub fn get_guid(guid: Option<Guid>) -> Guid {
    // This function doesn't return a Result, so all conversion errors are panics
    match guid {
        Some(guid) => {
            assert!(
                !guid.0.is_empty(),
                "our custom type converter already checked!"
            );
            guid
        }
        None => Guid("NewGuid".to_string()),
    }
}

#[uniffi::export]
pub fn get_ouid(ouid: Option<Ouid>) -> Ouid {
    ouid.unwrap_or_else(|| Ouid("Ouid".to_string()))
}

pub struct ANestedGuid(pub Guid);

uniffi::custom_newtype!(ANestedGuid, Guid);

// Dependent types might come in the "wrong" order - #2067 triggered
// a bug here because of the alphabetical order of these types.
pub struct ANestedOuid(pub Ouid);
uniffi::custom_newtype!(ANestedOuid, Ouid);

#[uniffi::export]
fn get_nested_ouid(nouid: Option<ANestedOuid>) -> ANestedOuid {
    nouid.unwrap_or_else(|| ANestedOuid(Ouid("ANestedOuid".to_string())))
}

uniffi::include_scaffolding!("custom_types");
