use anyhow::{Context, Result};
use camino::{Utf8Path, Utf8PathBuf};
use std::env;

pub fn repository_root() -> Result<Utf8PathBuf> {
    let dir = env::var("CARGO_MANIFEST_DIR").context("failed to get manifest dir")?;
    Ok(Utf8Path::new(&*dir).parent().unwrap().to_path_buf())
}

pub fn build_root() -> Result<Utf8PathBuf> {
    let dir = repository_root()?;
    Ok(dir.join("build"))
}

pub fn cpp_modules() -> Result<Utf8PathBuf> {
    let dir = repository_root()?;
    Ok(dir.join("cpp_modules"))
}

pub(crate) fn so_extension_name() -> &'static str {
    if cfg!(target_os = "windows") {
        "dll"
    } else if cfg!(target_os = "macos") {
        "dylib"
    } else if cfg!(target_os = "unix") {
        "so"
    } else {
        unimplemented!("Building only on windows, macos and unix supported right now")
    }
}

pub(crate) fn find<P: AsRef<Utf8Path>>(base: P, filename: &str) -> Option<Utf8PathBuf> {
    let path = glob::glob(&format!("{base}/**/{filename}", base = base.as_ref()))
        .unwrap()
        .find_map(Result::ok)?;
    let path: Utf8PathBuf = path.try_into().unwrap_or_else(|_| panic!("not a utf path"));
    Some(path)
}
