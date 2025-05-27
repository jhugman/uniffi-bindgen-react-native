/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/
 */
use std::rc::Rc;

use camino::{Utf8Path, Utf8PathBuf};

use crate::codegen::{RenderedFile, TemplateConfig};
use crate::templated_file;

pub(crate) fn get_files(config: Rc<TemplateConfig>) -> Vec<Rc<dyn RenderedFile>> {
    vec![
        // typescript
        IndexTsx::rc_new(config.clone()),
        // C++
        TMHeader::rc_new(config.clone()),
        TMCpp::rc_new(config.clone()),
        // Codegen (for installer)
        NativeCodegenTs::rc_new(config.clone()),
    ]
}

templated_file!(TMHeader, "TurboModuleTemplate.h");
impl RenderedFile for TMHeader {
    fn path(&self, project_root: &Utf8Path) -> Utf8PathBuf {
        let filename = format!("{}.h", self.config.project.cpp_filename());
        self.config.project.tm.cpp_path(project_root).join(filename)
    }
}

templated_file!(TMCpp, "TurboModuleTemplate.cpp");
impl RenderedFile for TMCpp {
    fn path(&self, project_root: &Utf8Path) -> Utf8PathBuf {
        let filename = format!("{}.cpp", self.config.project.cpp_filename());
        self.config.project.tm.cpp_path(project_root).join(filename)
    }
}

templated_file!(IndexTsx, "index.tsx");
impl RenderedFile for IndexTsx {
    fn path(&self, project_root: &Utf8Path) -> Utf8PathBuf {
        self.config.project.tm.entrypoint(project_root)
    }
}

templated_file!(NativeCodegenTs, "NativeCodegenTemplate.ts");
impl RenderedFile for NativeCodegenTs {
    fn path(&self, project_root: &Utf8Path) -> Utf8PathBuf {
        let filename = format!("{}.ts", self.config.project.codegen_filename());
        self.config.project.tm.ts_path(project_root).join(filename)
    }
}
