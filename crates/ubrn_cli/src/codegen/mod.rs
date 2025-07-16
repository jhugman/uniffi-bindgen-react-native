/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/
 */
use std::{cell::OnceCell, collections::BTreeMap, rc::Rc};

use anyhow::Result;
use askama::DynTemplate;
use camino::{Utf8Path, Utf8PathBuf};

use ubrn_bindgen::ModuleMetadata;
use ubrn_common::{mk_dir, CrateMetadata};

use crate::config::ProjectConfig;

pub(crate) trait RenderedFile: DynTemplate {
    fn path(&self, project_root: &Utf8Path) -> Utf8PathBuf;
    fn project_root(&self) -> Utf8PathBuf {
        // This provides a convenience for templates to calculate
        // relative paths between one another.
        Utf8PathBuf::new()
    }
    fn relative_to(&self, project_root: &Utf8Path, to: &Utf8Path) -> Utf8PathBuf {
        let file = self.path(project_root);
        let from = file
            .parent()
            .expect("Expected this file to have a directory");
        pathdiff::diff_utf8_paths(to, from).expect("Should be able to find a relative path")
    }
    fn filter_by(&self) -> bool {
        true
    }
    /// Optional hook to transform the text after rendered from the Askama template.
    fn transform_str(&self, _project_root: &Utf8Path, contents: String) -> Result<String> {
        Ok(contents)
    }
}

pub(crate) struct TemplateConfig {
    pub(crate) project: ProjectConfig,
    pub(crate) rust_crate: CrateMetadata,
    pub(crate) modules: Vec<ModuleMetadata>,
    pub(crate) uses_kotlin: OnceCell<bool>,
    pub(crate) native_bindings: bool,
}

impl TemplateConfig {
    pub(crate) fn new(
        project: ProjectConfig,
        rust_crate: CrateMetadata,
        modules: Vec<ModuleMetadata>,
        native_bindings: bool,
    ) -> Self {
        let mut modules = modules;
        modules.sort_by_key(|m| m.ts());
        Self {
            project,
            rust_crate,
            modules,
            native_bindings,
            uses_kotlin: OnceCell::new(),
        }
    }
}

pub(crate) fn get_template_config(
    project: ProjectConfig,
    rust_crate: CrateMetadata,
    modules: Vec<ModuleMetadata>,
    native_bindings: bool,
) -> Rc<TemplateConfig> {
    Rc::new(TemplateConfig::new(
        project,
        rust_crate,
        modules,
        native_bindings,
    ))
}

pub(crate) fn render_files(
    config: Rc<TemplateConfig>,
    files: impl Iterator<Item = Rc<dyn RenderedFile>>,
) -> Result<()> {
    let files = files.filter(|f| f.filter_by());
    let project_root = config.project.project_root();
    let map = render_templates(project_root, files)?;
    let exclude_files = config.project.exclude_files();
    for (path, contents) in map {
        // We don't want to write files that the config file has excluded.
        // In order to test if it is excluded, we need to get the file path
        // relative to the project_root.
        let rel = pathdiff::diff_utf8_paths(&path, project_root)
            .expect("path should be relative to root");
        if exclude_files.is_match(&rel) {
            continue;
        }
        let parent = path.parent().expect("Parent for path");
        mk_dir(parent)?;
        ubrn_common::write_file(path, contents)?;
    }

    Ok(())
}

fn render_templates(
    project_root: &Utf8Path,
    files: impl Iterator<Item = Rc<dyn RenderedFile>>,
) -> Result<BTreeMap<Utf8PathBuf, String>> {
    let mut map = BTreeMap::default();
    for f in files {
        let text = f.dyn_render()?;
        let path = f.path(project_root);
        map.insert(path, f.transform_str(project_root, text)?);
    }
    Ok(map)
}

