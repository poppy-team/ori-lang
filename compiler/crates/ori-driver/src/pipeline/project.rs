//! Project loading, import discovery, and source graph resolution for the Ori driver.
//!
//! This module owns project manifests, dependency scopes, stdlib lookup, and
//! the lex/parse/resolve traversal shared by compile, check, doc, and test.

use ori_ast::item::SourceFile;
use ori_diagnostics::{Diagnostic, DiagnosticSink, FileId, Label, SourceCache};
use ori_lexer::Token;
use ori_types::conditional::{CfgContext, ExecutionProfile};
use ori_types::resolve::ResolvedModule;
use std::collections::{BTreeMap, BTreeSet, HashSet};
use std::path::{Path, PathBuf};

use super::runtime::{env_flag, native_target_triple};
use super::timing::report_internal_pipeline_timing;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NewProjectKind {
    App,
    Lib,
}

#[derive(Clone, Debug)]
pub struct NewProjectOptions {
    pub name: Option<String>,
    pub kind: NewProjectKind,
    pub is_init: bool,
}

#[derive(Clone, Debug)]
pub struct NewProjectOutput {
    pub root: PathBuf,
    pub manifest: PathBuf,
    pub entry: PathBuf,
}

pub(super) fn dedup_paths(paths: Vec<PathBuf>) -> Vec<PathBuf> {
    let mut seen = HashSet::new();
    let mut out = Vec::new();
    for path in paths {
        let key = std::fs::canonicalize(&path).unwrap_or_else(|_| path.clone());
        if seen.insert(key) {
            out.push(path);
        }
    }
    out
}

/// Create a minimal project scaffold while keeping filesystem mutation out of
/// the pipeline facade.
pub fn run_new_project(
    root: &Path,
    options: NewProjectOptions,
) -> Result<NewProjectOutput, String> {
    if !options.is_init {
        if root.exists() {
            let mut entries = std::fs::read_dir(root)
                .map_err(|e| format!("cannot inspect `{}`: {e}", root.display()))?;
            if entries.next().is_some() {
                return Err(format!(
                    "project.new_exists: `{}` already exists and is not empty",
                    root.display()
                ));
            }
        }
        std::fs::create_dir_all(root)
            .map_err(|e| format!("cannot create project `{}`: {e}", root.display()))?;
    } else {
        if root.join("ori.proj").exists() || root.join("ori.pkg.toml").exists() {
            return Err(format!(
                "project.init_exists: `{}` already contains an ori.proj or ori.pkg.toml",
                root.display()
            ));
        }
        std::fs::create_dir_all(root)
            .map_err(|e| format!("cannot create project `{}`: {e}", root.display()))?;
    }

    std::fs::create_dir_all(root.join("docs"))
        .map_err(|e| format!("cannot create `{}`: {e}", root.join("docs").display()))?;

    let name = options
        .name
        .filter(|name| !name.trim().is_empty())
        .unwrap_or_else(|| default_project_name(root));
    let (kind_label, entry_rel, source) = match options.kind {
        NewProjectKind::App => (
            "app",
            "main.orl",
            "module app.main\n\nimport ori.io = io\n\nmain()\n    io.println(\"Hello, Ori!\")\nend\n",
        ),
        NewProjectKind::Lib => (
            "lib",
            "lib.orl",
            "module app.lib\n\npublic answer() -> int\n    return 42\nend\n",
        ),
    };

    let manifest = root.join("ori.proj");
    let entry = root.join(entry_rel);
    let manifest_source = format!(
        "manifest = 1\nname = \"{}\"\nversion = \"0.1.0\"\nkind = \"{}\"\nentry = \"{}\"\n\n[source]\nroot_namespace = \"app\"\n\n[docs]\npaths = [\"docs\"]\nmode = \"sidecar-first\"\nrequire_public = \"off\"\n",
        escape_manifest_string(&name),
        kind_label,
        escape_manifest_string(entry_rel),
    );

    std::fs::write(&manifest, manifest_source)
        .map_err(|e| format!("cannot write `{}`: {e}", manifest.display()))?;
    if !entry.exists() {
        std::fs::write(&entry, source)
            .map_err(|e| format!("cannot write `{}`: {e}", entry.display()))?;
    }

    Ok(NewProjectOutput {
        root: root.to_path_buf(),
        manifest,
        entry,
    })
}

pub(super) struct LoadedSource {
    pub(super) path: PathBuf,
    pub(super) file_id: FileId,
    pub(super) source: String,
    pub(super) tokens: Vec<Token>,
    pub(super) ast: SourceFile,
}

#[allow(dead_code)]
#[derive(Clone, Debug)]
pub(super) struct ProjectConfig {
    manifest_path: PathBuf,
    pub(super) root: PathBuf,
    name: Option<String>,
    version: Option<String>,
    kind: ProjectKind,
    entry: PathBuf,
    source_root: Option<PathBuf>,
    root_namespace: Option<String>,
    dependencies: Vec<ProjectDependency>,
    declared_features: BTreeSet<String>,
    default_features: BTreeSet<String>,
    pub(super) doc_paths: Vec<PathBuf>,
    pub(super) doc_mode: ProjectDocMode,
    pub(super) require_public_docs: DocRequirement,
}

#[derive(Clone, Debug)]
struct ProjectDependency {
    name: String,
    path: Option<PathBuf>,
    version: Option<String>,
    git: Option<crate::package::GitDependencySpec>,
}

#[derive(Clone, Debug, Default)]
pub(super) struct ImportContext {
    dependencies: Vec<ImportDependency>,
    pub(super) native_libs: Vec<NativeLibContext>,
    pub(super) native_configs: Vec<PackageNativeConfigContext>,
    cfg: CfgContext,
}

#[derive(Clone, Debug)]
pub(super) struct NativeLibContext {
    pub(super) name: String,
    pub(super) package_root: PathBuf,
}

#[derive(Clone, Debug)]
pub(super) struct PackageNativeConfigContext {
    pub(super) config: crate::package::NativeConfig,
    pub(super) package_root: PathBuf,
}

/// Resolved source graph shared by the compiler phases.
///
/// Keeping the loaded sources, semantic model, and dependency context together
/// prevents callers from accidentally pairing values from different graphs.
pub(super) struct ResolvedSources {
    pub(super) loaded: Vec<LoadedSource>,
    pub(super) resolved: ResolvedModule,
    pub(super) imports: ImportContext,
}

