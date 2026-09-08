/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/
 */
use std::{collections::BTreeMap, process::Command};

use anyhow::Result;
use camino::{Utf8Path, Utf8PathBuf};
use clap::Args;
use ubrn_common::{cp_file, mk_dir, rm_dir, run_cmd, write_file};

use crate::{
    commands::{building::CommonBuildArgs, ConfigArgs},
    config::{ExtraArgs, ProjectConfig},
    jsi::ios::config::{Platform, Target},
    jsi2::android::check_cdylib,
};

#[derive(Args, Debug)]
pub(crate) struct Jsi2IosBuildArgs {
    #[clap(flatten)]
    config: ConfigArgs,

    /// Only build for the simulator
    #[clap(long, default_value = "false")]
    sim_only: bool,

    /// Exclude builds for the simulator
    #[clap(long, conflicts_with_all = ["sim_only"], default_value = "false")]
    no_sim: bool,

    /// Stop after the per-platform framework bundles; skip xcodebuild
    #[clap(long, alias = "no-xcframework")]
    no_xcodebuild: bool,

    /// Comma separated list of targets, overriding `ios.targets`:
    /// aarch64-apple-ios,aarch64-apple-ios-sim,x86_64-apple-ios
    #[clap(short, long, value_parser, num_args = 1.., value_delimiter = ',')]
    pub(crate) targets: Vec<Target>,

    #[clap(flatten)]
    pub(crate) common_args: CommonBuildArgs,
}

impl Jsi2IosBuildArgs {
    /// Build the crate as a dylib per target, wrap each platform's slice as a
    /// `<name>.framework` bundle, and combine the bundles into
    /// `ios/<name>.xcframework`. Returns the dylibs: any one of them carries
    /// the uniffi metadata `generate` reads.
    pub(crate) fn build(&self) -> Result<Vec<Utf8PathBuf>> {
        let config = self.project_config()?;
        let crate_ = &config.crate_;
        let ios = &config.ios;
        let metadata = crate_.metadata()?;
        check_cdylib(&metadata)?;

        let targets: Vec<Target> = ios
            .targets
            .iter()
            .filter(|target| {
                let is_sim = target.platform == Platform::IosSimulator;
                if self.no_sim {
                    !is_sim
                } else if self.sim_only {
                    is_sim
                } else {
                    true
                }
            })
            .cloned()
            .collect();

        let profile = self.common_args.profile();
        let manifest_path = crate_.manifest_path()?;
        let rust_dir = crate_.crate_dir()?;
        let lib_name = metadata.library_name().to_string();

        // BTreeMap keyed by folder name: device ("ios") before simulator.
        let mut by_platform: BTreeMap<String, Vec<Utf8PathBuf>> = BTreeMap::new();
        let mut dylibs = Vec::new();
        for target in &targets {
            if !self.common_args.no_cargo {
                cargo_build(
                    &manifest_path,
                    target,
                    &ios.cargo_extras,
                    &config.jsi2.min_ios_version,
                    profile,
                    &rust_dir,
                )?;
            }
            let dylib = metadata.library_path(Some(&target.triple), profile, Some(true));
            by_platform
                .entry(target.platform.lib_folder_name().to_string())
                .or_default()
                .push(dylib.clone());
            dylibs.push(dylib);
        }
        dylibs.sort();

        let stage = metadata.target_dir().join("jsi2").join("ios");
        let mut frameworks = Vec::new();
        for (folder, slices) in by_platform {
            let dir = stage.join(&folder);
            rm_dir(&dir)?;
            mk_dir(&dir)?;
            let binary = if slices.len() == 1 {
                slices[0].clone()
            } else {
                lipo(&slices, &dir.join(format!("lib{lib_name}.dylib")))?
            };
            frameworks.push(make_framework(&dir, &binary, &lib_name, &folder, &config)?);
        }

        if !self.no_xcodebuild {
            let ios_dir = ios.directory(config.project_root());
            mk_dir(&ios_dir)?;
            let out = ios_dir.join(format!("{lib_name}.xcframework"));
            rm_dir(&out)?;
            let mut cmd = Command::new("xcodebuild");
            cmd.arg("-create-xcframework");
            for framework in &frameworks {
                cmd.arg("-framework").arg(framework);
            }
            cmd.arg("-output")
                .arg(&out)
                .args(ios.xcodebuild_extras.clone());
            run_cmd(&mut cmd)?;
        }

        Ok(dylibs)
    }

