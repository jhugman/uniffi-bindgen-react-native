/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/
 */
use heck::{ToLowerCamelCase, ToSnakeCase};
use proc_macro2::Span;
use syn::Ident;

pub(super) fn if_or_default<T>(flag: bool, then: T) -> T
where
    T: Default,
{
    if flag {
        then
    } else {
        Default::default()
    }
}

pub(super) fn if_then_map<F, T>(flag: bool, then: F) -> T
where
    F: FnOnce() -> T,
    T: Default,
{
    if flag {
        then()
    } else {
        Default::default()
    }
}

pub(super) fn map_or_default<F, T, U>(value: Option<T>, map: F) -> U
where
    F: FnOnce(T) -> U,
    U: Default,
{
    value.map_or_else(|| Default::default(), map)
}

pub(super) fn ident(id: &str) -> Ident {
    Ident::new(id, Span::call_site())
}

pub(super) fn snake_case_ident(s: &str) -> Ident {
    ident(&s.to_snake_case())
}

pub(super) fn camel_case_ident(field_name: &str) -> Ident {
    ident(&field_name.to_lower_camel_case())
}
