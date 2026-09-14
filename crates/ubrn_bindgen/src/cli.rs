/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/
 */

use std::collections::{HashMap, HashSet};

use anyhow::Result;
use camino::{Utf8Path, Utf8PathBuf};
use clap::Args;

use ubrn_common::{mk_dir, path_or_shim, CrateMetadata, Utf8PathBufExt as _};
use uniffi_bindgen::{
    cargo_metadata::CrateConfigSupplier,
    pipeline::{general, initial},
    BindgenLoader, BindgenPaths, BindgenPathsLayer, ComponentInterface, GlobalConfig,
};

#[cfg(feature = "wasm")]
use super::{bindings::gen_rust, wasm::generate_rs, wasm_metadata};
use super::{
    bindings::{gen_cpp, gen_typescript, metadata::ModuleMetadata},
    react_native::generate_cpp,
    switches::{AbiFlavor, SwitchArgs},
};

#[derive(Args, Debug)]
pub struct BindingsArgs {
    #[command(flatten)]
    pub(crate) source: SourceArgs,
    #[command(flatten)]
    pub(crate) output: OutputArgs,
    #[command(flatten)]
    switches: SwitchArgs,

    /// Set by the napi CLI; required for the `Napi` flavor (other flavors ignore it).
    #[clap(skip)]
    pub(crate) lib_resolution: Option<gen_typescript::ffi_module_player::LibResolution>,
}

impl BindingsArgs {
    pub fn new(switches: SwitchArgs, source: SourceArgs, output: OutputArgs) -> Self {
        Self {
            switches,
            source,
            output,
            lib_resolution: None,
        }
    }

    pub fn with_lib_resolution(
        mut self,
        resolution: gen_typescript::ffi_module_player::LibResolution,
    ) -> Self {
        self.lib_resolution = Some(resolution);
        self
    }

    pub fn ts_dir(&self) -> &Utf8Path {
        &self.output.ts_dir
    }

    pub fn cpp_dir(&self) -> &Utf8Path {
        &self.output.cpp_dir
    }

    pub fn switches(&self) -> SwitchArgs {
        self.switches.clone()
    }
}

#[derive(Args, Clone, Debug)]
pub struct OutputArgs {
    /// By default, bindgen will attempt to format the code with prettier and clang-format.
    #[clap(long)]
    pub(crate) no_format: bool,

    /// The directory in which to put the generated Typescript.
    #[clap(long)]
    pub(crate) ts_dir: Utf8PathBuf,

    /// The directory in which to put the generated C++.
    #[clap(long, alias = "abi-dir")]
    pub(crate) cpp_dir: Utf8PathBuf,
}

impl OutputArgs {
    pub fn new(ts_dir: &Utf8Path, cpp_dir: &Utf8Path, no_format: bool) -> Self {
        Self {
            ts_dir: ts_dir.to_owned(),
            cpp_dir: cpp_dir.to_owned(),
            no_format,
        }
    }
}

#[derive(Args, Clone, Debug, Default)]
pub struct SourceArgs {
    /// The path to a dynamic library to attempt to extract the definitions from
    /// and extend the component interface with.
    #[clap(long)]
    pub(crate) lib_file: Option<Utf8PathBuf>,

    /// Override the default crate name that is guessed from UDL file path.
    #[clap(long = "crate")]
    pub(crate) crate_name: Option<String>,

    /// The location of the uniffi.toml file
    #[clap(long)]
    pub(crate) config: Option<Utf8PathBuf>,

    /// Treat the input file as a library, extracting any Uniffi definitions from that.
    #[clap(long = "library", conflicts_with = "lib_file")]
    pub(crate) library_mode: bool,

    /// A UDL file or library file
    pub(crate) source: Utf8PathBuf,
}

impl SourceArgs {
    pub fn library(file: &Utf8PathBuf) -> Self {
        Self {
            library_mode: true,
            source: file.clone(),
            ..Default::default()
        }
    }

    /// Returns the source path (a UDL file or library file).
    pub fn source(&self) -> &Utf8PathBuf {
        &self.source
    }

    pub fn with_config(self, config: Option<Utf8PathBuf>) -> Self {
        Self {
            config,
            library_mode: self.library_mode,
            source: self.source,
            lib_file: self.lib_file,
            crate_name: self.crate_name,
        }
    }
}