#[macro_export]
macro_rules! templated_file {
    ($T:ty, $filename:literal) => {
        paste::paste! {
            #[derive(askama::Template)]
            #[template(path = $filename, escape = "none")]
            pub(crate) struct $T {
                #[allow(dead_code)]
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

pub(crate) mod files {
    use std::rc::Rc;

    use super::{RenderedFile, TemplateConfig};
    #[cfg(feature = "wasm")]
    use crate::wasm;
    use crate::{jsi, Platform};

    pub(crate) fn get_files_for(
        config: Rc<TemplateConfig>,
        platform: &Platform,
    ) -> Vec<Rc<dyn RenderedFile>> {
        let mut files = vec![];
        match platform {
            Platform::Android => {
                files.extend(jsi::crossplatform::get_files(config.clone()));
                files.extend(jsi::android::get_files(config.clone()));
            }
            Platform::Ios => {
                files.extend(jsi::crossplatform::get_files(config.clone()));
                files.extend(jsi::ios::get_files(config.clone()));
            }
            #[cfg(feature = "wasm")]
            Platform::Wasm => {
                files.extend(wasm::get_files(config.clone()));
            }
        }
        files
    }

    pub(crate) fn get_files(config: Rc<TemplateConfig>) -> Vec<Rc<dyn RenderedFile>> {
        let mut files = vec![];
        files.extend(jsi::get_files(config.clone()));
        #[cfg(feature = "wasm")]
        files.extend(wasm::get_files(config.clone()));
        files
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        config::rust_crate::CrateConfig,
        config::{self, BindingsConfig, ExtraArgs},
        jsi::android::config::AndroidConfig,
        jsi::crossplatform::TurboModulesConfig,
        jsi::ios::config::IOsConfig,
    };

    use super::*;

    impl ProjectConfig {
        pub(crate) fn empty(name: &str, crate_: CrateConfig) -> Self {
            let android = AndroidConfig {
                directory: "android".to_string(),
                jni_libs: "src/main/jniLibs".to_string(),
                targets: Default::default(),
                cargo_extras: ExtraArgs::default(),
                api_level: 21,
                package_name: "com.tester".to_string(),
                codegen_output_dir: "android/generated".to_string(),
            };
            let ios = IOsConfig {
                directory: "ios".to_string(),
                framework_name: "MyRustCrateFramework".to_string(),
                xcodebuild_extras: ExtraArgs::default(),
                targets: Default::default(),
                cargo_extras: ExtraArgs::default(),
                codegen_output_dir: "ios/generated".to_string(),
            };
            let bindings = BindingsConfig {
                cpp: "cpp/bindings".to_string(),
                ts: "src/bindings".to_string(),
                uniffi_toml: Default::default(),
            };
            let tm = TurboModulesConfig {
                name: "MyCrateSpec".to_string(),
                cpp: "cpp".to_string(),
                ts: "src".to_string(),
                spec_name: "MyRustCrate".to_string(),
                entrypoint: "index.react-native.tsx".to_string(),
            };
            let repository = format!("https://github.com/user/{name}");

            #[cfg(feature = "wasm")]
            let wasm = crate::wasm::WasmConfig::default();

            Self {
                name: name.to_string(),
                project_version: "0.1.0".to_string(),
                repository,
                crate_,
                android,
                ios,
                #[cfg(feature = "wasm")]
                wasm,
                bindings,
                tm,
                exclude_files: Default::default(),
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
        let modules = modules.iter().map(|s| ModuleMetadata::new(s)).collect();
        let template = TemplateConfig::new(project_config, crate_metadata, modules, false);
        Ok(Rc::new(template))
    }

    #[test]
    fn test_templating() -> Result<()> {
        templated_file!(TemplateTester, "TemplateTester.txt");
        impl RenderedFile for TemplateTester {
            fn path(&self, project_root: &Utf8Path) -> Utf8PathBuf {
                let name = self.config.project.module_cpp();
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
            config.project.module_cpp(),
            "MyTesterTemplateProject".to_string()
        );
        assert!(s.contains("module_cpp = MyTesterTemplateProject."));
        assert!(s.contains("list of modules = ['NativeAlice', 'NativeBob', 'NativeCharlie']"));
        Ok(())
    }
}
