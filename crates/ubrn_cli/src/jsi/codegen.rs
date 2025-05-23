/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/
 */
use std::rc::Rc;

use crate::codegen::{RenderedFile, TemplateConfig};
use crate::jsi::{android, crossplatform, ios};

pub(crate) fn get_files(config: Rc<TemplateConfig>) -> Vec<Rc<dyn RenderedFile>> {
    let mut files = Vec::new();
    files.extend(crossplatform::get_files(config.clone()));
    files.extend(ios::get_files(config.clone()));
    files.extend(android::get_files(config.clone()));
    files
}