impl BindingsArgs {
    pub fn run(&self, manifest_path: Option<&Utf8PathBuf>) -> Result<Vec<ModuleMetadata>> {
        let out = &self.output;

        mk_dir(&out.ts_dir)?;
        mk_dir(&out.cpp_dir)?;
        let ts_dir = out.ts_dir.canonicalize_utf8_or_shim()?;
        let abi_dir = out.cpp_dir.canonicalize_utf8_or_shim()?;
        let switches = self.switches();

        let source_path = path_or_shim(&self.source.source)?;
        let loader = self.create_loader(manifest_path)?;

        // TypeScript generation via pipeline.
        //
        // Every namespace gets its own crate's `uniffi.toml` (e.g. custom type
        // mappings), so build the initial root with the same loader the native
        // generators use. `run_typescript_pipeline` publishes each crate's config
        // as `[bindings.react-native]`, the table the 0.32 pipeline reads.
        let metadata = loader.load_metadata(&source_path)?;
        let initial_root = loader.load_pipeline_initial_root(&source_path, metadata)?;
        let explicit_discr_enums = collect_explicit_discr_enums(&initial_root);
        let general_root = run_typescript_pipeline(initial_root)?;

        // Build the api modules — which is also where the pipeline's validation
        // lives — before any native code is written. The native generators cannot
        // express everything the pipeline accepts (borrowed-bytes arguments are only
        // safe on synchronous Rust calls, see `validate_borrowed_bytes`), and the JSI
        // C++ generator runs first, so an unsupported binding must be rejected here
        // rather than after C++ has been emitted. The files themselves are still
        // written, below, after native generation.
        let api_modules = build_api_modules(&general_root, &switches, &explicit_discr_enums)?;

        // C++/Rust generation via ComponentInterface
        match &switches.flavor {
            AbiFlavor::Jsi => {
                let metadata = load_metadata(&loader, &source_path)?;
                let cis = loader.load_cis(metadata)?;
                let mut components = loader.load_components(cis, parse_cpp_config)?;
                for c in components.iter_mut() {
                    c.ci.derive_ffi_funcs()?;
                }
                generate_cpp(&components, &abi_dir, !out.no_format)?;
            }
            AbiFlavor::Napi => { /* No C++ generation for Napi */ }
            #[cfg(feature = "wasm")]
            AbiFlavor::Wasm => {
                let metadata = load_metadata(&loader, &source_path)?;
                let cis = loader.load_cis(metadata)?;
                let mut components = loader.load_components(cis, parse_rust_config)?;
                for c in components.iter_mut() {
                    c.ci.derive_ffi_funcs()?;
                }
                generate_rs(&components, &switches, &abi_dir, !out.no_format)?;
            }
            #[cfg(feature = "wasm")]
            AbiFlavor::Wasm2 => { /* No native shim for Wasm2 */ }
        }

        // Now write the TypeScript that the pipeline root describes: the low-level
        // module first, then the api modules built above.
        generate_ffi_from_pipeline(
            &general_root,
            &switches,
            &ts_dir,
            self.lib_resolution.clone(),
        )?;
        let modules = write_api_modules(api_modules, &ts_dir)?;
        if switches.flavor.supports_index_ts_at_generation() {
            generate_index_from_modules(&modules, &general_root, &switches, &ts_dir, &source_path)?;
        }
        if !out.no_format {
            gen_typescript::format_directory(&ts_dir)?;
        }
        Ok(modules)
    }

    fn create_loader(&self, manifest_path: Option<&Utf8PathBuf>) -> Result<BindgenLoader> {
        let mut bindgen_paths = BindgenPaths::default();
        let global_config = load_global_config(&mut bindgen_paths, self.source.config.as_deref())?;
        let cwd = Utf8PathBuf::from("Cargo.toml");
        let manifest_path = manifest_path.unwrap_or(&cwd);
        let cargo_metadata = CrateMetadata::cargo_metadata(manifest_path)?;
        let config_supplier = CrateConfigSupplier::from(cargo_metadata);
        bindgen_paths.add_layer(config_supplier);
        Ok(BindgenLoader::new(bindgen_paths, global_config))
    }
}

/// A [BindgenPathsLayer] that always resolves to the same config file, regardless
/// of crate name. Used to implement the `--config` CLI flag, which points at a
/// single `uniffi.toml`-style file to use for every crate.
struct ConfigOverrideLayer {
    path: Utf8PathBuf,
}

impl BindgenPathsLayer for ConfigOverrideLayer {
    fn get_config_path(&self, _crate_name: &str) -> Option<Utf8PathBuf> {
        Some(self.path.clone())
    }
}

