/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/
 */

use anyhow::Result;
use camino::Utf8PathBuf;
use uniffi_bindgen::Component;

use crate::bindings::{gen_cpp, metadata::ModuleMetadata};

pub(crate) fn generate_cpp(
    components: &[Component<gen_cpp::Config>],
    out_dir: &Utf8PathBuf,
    try_format_code: bool,
) -> Result<()> {
    for component in components {
        let module = ModuleMetadata::from(component);

        let cpp = gen_cpp::generate_cpp(&component.ci, &component.config, &module)?;
        let cpp_path = out_dir.join(module.cpp_filename());
        ubrn_common::write_file(cpp_path, cpp)?;

        let hpp = gen_cpp::generate_hpp(&component.ci, &component.config, &module)?;
        let hpp_path = out_dir.join(module.hpp_filename());
        ubrn_common::write_file(hpp_path, hpp)?;
    }
    if try_format_code {
        gen_cpp::format_directory(out_dir)?;
    }
    Ok(())
}
