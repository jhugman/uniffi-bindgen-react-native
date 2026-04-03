/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/
 */

use anyhow::Result;
use camino::Utf8PathBuf;
use uniffi_bindgen::Component;

use crate::{
    bindings::{gen_rust, metadata::ModuleMetadata},
    switches::SwitchArgs,
};

pub(crate) fn generate_rs(
    components: &[Component<gen_rust::Config>],
    switches: &SwitchArgs,
    out_dir: &Utf8PathBuf,
    try_format_code: bool,
) -> Result<()> {
    for component in components {
        let module = ModuleMetadata::from(component);

        let rs_code = gen_rust::generate_rs(
            &component.ci,
            &module,
            &component.config,
            switches,
            try_format_code,
        )?;
        let rs_path = out_dir.join(module.rs_filename());
        ubrn_common::write_file(rs_path, rs_code)?;
    }
    Ok(())
}