    pub(crate) fn project_config(&self) -> Result<ProjectConfig> {
        let mut config: ProjectConfig = self.config.clone().try_into()?;
        if !self.targets.is_empty() {
            config.ios.targets = self.targets.clone();
        }
        Ok(config)
    }
}

fn cargo_build(
    manifest_path: &Utf8Path,
    target: &Target,
    cargo_extras: &ExtraArgs,
    min_ios_version: &str,
    profile: &str,
    rust_dir: &Utf8Path,
) -> Result<()> {
    let mut cmd = Command::new("cargo");
    cmd.arg("build")
        .arg("--manifest-path")
        .arg(manifest_path)
        .arg("--target")
        .arg(&target.triple);
    if profile != "debug" {
        cmd.args(["--profile", profile]);
    }
    cmd.args(cargo_extras.clone());
    // rustc stamps this into the dylib's load command; the framework's
    // Info.plist says the same, so the two never disagree at embed time.
    cmd.env("IPHONEOS_DEPLOYMENT_TARGET", min_ios_version);
    run_cmd(cmd.current_dir(rust_dir))?;
    Ok(())
}

fn lipo(slices: &[Utf8PathBuf], output: &Utf8Path) -> Result<Utf8PathBuf> {
    let mut cmd = Command::new("lipo");
    cmd.arg("-create");
    for slice in slices {
        cmd.arg(slice);
    }
    cmd.arg("-output").arg(output);
    run_cmd(&mut cmd)?;
    Ok(output.to_path_buf())
}

/// `<dir>/<name>.framework/{<name>, Info.plist}`: the bundle CocoaPods embeds
/// and signs. The install name is the path the app's loader looks up.
fn make_framework(
    dir: &Utf8Path,
    binary: &Utf8Path,
    lib_name: &str,
    folder: &str,
    config: &ProjectConfig,
) -> Result<Utf8PathBuf> {
    let framework = dir.join(format!("{lib_name}.framework"));
    mk_dir(&framework)?;
    let executable = framework.join(lib_name);
    cp_file(binary, &executable)?;
    let mut cmd = Command::new("install_name_tool");
    cmd.arg("-id")
        .arg(format!("@rpath/{lib_name}.framework/{lib_name}"))
        .arg(&executable);
    run_cmd(&mut cmd)?;
    write_file(
        framework.join("Info.plist"),
        info_plist(lib_name, folder, config),
    )?;
    Ok(framework)
}

fn info_plist(lib_name: &str, folder: &str, config: &ProjectConfig) -> String {
    let jsi2 = &config.jsi2;
    // Bundle identifiers allow only alphanumerics, '-' and '.'; cdylib names
    // are snake_case.
    let bundle_id = format!("{}.{}", jsi2.bundle_id_prefix(), lib_name.replace('_', "-"));
    let version = config.project_version();
    let min = &jsi2.min_ios_version;
    let platform = supported_platform(folder);
    format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
	<key>CFBundleDevelopmentRegion</key><string>en</string>
	<key>CFBundleExecutable</key><string>{lib_name}</string>
	<key>CFBundleIdentifier</key><string>{bundle_id}</string>
	<key>CFBundleInfoDictionaryVersion</key><string>6.0</string>
	<key>CFBundleName</key><string>{lib_name}</string>
	<key>CFBundlePackageType</key><string>FMWK</string>
	<key>CFBundleShortVersionString</key><string>{version}</string>
	<key>CFBundleSupportedPlatforms</key><array><string>{platform}</string></array>
	<key>CFBundleVersion</key><string>1</string>
	<key>MinimumOSVersion</key><string>{min}</string>
</dict>
</plist>
"#
    )
}

/// The SDK name Xcode stamps into a framework it builds; App Store validation
/// expects an embedded framework to name the platform it was built for.
fn supported_platform(folder: &str) -> &'static str {
    match folder {
        f if f == Platform::IosSimulator.lib_folder_name() => "iPhoneSimulator",
        f if f == Platform::Macos.lib_folder_name() => "MacOSX",
        _ => "iPhoneOS",
    }
}
