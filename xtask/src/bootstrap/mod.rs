mod hermes;
mod test_runner;

use anyhow::Result;
use camino::Utf8PathBuf;
use clap::{Args, Subcommand};

use crate::clean::CleanCmd;

pub(crate) use self::hermes::HermesCmd;
pub(crate) use self::test_runner::TestRunnerCmd;

#[derive(Debug, Args)]
pub(crate) struct BootstrapCmd {
    /// Run the clean command immediately before
    #[clap(long, short = 'f', aliases = ["clean"])]
    force: bool,

    #[clap(subcommand)]
    cmd: Option<SubsystemCmd>,
}

impl BootstrapCmd {
    pub(crate) fn run(&self) -> Result<()> {
        let clean = self.force;

        if let Some(cmd) = &self.cmd {
            match cmd {
                SubsystemCmd::Hermes(c) => c.bootstrap(clean)?,
                SubsystemCmd::TestRunner(c) => c.bootstrap(clean)?,
            }
        } else {
            if clean {
                CleanCmd.run()?;
            }
            Self::prepare_all()?;
        }
        Ok(())
    }

    pub(crate) fn prepare_all() -> Result<()> {
        HermesCmd::default().bootstrap(false)?;
        TestRunnerCmd.bootstrap(false)?;
        Ok(())
    }

    pub(crate) fn clean_all() -> Result<()> {
        TestRunnerCmd::clean()?;
        HermesCmd::clean()?;
        Ok(())
    }
}

#[derive(Debug, Subcommand)]
enum SubsystemCmd {
    /// Facebook's Javascript engine now used as default for React Native apps.
    ///
    /// This command clones and compiles a copy for testing on the desktop.
    Hermes(HermesCmd),

    /// The C++ test runner that takes Javascript and .so libraries and runs them against
    /// Hermes.
    TestRunner(TestRunnerCmd),
}

pub(crate) trait Bootstrap {
    fn marker() -> Result<Utf8PathBuf>;
    fn clean() -> Result<()>;
    fn prepare(&self) -> Result<()>;

    fn bootstrap(&self, clean: bool) -> Result<()> {
        if clean {
            Self::clean()?;
        }
        self.ensure_ready()?;
        Ok(())
    }

    fn ensure_ready(&self) -> Result<()> {
        if !Self::marker()?.exists() {
            self.prepare()?;
        }
        Ok(())
    }
}
