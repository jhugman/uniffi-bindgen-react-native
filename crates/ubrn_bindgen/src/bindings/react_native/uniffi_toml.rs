/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/
 */

use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use uniffi_bindgen::backend::TemplateExpression;

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub(crate) struct ReactNativeConfig {
    #[serde(default, alias = "javascript", alias = "js", alias = "ts")]
    pub(crate) typescript: TsConfig,
    #[serde(default)]
    pub(crate) cpp: CppConfig,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(crate) struct TsConfig {
    #[serde(default)]
    pub(crate) log_level: LogLevel,
    #[serde(default)]
    pub(crate) console_import: Option<String>,
    #[serde(default)]
    pub(crate) custom_types: HashMap<String, CustomTypeConfig>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) enum LogLevel {
    #[default]
    None,
    Debug,
    Verbose,
}

impl LogLevel {
    fn is_verbose(&self) -> bool {
        matches!(self, Self::Verbose)
    }
    fn is_debug(&self) -> bool {
        matches!(self, Self::Debug | Self::Verbose)
    }
}

impl TsConfig {
    pub(crate) fn is_verbose(&self) -> bool {
        self.log_level.is_verbose()
    }
    pub(crate) fn is_debug(&self) -> bool {
        self.log_level.is_debug()
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(crate) struct CustomTypeConfig {
    #[serde(default)]
    pub(crate) imports: Vec<(String, String)>,
    pub(crate) type_name: Option<String>,
    #[serde(alias = "lift")]
    pub(crate) into_custom: TemplateExpression,
    #[serde(alias = "lower")]
    pub(crate) from_custom: TemplateExpression,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct CppConfig {}
