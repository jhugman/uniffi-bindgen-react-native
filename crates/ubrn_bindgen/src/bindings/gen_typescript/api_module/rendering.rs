/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/
 */

//! Template rendering for IR node types.

use std::fmt;

use askama::Template;

use super::nodes::*;

impl TsTypeDefinition {
    pub fn render_to_string(
        &self,
        is_verbose: &bool,
        supports_rust_backtrace: &bool,
        console_import: &Option<String>,
    ) -> String {
        match self {
            TsTypeDefinition::SimpleWrapper(wrapper) => SimpleWrapperRenderer { wrapper }
                .render()
                .unwrap_or_default(),
            TsTypeDefinition::StringHelper(helper) => {
                StringHelperRenderer { helper }.render().unwrap_or_default()
            }
            TsTypeDefinition::Custom(custom) => {
                CustomTypeRenderer { custom }.render().unwrap_or_default()
            }
            TsTypeDefinition::External(_) => String::new(),
            TsTypeDefinition::Enum(e) => EnumRenderer {
                e,
                is_verbose,
                supports_rust_backtrace,
                console_import,
            }
            .render()
            .unwrap_or_default(),
            TsTypeDefinition::Record(rec) => RecordRenderer {
                rec,
                is_verbose,
                supports_rust_backtrace,
                console_import,
            }
            .render()
            .unwrap_or_default(),
            TsTypeDefinition::Object(obj) => ObjectRenderer {
                obj,
                is_verbose,
                supports_rust_backtrace,
                console_import,
            }
            .render()
            .unwrap_or_default(),
            TsTypeDefinition::CallbackInterface(cbi) => CallbackInterfaceRenderer {
                cbi,
                is_verbose,
                console_import,
            }
            .render()
            .unwrap_or_default(),
        }
    }
}

impl TsCallable {
    pub fn render_as_function(
        &self,
        is_verbose: &bool,
        supports_rust_backtrace: &bool,
        console_import: &Option<String>,
    ) -> String {
        FunctionRenderer {
            func: self,
            is_verbose,
            supports_rust_backtrace,
            console_import,
        }
        .render()
        .unwrap_or_default()
    }
}

impl InitializationIR {
    pub fn render_to_string(&self) -> String {
        InitializationRenderer { init: self }
            .render()
            .unwrap_or_default()
    }
}

impl super::TsApiModule {
    pub fn render_all_types(&self) -> String {
        let mut out = String::new();

        for func in &self.functions {
            out.push_str(&func.render_as_function(
                &self.is_verbose,
                &self.supports_rust_backtrace,
                &self.console_import,
            ));
        }

        for td in &self.type_definitions {
            out.push_str(&td.render_to_string(
                &self.is_verbose,
                &self.supports_rust_backtrace,
                &self.console_import,
            ));
        }

        out.push_str(&self.initialization.render_to_string());

        out
    }
}

impl fmt::Display for TsTypeDefinition {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TsTypeDefinition::SimpleWrapper(wrapper) => SimpleWrapperRenderer { wrapper }
                .render_into(f)
                .map_err(|_| fmt::Error),
            TsTypeDefinition::StringHelper(helper) => StringHelperRenderer { helper }
                .render_into(f)
                .map_err(|_| fmt::Error),
            TsTypeDefinition::Custom(custom) => CustomTypeRenderer { custom }
                .render_into(f)
                .map_err(|_| fmt::Error),
            // These types need module-level context (is_verbose, console_import)
            // unavailable through Display; rendered via render_to_string() instead.
            TsTypeDefinition::External(_)
            | TsTypeDefinition::Enum(_)
            | TsTypeDefinition::Record(_)
            | TsTypeDefinition::Object(_)
            | TsTypeDefinition::CallbackInterface(_) => Ok(()),
        }
    }
}

#[derive(Template)]
#[template(syntax = "ts", escape = "none", path = "SimpleWrapperTemplate.ts")]
struct SimpleWrapperRenderer<'a> {
    wrapper: &'a TsSimpleWrapper,
}

#[derive(Template)]
#[template(syntax = "ts", escape = "none", path = "StringHelperTemplate.ts")]
struct StringHelperRenderer<'a> {
    helper: &'a TsStringHelper,
}

#[derive(Template)]
#[template(syntax = "ts", escape = "none", path = "CustomTypeTemplate.ts")]
struct CustomTypeRenderer<'a> {
    custom: &'a TsCustomType,
}

#[derive(Template)]
#[template(syntax = "ts", escape = "none", path = "EnumTemplate.ts")]
struct EnumRenderer<'a> {
    e: &'a TsEnum,
    is_verbose: &'a bool,
    supports_rust_backtrace: &'a bool,
    console_import: &'a Option<String>,
}

#[derive(Template)]
#[template(syntax = "ts", escape = "none", path = "RecordTemplate.ts")]
struct RecordRenderer<'a> {
    rec: &'a TsRecord,
    is_verbose: &'a bool,
    supports_rust_backtrace: &'a bool,
    console_import: &'a Option<String>,
}

#[derive(Template)]
#[template(syntax = "ts", escape = "none", path = "ObjectTemplate.ts")]
struct ObjectRenderer<'a> {
    obj: &'a TsObject,
    is_verbose: &'a bool,
    supports_rust_backtrace: &'a bool,
    console_import: &'a Option<String>,
}

#[derive(Template)]
#[template(syntax = "ts", escape = "none", path = "TopLevelFunctionTemplate.ts")]
struct FunctionRenderer<'a> {
    func: &'a TsFunction,
    is_verbose: &'a bool,
    supports_rust_backtrace: &'a bool,
    console_import: &'a Option<String>,
}

#[derive(Template)]
#[template(syntax = "ts", escape = "none", path = "InitializationTemplate.ts")]
struct InitializationRenderer<'a> {
    init: &'a InitializationIR,
}

#[derive(Template)]
#[template(syntax = "ts", escape = "none", path = "CallbackInterfaceTemplate.ts")]
struct CallbackInterfaceRenderer<'a> {
    cbi: &'a TsCallbackInterface,
    is_verbose: &'a bool,
    console_import: &'a Option<String>,
}
