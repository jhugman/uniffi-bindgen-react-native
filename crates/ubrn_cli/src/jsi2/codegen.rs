/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/
 */
use std::rc::Rc;

use camino::{Utf8Path, Utf8PathBuf};

use crate::codegen::{RenderedFile, TemplateConfig};
use crate::templated_file;

/// An assets-only library: the entrypoint, one manifest per platform, and the
/// empty ReactPackage Android autolinking insists on. No C++, no Kotlin, no
/// CMake, no codegen spec: the player owns all of that.
pub(crate) fn get_files(config: Rc<TemplateConfig>) -> Vec<Rc<dyn RenderedFile>> {
    vec![
        IndexTsx::rc_new(config.clone()),
        Podspec::rc_new(config.clone()),
        BuildGradle::rc_new(config.clone()),
        AndroidManifest::rc_new(config.clone()),
        JavaPackage::rc_new(config.clone()),
    ]
}

impl TemplateConfig {
    /// The cdylib name: what the bindings open, and what the framework and
    /// the .so are called.
    pub(crate) fn jsi2_lib_name(&self) -> &str {
        self.rust_crate.library_name()
    }

    pub(crate) fn jsi2_xcframework_path(&self, project_root: &Utf8Path) -> Utf8PathBuf {
        self.project
            .ios
            .directory(project_root)
            .join(format!("{}.xcframework", self.jsi2_lib_name()))
    }
}

templated_file!(IndexTsx, "jsi2-index.tsx");
impl RenderedFile for IndexTsx {
    fn path(&self, project_root: &Utf8Path) -> Utf8PathBuf {
        self.config.project.tm.entrypoint(project_root)
    }
}

templated_file!(Podspec, "jsi2-module-template.podspec");
impl RenderedFile for Podspec {
    fn path(&self, project_root: &Utf8Path) -> Utf8PathBuf {
        let name = self.config.project.podspec_filename();
        project_root.join(format!("{name}.podspec"))
    }
}

templated_file!(BuildGradle, "jsi2-build.gradle");
impl RenderedFile for BuildGradle {
    fn path(&self, project_root: &Utf8Path) -> Utf8PathBuf {
        self.config
            .project
            .android
            .directory(project_root)
            .join("build.gradle")
    }
}

templated_file!(AndroidManifest, "jsi2-AndroidManifest.xml");
impl RenderedFile for AndroidManifest {
    fn path(&self, project_root: &Utf8Path) -> Utf8PathBuf {
        self.config
            .project
            .android
            .src_main_dir(project_root)
            .join("AndroidManifest.xml")
    }
}

templated_file!(JavaPackage, "jsi2-PackageTemplate.java");
impl RenderedFile for JavaPackage {
    fn path(&self, project_root: &Utf8Path) -> Utf8PathBuf {
        let name = self.config.project.module_cpp();
        self.config
            .project
            .android
            .codegen_package_dir(project_root)
            .join(format!("{name}Package.java"))
    }
}
