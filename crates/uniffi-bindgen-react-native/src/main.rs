mod bindings;

use camino::Utf8PathBuf;
use clap::Parser;

use bindings::react_native::ReactNativeBindingGenerator;

#[derive(Parser)]
struct Args {
    #[clap(long, short)]
    config: Option<Utf8PathBuf>,
    #[clap(long, short)]
    out_dir: Option<Utf8PathBuf>,
    #[clap(long)]
    no_format: bool,
    #[clap(long)]
    lib_file: Option<Utf8PathBuf>,
    #[clap(long = "library", conflicts_with_all = ["config", "lib_file"], requires = "out_dir")]
    library_mode: bool,
    #[clap(long = "scaffolding")]
    scaffolding_mode: bool,
    #[clap(long = "crate")]
    crate_name: Option<String>,
    source: Utf8PathBuf,
}

fn main() {
    let args = Args::parse();

    let ts = ReactNativeBindingGenerator;

    if args.library_mode {
        uniffi_bindgen::library_mode::generate_external_bindings(
            &ts,
            &args.source,
            args.crate_name,
            args.config.as_deref(),
            &args.out_dir.unwrap(),
            !args.no_format,
        )
        .unwrap();
    } else {
        uniffi_bindgen::generate_external_bindings(
            &ts,
            args.source,
            args.config.as_deref(),
            args.out_dir,
            args.lib_file,
            args.crate_name.as_deref(),
            !args.no_format,
        )
        .unwrap();
    }
}
