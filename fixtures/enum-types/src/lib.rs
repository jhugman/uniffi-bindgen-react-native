/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/
 */

pub enum Animal {
    Dog,
    Cat,
}

// Though it has the proc-macro, we drop the variant
// literals if there is not a repr type defined
#[derive(uniffi::Enum)]
pub enum AnimalNoReprInt {
    Dog = 3,
    Cat = 4,
}

#[repr(u8)]
#[derive(uniffi::Enum)]
pub enum AnimalUInt {
    Dog = 3,
    Cat = 4,
}

#[repr(u64)]
#[derive(uniffi::Enum)]
pub enum AnimalLargeUInt {
    Dog = 4294967298, // u32::MAX as u64 + 3
    Cat = 4294967299, // u32::MAX as u64 + 4
}

#[repr(i8)]
#[derive(Debug, uniffi::Enum)]
pub enum AnimalSignedInt {
    Dog = -3,
    Cat = -2,
    Koala,   // -1
    Wallaby, // 0
    Wombat,  // 1
}

#[derive(uniffi::Record, Clone)]
pub struct AnimalRecord {
    value: u8,
}

#[derive(uniffi::Object)]
pub struct AnimalObject {
    pub value: AnimalRecord,
}

#[uniffi::export]
impl AnimalObject {
    #[uniffi::constructor]
    fn new(value: u8) -> Arc<Self> {
        Arc::new(Self {
            value: AnimalRecord { value },
        })
    }

    pub fn record(&self) -> AnimalRecord {
        self.value.clone()
    }
}

use std::sync::Arc;
// Adding an enum with a Associated Type that is a exported Arc<Object> with a exported Record field.
// This is done to check for compilation errors.
#[derive(uniffi::Enum)]
pub(crate) enum AnimalAssociatedType {
    Dog(Arc<AnimalObject>),
    Cat,
}

#[uniffi::export]
fn identity_enum_with_associated_type(value: AnimalAssociatedType) -> AnimalAssociatedType {
    value
}

#[uniffi::export]
fn identity_enum_with_named_associated_type(
    value: AnimalNamedAssociatedType,
) -> AnimalNamedAssociatedType {
    value
}

#[derive(uniffi::Enum)]
pub(crate) enum AnimalNamedAssociatedType {
    Dog { value: Arc<AnimalObject> },
    Cat,
}

#[uniffi::export]
fn get_animal(a: Option<Animal>) -> Animal {
    a.unwrap_or(Animal::Dog)
}

#[derive(uniffi::Enum)]
pub(crate) enum CollidingVariants {
    AnimalRecord(AnimalRecord),
    AnimalObjectInterface(Arc<AnimalObject>),
    AnimalObject(Arc<AnimalObject>),
    Animal(Animal),
    #[allow(clippy::enum_variant_names)]
    CollidingVariants,
}

#[uniffi::export]
fn identity_colliding_variants(value: CollidingVariants) -> CollidingVariants {
    value
}

#[derive(uniffi::Enum)]
pub(crate) enum OptionalFields {
    Named {
        required: String,
        maybe_string: Option<String>,
        maybe_record: Option<AnimalRecord>,
    },
    Empty,
}

#[uniffi::export]
fn identity_optional_fields(value: OptionalFields) -> OptionalFields {
    value
}

// A recursive enum: `Cons` holds a `Box<IntList>` pointing back to the same
// type. Without the `Box` this would be an infinite-size type and fail to
// compile; uniffi 0.32 added automatic FFI support for `Box<T>` so recursive
// enums like this can cross the FFI at all.
#[derive(uniffi::Enum, Debug, Clone, PartialEq, Eq)]
pub enum IntList {
    Cons(i32, Box<IntList>),
    Nil,
}

#[uniffi::export]
fn identity_int_list(value: IntList) -> IntList {
    value
}

#[uniffi::export]
fn make_int_list(values: Vec<i32>) -> IntList {
    let mut list = IntList::Nil;
    for v in values.into_iter().rev() {
        list = IntList::Cons(v, Box::new(list));
    }
    list
}

#[uniffi::export]
fn int_list_sum(value: IntList) -> i32 {
    match value {
        IntList::Cons(v, rest) => v + int_list_sum(*rest),
        IntList::Nil => 0,
    }
}

uniffi::include_scaffolding!("enum_types");

#[cfg(test)]
mod test {
    use crate::AnimalSignedInt;

    #[test]
    fn check_signed() {
        assert_eq!(AnimalSignedInt::Koala as i8, -1);
        assert_eq!(AnimalSignedInt::Wallaby as i8, 0);
        assert_eq!(AnimalSignedInt::Wombat as i8, 1);
    }
}
