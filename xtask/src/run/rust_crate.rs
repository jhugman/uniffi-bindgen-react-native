use crate::util::{run_cmd_quietly, so_extension_name};

use std::process::Command;

use camino::Utf8PathBuf;
use cargo_metadata::MetadataCommand;
use clap::Args;

use anyhow::Result;

#[derive(Debug, Args)]
pub(crate) struct CrateArg {
    /// The path to the crate
    #[clap(long = "crate")]
    pub(crate) crate_dir: Option<Utf8PathBuf>,

    /// Build for release
    #[clap(long, requires = "crate_dir", default_value = "false")]
    pub(crate) release: bool,

    /// Do not invoke cargo build.
    ///
    /// This is useful when invoking from within a test.
    #[clap(long, requires = "crate_dir", conflicts_with_all = ["clean"], default_value = "false")]
    pub(crate) no_cargo: bool,
}

impl CrateArg {
    pub(crate) fn cargo_build(&self, clean: bool) -> Result<CrateInfo> {
        let crate_info = CrateInfo::try_from(self.crate_dir.clone().expect("crate has no path"))?;
        let lib_path = crate_info.library_path(self.release);
        if lib_path.exists() && clean {
            crate_info.cargo_clean()?;
        }
        if !lib_path.exists() || !self.no_cargo {
            crate_info.cargo_build(self.release)?;
        }
        Ok(crate_info)
    }
}

pub(crate) struct CrateInfo {
    pub(crate) crate_dir: Utf8PathBuf,
    #[allow(unused)]
    pub(crate) manifest_path: Utf8PathBuf,
    pub(crate) target_dir: Utf8PathBuf,
    pub(crate) library_name: String,
}

impl TryFrom<Utf8PathBuf> for CrateInfo {
    type Error = anyhow::Error;

    fn try_from(arg: Utf8PathBuf) -> Result<Self> {
        let crate_dir = arg.canonicalize_utf8()?;
        let manifest_path = crate_dir.join("Cargo.toml");
        if !manifest_path.exists() {
            anyhow::bail!("Crate manifest doesn't exist");
        }
        // Run `cargo metadata`
        let metadata = MetadataCommand::new()
            .current_dir(&crate_dir)
            .manifest_path(&manifest_path)
            .exec()?;

        // Get the library name
        let lib = "lib".to_owned();
        let library_name = metadata
            .packages
            .iter()
            .find(|package| package.manifest_path == *manifest_path)
            .and_then(|package| {
                package
                    .targets
                    .iter()
                    .find(|target| target.kind.contains(&lib))
            })
            .map(|target| target.name.clone())
            .expect("The crate isn't a library: it needs a [lib] section in Cargo.toml");

        let target_dir = metadata.target_directory;

        Ok(Self {
            crate_dir,
            manifest_path,
            library_name,
            target_dir,
        })
    }
}

impl CrateInfo {
    pub(crate) fn library_path(&self, is_release: bool) -> Utf8PathBuf {
        let lib_name = &self.library_name;
        let ext = so_extension_name();

        let build_type = if is_release { "release" } else { "debug" };
        let lib_name = format!("lib{lib_name}.{ext}");
        let target = &self.target_dir;
        target.join(build_type).join(lib_name)
    }

    pub(crate) fn cargo_clean(&self) -> Result<()> {
        let mut cmd = Command::new("cargo");
        run_cmd_quietly(cmd.arg("clean").current_dir(&self.crate_dir))?;
        Ok(())
    }

    pub(crate) fn cargo_build(&self, release: bool) -> Result<()> {
        let mut cmd = Command::new("cargo");
        cmd.current_dir(&self.crate_dir);
        cmd.arg("build");
        if release {
            cmd.arg("--release");
        }
        run_cmd_quietly(&mut cmd)?;
        Ok(())
    }
}