fn load_global_config(paths: &mut BindgenPaths, path: Option<&Utf8Path>) -> Result<GlobalConfig> {
    let Some(path) = path else {
        return Ok(GlobalConfig::default());
    };
    let raw: toml::Table = toml::from_str(&std::fs::read_to_string(path)?)?;
    if ["crate-roots", "defaults", "crates"]
        .iter()
        .any(|key| raw.contains_key(*key))
    {
        let (config, roots) = GlobalConfig::from_file(path)?;
        if let Some(roots) = roots {
            paths.add_layer(roots);
        }
        Ok(config)
    } else {
        paths.add_layer(ConfigOverrideLayer {
            path: path.to_owned(),
        });
        Ok(GlobalConfig::default())
    }
}

fn run_typescript_pipeline(mut root: initial::Root) -> Result<general::Root> {
    for namespace in root.namespaces.values_mut() {
        let mut config: toml::Table =
            toml::from_str(namespace.config_toml.as_deref().unwrap_or_default())?;
        let bindings = gen_typescript::Config::bindings_table(&config)?;
        config
            .entry("bindings")
            .or_insert_with(|| toml::Value::Table(toml::Table::new()))
            .as_table_mut()
            .unwrap()
            .insert("react-native".into(), toml::Value::Table(bindings));
        namespace.config_toml = Some(toml::to_string(&config)?);
    }
    let mut context = general::Context::new("react-native");
    context.update_from_root(&root)?;
    let root = general::pipeline("react-native").execute(root)?;
    initial::MapNode::map_node(root, &BoxRenameFix(context.rename_tables))
}

struct BoxRenameFix(HashMap<String, toml::Table>);

impl BoxRenameFix {
    fn fix_type(&self, ty: &mut general::Type, inside_box: bool) {
        use general::Type;
        match ty {
            Type::Box { inner_type } => self.fix_type(inner_type, true),
            Type::Optional { inner_type }
            | Type::Sequence { inner_type }
            | Type::Set { inner_type } => self.fix_type(inner_type, inside_box),
            Type::Map {
                key_type,
                value_type,
            } => {
                self.fix_type(key_type, inside_box);
                self.fix_type(value_type, inside_box);
            }
            Type::Record {
                namespace,
                name,
                orig_name,
            }
            | Type::Enum {
                namespace,
                name,
                orig_name,
            }
            | Type::Interface {
                namespace,
                name,
                orig_name,
                ..
            }
            | Type::CallbackInterface {
                namespace,
                name,
                orig_name,
            }
            | Type::Custom {
                namespace,
                name,
                orig_name,
                ..
            } if inside_box => {
                if let Some(renamed) = self
                    .0
                    .get(namespace)
                    .and_then(|table| table.get(orig_name))
                    .and_then(toml::Value::as_str)
                {
                    *name = renamed.to_owned();
                }
            }
            _ => {}
        }
    }
}

impl initial::MapNode<general::TypeNode, BoxRenameFix> for general::TypeNode {
    fn map_node(mut self, context: &BoxRenameFix) -> Result<Self> {
        context.fix_type(&mut self.ty, false);
        Ok(self)
    }
}

impl initial::MapNode<general::BuiltinTypes, BoxRenameFix> for general::BuiltinTypes {
    fn map_node(self, context: &BoxRenameFix) -> Result<Self> {
        Ok(Self {
            u8: initial::MapNode::map_node(self.u8, context)?,
            i8: initial::MapNode::map_node(self.i8, context)?,
            u16: initial::MapNode::map_node(self.u16, context)?,
            i16: initial::MapNode::map_node(self.i16, context)?,
            u32: initial::MapNode::map_node(self.u32, context)?,
            i32: initial::MapNode::map_node(self.i32, context)?,
            u64: initial::MapNode::map_node(self.u64, context)?,
            i64: initial::MapNode::map_node(self.i64, context)?,
            f32: initial::MapNode::map_node(self.f32, context)?,
            f64: initial::MapNode::map_node(self.f64, context)?,
            string: initial::MapNode::map_node(self.string, context)?,
        })
    }
}

impl initial::MapNode<general::Argument, BoxRenameFix> for general::Argument {
    fn map_node(self, context: &BoxRenameFix) -> Result<Self> {
        Ok(Self {
            name: self.name,
            orig_name: self.orig_name,
            ty: initial::MapNode::map_node(self.ty, context)?,
            by_ref: self.by_ref,
            optional: self.optional,
            default: initial::MapNode::map_node(self.default, context)?,
        })
    }
}

macro_rules! map_box_rename_children {
    ($($ty:ident),* $(,)?) => {
        $(impl initial::MapNode<general::$ty, BoxRenameFix> for general::$ty {
            fn map_node(self, context: &BoxRenameFix) -> Result<Self> {
                self.uniffi_auto_map_node(context)
            }
        })*
    };
}

