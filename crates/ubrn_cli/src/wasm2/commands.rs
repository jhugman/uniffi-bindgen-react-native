/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/
 */
use std::process::Command;

use anyhow::{bail, Result};
use camino::{Utf8Path, Utf8PathBuf};
use clap::Args;
use ubrn_common::{run_cmd, CrateMetadata};

use super::{config::Target, Wasm2Config};
use crate::{
    commands::{building::CommonBuildArgs, ConfigArgs},
    config::ProjectConfig,
};

/// The crate providing `__ubrn_alloc` / `__ubrn_free` and the panic hook.
const RUNTIME_CRATE: &str = "uniffi-runtime-wasm";
/// uniffi's core, and the feature that drops its `Send + Sync` requirement on
/// exported objects — wasm being single-threaded.
const UNIFFI_CORE: &str = "uniffi_core";
const UNIFFI: &str = "uniffi";
const SINGLE_THREADED: &str = "wasm-unstable-single-threaded";

#[derive(Args, Debug)]
pub(crate) struct Wasm2BuildArgs {
    #[clap(flatten)]
    config: ConfigArgs,

    /// Opts out of generating the bindings and wasm-crate.
    #[clap(long, conflicts_with_all = ["and_generate"])]
    pub(crate) no_generate: bool,

    /// Opts out of running cargo build for wasm32-unknown-unknown.
    #[clap(long, conflicts_with_all = ["no_generate"])]
    pub(crate) no_wasm_build: bool,

    #[clap(flatten)]
    pub(crate) common_args: CommonBuildArgs,
}

impl Wasm2BuildArgs {
    /// Build the user's crate for `wasm32-unknown-unknown` and return the
    /// `.wasm`. It is both what the player loads and what the bindgen reads
    /// metadata from, so there is no second, native build.
    pub(crate) fn build(&self) -> Result<Vec<Utf8PathBuf>> {
        let config = self.project_config()?;
        let target = config.wasm2.targets.first().cloned().unwrap_or_default();
        let crate_ = &config.crate_;
        let metadata = crate_.metadata()?;
        check_wasm_ready(&metadata)?;
        if !self.no_wasm_build {
            self.cargo_build_wasm(
                &crate_.manifest_path()?,
                &config.wasm2,
                &target,
                &crate_.crate_dir()?,
            )?;
        }
        let wasm = self.built_wasm(&metadata, &target)?;
        Ok(vec![wasm])
    }

    /// Copy the built module next to the generated TypeScript.
    ///
    /// Runs after `generate`, which creates the destination. Staging a copy is
    /// also what keeps the table rewrite off cargo's own artifact.
    pub(crate) fn then_build(&self) -> Result<()> {
        let config = self.project_config()?;
        let target = config.wasm2.targets.first().cloned().unwrap_or_default();
        let metadata = config.crate_.metadata()?;
        let wasm = self.built_wasm(&metadata, &target)?;
        let ts_dir = config.wasm2_bindings_ts_path(config.project_root());
        // No dead-code elimination: the keep-list is a heuristic, and this is
        // the user's crate, which may export symbols for another consumer.
        let staged = ubrn_common::stage_wasm(&wasm, &ts_dir, metadata.library_name(), false)?;
        println!("Staged {staged}");
        Ok(())
    }

    fn built_wasm(&self, metadata: &CrateMetadata, target: &Target) -> Result<Utf8PathBuf> {
        let wasm = metadata.library_path(Some(target.triple()), self.common_args.profile(), None);
        if !wasm.exists() {
            bail!("No wasm module at {wasm}. Drop --no-wasm-build to have it built.");
        }
        Ok(wasm)
    }

    pub(crate) fn project_config(&self) -> Result<ProjectConfig> {
        let config: ProjectConfig = self.config.clone().try_into()?;
        Ok(config)
    }

    fn cargo_build_wasm(
        &self,
        manifest_path: &Utf8Path,
        wasm_config: &Wasm2Config,
        target: &Target,
        rust_dir: &Utf8Path,
    ) -> Result<()> {
        let triple = target.triple();
        println!("Compiling {manifest_path} for {triple}");
        let mut cmd = Command::new("cargo");
        cmd.arg("build")
            // Only the cdylib is consumed; a `[[bin]]` in the same crate would
            // otherwise be built for wasm32 and thrown away.
            .arg("--lib")
            .arg("--manifest-path")
            .arg(manifest_path)
            .arg("--target")
            .arg(triple);
        let profile = self.common_args.profile();
        if profile != "debug" {
            cmd.arg("--profile").arg(profile);
        }
        if let Some(features) = &wasm_config.features {
            cmd.arg("--features").arg(features.join(","));
        }
        // Cargo has no `--default-features`; enabling them is what it already
        // does, so only the opt-out needs a flag.
        if wasm_config.default_features == Some(false) {
            cmd.arg("--no-default-features");
        }
        cmd.args(wasm_config.cargo_extras.clone())
            .current_dir(rust_dir);

        let rustflags: Vec<String> = wasm_config.rustflags.clone().into_iter().collect();
        if !rustflags.is_empty() {
            cmd.env("RUSTFLAGS", rustflags.join(" "));
        }

        run_cmd(&mut cmd)?;
        Ok(())
    }
}

