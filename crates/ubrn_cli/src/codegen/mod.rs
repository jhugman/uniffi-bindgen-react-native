/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/
 */
use anyhow::Result;
use askama::DynTemplate;
use camino::{Utf8Path, Utf8PathBuf};
use clap::Args;
use std::{collections::BTreeMap, rc::Rc};

use ubrn_bindgen::ModuleMetadata;
use ubrn_common::{mk_dir, CrateMetadata};

use crate::config::ProjectConfig;

#[derive(Args, Debug)]
pub(crate) struct TurboModuleArgs {
    /// The configuration file for this build
    #[clap(long)]
    config: Utf8PathBuf,

    /// The namespaces that are generated by `generate bindings`.
    namespaces: Vec<String>,
}

impl TurboModuleArgs {
    pub(crate) fn run(&self) -> Result<()> {
        let project = ProjectConfig::try_from(self.config.clone())?;
        let modules = self
            .namespaces
            .iter()
            .map(|s| ModuleMetadata::new(s))
            .collect();
        let rust_crate = project.crate_.metadata()?;
        render_files(project, rust_crate, modules)?;
        Ok(())
    }
}

pub(crate) trait RenderedFile: DynTemplate {
    fn path(&self, project_root: &Utf8Path) -> Utf8PathBuf;
    fn project_root(&self) -> Utf8PathBuf {
        Utf8PathBuf::new()
    }
    fn relative_to(&self, project_root: &Utf8Path, to: &Utf8Path) -> Utf8PathBuf {
        let file = self.path(project_root);
        let from = file
            .parent()
            .expect("Expected this file to have a directory");
        pathdiff::diff_utf8_paths(to, from).expect("Should be able to find a relative path")
    }
}

pub(crate) struct TemplateConfig {
    pub(crate) project: ProjectConfig,
    pub(crate) rust_crate: CrateMetadata,
    pub(crate) modules: Vec<ModuleMetadata>,
}

impl TemplateConfig {
    pub(crate) fn new(
        project: ProjectConfig,
        rust_crate: CrateMetadata,
        modules: Vec<ModuleMetadata>,
    ) -> Self {
        Self {
            project,
            rust_crate,
            modules,
        }
    }
}

pub(crate) fn render_files(
    project: ProjectConfig,
    rust_crate: CrateMetadata,
    modules: Vec<ModuleMetadata>,
) -> Result<()> {
    let config = Rc::new(TemplateConfig::new(project, rust_crate, modules));
    let files = files::get_files(config.clone());

    let project_root = config.project.project_root();
    let map = render_templates(project_root, files)?;
    for (path, contents) in map {
        let parent = path.parent().expect("Parent for path");
        mk_dir(parent)?;
        std::fs::write(path, contents)?;
    }

    Ok(())
}

fn render_templates(
    project_root: &Utf8Path,
    files: Vec<Rc<dyn RenderedFile>>,
) -> Result<BTreeMap<Utf8PathBuf, String>> {
    let mut map = BTreeMap::default();
    for f in &files {
        let text = f.dyn_render()?;
        let path = f.path(project_root);
        map.insert(path, text);
    }
    Ok(map)
}

macro_rules! templated_file {
    ($T:ty, $filename:literal) => {
        paste::paste! {
            #[derive(askama::Template)]
            #[template(path = $filename, escape = "none")]
            pub(crate) struct $T {
                config: Rc<TemplateConfig>
            }

            #[allow(dead_code)]
            impl $T {
                pub(crate) fn new(config: Rc<TemplateConfig>) -> Self {
                    Self { config }
                }
                pub(crate) fn rc_new(config: Rc<TemplateConfig>) -> Rc<dyn RenderedFile> {
                    Rc::new(Self::new(config.clone()))
                }
            }
        }
    };
}

mod files {
    use super::RenderedFile;
    use super::TemplateConfig;
    use camino::Utf8Path;
    use camino::Utf8PathBuf;
    use std::rc::Rc;

    pub(super) fn get_files(config: Rc<TemplateConfig>) -> Vec<Rc<dyn RenderedFile>> {
        vec![
            // typescript
            IndexTs::rc_new(config.clone()),
            // C++
            TMHeader::rc_new(config.clone()),
            TMCpp::rc_new(config.clone()),
            // Codegen (for installer)
            NativeCodegenTs::rc_new(config.clone()),
            // Android
            JavaModule::rc_new(config.clone()),
            JavaPackage::rc_new(config.clone()),
            CMakeLists::rc_new(config.clone()),
            CppAdapter::rc_new(config.clone()),
            // iOS
            ModuleTemplateH::rc_new(config.clone()),
            ModuleTemplateMm::rc_new(config.clone()),
            PodspecTemplate::rc_new(config.clone()),
        ]
    }

    templated_file!(JavaModule, "ModuleTemplate.java");
    impl RenderedFile for JavaModule {
        fn path(&self, project_root: &Utf8Path) -> Utf8PathBuf {
            let name = self.config.project.name_upper_camel();
            let filename = format!("{name}Module.java");
            self.config
                .project
                .android
                .codegen_package_dir(project_root)
                .join(filename)
        }
    }