map_box_rename_children!(
    Root,
    Namespace,
    Function,
    TypeDefinition,
    Constructor,
    Method,
    Callable,
    CallableKind,
    ReturnType,
    ThrowsType,
    AsyncData,
    DefaultValue,
    Literal,
    Record,
    FieldsKind,
    Field,
    Enum,
    Variant,
    Interface,
    CallbackInterface,
    VTable,
    VTableMethod,
    ObjectTraitImpl,
    CustomType,
    BoxedType,
    OptionalType,
    SequenceType,
    MapType,
    SetType,
    ExternalType,
    FfiDefinition,
    RustFfiFunctionName,
    FfiStructName,
    FfiFunctionTypeName,
    FfiFunction,
    FfiFunctionKind,
    FfiFunctionType,
    FfiReturnType,
    FfiStruct,
    FfiField,
    FfiArgument,
    FfiType,
    HandleKind,
    Checksum,
    UniffiTraitMethods,
    ObjectImpl,
    EnumShape,
    Radix,
    TraitKind,
);

/// Namespace name -> orig_names of enums that declared an explicit `#[repr(...)]`
/// discriminant type in the Rust source.
///
/// This is only available from the `initial` pipeline IR: the `general` pipeline
/// collapses "explicit repr" and "inferred repr" enums down to the same
/// `Enum::discr_type: TypeNode`, so we capture the distinction up front.
type ExplicitDiscrEnums = HashMap<String, HashSet<String>>;

fn collect_explicit_discr_enums(root: &initial::Root) -> ExplicitDiscrEnums {
    root.namespaces
        .iter()
        .map(|(name, namespace)| {
            let enums = namespace
                .type_definitions
                .iter()
                .filter_map(|td| match td {
                    initial::TypeDefinition::Enum(e) if e.discr_type.is_some() => {
                        Some(e.orig_name.clone())
                    }
                    _ => None,
                })
                .collect();
            (name.clone(), enums)
        })
        .collect()
}

/// An api module built but not yet written to disk.
type BuiltApiModules = Vec<(ModuleMetadata, gen_typescript::api_module::TsApiModule)>;

/// Build the api module for every namespace.
///
/// Building is also validating: `TsApiModule::from_general` rejects bindings the
/// native generators cannot express safely (borrowed-bytes arguments in async or
/// callback calls, `forceAsync` over synchronous methods). Callers run this before
/// the native generators so such a binding fails before any code is emitted.
fn build_api_modules(
    general_root: &general::Root,
    switches: &SwitchArgs,
    explicit_discr_enums: &ExplicitDiscrEnums,
) -> Result<BuiltApiModules> {
    let empty = HashSet::new();
    let mut modules = Vec::new();
    for (name, namespace) in &general_root.namespaces {
        let config = extract_ts_config(namespace)?;
        let module = ModuleMetadata::new(name);
        let ffi_module = gen_typescript::ffi_module::TsFfiModule::from_general(
            namespace,
            &switches.flavor,
            &config,
        );
        let ffi_exports = ffi_module.exported_names();
        let explicit_discr_enums = explicit_discr_enums.get(name).unwrap_or(&empty);
        let api_module = gen_typescript::api_module::TsApiModule::from_general(
            &config,
            namespace,
            switches.flavor.clone(),
            ffi_exports,
            explicit_discr_enums,
        )?;
        modules.push((module, api_module));
    }
    Ok(modules)
}

fn write_api_modules(modules: BuiltApiModules, ts_dir: &Utf8Path) -> Result<Vec<ModuleMetadata>> {
    let mut written = Vec::new();
    for (module, api_module) in modules {
        let code = gen_typescript::generate_api_code_from_ir(api_module)?;
        let path = ts_dir.join(module.ts_filename());
        ubrn_common::write_file(path, code)?;
        written.push(module);
    }
    Ok(written)
}

