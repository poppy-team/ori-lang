//! Small, content-addressed cache for repeated native builds.
//!
//! The cache deliberately stores only the build fingerprint and output paths;
//! compiler IR stays in memory and is never deserialised across versions.  A
//! compiler version change, source change, manifest change, or option change
//! therefore causes a normal rebuild automatically.

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};

const CACHE_FORMAT: u32 = 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BuildOptions {
    pub shared: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CacheHit {
    pub output: PathBuf,
    pub header: Option<PathBuf>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModuleChange {
    pub path: PathBuf,
    pub changed: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModuleArtifact {
    pub path: PathBuf,
    pub fingerprint: String,
    pub object: PathBuf,
}

#[derive(Debug, Serialize, Deserialize)]
struct CacheRecord {
    format: u32,
    compiler: String,
    fingerprint: String,
    output: String,
    header: Option<String>,
    #[serde(default)]
    modules: Vec<ModuleRecord>,
}

#[derive(Debug, Serialize, Deserialize)]
struct ModuleRecord {
    path: String,
    fingerprint: String,
    #[serde(default)]
    artifact: Option<String>,
}

/// Return a cache hit when the previously emitted output still matches the
/// source graph and compiler options.
pub fn try_reuse(
    source_path: &Path,
    output: &Path,
    options: BuildOptions,
) -> Result<Option<CacheHit>, String> {
    if cache_disabled() {
        return Ok(None);
    }
    let root = project_root(source_path);
    // Without a lockfile, registry/Git resolution can change outside the
    // project tree and the lightweight cache cannot prove which checkout was
    // used.  Require `ori lock` before reusing a dependency-bearing build.
    if !root.join("ori.lock").is_file() && manifest_declares_dependencies(&root) {
        return Ok(None);
    }
    let record_path = cache_record_path(&root);
    if !record_path.is_file() {
        return Ok(None);
    }
    let source = fs::read_to_string(&record_path).map_err(|err| {
        format!(
            "incremental.cache_read_failed: cannot read `{}`: {err}",
            record_path.display()
        )
    })?;
    let record: CacheRecord = serde_json::from_str(&source).map_err(|err| {
        format!(
            "incremental.cache_invalid: cannot parse `{}`: {err}",
            record_path.display()
        )
    })?;
    if record.format != CACHE_FORMAT
        || record.compiler != env!("CARGO_PKG_VERSION")
        || record.output != normalize_path(output)
        || record.fingerprint != fingerprint(source_path, options)?
    {
        return Ok(None);
    }
    let output_path = PathBuf::from(&record.output);
    if !output_path.is_file() {
        return Ok(None);
    }
    let header = record.header.map(PathBuf::from);
    if header.as_ref().is_some_and(|path| !path.is_file()) {
        return Ok(None);
    }
    Ok(Some(CacheHit {
        output: output_path,
        header,
    }))
}

/// Persist a successful native build fingerprint.  Failures are returned to
/// the caller because a cache that cannot be written should be visible in CI,
/// while the generated binary itself remains valid.
pub fn record_success(
    source_path: &Path,
    output: &Path,
    header: Option<&Path>,
    options: BuildOptions,
) -> Result<(), String> {
    record_success_with_artifacts(source_path, output, header, &[], options)
}

/// Persist a successful build and the reusable object emitted for each module.
/// Artifact paths are deterministic, so a later build can reuse unchanged
/// modules without trusting stale HIR or compiler-internal serialization.
pub fn record_success_with_artifacts(
    source_path: &Path,
    output: &Path,
    header: Option<&Path>,
    artifacts: &[ModuleArtifact],
    options: BuildOptions,
) -> Result<(), String> {
    if cache_disabled() {
        return Ok(());
    }
    let root = project_root(source_path);
    let cache_dir = root.join(".ori");
    fs::create_dir_all(&cache_dir).map_err(|err| {
        format!(
            "incremental.cache_write_failed: cannot create `{}`: {err}",
            cache_dir.display()
        )
    })?;
    let artifact_by_path = artifacts
        .iter()
        .map(|artifact| {
            (
                normalize_path(&artifact.path),
                normalize_path(&artifact.object),
            )
        })
        .collect::<HashMap<_, _>>();
    let mut modules = module_records(source_path)?;
    for module in &mut modules {
        module.artifact = artifact_by_path.get(&module.path).cloned();
    }
    let record = CacheRecord {
        format: CACHE_FORMAT,
        compiler: env!("CARGO_PKG_VERSION").to_string(),
        fingerprint: fingerprint(source_path, options)?,
        output: normalize_path(output),
        header: header.map(normalize_path),
        modules,
    };
    let body = serde_json::to_vec_pretty(&record)
        .map_err(|err| format!("incremental.cache_serialize_failed: {err}"))?;
    let record_path = cache_record_path(&root);
    let temporary = record_path.with_extension("json.tmp");
    fs::write(&temporary, body).map_err(|err| {
        format!(
            "incremental.cache_write_failed: cannot write `{}`: {err}",
            temporary.display()
        )
    })?;
    fs::rename(&temporary, record_path).map_err(|err| {
        format!(
            "incremental.cache_write_failed: cannot replace `{}`: {err}",
            cache_record_path(&root).display()
        )
    })
}

/// Compare the source modules with the last successful build. This is
/// intentionally separate from `try_reuse`: a project-level hit skips the
/// whole build, while a miss still lets the native pipeline reuse unchanged
/// content-addressed module objects.
pub fn changed_modules(
    source_path: &Path,
    _options: BuildOptions,
) -> Result<Vec<ModuleChange>, String> {
    if cache_disabled() {
        return Ok(Vec::new());
    }
    let root = project_root(source_path);
    let cache_path = cache_record_path(&root);
    let previous = if cache_path.is_file() {
        let source = fs::read_to_string(&cache_path).map_err(|err| {
            format!(
                "incremental.cache_read_failed: cannot read `{}`: {err}",
                cache_path.display()
            )
        })?;
        serde_json::from_str::<CacheRecord>(&source)
            .map_err(|err| format!("incremental.cache_invalid: cannot parse module index: {err}"))?
            .modules
    } else {
        Vec::new()
    };
    let previous = previous
        .into_iter()
        .map(|module| (module.path, module.fingerprint))
        .collect::<HashMap<_, _>>();
    let current = module_records(source_path)?;
    let current_paths = current
        .iter()
        .map(|module| module.path.clone())
        .collect::<HashSet<_>>();
    let mut changes = current
        .into_iter()
        .map(|module| ModuleChange {
            changed: previous
                .get(&module.path)
                .is_none_or(|fingerprint| fingerprint != &module.fingerprint),
            path: PathBuf::from(module.path),
        })
        .collect::<Vec<_>>();
    // Removed modules also invalidate a future per-module object cache, even
    // though they do not appear in the current source scan.
    changes.extend(
        previous
            .into_iter()
            .filter(|(path, _)| !current_paths.contains(path))
            .map(|(path, _)| ModuleChange {
                path: PathBuf::from(path),
                changed: true,
            }),
    );
    changes.sort_by(|left, right| left.path.cmp(&right.path));
    Ok(changes)
}

fn cache_disabled() -> bool {
    std::env::var_os("ORI_DISABLE_INCREMENTAL").is_some()
        || std::env::var_os("ORI_DEBUG_INSTRUMENT").is_some()
        || std::env::var_os("ORI_DEBUG_PORT").is_some()
}

pub fn cache_enabled() -> bool {
    !cache_disabled()
}

/// Fingerprint one source module for artifact reuse.
pub fn module_fingerprint(path: &Path) -> Result<String, String> {
    let bytes = fs::read(path).map_err(|err| {
        format!(
            "incremental.input_read_failed: cannot read `{}`: {err}",
            path.display()
        )
    })?;
    let mut digest = Sha256::new();
    digest.update(bytes);
    Ok(format!("{:x}", digest.finalize()))
}

/// Return the stable object path for a module in the current build context.
/// The interface fingerprint invalidates objects when a public signature or
/// shared type layout changes, while implementation-only edits stay local.
pub fn module_artifact_path(
    source_path: &Path,
    module_path: &Path,
    module_fingerprint: &str,
    interface_fingerprint: &str,
    options: BuildOptions,
) -> PathBuf {
    let root = project_root(source_path);
    let mut digest = Sha256::new();
    digest.update(env!("CARGO_PKG_VERSION").as_bytes());
    digest.update(normalize_path(module_path).as_bytes());
    digest.update(module_fingerprint.as_bytes());
    digest.update(interface_fingerprint.as_bytes());
    digest.update([u8::from(options.shared)]);
    update_cfg_manifest_fingerprint(&mut digest, &root);
    for variable in [
        "ORI_OPT",
        "ORI_NATIVE_LINKER",
        "ORI_USE_SYSTEM_LINKER",
        "ORI_USE_BUNDLED_RUST_LLD",
        "ORI_TARGET_TRIPLE",
        "ORI_EXECUTION_PROFILE",
        "ORI_FEATURES",
        "ORI_NO_DEFAULT_FEATURES",
    ] {
        digest.update(variable.as_bytes());
        digest.update([0]);
        digest.update(compilation_environment_value(variable).as_bytes());
        digest.update([0xff]);
    }
    digest.update(std::env::consts::ARCH.as_bytes());
    digest.update(std::env::consts::OS.as_bytes());
    let extension = if cfg!(windows) { "obj" } else { "o" };
    root.join(".ori")
        .join("modules")
        .join(format!("{:x}.{extension}", digest.finalize()))
}

/// Include the root feature declarations and defaults in every module object
/// key. The project-level cache already hashes manifests, but a project miss
/// can still reuse module objects; hashing both supported root manifests keeps
/// that reuse correct when only `[features]` changes.
fn update_cfg_manifest_fingerprint(digest: &mut Sha256, root: &Path) {
    for name in ["ori.proj", "ori.pkg.toml"] {
        let path = root.join(name);
        digest.update(name.as_bytes());
        digest.update([0]);
        if let Ok(bytes) = fs::read(path) {
            digest.update((bytes.len() as u64).to_le_bytes());
            digest.update(bytes);
        }
        digest.update([0xff]);
    }
}

/// Fingerprint declarations that affect another module's generated code.
/// Function bodies are intentionally excluded: the owning module's source
/// fingerprint covers them, while unchanged consumers can keep their object.
pub fn interface_fingerprint(hir: &ori_hir::HirModule) -> String {
    let mut digest = Sha256::new();
    digest.update(ori_runtime::ORI_ABI_VERSION.as_bytes());
    digest.update(hir.namespace.as_bytes());
    for structure in &hir.structs {
        digest.update(
            format!(
                "struct:{:?}:{:?}:{:?}",
                structure.def_id, structure.name, structure.repr_c
            )
            .as_bytes(),
        );
        for field in &structure.fields {
            digest.update(format!("field:{}:{:?}", field.name, field.ty).as_bytes());
        }
    }
    for enumeration in &hir.enums {
        digest.update(format!("enum:{:?}:{}", enumeration.def_id, enumeration.name).as_bytes());
        for variant in &enumeration.variants {
            digest.update(variant.name.as_bytes());
            for field in &variant.fields {
                digest.update(format!("{}:{:?}", field.name, field.ty).as_bytes());
            }
        }
    }
    for function in &hir.funcs {
        digest.update(
            format!(
                "fn:{}:{:?}:{:?}:{}:{}:{:?}",
                function.name,
                function.return_ty,
                function.is_async,
                function.is_mut,
                function.is_public,
                function.c_export_name
            )
            .as_bytes(),
        );
        for parameter in &function.params {
            digest.update(
                format!(
                    "param:{}:{:?}:{}",
                    parameter.name, parameter.ty, parameter.variadic
                )
                .as_bytes(),
            );
        }
    }
    for implementation in &hir.trait_impls {
        digest.update(
            format!(
                "impl:{:?}:{:?}",
                implementation.trait_def_id, implementation.type_def_id
            )
            .as_bytes(),
        );
        for method in &implementation.methods {
            digest.update(method.func_name.as_bytes());
        }
    }
    for constant in &hir.consts {
        digest.update(
            format!(
                "const:{}:{:?}:{}",
                constant.name, constant.ty, constant.mutable
            )
            .as_bytes(),
        );
    }
    format!("{:x}", digest.finalize())
}

fn cache_record_path(root: &Path) -> PathBuf {
    root.join(".ori").join("incremental.json")
}

fn module_records(source_path: &Path) -> Result<Vec<ModuleRecord>, String> {
    let root = project_root(source_path);
    let mut roots = Vec::new();
    collect_dependency_roots(&root, &mut roots).map_err(|err| {
        format!(
            "incremental.input_scan_failed: cannot scan `{}`: {err}",
            root.display()
        )
    })?;
    let mut files = Vec::new();
    for input_root in roots {
        if input_root.as_os_str().is_empty() {
            continue;
        }
        collect_inputs(&input_root, &mut files).map_err(|err| {
            format!(
                "incremental.input_scan_failed: cannot scan `{}`: {err}",
                input_root.display()
            )
        })?;
    }
    files.retain(|path| path.extension().and_then(|ext| ext.to_str()) == Some("orl"));
    files.sort();
    files
        .into_iter()
        .map(|path| {
            let bytes = fs::read(&path).map_err(|err| {
                format!(
                    "incremental.input_read_failed: cannot read `{}`: {err}",
                    path.display()
                )
            })?;
            let mut digest = Sha256::new();
            digest.update(&bytes);
            Ok(ModuleRecord {
                path: normalize_path(&path),
                fingerprint: format!("{:x}", digest.finalize()),
                artifact: None,
            })
        })
        .collect()
}

fn manifest_declares_dependencies(root: &Path) -> bool {
    let manifest = [root.join("ori.pkg.toml"), root.join("ori.proj")]
        .into_iter()
        .find(|path| path.is_file());
    let Some(manifest) = manifest else {
        return false;
    };
    let Ok(source) = fs::read_to_string(manifest) else {
        return true;
    };
    let mut in_dependencies = false;
    source.lines().any(|raw_line| {
        let line = raw_line
            .split_once('#')
            .map_or(raw_line, |(code, _)| code)
            .trim();
        if line.starts_with('[') && line.ends_with(']') {
            in_dependencies = line[1..line.len() - 1].trim() == "dependencies";
            return false;
        }
        in_dependencies && !line.is_empty()
    })
}

fn project_root(source_path: &Path) -> PathBuf {
    let start = if source_path.is_dir() {
        source_path.to_path_buf()
    } else {
        // A bare file name such as `main.orl` has an *empty* parent, not
        // `None`; scanning `""` fails, so both cases collapse to the cwd.
        source_path
            .parent()
            .filter(|parent| !parent.as_os_str().is_empty())
            .map(Path::to_path_buf)
            .unwrap_or_else(|| PathBuf::from("."))
    };
    for ancestor in start.ancestors() {
        if ancestor.join("ori.pkg.toml").is_file() || ancestor.join("ori.proj").is_file() {
            return fs::canonicalize(ancestor).unwrap_or_else(|_| ancestor.to_path_buf());
        }
    }
    fs::canonicalize(&start).unwrap_or(start)
}

fn fingerprint(source_path: &Path, options: BuildOptions) -> Result<String, String> {
    let root = project_root(source_path);
    let mut roots = Vec::new();
    collect_dependency_roots(&root, &mut roots).map_err(|err| {
        format!(
            "incremental.input_scan_failed: cannot scan `{}`: {err}",
            root.display()
        )
    })?;
    let mut files = Vec::new();
    for input_root in roots {
        if input_root.as_os_str().is_empty() {
            continue;
        }
        collect_inputs(&input_root, &mut files).map_err(|err| {
            format!(
                "incremental.input_scan_failed: cannot scan `{}`: {err}",
                input_root.display()
            )
        })?;
    }
    files.sort();
    let mut digest = Sha256::new();
    digest.update(env!("CARGO_PKG_VERSION").as_bytes());
    digest.update([u8::from(options.shared)]);
    for variable in [
        "ORI_OPT",
        "ORI_NATIVE_LINKER",
        "ORI_USE_SYSTEM_LINKER",
        "ORI_USE_BUNDLED_RUST_LLD",
        "ORI_RUNTIME_LIB",
        "ORI_TARGET_TRIPLE",
        "ORI_EXECUTION_PROFILE",
        "ORI_FEATURES",
        "ORI_NO_DEFAULT_FEATURES",
    ] {
        digest.update(variable.as_bytes());
        digest.update([0]);
        digest.update(compilation_environment_value(variable).as_bytes());
        digest.update([0xff]);
    }
    digest.update(std::env::consts::ARCH.as_bytes());
    digest.update(std::env::consts::OS.as_bytes());
    digest.update(std::env::consts::FAMILY.as_bytes());
    for path in files {
        digest.update(normalize_path(&path).as_bytes());
        let bytes = fs::read(&path).map_err(|err| {
            format!(
                "incremental.input_read_failed: cannot read `{}`: {err}",
                path.display()
            )
        })?;
        digest.update((bytes.len() as u64).to_le_bytes());
        digest.update(bytes);
    }
    Ok(format!("{:x}", digest.finalize()))
}

fn compilation_environment_value(variable: &str) -> String {
    let value = std::env::var(variable).unwrap_or_default();
    normalize_compilation_environment_value(variable, &value)
}

fn normalize_compilation_environment_value(variable: &str, value: &str) -> String {
    match variable {
        "ORI_NO_DEFAULT_FEATURES" => {
            return if matches!(value, "1" | "true" | "TRUE" | "yes" | "YES") {
                "1".to_string()
            } else {
                "0".to_string()
            };
        }
        "ORI_TARGET_TRIPLE" | "ORI_EXECUTION_PROFILE" => return value.trim().to_string(),
        "ORI_FEATURES" => {}
        _ => return value.to_string(),
    }
    let mut features: Vec<_> = value
        .split(',')
        .map(str::trim)
        .filter(|feature| !feature.is_empty())
        .collect();
    features.sort_unstable();
    features.dedup();
    features.join(",")
}

/// Include materialised path/registry/Git dependencies in the graph
/// fingerprint.  A project-level cache is only safe when editing a package
/// outside the application also invalidates the application output.
fn collect_dependency_roots(root: &Path, roots: &mut Vec<PathBuf>) -> std::io::Result<()> {
    let canonical = fs::canonicalize(root).unwrap_or_else(|_| root.to_path_buf());
    if roots.iter().any(|known| known == &canonical) {
        return Ok(());
    }
    roots.push(canonical.clone());

    let lock_path = canonical.join("ori.lock");
    if !lock_path.is_file() {
        return Ok(());
    }
    let source = fs::read_to_string(lock_path)?;
    let cache_root = package_cache_root();
    let mut source_kind = String::new();
    let mut dependency_name = String::new();
    let mut dependency_version = String::new();
    let mut dependency_path = None;
    let mut dependency_roots = Vec::new();

    let flush = |source_kind: &str,
                 dependency_name: &str,
                 dependency_version: &str,
                 dependency_path: &Option<String>,
                 dependency_roots: &mut Vec<PathBuf>| {
        let candidate = match source_kind {
            "path" => dependency_path.as_ref().map(|path| canonical.join(path)),
            "registry" | "git" if !dependency_name.is_empty() && !dependency_version.is_empty() => {
                cache_root
                    .as_ref()
                    .map(|cache| cache.join(dependency_name).join(dependency_version))
            }
            _ => None,
        };
        if let Some(path) = candidate {
            dependency_roots.push(path);
        }
    };

    for line in source.lines() {
        let line = line.trim();
        if line == "[[dependency]]" {
            flush(
                &source_kind,
                &dependency_name,
                &dependency_version,
                &dependency_path,
                &mut dependency_roots,
            );
            source_kind.clear();
            dependency_name.clear();
            dependency_version.clear();
            dependency_path = None;
            continue;
        }
        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        let value = value.trim().trim_matches('"');
        match key.trim() {
            "source" => source_kind = value.to_owned(),
            "name" => dependency_name = value.to_owned(),
            "version" => dependency_version = value.to_owned(),
            "path" => dependency_path = Some(value.to_owned()),
            _ => {}
        }
    }
    flush(
        &source_kind,
        &dependency_name,
        &dependency_version,
        &dependency_path,
        &mut dependency_roots,
    );
    for dependency_root in dependency_roots {
        if dependency_root.is_dir() {
            collect_dependency_roots(&dependency_root, roots)?;
        }
    }
    Ok(())
}

fn package_cache_root() -> Option<PathBuf> {
    std::env::var_os("ORI_PACKAGE_CACHE")
        .map(PathBuf::from)
        .or_else(|| {
            std::env::var_os("USERPROFILE").map(|home| PathBuf::from(home).join(".ori/packages"))
        })
        .or_else(|| std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".ori/packages")))
}

fn collect_inputs(root: &Path, files: &mut Vec<PathBuf>) -> std::io::Result<()> {
    for entry in fs::read_dir(root)? {
        let entry = entry?;
        let path = entry.path();
        let name = entry.file_name();
        if name == ".git" || name == "target" || name == ".ori" {
            continue;
        }
        // Skip broken symlinks instead of aborting the whole build fingerprint.
        let file_type = match entry.file_type() {
            Ok(file_type) => file_type,
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => continue,
            Err(err) => return Err(err),
        };
        if file_type.is_dir() {
            collect_inputs(&path, files)?;
        } else if file_type.is_file()
            && (path.extension().and_then(|ext| ext.to_str()) == Some("orl")
                || path.file_name().and_then(|name| name.to_str()) == Some("ori.pkg.toml")
                || path.file_name().and_then(|name| name.to_str()) == Some("ori.proj")
                || path.file_name().and_then(|name| name.to_str()) == Some("ori.lock"))
        {
            files.push(path);
        }
    }
    Ok(())
}

fn normalize_path(path: &Path) -> String {
    fs::canonicalize(path)
        .unwrap_or_else(|_| path.to_path_buf())
        .to_string_lossy()
        .replace('\\', "/")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cache_reuses_only_matching_content() {
        let root = std::env::temp_dir().join(format!(
            "ori_incremental_{}_{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("system clock must be after epoch")
                .as_nanos()
        ));
        fs::create_dir_all(&root).expect("create incremental fixture");
        let source = root.join("main.orl");
        let helper = root.join("helper.orl");
        let output = root.join("main");
        fs::write(&source, "module app.main\n").expect("write source");
        fs::write(&output, b"native").expect("write output");
        record_success(&source, &output, None, BuildOptions { shared: false })
            .expect("record cache");
        let unchanged =
            changed_modules(&source, BuildOptions { shared: false }).expect("read module index");
        assert_eq!(unchanged.len(), 1);
        assert!(!unchanged[0].changed);
        assert!(try_reuse(&source, &output, BuildOptions { shared: false })
            .expect("read cache")
            .is_some());
        fs::write(&source, "module app.main\n\nconst changed = 1\n").expect("change source");
        let changed = changed_modules(&source, BuildOptions { shared: false })
            .expect("read changed module index");
        assert_eq!(changed.len(), 1);
        assert!(changed[0].changed);
        record_success(&source, &output, None, BuildOptions { shared: false })
            .expect("refresh cache after source change");
        fs::write(&helper, "module app.helper\n").expect("add module");
        let added = changed_modules(&source, BuildOptions { shared: false })
            .expect("read added module index");
        assert_eq!(added.len(), 2);
        assert!(added
            .iter()
            .any(|module| module.path == helper && module.changed));
        record_success(&source, &output, None, BuildOptions { shared: false })
            .expect("record added module");
        fs::remove_file(&helper).expect("remove module");
        let removed = changed_modules(&source, BuildOptions { shared: false })
            .expect("read removed module index");
        assert_eq!(removed.len(), 2);
        assert!(removed
            .iter()
            .any(|module| module.path == source && !module.changed));
        assert!(removed
            .iter()
            .any(|module| module.path == helper && module.changed));
        assert!(try_reuse(&source, &output, BuildOptions { shared: false })
            .expect("read changed cache")
            .is_none());
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn cfg_environment_fingerprint_is_semantically_normalized() {
        assert_eq!(
            normalize_compilation_environment_value("ORI_FEATURES", "zeta, alpha,zeta"),
            "alpha,zeta"
        );
        assert_eq!(
            normalize_compilation_environment_value("ORI_NO_DEFAULT_FEATURES", "yes"),
            "1"
        );
        assert_eq!(
            normalize_compilation_environment_value("ORI_NO_DEFAULT_FEATURES", "0"),
            "0"
        );
        assert_eq!(
            normalize_compilation_environment_value(
                "ORI_TARGET_TRIPLE",
                " x86_64-unknown-linux-gnu ",
            ),
            "x86_64-unknown-linux-gnu"
        );
    }
}