#[derive(Clone, Debug)]
struct ImportDependency {
    name: String,
    root: PathBuf,
    entry: PathBuf,
    source_root: Option<PathBuf>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ProjectKind {
    App,
    Lib,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum ProjectDocMode {
    SidecarFirst,
    InlineFirst,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum DocRequirement {
    Off,
    Warn,
    Error,
}
pub(super) fn read_file(path: &Path) -> Result<String, String> {
    std::fs::read_to_string(path).map_err(|e| format!("cannot read `{}`: {}", path.display(), e))
}

fn resolve_entry_path(path: &Path) -> Result<PathBuf, String> {
    if path.is_dir() {
        let manifest = path.join("ori.proj");
        if !manifest.is_file() {
            return Err(format!(
                "project manifest `{}` not found",
                manifest.display()
            ));
        }
        return read_project_config(&manifest).map(|config| config.entry);
    }

    if path.file_name().and_then(|name| name.to_str()) == Some("ori.proj") {
        return read_project_config(path).map(|config| config.entry);
    }

    if path.file_name().and_then(|name| name.to_str()) == Some("ori.pkg.toml") {
        return crate::package::load_package_manifest(path).map(|manifest| manifest.entry);
    }

    Ok(path.to_owned())
}

fn read_project_config(manifest: &Path) -> Result<ProjectConfig, String> {
    let source = read_file(manifest)?;
    let root = manifest.parent().unwrap_or_else(|| Path::new("."));
    let mut entry = None;
    let mut name = None;
    let mut version = None;
    let mut kind = ProjectKind::App;
    let mut source_root = None;
    let mut root_namespace = None;
    let mut dependencies = Vec::new();
    let mut declared_features = BTreeSet::new();
    let mut default_features = BTreeSet::new();
    let mut doc_paths = Vec::new();
    let mut doc_mode = ProjectDocMode::SidecarFirst;
    let mut require_public_docs = DocRequirement::Off;
    let mut section = ManifestSection::Root;

    for line in source.lines() {
        let line = strip_manifest_comment(line).trim();
        if line.is_empty() {
            continue;
        }
        if line.starts_with('[') && line.ends_with(']') {
            section = match &line[1..line.len() - 1] {
                "source" => ManifestSection::Source,
                "dependencies" => ManifestSection::Dependencies,
                "features" => ManifestSection::Features,
                "docs" => ManifestSection::Docs,
                _ => ManifestSection::Other,
            };
            continue;
        }

        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        let key = key.trim();
        let value = value.trim();
        match (&section, key) {
            (ManifestSection::Root, "name") => {
                name = Some(parse_manifest_string(value, "name", manifest)?);
            }
            (ManifestSection::Root, "version") => {
                version = Some(parse_manifest_string(value, "version", manifest)?);
            }
            (ManifestSection::Root, "kind") => {
                kind =
                    parse_project_kind(&parse_manifest_string(value, "kind", manifest)?, manifest)?;
            }
            (ManifestSection::Root, "entry") => {
                entry = Some(root.join(parse_manifest_string(value, "entry", manifest)?));
            }
            (ManifestSection::Root, "manifest") => {
                let _ = parse_manifest_number(value, "manifest", manifest)?;
            }
            (ManifestSection::Source, "root") => {
                source_root =
                    Some(root.join(parse_manifest_string(value, "source.root", manifest)?));
            }
            (ManifestSection::Source, "root_namespace" | "namespace") => {
                root_namespace = Some(parse_manifest_string(
                    value,
                    "source.root_namespace",
                    manifest,
                )?);
            }
            (ManifestSection::Dependencies, name) => {
                dependencies.push(parse_project_dependency(name, value, manifest, root)?);
            }
            (ManifestSection::Features, "default") => {
                default_features =
                    parse_manifest_string_array(value, "features.default", manifest)?
                        .into_iter()
                        .collect();
            }
            (ManifestSection::Features, name) => {
                validate_feature_name(name, manifest)?;
                let dependencies =
                    parse_manifest_string_array(value, &format!("features.{name}"), manifest)?;
                if !dependencies.is_empty() {
                    return Err(format!(
                        "project manifest `{}` feature `{name}` must use an empty array in cfg v1",
                        manifest.display()
                    ));
                }
                if !declared_features.insert(name.to_string()) {
                    return Err(format!(
                        "project manifest `{}` declares feature `{name}` more than once",
                        manifest.display()
                    ));
                }
            }
            (ManifestSection::Docs, "paths") => {
                doc_paths = parse_manifest_string_array(value, "docs.paths", manifest)?
                    .into_iter()
                    .map(|path| root.join(path))
                    .collect();
            }
            (ManifestSection::Docs, "mode") => {
                doc_mode = parse_project_doc_mode(
                    &parse_manifest_string(value, "docs.mode", manifest)?,
                    manifest,
                )?;
            }
            (ManifestSection::Docs, "require_public") => {
                require_public_docs = parse_doc_requirement(
                    &parse_manifest_string(value, "docs.require_public", manifest)?,
                    manifest,
                )?;
            }
            _ => {}
        }
    }

    let Some(entry) = entry else {
        return Err(format!(
            "project manifest `{}` is missing `entry`",
            manifest.display()
        ));
    };
    if !entry.is_file() {
        return Err(format!(
            "project entry `{}` does not exist",
            entry.display()
        ));
    }
    validate_default_features(&declared_features, &default_features, manifest, "project")?;
    Ok(ProjectConfig {
        manifest_path: manifest.to_path_buf(),
        root: root.to_path_buf(),
        name,
        version,
        kind,
        entry,
        source_root,
        root_namespace,
        dependencies,
        declared_features,
        default_features,
        doc_paths,
        doc_mode,
        require_public_docs,
    })
}

#[derive(Debug)]
enum ManifestSection {
    Root,
    Source,
    Dependencies,
    Features,
    Docs,
    Other,
}

fn strip_manifest_comment(line: &str) -> &str {
    let mut in_string = false;
    let mut previous = '\0';
    for (index, ch) in line.char_indices() {
        if ch == '"' && previous != '\\' {
            in_string = !in_string;
        }
        if !in_string && ch == '-' && line[index..].starts_with("--") {
            return &line[..index];
        }
        previous = ch;
    }
    line
}

pub(super) fn default_project_name(root: &Path) -> String {
    root.file_name()
        .and_then(|name| name.to_str())
        .filter(|name| !name.trim().is_empty())
        .unwrap_or("app")
        .to_string()
}

pub(super) fn escape_manifest_string(value: &str) -> String {
    value.replace('\\', "\\\\").replace('"', "\\\"")
}

fn parse_manifest_string(value: &str, key: &str, manifest: &Path) -> Result<String, String> {
    let value = value.trim();
    if value.len() >= 2 && value.starts_with('"') && value.ends_with('"') {
        return Ok(value[1..value.len() - 1].replace("\\\"", "\""));
    }
    Err(format!(
        "project manifest `{}` field `{key}` must be a quoted string",
        manifest.display()
    ))
}

fn parse_manifest_number(value: &str, key: &str, manifest: &Path) -> Result<u32, String> {
    value.trim().parse::<u32>().map_err(|_| {
        format!(
            "project manifest `{}` field `{key}` must be a number",
            manifest.display()
        )
    })
}

fn parse_manifest_string_array(
    value: &str,
    key: &str,
    manifest: &Path,
) -> Result<Vec<String>, String> {
    let value = value.trim();
    if !(value.starts_with('[') && value.ends_with(']')) {
        return Err(format!(
            "project manifest `{}` field `{key}` must be an array of quoted strings",
            manifest.display()
        ));
    }
    let inner = value[1..value.len() - 1].trim();
    if inner.is_empty() {
        return Ok(Vec::new());
    }
    inner
        .split(',')
        .map(|part| parse_manifest_string(part.trim(), key, manifest))
        .collect()
}

fn parse_project_dependency(
    name: &str,
    value: &str,
    manifest: &Path,
    root: &Path,
) -> Result<ProjectDependency, String> {
    let name = name.trim().to_string();
    validate_project_dependency_name(&name, manifest)?;
    let value = value.trim();
    if value.starts_with('"') {
        return Ok(ProjectDependency {
            name,
            path: None,
            version: Some(parse_manifest_string(
                value,
                "dependencies.version",
                manifest,
            )?),
            git: None,
        });
    }
    if value.starts_with('{') {
        let table = parse_manifest_inline_table(value, manifest)?;
        let version = table.get("version").cloned();
        if let Some(git_url) = table.get("git").cloned() {
            if table.contains_key("path") {
                return Err(format!(
                    "project manifest `{}` dependency `{name}` cannot combine `git` and `path`",
                    manifest.display()
                ));
            }
            let rev = table.get("rev").cloned();
            let tag = table.get("tag").cloned();
            let branch = table.get("branch").cloned();
            let pin_count = [rev.is_some(), tag.is_some(), branch.is_some()]
                .into_iter()
                .filter(|v| *v)
                .count();
            if pin_count > 1 {
                return Err(format!(
                    "project manifest `{}` dependency `{name}` may set only one of `rev`, `tag`, or `branch`",
                    manifest.display()
                ));
            }
            let git_version = version.clone();
            return Ok(ProjectDependency {
                name,
                path: None,
                version,
                git: Some(crate::package::GitDependencySpec {
                    url: git_url,
                    rev,
                    tag,
                    branch,
                    version: git_version,
                }),
            });
        }
        let path = table.get("path").map(|path| root.join(path));
        if path.is_none() {
            return Err(format!(
                "project manifest `{}` dependency `{name}` needs `path` or `git`",
                manifest.display()
            ));
        }
        return Ok(ProjectDependency {
            name,
            path,
            version,
            git: None,
        });
    }
    Err(format!(
        "project manifest `{}` dependency `{name}` must be a quoted version, `{{ path = \"...\" }}`, or `{{ git = \"...\" }}`",
        manifest.display()
    ))
}

fn validate_project_dependency_name(name: &str, manifest: &Path) -> Result<(), String> {
    if name.is_empty() {
        return Err(format!(
            "project manifest `{}` has an empty dependency name",
            manifest.display()
        ));
    }
    for segment in name.split('.') {
        let mut chars = segment.chars();
        let Some(first) = chars.next() else {
            return Err(format!(
                "project manifest `{}` dependency `{name}` has an empty namespace segment",
                manifest.display()
            ));
        };
        if !(first == '_' || first.is_ascii_alphabetic()) {
            return Err(format!(
                "project manifest `{}` dependency `{name}` must start each segment with a letter or `_`",
                manifest.display()
            ));
        }
        if !chars.all(|ch| ch == '_' || ch.is_ascii_alphanumeric()) {
            return Err(format!(
                "project manifest `{}` dependency `{name}` may contain only letters, digits, and `_`",
                manifest.display()
            ));
        }
    }
    Ok(())
}

fn validate_feature_name(name: &str, manifest: &Path) -> Result<(), String> {
    let mut chars = name.chars();
    let valid = chars
        .next()
        .is_some_and(|first| first == '_' || first.is_ascii_alphabetic())
        && chars.all(|ch| ch == '_' || ch.is_ascii_alphanumeric());
    if valid && name != "default" {
        return Ok(());
    }
    Err(format!(
        "project manifest `{}` has invalid feature name `{name}`; use an ASCII identifier other than `default`",
        manifest.display()
    ))
}

fn validate_default_features(
    declared: &BTreeSet<String>,
    defaults: &BTreeSet<String>,
    manifest: &Path,
    kind: &str,
) -> Result<(), String> {
    if let Some(unknown) = defaults.iter().find(|feature| !declared.contains(*feature)) {
        return Err(format!(
            "{kind} manifest `{}` enables undeclared default feature `{unknown}`",
            manifest.display()
        ));
    }
    Ok(())
}

fn parse_manifest_inline_table(
    value: &str,
    manifest: &Path,
) -> Result<BTreeMap<String, String>, String> {
    let trimmed = value.trim();
    if !trimmed.starts_with('{') || !trimmed.ends_with('}') {
        return Err(format!(
            "project manifest `{}` inline table must start with `{{` and end with `}}`",
            manifest.display()
        ));
    }
    let inner = trimmed[1..trimmed.len() - 1].trim();
    let mut table = BTreeMap::new();
    if inner.is_empty() {
        return Ok(table);
    }
    for part in split_manifest_inline_items(inner) {
        let Some((key, raw_value)) = part.split_once('=') else {
            return Err(format!(
                "project manifest `{}` inline table item must use `key = value`",
                manifest.display()
            ));
        };
        let key = key.trim().to_string();
        let parsed_value = parse_manifest_string(raw_value.trim(), &key, manifest)?;
        table.insert(key, parsed_value);
    }
    Ok(table)
}

fn split_manifest_inline_items(value: &str) -> Vec<&str> {
    let mut items = Vec::new();
    let mut start = 0usize;
    let mut in_string = false;
    let mut escaped = false;
    for (index, ch) in value.char_indices() {
        if escaped {
            escaped = false;
            continue;
        }
        if ch == '\\' && in_string {
            escaped = true;
            continue;
        }
        if ch == '"' {
            in_string = !in_string;
            continue;
        }
        if ch == ',' && !in_string {
            items.push(value[start..index].trim());
            start = index + 1;
        }
    }
    items.push(value[start..].trim());
    items
}

fn parse_project_kind(value: &str, manifest: &Path) -> Result<ProjectKind, String> {
    match value {
        "app" => Ok(ProjectKind::App),
        "lib" => Ok(ProjectKind::Lib),
        other => Err(format!(
            "project manifest `{}` field `kind` must be `app` or `lib`, got `{other}`",
            manifest.display()
        )),
    }
}

fn parse_project_doc_mode(value: &str, manifest: &Path) -> Result<ProjectDocMode, String> {
    match value {
        "sidecar-first" => Ok(ProjectDocMode::SidecarFirst),
        "inline-first" => Ok(ProjectDocMode::InlineFirst),
        other => Err(format!(
            "project manifest `{}` field `docs.mode` must be `sidecar-first` or `inline-first`, got `{other}`",
            manifest.display()
        )),
    }
}

fn parse_doc_requirement(value: &str, manifest: &Path) -> Result<DocRequirement, String> {
    match value {
        "off" => Ok(DocRequirement::Off),
        "warn" => Ok(DocRequirement::Warn),
        "error" => Ok(DocRequirement::Error),
        other => Err(format!(
            "project manifest `{}` field `docs.require_public` must be `off`, `warn`, or `error`, got `{other}`",
            manifest.display()
        )),
    }
}

pub(super) fn project_config_for_docs(path: &Path) -> Result<Option<ProjectConfig>, String> {
    if path.is_dir() {
        let manifest = path.join("ori.proj");
        return manifest
            .is_file()
            .then(|| read_project_config(&manifest))
            .transpose();
    }

    if path.file_name().and_then(|name| name.to_str()) == Some("ori.proj") {
        return read_project_config(path).map(Some);
    }

    let start = path.parent().unwrap_or_else(|| Path::new("."));
    find_project_root(start)
        .map(|root| read_project_config(&root.join("ori.proj")))
        .transpose()
}

fn import_context_for_entry(entry: &Path) -> Result<ImportContext, String> {
    import_context_for_entry_with_cfg(entry, None)
}

fn import_context_for_entry_with_cfg(
    entry: &Path,
    cfg_override: Option<CfgContext>,
) -> Result<ImportContext, String> {
    let mut context = ImportContext::default();
    let start = entry.parent().unwrap_or_else(|| Path::new("."));

    if let Some(root) = find_project_root(start) {
        let config = read_project_config(&root.join("ori.proj"))?;
        add_project_dependencies(&config, &mut context)?;
        crate::package::validate_package_lock(&root)?;
        let package_manifest = root.join("ori.pkg.toml");
        if package_manifest.is_file() {
            add_package_manifest_dependencies(&package_manifest, &mut context)?;
        }
    } else if let Some(package_manifest) = find_package_manifest(start) {
        if let Some(root) = package_manifest.parent() {
            crate::package::validate_package_lock(root)?;
        }
        add_package_manifest_dependencies(&package_manifest, &mut context)?;
    }

    context.cfg = match cfg_override {
        Some(cfg) => cfg,
        None => cfg_context_for_entry(entry)?,
    };

    Ok(context)
}

fn cfg_context_for_entry(entry: &Path) -> Result<CfgContext, String> {
    let start = entry.parent().unwrap_or_else(|| Path::new("."));
    let mut declared = BTreeSet::new();
    let mut defaults = BTreeSet::new();

    if let Some(root) = find_project_root(start) {
        let project = read_project_config(&root.join("ori.proj"))?;
        declared.extend(project.declared_features);
        defaults.extend(project.default_features);
        let package_path = root.join("ori.pkg.toml");
        if package_path.is_file() {
            let package = crate::package::load_package_manifest(&package_path)?;
            declared.extend(package.declared_features);
            defaults.extend(package.default_features);
        }
    } else if let Some(package_path) = find_package_manifest(start) {
        let package = crate::package::load_package_manifest(&package_path)?;
        declared.extend(package.declared_features);
        defaults.extend(package.default_features);
    }

    let profile = execution_profile_from_environment()?;
    let mut enabled = if env_flag("ORI_NO_DEFAULT_FEATURES") {
        BTreeSet::new()
    } else {
        defaults
    };
    for feature in requested_features()? {
        if !declared.contains(&feature) {
            return Err(format!(
                "cfg.feature_not_declared: feature `{feature}` is not declared under `[features]`"
            ));
        }
        enabled.insert(feature);
    }

    Ok(CfgContext::new(
        selected_target_triple()?,
        profile,
        declared,
        enabled,
    ))
}

pub fn filter_source_for_current_configuration(
    path: &Path,
    source_file: &mut SourceFile,
    file_id: FileId,
    sink: &mut DiagnosticSink,
) -> Result<(), String> {
    let context = cfg_context_for_entry(path)?;
    ori_types::conditional::filter_source_file(source_file, file_id, &context, sink);
    Ok(())
}

/// Filter compiler-owned source such as the standard library without making
/// it inherit application feature flags. Target and execution profile remain
/// relevant, while project features belong only to the root application
/// namespace in cfg v1.
pub fn filter_intrinsic_source_for_current_configuration(
    source_file: &mut SourceFile,
    file_id: FileId,
    sink: &mut DiagnosticSink,
) -> Result<(), String> {
    let context = CfgContext::new(
        selected_target_triple()?,
        execution_profile_from_environment()?,
        BTreeSet::new(),
        BTreeSet::new(),
    );
    ori_types::conditional::filter_source_file(source_file, file_id, &context, sink);
    Ok(())
}

fn execution_profile_from_environment() -> Result<ExecutionProfile, String> {
    let profile_name =
        std::env::var("ORI_EXECUTION_PROFILE").unwrap_or_else(|_| "standalone".to_string());
    ExecutionProfile::parse(profile_name.trim()).ok_or_else(|| {
        format!(
            "cfg.execution_profile_invalid: expected `standalone` or `embedded`, got `{}`",
            profile_name.trim()
        )
    })
}

fn selected_target_triple() -> Result<String, String> {
    let target = native_target_triple();
    if target.is_empty()
        || target.chars().any(char::is_whitespace)
        || !ori_types::conditional::is_supported_target_triple(&target)
    {
        return Err(
            format!(
                "cfg.target_invalid: target `{target}` cannot be represented by the closed cfg v1 target set"
            ),
        );
    }
    Ok(target)
}

fn requested_features() -> Result<BTreeSet<String>, String> {
    let raw = std::env::var("ORI_FEATURES").unwrap_or_default();
    let mut features = BTreeSet::new();
    for feature in raw
        .split(',')
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        let mut chars = feature.chars();
        let valid = chars
            .next()
            .is_some_and(|first| first == '_' || first.is_ascii_alphabetic())
            && chars.all(|ch| ch == '_' || ch.is_ascii_alphanumeric());
        if !valid || feature == "default" {
            return Err(format!(
                "cfg.feature_invalid: `{feature}` is not a valid feature identifier"
            ));
        }
        features.insert(feature.to_string());
    }
    Ok(features)
}

fn add_project_dependencies(
    config: &ProjectConfig,
    context: &mut ImportContext,
) -> Result<(), String> {
    let cache_root = crate::package::default_package_cache_root().ok();
    for dependency in &config.dependencies {
        if let Some(git) = &dependency.git {
            let cache = cache_root.clone().ok_or_else(|| {
                "package.cache_home_missing: set ORI_PACKAGE_CACHE to resolve git dependencies"
                    .to_string()
            })?;
            let root = crate::package::ensure_git_dependency_cached(&dependency.name, git, &cache)?;
            let import_dependency = import_dependency_from_root(
                &dependency.name,
                &root,
                dependency.version.as_deref(),
            )?;
            add_import_dependency(context, import_dependency);
            // Transitive native libs (e.g. sqlite from path/git package).
            push_package_native_libs(context, &root)?;
            continue;
        }
        if let Some(path) = &dependency.path {
            let import_dependency =
                import_dependency_from_root(&dependency.name, path, dependency.version.as_deref())?;
            add_import_dependency(context, import_dependency);
            push_package_native_libs(context, path)?;
            continue;
        }
        if let Some(version) = &dependency.version {
            let cache = cache_root.clone().ok_or_else(|| {
                "package.cache_home_missing: set ORI_PACKAGE_CACHE to resolve version dependencies"
                    .to_string()
            })?;
            let root =
                crate::package::resolve_cached_version_package(&dependency.name, version, &cache)?;
            let import_dependency =
                import_dependency_from_root(&dependency.name, &root, Some(version))?;
            add_import_dependency(context, import_dependency);
            push_package_native_libs(context, &root)?;
        }
    }
    Ok(())
}

fn add_package_manifest_dependencies(
    manifest_path: &Path,
    context: &mut ImportContext,
) -> Result<(), String> {
    let manifest = crate::package::load_package_manifest(manifest_path)?;
    let cache_root = crate::package::default_package_cache_root().ok();
    for dependency in &manifest.dependencies {
        match &dependency.requirement {
            crate::package::DependencyRequirement::Path { path, version } => {
                let root = manifest.root.join(path);
                let import_dependency =
                    import_dependency_from_root(&dependency.name, &root, version.as_deref())?;
                add_import_dependency(context, import_dependency);
                push_package_native_libs(context, &root)?;
            }
            crate::package::DependencyRequirement::Git {
                url,
                rev,
                tag,
                branch,
                version,
            } => {
                let cache = cache_root.clone().ok_or_else(|| {
                    "package.cache_home_missing: set ORI_PACKAGE_CACHE to resolve git dependencies"
                        .to_string()
                })?;
                let spec = crate::package::GitDependencySpec {
                    url: url.clone(),
                    rev: rev.clone(),
                    tag: tag.clone(),
                    branch: branch.clone(),
                    version: version.clone(),
                };
                let root =
                    crate::package::ensure_git_dependency_cached(&dependency.name, &spec, &cache)?;
                let import_dependency =
                    import_dependency_from_root(&dependency.name, &root, version.as_deref())?;
                add_import_dependency(context, import_dependency);
                push_package_native_libs(context, &root)?;
            }
            crate::package::DependencyRequirement::Version(version) => {
                let cache = cache_root.clone().ok_or_else(|| {
                    "package.cache_home_missing: set ORI_PACKAGE_CACHE to resolve version dependencies"
                        .to_string()
                })?;
                let root = crate::package::resolve_cached_version_package(
                    &dependency.name,
                    version,
                    &cache,
                )?;
                let import_dependency =
                    import_dependency_from_root(&dependency.name, &root, Some(version))?;
                add_import_dependency(context, import_dependency);
                push_package_native_libs(context, &root)?;
            }
        }
    }
    for lib in manifest.native_libs {
        push_native_lib(context, lib, manifest.root.clone());
    }
    push_package_native_config(context, manifest.native_config, manifest.root);
    Ok(())
}

/// Register `native_libs` and `native_config` from a dependency package (and its nested package deps once).
fn push_package_native_libs(
    context: &mut ImportContext,
    package_root: &Path,
) -> Result<(), String> {
    let package_manifest = package_root.join("ori.pkg.toml");
    if !package_manifest.is_file() {
        return Ok(());
    }
    let manifest = crate::package::load_package_manifest(&package_manifest)?;
    for lib in &manifest.native_libs {
        push_native_lib(context, lib.clone(), manifest.root.clone());
    }
    push_package_native_config(
        context,
        manifest.native_config.clone(),
        manifest.root.clone(),
    );
    // One level of nested package deps (adapter → sqlite).
    for dependency in &manifest.dependencies {
        if let crate::package::DependencyRequirement::Path { path, .. } = &dependency.requirement {
            let nested = manifest.root.join(path);
            let nested_pkg = nested.join("ori.pkg.toml");
            if nested_pkg.is_file() {
                let nested_manifest = crate::package::load_package_manifest(&nested_pkg)?;
                for lib in nested_manifest.native_libs {
                    push_native_lib(context, lib, nested_manifest.root.clone());
                }
                push_package_native_config(
                    context,
                    nested_manifest.native_config,
                    nested_manifest.root.clone(),
                );
            }
        }
    }
    Ok(())
}

fn push_package_native_config(
    context: &mut ImportContext,
    config: crate::package::NativeConfig,
    package_root: PathBuf,
) {
    if config.dependencies.is_empty()
        && config.platforms.linux.libraries.is_empty()
        && config.platforms.linux.frameworks.is_empty()
        && config.platforms.linux.library_dirs.is_empty()
        && config.platforms.linux.link_flags.is_empty()
        && config.platforms.windows.libraries.is_empty()
        && config.platforms.windows.frameworks.is_empty()
        && config.platforms.windows.library_dirs.is_empty()
        && config.platforms.windows.link_flags.is_empty()
        && config.platforms.macos.libraries.is_empty()
        && config.platforms.macos.frameworks.is_empty()
        && config.platforms.macos.library_dirs.is_empty()
        && config.platforms.macos.link_flags.is_empty()
        && config.platforms.all.libraries.is_empty()
        && config.platforms.all.frameworks.is_empty()
        && config.platforms.all.library_dirs.is_empty()
        && config.platforms.all.link_flags.is_empty()
    {
        return;
    }
    context.native_configs.push(PackageNativeConfigContext {
        config,
        package_root,
    });
}

fn push_native_lib(context: &mut ImportContext, name: String, package_root: PathBuf) {
    if context
        .native_libs
        .iter()
        .any(|existing| existing.name == name && existing.package_root == package_root)
    {
        return;
    }
    context
        .native_libs
        .push(NativeLibContext { name, package_root });
}

fn import_dependency_from_root(
    expected_name: &str,
    root: &Path,
    expected_version: Option<&str>,
) -> Result<ImportDependency, String> {
    let package_manifest = root.join("ori.pkg.toml");
    if package_manifest.is_file() {
        let manifest = crate::package::load_package_manifest(&package_manifest)?;
        if manifest.name != expected_name {
            return Err(format!(
                "package.dependency_name_mismatch: dependency `{expected_name}` points to package `{}`",
                manifest.name
            ));
        }
        if let Some(version) = expected_version {
            if manifest.version != version {
                return Err(format!(
                    "package.dependency_version_mismatch: dependency `{expected_name}` expected `{version}`, found `{}`",
                    manifest.version
                ));
            }
        }
        return Ok(ImportDependency {
            name: manifest.name,
            root: manifest.root,
            source_root: manifest.entry.parent().map(Path::to_path_buf),
            entry: manifest.entry,
        });
    }

    let project_manifest = root.join("ori.proj");
    if project_manifest.is_file() {
        let config = read_project_config(&project_manifest)?;
        if let Some(version) = expected_version {
            if config.version.as_deref() != Some(version) {
                return Err(format!(
                    "package.dependency_version_mismatch: dependency `{expected_name}` expected `{version}`, found `{}`",
                    config.version.as_deref().unwrap_or("<missing>")
                ));
            }
        }
        return Ok(ImportDependency {
            name: expected_name.to_string(),
            root: config.root,
            entry: config.entry,
            source_root: config.source_root,
        });
    }

    Err(format!(
        "package.dependency_manifest_missing: dependency `{expected_name}` needs `ori.pkg.toml` or `ori.proj` under `{}`",
        root.display()
    ))
}

fn add_import_dependency(context: &mut ImportContext, dependency: ImportDependency) {
    if !context
        .dependencies
        .iter()
        .any(|existing| existing.name == dependency.name && existing.entry == dependency.entry)
    {
        context.dependencies.push(dependency);
    }
}

fn find_package_manifest(start: &Path) -> Option<PathBuf> {
    for ancestor in start.ancestors() {
        let manifest = ancestor.join("ori.pkg.toml");
        if manifest.is_file() {
            return Some(manifest);
        }
    }
    None
}

pub(super) fn load_and_resolve(
    path: &Path,
    cache: &mut SourceCache,
    sink: &mut DiagnosticSink,
) -> Result<ResolvedSources, String> {
    let entry = resolve_entry_path(path)?;
    let context = import_context_for_entry(&entry)?;
    let (loaded, resolved) = load_and_resolve_entry(&entry, None, &context, cache, sink)?;
    Ok(ResolvedSources {
        loaded,
        resolved,
        imports: context,
    })
}

pub(super) fn load_and_resolve_with_entry_source_and_cfg(
    path: &Path,
    source: String,
    cfg: Option<CfgContext>,
    cache: &mut SourceCache,
    sink: &mut DiagnosticSink,
) -> Result<ResolvedSources, String> {
    let entry = resolve_entry_path(path)?;
    let entry = std::fs::canonicalize(entry).unwrap_or_else(|_| path.to_owned());
    let context = import_context_for_entry_with_cfg(&entry, cfg)?;
    let (loaded, resolved) =
        load_and_resolve_entry(&entry, Some((&entry, &source)), &context, cache, sink)?;
    Ok(ResolvedSources {
        loaded,
        resolved,
        imports: context,
    })
}

fn load_and_resolve_entry(
    entry: &Path,
    entry_source: Option<(&Path, &str)>,
    context: &ImportContext,
    cache: &mut SourceCache,
    sink: &mut DiagnosticSink,
) -> Result<(Vec<LoadedSource>, ResolvedModule), String> {
    let mut progress = SourceLoadProgress::default();
    let load_started = std::time::Instant::now();
    load_source_recursive(entry, cache, sink, &mut progress, entry_source, context)?;
    report_internal_pipeline_timing("frontend.load_lex_parse", load_started.elapsed());
    let loaded = progress.loaded;
    let entry_namespace = loaded
        .first()
        .map(|s| namespace_of(&s.ast))
        .unwrap_or_default();
    let files: Vec<_> = loaded.iter().map(|s| (&s.ast, s.file_id)).collect();
    let resolve_started = std::time::Instant::now();
    let resolved = ori_types::resolve::resolve_many(&files, entry_namespace, sink);
    report_internal_pipeline_timing("frontend.resolve", resolve_started.elapsed());
    Ok((loaded, resolved))
}

#[derive(Default)]
struct SourceLoadProgress {
    seen: HashSet<PathBuf>,
    active: Vec<PathBuf>,
    loaded: Vec<LoadedSource>,
}

fn load_source_recursive(
    path: &Path,
    cache: &mut SourceCache,
    sink: &mut DiagnosticSink,
    progress: &mut SourceLoadProgress,
    entry_source: Option<(&Path, &str)>,
    context: &ImportContext,
) -> Result<(), String> {
    let path = std::fs::canonicalize(path).unwrap_or_else(|_| path.to_owned());
    if !progress.seen.insert(path.clone()) {
        return Ok(());
    }
    let source = match entry_source {
        Some((entry_path, source)) if entry_path == path.as_path() => source.to_string(),
        _ => read_file(&path)?,
    };
    let file_id = cache.add(&path, source.clone());
    let tokens = ori_lexer::lex(&source, file_id, sink);
    let mut ast = ori_parser::parse(&tokens, &source, file_id, sink);
    ori_types::conditional::filter_source_file(&mut ast, file_id, &context.cfg, sink);
    let imports: Vec<_> = ast
        .imports
        .iter()
        .map(|i| (i.path.to_string(), i.span, !i.selected.is_empty()))
        .collect();
    progress.loaded.push(LoadedSource {
        path: path.clone(),
        file_id,
        source,
        tokens,
        ast,
    });
    progress.active.push(path.clone());
    for (import, span, has_selected_items) in imports {
        match classify_stdlib_import(&import, has_selected_items) {
            StdlibImportStatus::Implemented => continue,
            StdlibImportStatus::StdlibSources(sources) => {
                for (source_path, expected_namespace) in sources {
                    // Hybrid stdlib pattern: `module ori.net` + `import ori.net = net`
                    // re-exports Layer 1 symbols. The import resolves to the same
                    // `.orl` file already being loaded — not a real cycle.
                    if source_path == *path {
                        continue;
                    }
                    if progress.active.contains(&source_path) {
                        let cycle = import_cycle_description(
                            &progress.active,
                            &progress.loaded,
                            &source_path,
                            &import,
                        );
                        sink.emit(
                            Diagnostic::error(
                                "project.circular_import",
                                format!("import cycle detected: {}", cycle),
                            )
                            .with_label(Label::primary(file_id, span, "cyclic import here"))
                            .with_action(
                                "remove one import or move shared definitions into an acyclic module",
                            ),
                        );
                        validate_import_namespace(
                            &progress.loaded,
                            &source_path,
                            &expected_namespace,
                            file_id,
                            span,
                            sink,
                        );
                        continue;
                    }
                    load_source_recursive(
                        &source_path,
                        cache,
                        sink,
                        progress,
                        entry_source,
                        context,
                    )?;
                    validate_import_namespace(
                        &progress.loaded,
                        &source_path,
                        &expected_namespace,
                        file_id,
                        span,
                        sink,
                    );
                }
                continue;
            }
            StdlibImportStatus::Unknown => {
                sink.emit(
                    Diagnostic::error(
                        "bind.stdlib_module_unknown",
                        format!("standard library module `{}` is not known", import),
                    )
                    .with_label(Label::primary(file_id, span, "stdlib import here"))
                    .with_action("check the module name or use an implemented `ori.*` module"),
                );
                continue;
            }
            StdlibImportStatus::NotStdlib => {}
        }
        match resolve_import_path(&path, &import, context) {
            ImportResolution::Found(import_path) => {
                if progress.active.contains(&import_path) {
                    let cycle = import_cycle_description(
                        &progress.active,
                        &progress.loaded,
                        &import_path,
                        &import,
                    );
                    sink.emit(
                        Diagnostic::error(
                            "project.circular_import",
                            format!("import cycle detected: {}", cycle),
                        )
                        .with_label(Label::primary(file_id, span, "cyclic import here"))
                        .with_action(
                            "remove one import or move shared definitions into an acyclic module",
                        ),
                    );
                    validate_import_namespace(
                        &progress.loaded,
                        &import_path,
                        &import,
                        file_id,
                        span,
                        sink,
                    );
                    continue;
                }
                load_source_recursive(&import_path, cache, sink, progress, entry_source, context)?;
                validate_import_namespace(
                    &progress.loaded,
                    &import_path,
                    &import,
                    file_id,
                    span,
                    sink,
                );
            }
            ImportResolution::Ambiguous(paths) => {
                let mut diagnostic = Diagnostic::error(
                    "bind.import_ambiguous",
                    format!("import `{}` matches more than one file", import),
                )
                    .with_label(Label::primary(file_id, span, "ambiguous import here"))
                    .with_why("the current import search policy found multiple matching `.orl` files")
                    .with_action("keep only one matching file or import through a path that resolves to a single file");
                for path in paths {
                    diagnostic = diagnostic.with_note(format!("candidate: {}", path.display()));
                }
                sink.emit(diagnostic);
            }
            ImportResolution::Missing => {
                sink.emit(
                    Diagnostic::error(
                        "bind.import_not_found",
                        format!("import `{}` not found", import),
                    )
                    .with_label(Label::primary(file_id, span, "imported here"))
                    .with_action("place the imported namespace in a matching `.orl` file"),
                );
            }
        }
    }
    progress.active.pop();
    Ok(())
}

fn validate_import_namespace(
    loaded: &[LoadedSource],
    import_path: &Path,
    import: &str,
    file_id: FileId,
    span: ori_diagnostics::Span,
    sink: &mut DiagnosticSink,
) {
    if let Some(imported) = loaded.iter().find(|s| s.path == import_path) {
        let declared = namespace_of(&imported.ast);
        if declared != import {
            sink.emit(
                Diagnostic::error(
                    "project.namespace_file_mismatch",
                    format!(
                        "import `{}` resolved to file declaring `{}`",
                        import, declared
                    ),
                )
                .with_label(Label::primary(file_id, span, "imported here"))
                .with_action("make the imported file namespace match the import path"),
            );
        }
    }
}

fn import_cycle_description(
    active: &[PathBuf],
    loaded: &[LoadedSource],
    import_path: &Path,
    import: &str,
) -> String {
    let start = active.iter().position(|p| p == import_path).unwrap_or(0);
    let mut parts: Vec<String> = active[start..]
        .iter()
        .map(|path| {
            loaded
                .iter()
                .find(|s| s.path == *path)
                .map(|s| namespace_of(&s.ast))
                .unwrap_or_else(|| path.display().to_string())
        })
        .collect();
    parts.push(import.to_string());
    parts.join(" -> ")
}

pub(super) fn namespace_of(file: &SourceFile) -> String {
    file.namespace.name.to_string()
}

enum StdlibImportStatus {
    Implemented,
    StdlibSources(Vec<(PathBuf, String)>),
    Unknown,
    NotStdlib,
}

fn classify_stdlib_import(import: &str, _has_selected_items: bool) -> StdlibImportStatus {
    if import != "ori" && !import.starts_with("ori.") {
        return StdlibImportStatus::NotStdlib;
    }
    if ori_types::stdlib::is_implemented_stdlib_module(import) {
        if let Some(sources) = find_stdlib_selective_sources(import) {
            return StdlibImportStatus::StdlibSources(sources);
        }
        return StdlibImportStatus::Implemented;
    }
    if let Some(sources) = find_stdlib_selective_sources(import) {
        return StdlibImportStatus::StdlibSources(sources);
    }
    StdlibImportStatus::Unknown
}

/// Resolve a stdlib module import (`ori.string.utils`) to its `.orl` source path.
pub fn stdlib_source_path(import: &str) -> Option<PathBuf> {
    find_stdlib_selective_sources(import)
        .and_then(|sources| sources.into_iter().map(|(path, _)| path).next())
}

fn find_stdlib_selective_sources(import: &str) -> Option<Vec<(PathBuf, String)>> {
    if let Some(path) = find_stdlib_source_module(import) {
        return Some(vec![(path, import.to_string())]);
    }
    find_stdlib_flatten_submodules(import)
}

fn find_stdlib_flatten_submodules(import: &str) -> Option<Vec<(PathBuf, String)>> {
    let relative = import.strip_prefix("ori.")?;
    let stdlib_root = find_stdlib_root()?;
    let mut dir = stdlib_root.clone();
    for segment in relative.split('.') {
        dir.push(segment);
    }
    let mut sources = Vec::new();
    for sub in ["utils", "algorithms"] {
        let candidate = dir.join(format!("{sub}.orl"));
        if candidate.is_file() {
            sources.push((candidate, format!("{import}.{sub}")));
        }
    }
    if sources.is_empty() {
        None
    } else {
        Some(sources)
    }
}

fn find_stdlib_source_module(import: &str) -> Option<PathBuf> {
    let relative = import.strip_prefix("ori.")?;
    let stdlib_root = find_stdlib_root()?;
    let mut relative_path = PathBuf::new();
    for segment in relative.split('.') {
        relative_path.push(segment);
    }
    let candidate = stdlib_root.join(&relative_path).with_extension("orl");
    if candidate.is_file() {
        Some(candidate)
    } else {
        None
    }
}

/// Resolve the stdlib root directory.
///
/// Order:
/// 1. `ORI_STDLIB_ROOT`
/// 2. End-user / local package next to the binary:
///    - `<exe_dir>/stdlib` (flat package)
///    - `<exe_dir>/../stdlib` (`…/bin/ori` + `…/stdlib`, e.g. `~/.local/share/ori`)
/// 3. Dev layout from the remapped driver manifest or a discovered workspace
///    (only when present; never preferred over a package layout next to the binary)
/// 4. Walk cwd parents for a `stdlib/` directory
pub fn find_stdlib_root() -> Option<PathBuf> {
    let dir_if_stdlib = |path: PathBuf| -> Option<PathBuf> {
        if path.is_dir() {
            Some(std::fs::canonicalize(&path).unwrap_or(path))
        } else {
            None
        }
    };

    if let Ok(path) = std::env::var("ORI_STDLIB_ROOT") {
        if let Some(root) = dir_if_stdlib(PathBuf::from(path)) {
            return Some(root);
        }
    }

    let package_near_exe = || -> Option<PathBuf> {
        let exe = std::env::current_exe().ok()?;
        let exe_dir = exe.parent()?;
        // Layout A: stdlib beside the binary
        if let Some(root) = dir_if_stdlib(exe_dir.join("stdlib")) {
            return Some(root);
        }
        // Layout B: `…/bin/ori` + `…/stdlib` (user install under ~/.local/share/ori)
        if let Some(prefix) = exe_dir.parent() {
            if let Some(root) = dir_if_stdlib(prefix.join("stdlib")) {
                return Some(root);
            }
        }
        None
    };

    // Always prefer a real install next to the running binary over the
    // source workspace used during compilation or discovered at runtime.
    if let Some(path) = package_near_exe() {
        return Some(path);
    }

    let packaged_only = std::env::var_os("ORI_REQUIRE_PACKAGED_RUNTIME").is_some_and(|v| {
        let s = v.to_string_lossy();
        s == "1" || s.eq_ignore_ascii_case("true") || s.eq_ignore_ascii_case("yes")
    });
    if packaged_only {
        return None;
    }

    let dev_candidate = super::runtime::repo_root().join("stdlib");
    if let Some(root) = dir_if_stdlib(dev_candidate) {
        return Some(root);
    }

    if let Ok(cwd) = std::env::current_dir() {
        let mut dir = cwd;
        for _ in 0..8 {
            let candidate = dir.join("stdlib");
            // Accept either nested helpers (string/) or parent modules (string.orl).
            if candidate.is_dir()
                && (candidate.join("string.orl").is_file()
                    || candidate.join("string").is_dir()
                    || candidate.join("list.orl").is_file()
                    || candidate.join("list").is_dir())
            {
                return dir_if_stdlib(candidate);
            }
            if let Some(parent) = dir.parent() {
                dir = parent.to_owned();
            } else {
                break;
            }
        }
    }
    None
}

enum ImportResolution {
    Found(PathBuf),
    Ambiguous(Vec<PathBuf>),
    Missing,
}

fn resolve_import_path(importer: &Path, import: &str, context: &ImportContext) -> ImportResolution {
    // Directory of the file performing the import
    let Some(dir) = importer.parent() else {
        return ImportResolution::Missing;
    };

    // Determine the package/project scope for the importing file.  A package
    // may resolve its own local modules, but it must not accidentally reach
    // into a sibling package or the application that consumes it.
    let scope_root = find_package_manifest(dir)
        .and_then(|manifest| manifest.parent().map(Path::to_path_buf))
        .or_else(|| find_project_root(dir))
        .and_then(|path| std::fs::canonicalize(path).ok());

    let mut matches = Vec::new();

    // Walk ancestors from the importer's directory upwards, stopping at the
    // package/project boundary.
    for base in dir.ancestors() {
        for candidate in import_candidates(base, import) {
            if candidate.is_file() {
                let path = std::fs::canonicalize(&candidate).unwrap_or(candidate);
                if !matches.contains(&path) {
                    matches.push(path);
                }
            }
        }
        if scope_root
            .as_ref()
            .is_some_and(|root| std::fs::canonicalize(base).ok().as_ref() == Some(root))
        {
            break;
        }
    }

    for dependency in &context.dependencies {
        for candidate in dependency_import_candidates(dependency, import) {
            if candidate.is_file() {
                let path = std::fs::canonicalize(&candidate).unwrap_or(candidate);
                if !matches.contains(&path) {
                    matches.push(path);
                }
            }
        }
    }

    match matches.len() {
        0 => ImportResolution::Missing,
        1 => ImportResolution::Found(matches.remove(0)),
        _ => ImportResolution::Ambiguous(matches),
    }
}

fn dependency_import_candidates(dependency: &ImportDependency, import: &str) -> Vec<PathBuf> {
    let mut candidates = Vec::new();
    if import == dependency.name {
        candidates.push(dependency.entry.clone());
        return candidates;
    }

    let prefix = format!("{}.", dependency.name);
    let suffix = import.strip_prefix(&prefix);
    // Dependency modules are package-qualified.  Searching every dependency
    // for a bare module made two packages with the same `util` module collide
    // nondeterministically; the caller must write `package.util` instead.
    let Some(suffix) = suffix else {
        return candidates;
    };
    for base in dependency_search_bases(dependency) {
        candidates.extend(import_candidates(&base, suffix));
    }
    candidates
}

fn dependency_search_bases(dependency: &ImportDependency) -> Vec<PathBuf> {
    let mut bases = Vec::new();
    if let Some(source_root) = &dependency.source_root {
        bases.push(source_root.clone());
    }
    if let Some(parent) = dependency.entry.parent() {
        bases.push(parent.to_path_buf());
    }
    bases.push(dependency.root.clone());
    dedup_paths(bases)
}

fn import_candidates(base: &Path, import: &str) -> Vec<PathBuf> {
    let parts: Vec<_> = import.split('.').filter(|p| !p.is_empty()).collect();
    let mut candidates = Vec::new();
    if !parts.is_empty() {
        let mut nested_dir = base.to_path_buf();
        for part in &parts {
            nested_dir.push(part);
        }
        let mut nested = nested_dir.clone();
        nested.set_extension("orl");
        candidates.push(nested.clone());
        candidates.push(nested_dir.join("mod.orl"));
        candidates.push(nested_dir.join("index.orl"));

        if let Some(last) = parts.last() {
            candidates.push(base.join(format!("{last}.orl")));
            candidates.push(base.join(last).join("mod.orl"));
            candidates.push(base.join(last).join("index.orl"));
        }
    }
    candidates
}

/// Walk ancestors upwards from `start` until an `ori.proj` file is found. The directory
/// that contains the manifest is considered the project root.
fn find_project_root(start: &Path) -> Option<PathBuf> {
    for ancestor in start.ancestors() {
        let proj = ancestor.join("ori.proj");
        if proj.is_file() {
            return Some(ancestor.to_path_buf());
        }
    }
    None
}
