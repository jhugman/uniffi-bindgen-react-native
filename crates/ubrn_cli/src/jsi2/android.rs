/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/
 */
use std::process::Command;

use anyhow::{bail, Result};
use camino::{Utf8Path, Utf8PathBuf};
use clap::Args;
use ubrn_common::{cp_file, mk_dir, rm_dir, run_cmd, CrateMetadata};

use crate::{
    commands::{building::CommonBuildArgs, ConfigArgs},
    config::{ExtraArgs, ProjectConfig},
    jsi::android::config::Target,
};

#[derive(Args, Debug)]
pub(crate) struct Jsi2AndroidBuildArgs {
    #[clap(flatten)]
    config: ConfigArgs,

    /// Comma separated ABIs, overriding `jsi2.androidTargets`:
    /// arm64-v8a,x86_64,armeabi-v7a,x86 or their cargo triples.
    #[clap(short, long, value_parser, num_args = 1.., value_delimiter = ',')]
    pub(crate) targets: Vec<Target>,

    #[clap(flatten)]
    pub(crate) common_args: CommonBuildArgs,

    /// Suppress the copying of the Rust library into the JNI library directories.
    #[clap(long = "no-jniLibs")]
    no_jni_libs: bool,
}

impl Jsi2AndroidBuildArgs {
    /// Build the crate as `lib<name>.so` per ABI and copy each into
    /// `android/src/main/jniLibs/<abi>/`, where Gradle packages it and the
    /// player's resolver finds it by name.
    pub(crate) fn build(&self) -> Result<Vec<Utf8PathBuf>> {
        let config = self.project_config()?;
        let crate_ = &config.crate_;
        let metadata = crate_.metadata()?;
        check_cdylib(&metadata)?;

        let android = &config.android;
        let profile = self.common_args.profile();
        let manifest_path = crate_.manifest_path()?;
        let rust_dir = crate_.crate_dir()?;

        let mut built = Vec::new();
        for target in &self.targets_for(&config) {
            if !self.common_args.no_cargo {
                cargo_ndk_build(
                    &manifest_path,
                    target,
                    &android.cargo_extras,
                    android.api_level,
                    profile,
                    &rust_dir,
                    metadata.library_name(),
                )?;
            }
            let so = metadata.library_path(Some(target.triple()), profile, Some(true));
            built.push((target.clone(), so));
        }

        if !self.no_jni_libs {
            let jni_libs = android.jni_libs(config.project_root());
            println!("-- Copying into {jni_libs}");
            rm_dir(&jni_libs)?;
            for (target, so) in &built {
                let dst_dir = jni_libs.join(target.to_string());
                mk_dir(&dst_dir)?;
                let dst = dst_dir.join(metadata.library_file(Some(target.triple()), Some(true)));
                println!("cp {so} {dst}");
                cp_file(so, &dst)?;
            }
        }

        Ok(built.into_iter().map(|(_, so)| so).collect())
    }

    fn targets_for(&self, config: &ProjectConfig) -> Vec<Target> {
        if self.targets.is_empty() {
            config.jsi2.android_targets.clone()
        } else {
            self.targets.clone()
        }
    }

    pub(crate) fn project_config(&self) -> Result<ProjectConfig> {
        let config: ProjectConfig = self.config.clone().try_into()?;
        Ok(config)
    }
}

/// Reject a crate the player cannot load before cargo fails less legibly.
pub(crate) fn check_cdylib(crate_: &CrateMetadata) -> Result<()> {
    if !crate_.builds_cdylib() {
        bail!(
            "{} does not build a cdylib, which the JSI player dlopens.\n\
             Add it to the [lib] section:\n\
             \n    [lib]\n    crate-type = [\"lib\", \"cdylib\"]\n",
            crate_.manifest_path()
        );
    }
    Ok(())
}

fn cargo_ndk_build(
    manifest_path: &Utf8Path,
    target: &Target,
    cargo_extras: &ExtraArgs,
    api_level: usize,
    profile: &str,
    rust_dir: &Utf8Path,
    library_name: &str,
) -> Result<()> {
    let mut cmd = Command::new("cargo");
    cmd.arg("ndk")
        .arg("--manifest-path")
        .arg(manifest_path)
        .arg("--target")
        .arg(target.to_string())
        .arg("--platform")
        .arg(format!("{api_level}"));
    cmd.arg("--").arg("build");
    if profile != "debug" {
        cmd.args(["--profile", profile]);
    }
    cmd.args(cargo_extras.clone());
    // Android 15 refuses libraries that are not 16 KB page-aligned; without a
    // soname a consumer that links this .so records the build path as
    // DT_NEEDED, which bionic can't resolve at runtime.
    let rustflags_env = format!(
        "CARGO_TARGET_{}_RUSTFLAGS",
        target.triple().to_uppercase().replace('-', "_")
    );
    cmd.env(
        rustflags_env,
        format!(
            "-C link-arg=-Wl,-z,max-page-size=16384 -C link-arg=-Wl,-soname,lib{library_name}.so"
        ),
    );
    run_cmd(cmd.current_dir(rust_dir))?;
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
    fn accepts_a_crate_declaring_cdylib() {
        check_cdylib(&metadata_for("examples/arithmetic/Cargo.toml")).unwrap();
    }

    #[test]
    fn rejects_a_crate_that_builds_no_cdylib() {
        let err = check_cdylib(&metadata_for("crates/ubrn_common/Cargo.toml"))
            .expect_err("ubrn_common is a plain lib");
        assert!(err.to_string().contains("cdylib"), "{err}");
    }
}
