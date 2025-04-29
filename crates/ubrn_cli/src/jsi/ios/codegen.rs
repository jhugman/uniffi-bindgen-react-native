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
        ModuleTemplateH::rc_new(config.clone()),
        ModuleTemplateMm::rc_new(config.clone()),
        PodspecTemplate::rc_new(config.clone()),
    ]
}

templated_file!(ModuleTemplateH, "ModuleTemplate.h");
impl RenderedFile for ModuleTemplateH {
    fn path(&self, project_root: &Utf8Path) -> Utf8PathBuf {
        let name = self.config.project.module_cpp();
        let filename = format!("{name}.h");
        self.config
            .project
            .ios
            .directory(project_root)
            .join(filename)
    }
}

templated_file!(ModuleTemplateMm, "ModuleTemplate.mm");
impl RenderedFile for ModuleTemplateMm {
    fn path(&self, project_root: &Utf8Path) -> Utf8PathBuf {
        let name = self.config.project.module_cpp();
        let filename = format!("{name}.mm");
        self.config
            .project
            .ios
            .directory(project_root)
            .join(filename)
    }
}

templated_file!(PodspecTemplate, "module-template.podspec");
impl RenderedFile for PodspecTemplate {
    fn path(&self, project_root: &Utf8Path) -> Utf8PathBuf {
        let name = self.config.project.podspec_filename();
        let filename = format!("{name}.podspec");
        project_root.join(filename)
    }
}
