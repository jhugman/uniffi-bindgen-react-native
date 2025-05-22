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
        // Android
        CMakeLists::rc_new(config.clone()),
        CppAdapter::rc_new(config.clone()),
        AndroidManifest::rc_new(config.clone()),
        // Android with Java
        JavaModule::rc_new(config.clone()),
        JavaPackage::rc_new(config.clone()),
        BuildGradle::rc_new(config.clone()),
        // Android with Kotlin
        KtModule::rc_new(config.clone()),
        KtPackage::rc_new(config.clone()),
        KtBuildGradle::rc_new(config.clone()),
    ]
}

impl TemplateConfig {
    pub(crate) fn uses_kotlin(self: &Rc<Self>) -> bool {
        *self.uses_kotlin.get_or_init(|| {
            let project_root = self.project.project_root();
            let gradle_file = BuildGradle::new(self.clone()).path(project_root);
            if gradle_file.exists() {
                let file = ubrn_common::read_to_string(gradle_file)
                    .expect("Cannot read build.gradle file");
                file.contains("kotlin")
            } else {
                // assume that if the user blew away the gradle file,
                // then we should remake it as one with kotlin.
                true
            }
        })
    }
}

templated_file!(CppAdapter, "cpp-adapter.cpp");
impl RenderedFile for CppAdapter {
    fn path(&self, project_root: &Utf8Path) -> Utf8PathBuf {
        let filename = "cpp-adapter.cpp";
        self.config
            .project
            .android
            .directory(project_root)
            .join(filename)
    }
}

templated_file!(AndroidManifest, "AndroidManifest.xml");
impl RenderedFile for AndroidManifest {
    fn path(&self, project_root: &Utf8Path) -> Utf8PathBuf {
        let filename = "AndroidManifest.xml";
        self.config
            .project
            .android
            .src_main_dir(project_root)
            .join(filename)
    }
}
templated_file!(JavaModule, "ModuleTemplate.java");
impl RenderedFile for JavaModule {
    fn path(&self, project_root: &Utf8Path) -> Utf8PathBuf {
        let name = self.config.project.module_cpp();
        let filename = format!("{name}Module.java");
        self.config
            .project
            .android
            .codegen_package_dir(project_root)
            .join(filename)
    }
    fn filter_by(&self) -> bool {
        !self.config.uses_kotlin()
    }
}

templated_file!(JavaPackage, "PackageTemplate.java");
impl RenderedFile for JavaPackage {
    fn path(&self, project_root: &Utf8Path) -> Utf8PathBuf {
        let name = self.config.project.module_cpp();
        let filename = format!("{name}Package.java");
        self.config
            .project
            .android
            .codegen_package_dir(project_root)
            .join(filename)
    }
    fn filter_by(&self) -> bool {
        !self.config.uses_kotlin()
    }
}

templated_file!(BuildGradle, "build.gradle");
impl RenderedFile for BuildGradle {
    fn path(&self, project_root: &Utf8Path) -> Utf8PathBuf {
        let filename = "build.gradle";
        self.config
            .project
            .android
            .directory(project_root)
            .join(filename)
    }
    fn filter_by(&self) -> bool {
        !self.config.uses_kotlin()
    }
}

templated_file!(KtModule, "ModuleTemplate.kt");
impl RenderedFile for KtModule {
    fn path(&self, project_root: &Utf8Path) -> Utf8PathBuf {
        let name = self.config.project.module_cpp();
        let filename = format!("{name}Module.kt");
        self.config
            .project
            .android
            .codegen_package_dir(project_root)
            .join(filename)
    }
    fn filter_by(&self) -> bool {
        self.config.uses_kotlin()
    }
}

templated_file!(KtPackage, "PackageTemplate.kt");
impl RenderedFile for KtPackage {
    fn path(&self, project_root: &Utf8Path) -> Utf8PathBuf {
        let name = self.config.project.module_cpp();
        let filename = format!("{name}Package.kt");
        self.config
            .project
            .android
            .codegen_package_dir(project_root)
            .join(filename)
    }
    fn filter_by(&self) -> bool {
        self.config.uses_kotlin()
    }
}

templated_file!(KtBuildGradle, "build.kt.gradle");
impl RenderedFile for KtBuildGradle {
    fn path(&self, project_root: &Utf8Path) -> Utf8PathBuf {
        let filename = "build.gradle";
        self.config
            .project
            .android
            .directory(project_root)
            .join(filename)
    }
    fn filter_by(&self) -> bool {
        self.config.uses_kotlin()
    }
}

templated_file!(CMakeLists, "CMakeLists.txt");
impl RenderedFile for CMakeLists {
    fn path(&self, project_root: &Utf8Path) -> Utf8PathBuf {
        let filename = "CMakeLists.txt";
        self.config
            .project
            .android
            .directory(project_root)
            .join(filename)
    }
}