    templated_file!(JavaPackage, "PackageTemplate.java");
    impl RenderedFile for JavaPackage {
        fn path(&self, project_root: &Utf8Path) -> Utf8PathBuf {
            let name = self.config.project.name_upper_camel();
            let filename = format!("{name}Package.java");
            self.config
                .project
                .android
                .codegen_package_dir(project_root)
                .join(filename)
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

    templated_file!(IndexTs, "index.ts");
    impl RenderedFile for IndexTs {
        fn path(&self, project_root: &Utf8Path) -> Utf8PathBuf {
            let filename = "index.ts";
            self.config.project.tm.ts_path(project_root).join(filename)
        }
    }

    templated_file!(NativeCodegenTs, "NativeCodegenTemplate.ts");
    impl RenderedFile for NativeCodegenTs {
        fn path(&self, project_root: &Utf8Path) -> Utf8PathBuf {
            let filename = format!("{}.ts", self.config.project.codegen_filename());
            self.config.project.tm.ts_path(project_root).join(filename)
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

    templated_file!(ModuleTemplateH, "ModuleTemplate.h");
    impl RenderedFile for ModuleTemplateH {
        fn path(&self, project_root: &Utf8Path) -> Utf8PathBuf {
            let name = self.config.project.name_upper_camel();
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
            let name = self.config.project.name_upper_camel();
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
            let name = self.config.project.raw_name();
            let filename = format!("{name}.podspec");
            project_root.join(filename)
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        android::AndroidConfig,
        config::{self, BindingsConfig, TurboModulesConfig},
        ios::IOsConfig,
        rust::CrateConfig,
    };

    use super::*;

    impl ProjectConfig {
        pub(crate) fn empty(name: &str, crate_: CrateConfig) -> Self {
            use crate::building::ExtraArgs;

            let android = AndroidConfig {
                directory: "android".to_string(),
                jni_libs: "src/main/jniLibs".to_string(),
                targets: Default::default(),
                cargo_extras: ExtraArgs::default(),
                api_level: 21,
                package_name: "com.tester".to_string(),
            };
            let ios = IOsConfig {
                directory: "ios".to_string(),
                framework_name: "MyRustCrateFramework".to_string(),
                xcodebuild_extras: ExtraArgs::default(),
                targets: Default::default(),
                cargo_extras: ExtraArgs::default(),
            };
            let bindings = BindingsConfig {
                cpp: "cpp/bindings".to_string(),
                ts: "src/bindings".to_string(),
            };
            let tm = TurboModulesConfig {
                cpp: "cpp".to_string(),
                ts: "src".to_string(),
                spec_name: "MyRustCrate".to_string(),
            };

            Self {
                name: name.to_string(),
                crate_,
                android,
                ios,
                bindings,
                tm,
            }
        }
    }

    fn create_template_config(name: &str, modules: &[&str]) -> Result<Rc<TemplateConfig>> {
        let manifest_dir: Utf8PathBuf = std::env::var("CARGO_MANIFEST_DIR").unwrap().into();
        let crate_metadata = CrateMetadata::try_from(manifest_dir.clone())?;
        let crate_config: CrateConfig = crate_metadata.clone().try_into()?;
        assert_eq!("crates/ubrn_cli/Cargo.toml", crate_config.manifest_path);
        assert_eq!(
            manifest_dir.join("Cargo.toml"),
            crate_config.manifest_path()?
        );

        let project_config = config::ProjectConfig::empty(name, crate_config);
        let modules = modules
            .into_iter()
            .map(|s| ModuleMetadata::new(s))
            .collect();
        let template = TemplateConfig::new(project_config, crate_metadata, modules);
        Ok(Rc::new(template))
    }

    #[test]
    fn test_templating() -> Result<()> {
        templated_file!(TemplateTester, "TemplateTester.txt");
        impl RenderedFile for TemplateTester {
            fn path(&self, project_root: &Utf8Path) -> Utf8PathBuf {
                let name = self.config.project.name_upper_camel();
                let filename = format!("{name}.txt");
                self.config
                    .project
                    .android
                    .codegen_package_dir(project_root)
                    .join(filename)
            }
        }

        let config =
            create_template_config("my-tester-template-project", &["alice", "bob", "charlie"])?;
        let file = TemplateTester::new(config.clone());
        let project_root = Utf8PathBuf::new();
        let s = file.dyn_render()?;
        assert_eq!(
            "android/src/main/java/com/tester/MyTesterTemplateProject.txt".to_string(),
            file.path(&project_root).to_string()
        );
        // This is hard coded into the file. If this isn't here, then the test file hasn't rendered.
        assert!(s.contains("hardcoded into template."));
        assert_eq!(
            config.project.name_upper_camel(),
            "MyTesterTemplateProject".to_string()
        );
        assert!(s.contains("name_upper_camel = MyTesterTemplateProject."));
        assert!(s.contains("list of modules = ['NativeAlice', 'NativeBob', 'NativeCharlie']"));
        Ok(())
    }
}