/// Reject a crate the player cannot load, before `cargo build` fails less
/// legibly — or worse, succeeds and defers the failure to the first call.
fn check_wasm_ready(crate_: &CrateMetadata) -> Result<()> {
    let manifest_path = crate_.manifest_path();
    if !crate_.builds_cdylib() {
        bail!(
            "{manifest_path} does not build a cdylib, which the wasm2 player loads.\n\
             Add it to the [lib] section:\n\
             \n    [lib]\n    crate-type = [\"lib\", \"cdylib\"]\n"
        );
    }
    if !crate_.has_dependency(RUNTIME_CRATE) {
        bail!(
            "{manifest_path} does not depend on {RUNTIME_CRATE}, which provides the \
             allocator and panic hook the wasm2 player calls into.\n\
             Add it for the wasm32 target:\n\
             \n    [target.'cfg(target_arch = \"wasm32\")'.dependencies]\n    \
             {RUNTIME_CRATE} = \"{}\"\n",
            env!("CARGO_PKG_VERSION")
        );
    }
    // Either spelling enables it: directly on uniffi_core, or through uniffi,
    // which re-exports it.
    let single_threaded = crate_.declares_dependency_feature(UNIFFI_CORE, SINGLE_THREADED)
        || crate_.declares_dependency_feature(UNIFFI, SINGLE_THREADED);
    if !single_threaded {
        bail!(
            "{manifest_path} resolves {UNIFFI_CORE} without the `{SINGLE_THREADED}` \
             feature. wasm is single-threaded, so uniffi otherwise requires \
             `Send + Sync` on exported objects and the crate will not compile \
             for wasm32.\n\
             Enable it for the wasm32 target:\n\
             \n    [target.'cfg(target_arch = \"wasm32\")'.dependencies]\n    \
             {UNIFFI_CORE} = {{ version = \"0.31\", features = [\"{SINGLE_THREADED}\"] }}\n"
        );
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn metadata_for(relative: &str) -> CrateMetadata {
        let manifest = Utf8PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../..")
            .join(relative);
        CrateMetadata::try_from(manifest).expect("cargo metadata")
    }

    #[test]
    fn accepts_a_crate_declaring_cdylib_and_the_runtime() {
        // coverall takes the runtime as a `cfg(target_arch = "wasm32")` dep,
        // which is the shape the error message recommends.
        check_wasm_ready(&metadata_for("fixtures/coverall/Cargo.toml")).unwrap();
    }

    #[test]
    fn rejects_a_crate_that_builds_no_cdylib() {
        let err = check_wasm_ready(&metadata_for("crates/ubrn_common/Cargo.toml"))
            .expect_err("ubrn_common is a plain lib");
        assert!(err.to_string().contains("cdylib"), "{err}");
    }

    #[test]
    fn rejects_a_crate_missing_the_single_threaded_feature() {
        // The arithmetic example is a cdylib but targets JSI, so it enables
        // neither the runtime dependency nor the wasm uniffi_core feature.
        let md = metadata_for("examples/arithmetic/Cargo.toml");
        assert!(
            !md.declares_dependency_feature(UNIFFI_CORE, SINGLE_THREADED),
            "expected the arithmetic example to lack {SINGLE_THREADED}"
        );
    }

    #[test]
    fn accepts_a_fixture_enabling_the_single_threaded_feature() {
        let md = metadata_for("fixtures/coverall/Cargo.toml");
        assert!(
            md.declares_dependency_feature(UNIFFI_CORE, SINGLE_THREADED),
            "coverall enables it for wasm32; features union across targets"
        );
    }

    #[test]
    fn rejects_a_cdylib_without_the_runtime() {
        let err = check_wasm_ready(&metadata_for("examples/arithmetic/Cargo.toml"))
            .expect_err("the arithmetic example targets JSI, not the wasm2 player");
        assert!(err.to_string().contains(RUNTIME_CRATE), "{err}");
    }
}