fn generate_index_from_modules(
    modules: &[ModuleMetadata],
    general_root: &general::Root,
    switches: &SwitchArgs,
    ts_dir: &Utf8Path,
    source_path: &Utf8Path,
) -> Result<()> {
    // For wasm2 the source *is* the module, and staging copies it beside the
    // generated TypeScript under the same name.
    let wasm_stem = source_path.file_stem().unwrap_or_default().to_string();
    // Staging will run wasm-bindgen over a module that imports its placeholder
    // namespace, leaving a `<stem>_bg.js` beside the wasm. Ask the same
    // question here so the index imports exactly what staging produces.
    #[cfg(feature = "wasm")]
    let has_wasm_bindgen_glue = switches.flavor.is_wasm2()
        && ubrn_common::has_wasm_bindgen_imports(source_path).unwrap_or(false);
    // Without the feature only Napi reaches here, and it stages no wasm.
    #[cfg(not(feature = "wasm"))]
    let has_wasm_bindgen_glue = false;
    // Only when every namespace opts in; the index re-exports all of them.
    let mut strict_type_checking = !general_root.namespaces.is_empty();
    for namespace in general_root.namespaces.values() {
        strict_type_checking &= extract_ts_config(namespace)?.strict_type_checking;
    }
    let code = gen_typescript::generate_index_code(
        modules.to_vec(),
        switches.flavor.clone(),
        wasm_stem,
        has_wasm_bindgen_glue,
        strict_type_checking,
    )?;
    let path = ts_dir.join("index.ts");
    ubrn_common::write_file(path, code)?;
    Ok(())
}

fn extract_ts_config(namespace: &general::Namespace) -> Result<gen_typescript::Config> {
    gen_typescript::Config::from_root(namespace.config_toml.as_deref())
}

fn generate_ffi_from_pipeline(
    root: &general::Root,
    switches: &SwitchArgs,
    ts_dir: &Utf8Path,
    lib_resolution: Option<gen_typescript::ffi_module_player::LibResolution>,
) -> Result<()> {
    for (name, namespace) in &root.namespaces {
        let module = ModuleMetadata::new(name);
        let path = ts_dir.join(module.ts_ffi_filename());

        let config = extract_ts_config(namespace)?;
        let code = match &switches.flavor {
            AbiFlavor::Napi => {
                let lib_resolution = lib_resolution.clone().ok_or_else(|| {
                    anyhow::anyhow!(
                        "napi codegen requires a LibResolution; pass --lib-colocated, --lib-absolute, or --lib-package-base"
                    )
                })?;
                let crate_name = namespace.crate_name.clone();
                let player_module =
                    gen_typescript::ffi_module_player::PlayerFfiModule::from_general(
                        namespace,
                        &config,
                        &switches.flavor,
                        crate_name,
                        Some(lib_resolution),
                    );
                gen_typescript::generate_player_lowlevel_code(player_module)?
            }
            #[cfg(feature = "wasm")]
            AbiFlavor::Wasm2 => {
                let crate_name = namespace.crate_name.clone();
                let player_module =
                    gen_typescript::ffi_module_player::PlayerFfiModule::from_general(
                        namespace,
                        &config,
                        &switches.flavor,
                        crate_name,
                        None,
                    );
                gen_typescript::generate_player_lowlevel_code(player_module)?
            }
            _ => {
                let ffi_module = gen_typescript::ffi_module::TsFfiModule::from_general(
                    namespace,
                    &switches.flavor,
                    &config,
                );
                gen_typescript::generate_lowlevel_code(ffi_module)?
            }
        };

        ubrn_common::write_file(path, code)?;
    }
    Ok(())
}

/// Load metadata, transparently handling `wasm32-unknown-unknown` cdylibs.
///
/// When the `wasm` feature is enabled, this peeks at the source file and, if
/// it looks like a WebAssembly module, extracts UNIFFI_META_* blobs directly
/// from its globals + data segments instead of falling through to the native
/// dylib symbol-table reader. Non-wasm sources (UDL, native libraries) take
/// the unchanged default path.
fn load_metadata(
    loader: &uniffi_bindgen::BindgenLoader,
    source_path: &Utf8Path,
) -> Result<uniffi_meta::MetadataGroupMap> {
    #[cfg(feature = "wasm")]
    {
        loader.load_metadata_specialized(source_path, |_path, bytes| {
            if wasm_metadata::looks_like_wasm(bytes) {
                Ok(Some(wasm_metadata::extract_from_wasm_bytes(bytes)?))
            } else {
                Ok(None)
            }
        })
    }
    #[cfg(not(feature = "wasm"))]
    {
        loader.load_metadata(source_path)
    }
}

fn parse_cpp_config(_ci: &ComponentInterface, toml: toml::Value) -> Result<gen_cpp::Config> {
    match toml.get("bindings").and_then(|b| b.get("cpp")) {
        Some(v) => Ok(v.clone().try_into()?),
        None => Ok(Default::default()),
    }
}

#[cfg(feature = "wasm")]
fn parse_rust_config(_ci: &ComponentInterface, toml: toml::Value) -> Result<gen_rust::Config> {
    let value = toml
        .get("bindings")
        .and_then(|b| b.get("rust").or_else(|| b.get("rs")));
    match value {
        Some(v) => Ok(v.clone().try_into()?),
        None => Ok(Default::default()),
    }
}
