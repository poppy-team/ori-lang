//! Native runtime discovery and link metadata for the Ori driver.
//!
//! This module owns platform artifact lookup, ABI/version validation, and the
//! Cargo fallback used by AOT and JIT routes. The parent pipeline re-exports
//! the small public policy functions so existing CLI/LSP callers keep working.

use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};

pub(super) const ORI_VERSION: &str = env!("CARGO_PKG_VERSION");
pub(super) const ORI_DRIVER_ABI_VERSION: &str = ori_runtime::ORI_ABI_VERSION;
pub(super) const NATIVE_RUNTIME_MISSING: &str = "native.runtime_missing";
pub(super) const NATIVE_RUNTIME_METADATA_INVALID: &str = "native.runtime_metadata_invalid";
pub(super) const NATIVE_RUNTIME_METADATA_MISMATCH: &str = "native.runtime_metadata_mismatch";
pub(super) const NATIVE_ABI_MISMATCH: &str = "native.abi_mismatch";

#[derive(Clone, Debug)]
pub(super) struct NativeRuntimeLink {
    pub(super) runtime_lib: PathBuf,
    pub(super) native_static_libs: Vec<String>,
}

impl NativeRuntimeLink {
    pub(super) fn link_args(&self) -> Vec<PathBuf> {
        self.link_args_for(false)
    }

    /// Link line for executables (`shared = false`) or embeddable shared
    /// libraries (`shared = true`). Shared mode prefers the runtime **cdylib**
    /// (avoids rustc compiler-builtins vs libgcc clashes with `--whole-archive`
    /// on the staticlib) and drops executable-only flags like `-no-pie`.
    pub(super) fn link_args_for(&self, shared: bool) -> Vec<PathBuf> {
        let mut args = Vec::with_capacity(1 + self.native_static_libs.len());
        if shared {
            let cdylib = runtime_cdylib_beside_static(&self.runtime_lib);
            if let Some(so) = cdylib {
                args.push(so);
            } else if cfg!(windows) {
                // MSVC link.exe/rust-lld cannot consume a DLL directly. The
                // staged `.lib` is the static runtime (an import library, if
                // a separate DLL import archive is available, is not part of
                // the stable package contract), so link it into the generated
                // library and keep the runtime DLL for hosted JIT loading.
                args.push(self.runtime_lib.clone());
            } else {
                // Fallback: staticlib without whole-archive (may miss unused symbols).
                args.push(self.runtime_lib.clone());
                args.push(PathBuf::from("-Wl,-u,ori_rt_init"));
                args.push(PathBuf::from("-Wl,-u,ori_rt_shutdown"));
            }
        } else {
            args.push(self.runtime_lib.clone());
        }
        for flag in &self.native_static_libs {
            if shared && (flag == "-no-pie" || flag == "-pie" || flag == "-lc") {
                continue;
            }
            args.push(PathBuf::from(flag));
        }
        args
    }
}

fn runtime_cdylib_beside_static(static_lib: &Path) -> Option<PathBuf> {
    let parent = static_lib.parent()?;
    let name = static_lib.file_name()?.to_str()?;
    // libori_runtime.a → libori_runtime.so / ori_runtime.dll / libori_runtime.dylib
    let so = if name.ends_with(".a") {
        let stem = name.trim_end_matches(".a");
        if cfg!(windows) {
            parent.join(format!("{}.dll", stem.trim_start_matches("lib")))
        } else if cfg!(target_os = "macos") {
            parent.join(format!("{stem}.dylib"))
        } else {
            parent.join(format!("{stem}.so"))
        }
    } else {
        return None;
    };
    so.is_file().then_some(so)
}

/// Locate and validate the native runtime static library for the current target.
pub(super) fn find_native_runtime_link() -> Result<NativeRuntimeLink, String> {
    static CACHED: std::sync::OnceLock<Result<NativeRuntimeLink, String>> =
        std::sync::OnceLock::new();
    CACHED
        .get_or_init(find_native_runtime_link_uncached)
        .clone()
}

fn find_native_runtime_link_uncached() -> Result<NativeRuntimeLink, String> {
    if let Ok(path) = std::env::var("ORI_RUNTIME_LIB") {
        let path = PathBuf::from(path);
        return if path.is_file() {
            let target = native_target_triple();
            let artifact = native_runtime_artifact_name(&target);
            native_runtime_link_for(path, &target, artifact)
        } else {
            Err(format!(
                "ORI_RUNTIME_LIB points to `{}`, but that file does not exist",
                path.display()
            ))
        };
    }

    let target = native_target_triple();
    let artifact = native_runtime_artifact_name(&target);
    let mut searched = Vec::new();
    let packaged_candidates = packaged_runtime_candidates(&target, artifact);

    for candidate in &packaged_candidates {
        if candidate.is_file() {
            return native_runtime_link_for(candidate.clone(), &target, artifact);
        }
    }
    searched.extend(packaged_candidates);

    if env_flag("ORI_REQUIRE_PACKAGED_RUNTIME") {
        return Err(missing_native_runtime_message(
            &target, artifact, &searched, true,
        ));
    }

    build_native_runtime_with_cargo()?;

    let cargo_candidates = cargo_runtime_candidates(&target, artifact);
    for candidate in &cargo_candidates {
        if candidate.is_file() {
            return native_runtime_link_for(candidate.clone(), &target, artifact);
        }
    }
    searched.extend(cargo_candidates);

    Err(missing_native_runtime_message(
        &target, artifact, &searched, false,
    ))
}

pub fn env_flag(name: &str) -> bool {
    matches!(
        std::env::var(name).ok().as_deref(),
        Some("1" | "true" | "TRUE" | "yes" | "YES")
    )
}

/// Returns true when `ori run` should use the JIT path instead of AOT compile+link.
///
/// - Explicit opt-in: `ORI_USE_JIT=1`
/// - Explicit opt-out: `ORI_USE_AOT=1`
/// - Default: JIT when a runtime cdylib is available (packaged layout or cargo-built)
pub fn should_use_jit_for_run() -> bool {
    if env_flag("ORI_USE_AOT") {
        return false;
    }
    if env_flag("ORI_USE_JIT") {
        return true;
    }
    find_native_runtime_cdylib().is_ok()
}

pub(super) fn find_native_runtime_cdylib() -> Result<PathBuf, String> {
    if let Ok(path) = std::env::var("ORI_RUNTIME_CDYLIB") {
        let path = PathBuf::from(path);
        return if path.is_file() {
            Ok(path)
        } else {
            Err(format!(
                "ORI_RUNTIME_CDYLIB points to `{}`, but that file does not exist",
                path.display()
            ))
        };
    }

    let target = native_target_triple();
    let cdylib_artifact = native_runtime_cdylib_name(&target);
    let mut searched = Vec::new();

    let packaged_candidates = packaged_runtime_candidates(&target, cdylib_artifact);
    for candidate in &packaged_candidates {
        if candidate.is_file() {
            return Ok(candidate.clone());
        }
    }
    searched.extend(packaged_candidates);

    if env_flag("ORI_REQUIRE_PACKAGED_RUNTIME") {
        return Err(missing_native_runtime_message(
            &target,
            cdylib_artifact,
            &searched,
            true,
        ));
    }

    let cargo_candidates = cargo_runtime_candidates(&target, cdylib_artifact);
    for candidate in &cargo_candidates {
        if candidate.is_file() {
            return Ok(candidate.clone());
        }
    }
    searched.extend(cargo_candidates);

    Err(missing_native_runtime_message(
        &target,
        cdylib_artifact,
        &searched,
        false,
    ))
}

pub(super) fn missing_native_runtime_message(
    target: &str,
    artifact: &str,
    searched: &[PathBuf],
    packaged_only: bool,
) -> String {
    let mut message = format!(
        "{NATIVE_RUNTIME_MISSING}: native Ori runtime `{artifact}` for target `{target}` was not found."
    );
    if packaged_only {
        message.push_str(" Packaged runtime mode is enabled by ORI_REQUIRE_PACKAGED_RUNTIME=1.");
    }
    message.push_str(&format!(
        "\nexpected package path: runtime/{target}/{artifact}\nstaging command: .\\tools\\stage_native_runtime.ps1 -Target {target}"
    ));
    if !packaged_only {
        message.push_str("\nworkspace fallback: cargo build -p ori-runtime --lib");
    }
    if !searched.is_empty() {
        message.push_str("\nsearched paths:");
        for path in searched {
            message.push_str(&format!("\n- {}", path.display()));
        }
    }
    message
}

pub(super) fn native_runtime_link_for(
    runtime_lib: PathBuf,
    target: &str,
    artifact: &str,
) -> Result<NativeRuntimeLink, String> {
    let metadata_path = runtime_lib
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .join("runtime-link.json");
    let native_static_libs = if metadata_path.is_file() {
        let metadata = read_runtime_link_metadata(&metadata_path)?;
        if metadata.target != target {
            return Err(format!(
                "{NATIVE_RUNTIME_METADATA_MISMATCH}: runtime metadata `{}` targets `{}`, but the current target is `{target}`",
                metadata_path.display(),
                metadata.target
            ));
        }
        if metadata.runtime != artifact {
            return Err(format!(
                "{NATIVE_RUNTIME_METADATA_MISMATCH}: runtime metadata `{}` names runtime `{}`, but `{artifact}` was expected",
                metadata_path.display(),
                metadata.runtime
            ));
        }
        if metadata.ori_version != ORI_VERSION {
            return Err(format!(
                "{NATIVE_RUNTIME_METADATA_MISMATCH}: runtime metadata `{}` was staged for Ori {}, but the driver is Ori {}",
                metadata_path.display(),
                metadata.ori_version,
                ORI_VERSION
            ));
        }
        if metadata.abi_version != ORI_DRIVER_ABI_VERSION {
            return Err(format!(
                "{NATIVE_ABI_MISMATCH}: runtime metadata `{}` uses ABI {}, but the driver expects ABI {}",
                metadata_path.display(),
                metadata.abi_version,
                ORI_DRIVER_ABI_VERSION
            ));
        }
        match metadata.runtime_sha256.as_deref() {
            Some(expected) => {
                let actual = sha256_file(&runtime_lib)?;
                if !actual.eq_ignore_ascii_case(expected) {
                    return Err(format!(
                        "{NATIVE_ABI_MISMATCH}: runtime static library `{}` digest mismatch (metadata {}, actual {})",
                        runtime_lib.display(), expected, actual
                    ));
                }
            }
            None if env_flag("ORI_REQUIRE_PACKAGED_RUNTIME") => {
                return Err(format!(
                    "{NATIVE_ABI_MISMATCH}: runtime metadata `{}` is missing runtime_sha256",
                    metadata_path.display()
                ));
            }
            None => {}
        }
        metadata.native_static_libs
    } else {
        native_static_libs_for_target(target)
            .iter()
            .map(|lib| (*lib).to_string())
            .collect()
    };

    Ok(NativeRuntimeLink {
        runtime_lib,
        native_static_libs,
    })
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct RuntimeLinkMetadata {
    pub(super) target: String,
    pub(super) runtime: String,
    pub(super) runtime_cdylib: Option<String>,
    pub(super) runtime_sha256: Option<String>,
    pub(super) ori_version: String,
    pub(super) abi_version: String,
    pub(super) native_static_libs: Vec<String>,
}

pub(super) fn read_runtime_link_metadata(path: &Path) -> Result<RuntimeLinkMetadata, String> {
    let source = std::fs::read_to_string(path).map_err(|e| {
        format!(
            "{NATIVE_RUNTIME_METADATA_INVALID}: cannot read runtime metadata `{}`: {e}",
            path.display()
        )
    })?;
    let target = json_string_field(&source, "target").ok_or_else(|| {
        format!(
            "{NATIVE_RUNTIME_METADATA_INVALID}: runtime metadata `{}` is missing string field `target`",
            path.display()
        )
    })?;
    let runtime = json_string_field(&source, "runtime").ok_or_else(|| {
        format!(
            "{NATIVE_RUNTIME_METADATA_INVALID}: runtime metadata `{}` is missing string field `runtime`",
            path.display()
        )
    })?;
    let runtime_cdylib =
        json_string_field(&source, "runtime_cdylib").filter(|value| !value.is_empty());
    let runtime_sha256 =
        json_string_field(&source, "runtime_sha256").filter(|value| !value.is_empty());
    let ori_version = json_string_field(&source, "ori_version").ok_or_else(|| {
        format!(
            "{NATIVE_RUNTIME_METADATA_INVALID}: runtime metadata `{}` is missing string field `ori_version`",
            path.display()
        )
    })?;
    let abi_version = json_string_field(&source, "abi_version").ok_or_else(|| {
        format!(
            "{NATIVE_RUNTIME_METADATA_INVALID}: runtime metadata `{}` is missing string field `abi_version`",
            path.display()
        )
    })?;
    let native_static_libs =
        json_string_array_field(&source, "native_static_libs").ok_or_else(|| {
            format!(
                "{NATIVE_RUNTIME_METADATA_INVALID}: runtime metadata `{}` is missing string array field `native_static_libs`",
                path.display()
            )
        })?;
    Ok(RuntimeLinkMetadata {
        target,
        runtime,
        runtime_cdylib,
        runtime_sha256,
        ori_version,
        abi_version,
        native_static_libs,
    })
}

fn sha256_file(path: &Path) -> Result<String, String> {
    let mut file = std::fs::File::open(path).map_err(|error| {
        format!(
            "{NATIVE_RUNTIME_METADATA_INVALID}: cannot read runtime artifact `{}`: {error}",
            path.display()
        )
    })?;
    let mut digest = Sha256::new();
    let mut buffer = [0_u8; 128 * 1024];
    loop {
        let read = std::io::Read::read(&mut file, &mut buffer).map_err(|error| {
            format!(
                "{NATIVE_RUNTIME_METADATA_INVALID}: cannot hash runtime artifact `{}`: {error}",
                path.display()
            )
        })?;
        if read == 0 {
            break;
        }
        digest.update(&buffer[..read]);
    }
    Ok(format!("{:x}", digest.finalize()))
}

fn json_string_field(source: &str, field: &str) -> Option<String> {
    let rest = json_field_value(source, field)?;
    let (value, _) = parse_json_string(rest.trim_start())?;
    Some(value)
}

fn json_string_array_field(source: &str, field: &str) -> Option<Vec<String>> {
    let mut rest = json_field_value(source, field)?.trim_start();
    rest = rest.strip_prefix('[')?.trim_start();
    let mut values = Vec::new();
    loop {
        if rest.starts_with(']') {
            return Some(values);
        }
        let (value, consumed) = parse_json_string(rest)?;
        values.push(value);
        rest = rest[consumed..].trim_start();
        if let Some(next) = rest.strip_prefix(',') {
            rest = next.trim_start();
            continue;
        }
        rest.strip_prefix(']')?;
        return Some(values);
    }
}

fn json_field_value<'a>(source: &'a str, field: &str) -> Option<&'a str> {
    let key = format!("\"{field}\"");
    let after_key = source.split_once(&key)?.1;
    let after_colon = after_key.split_once(':')?.1;
    Some(after_colon)
}

fn parse_json_string(source: &str) -> Option<(String, usize)> {
    let mut chars = source.char_indices();
    let (_, first) = chars.next()?;
    if first != '"' {
        return None;
    }
    let mut out = String::new();
    let mut escaped = false;
    for (index, ch) in chars {
        if escaped {
            let value = match ch {
                '"' => '"',
                '\\' => '\\',
                '/' => '/',
                'b' => '\u{0008}',
                'f' => '\u{000c}',
                'n' => '\n',
                'r' => '\r',
                't' => '\t',
                _ => ch,
            };
            out.push(value);
            escaped = false;
            continue;
        }
        match ch {
            '\\' => escaped = true,
            '"' => return Some((out, index + ch.len_utf8())),
            _ => out.push(ch),
        }
    }
    None
}

pub(crate) fn native_target_triple() -> String {
    std::env::var("ORI_TARGET_TRIPLE")
        .ok()
        .filter(|value| !value.trim().is_empty())
        .map(|value| value.trim().to_string())
        .unwrap_or_else(default_native_target_triple)
}

/// Native Cranelift and the staged runtime currently target the host triple.
///
/// `ORI_TARGET_TRIPLE` is still used by the front-end for conditional
/// compilation, but accepting a different triple here would silently emit
/// host machine code and then link it with the wrong runtime artifact. Fail
/// before code generation until a real cross-target backend and runtime matrix
/// exists.
pub(crate) fn ensure_native_codegen_target() -> Result<(), String> {
    let requested = native_target_triple();
    let host = default_native_target_triple();
    validate_native_codegen_target(&requested, &host)
}

fn validate_native_codegen_target(requested: &str, host: &str) -> Result<(), String> {
    if requested == host {
        Ok(())
    } else {
        Err(format!(
            "native.target_unsupported: requested target `{requested}` differs from host `{host}`; native code generation is host-only until a cross-target runtime is available"
        ))
    }
}

fn default_native_target_triple() -> String {
    if cfg!(all(windows, target_env = "msvc")) {
        format!("{}-pc-windows-msvc", native_target_arch())
    } else if cfg!(all(windows, target_env = "gnu")) {
        format!("{}-pc-windows-gnu", native_target_arch())
    } else if cfg!(target_os = "linux") {
        format!("{}-unknown-linux-gnu", native_target_arch())
    } else if cfg!(target_os = "macos") {
        format!("{}-apple-darwin", native_target_arch())
    } else {
        format!("{}-{}", std::env::consts::ARCH, std::env::consts::OS)
    }
}

fn native_target_arch() -> &'static str {
    match std::env::consts::ARCH {
        "x86" => "i686",
        arch => arch,
    }
}

pub(super) fn native_runtime_artifact_name(target: &str) -> &'static str {
    if target.contains("windows-msvc") {
        "ori_runtime.lib"
    } else {
        "libori_runtime.a"
    }
}

fn native_runtime_cdylib_name(target: &str) -> &'static str {
    if target.contains("windows-msvc") {
        "ori_runtime.dll"
    } else if target.contains("apple-darwin") {
        "libori_runtime.dylib"
    } else {
        "libori_runtime.so"
    }
}

pub(super) fn native_lib_cdylib_name(target: &str, name: &str) -> String {
    if target.contains("windows-msvc") {
        format!("{}.dll", name)
    } else if target.contains("apple-darwin") {
        format!("lib{}.dylib", name)
    } else {
        format!("lib{}.so", name)
    }
}

pub(super) fn native_lib_static_name(target: &str, name: &str) -> String {
    if target.contains("windows-msvc") {
        format!("{}.lib", name)
    } else {
        format!("lib{}.a", name)
    }
}

fn packaged_runtime_candidates(target: &str, artifact: &str) -> Vec<PathBuf> {
    let mut candidates = Vec::new();
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            candidates.push(dir.join("runtime").join(target).join(artifact));
            if let Some(parent) = dir.parent() {
                candidates.push(parent.join("runtime").join(target).join(artifact));
            }
        }
    }
    candidates.push(repo_root().join("runtime").join(target).join(artifact));
    candidates
}

fn cargo_runtime_candidates(target: &str, artifact: &str) -> Vec<PathBuf> {
    let target_dir = cargo_target_dir();
    let preferred = if cfg!(debug_assertions) {
        ["debug", "release"]
    } else {
        ["release", "debug"]
    };
    let mut candidates = Vec::new();
    for profile in preferred {
        candidates.push(target_dir.join(profile).join(artifact));
        candidates.push(target_dir.join(target).join(profile).join(artifact));
    }
    candidates
}

fn cargo_target_dir() -> PathBuf {
    std::env::var("CARGO_TARGET_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|_| cargo_workspace_root().join("target"))
}

/// The Cargo workspace holding this crate (`<repo>/compiler`): where the
/// fallback `cargo build -p ori-runtime` must run and where `target/` lives.
pub(super) fn cargo_workspace_root() -> PathBuf {
    let manifest = Path::new(env!("ORI_DRIVER_MANIFEST_DIR"));
    if manifest.is_absolute() && manifest.join("Cargo.toml").is_file() {
        return manifest
            .ancestors()
            .nth(2)
            .unwrap_or(manifest)
            .to_path_buf();
    }
    std::env::current_exe()
        .ok()
        .and_then(|exe| development_workspace_from(&exe))
        .or_else(|| {
            std::env::current_dir()
                .ok()
                .and_then(|cwd| development_workspace_from(&cwd))
        })
        .unwrap_or_else(|| {
            manifest
                .ancestors()
                .nth(2)
                .unwrap_or(manifest)
                .to_path_buf()
        })
}

fn development_workspace_from(path: &Path) -> Option<PathBuf> {
    path.ancestors().find_map(|ancestor| {
        [ancestor.to_path_buf(), ancestor.join("compiler")]
            .into_iter()
            .find(|candidate| {
                candidate.join("Cargo.toml").is_file()
                    && candidate.join("crates/ori-driver/Cargo.toml").is_file()
            })
    })
}

/// The repository root (one level above the Cargo workspace): where
/// `tools/stage_native_runtime.sh` stages `runtime/<target>/` artifacts.
pub(super) fn repo_root() -> PathBuf {
    let workspace = cargo_workspace_root();
    workspace.parent().unwrap_or(&workspace).to_path_buf()
}

fn build_native_runtime_with_cargo() -> Result<(), String> {
    let cargo = std::env::var("CARGO").unwrap_or_else(|_| "cargo".to_string());
    let mut cmd = std::process::Command::new(&cargo);
    cmd.current_dir(cargo_workspace_root())
        .arg("build")
        .arg("-p")
        .arg("ori-runtime")
        .arg("--lib");
    if !cfg!(debug_assertions) {
        cmd.arg("--release");
    }

    let output = cmd
        .output()
        .map_err(|e| {
            format!(
                "{NATIVE_RUNTIME_MISSING}: failed to start Cargo while building the native Ori runtime: {e}"
            )
        })?;
    if output.status.success() {
        Ok(())
    } else {
        Err(format!(
            "{NATIVE_RUNTIME_MISSING}: failed to build native Ori runtime with `{cargo} build -p ori-runtime --lib`\nstatus: {}\nstdout:\n{}\nstderr:\n{}",
            output.status,
            String::from_utf8_lossy(&output.stdout).trim(),
            String::from_utf8_lossy(&output.stderr).trim()
        ))
    }
}

#[cfg(test)]
pub(super) fn runtime_link_metadata_json(target: &str, artifact: &str) -> String {
    let native_static_libs = native_static_libs_for_target(target);
    format!(
        "{{\n  \"target\": \"{target}\",\n  \"runtime\": \"{artifact}\",\n  \"ori_version\": \"{ORI_VERSION}\",\n  \"abi_version\": \"{ORI_DRIVER_ABI_VERSION}\",\n  \"native_static_libs\": [{}]\n}}\n",
        native_static_libs
            .iter()
            .map(|lib| format!("\"{lib}\""))
            .collect::<Vec<_>>()
            .join(", ")
    )
}

pub(super) fn native_static_libs_for_target(target: &str) -> &'static [&'static str] {
    if target.contains("windows-msvc") {
        &[
            "legacy_stdio_definitions.lib",
            "bcrypt.lib",
            "advapi32.lib",
            "kernel32.lib",
            "ntdll.lib",
            "userenv.lib",
            "ws2_32.lib",
            "dbghelp.lib",
            "/defaultlib:msvcrt",
        ]
    } else if target.contains("windows-gnu") {
        &[
            "-lbcrypt",
            "-ladvapi32",
            "-lkernel32",
            "-lntdll",
            "-luserenv",
            "-lws2_32",
            "-ldbghelp",
        ]
    } else if target.contains("linux") {
        &["-lpthread", "-ldl", "-lm", "-no-pie"]
    } else {
        &[]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn development_root_follows_relocated_target_layouts() {
        let root = std::env::temp_dir().join(format!("ori_dev_roots_{}", std::process::id()));
        let workspace = root.join("compiler");
        std::fs::create_dir_all(workspace.join("crates/ori-driver")).unwrap();
        std::fs::write(workspace.join("Cargo.toml"), "[workspace]\n").unwrap();
        std::fs::write(
            workspace.join("crates/ori-driver/Cargo.toml"),
            "[package]\n",
        )
        .unwrap();
        for layout in [
            "target/debug/ori",
            "target/release/ori",
            "target/x86_64-unknown-linux-gnu/release/ori",
            "target/debug/deps/test",
        ] {
            assert_eq!(
                development_workspace_from(&workspace.join(layout)),
                Some(workspace.clone())
            );
        }
        for path in [&root, &root.join("examples/hello"), &workspace] {
            assert_eq!(development_workspace_from(path), Some(workspace.clone()));
        }
        assert_eq!(
            development_workspace_from(&root.with_extension("unrelated").join("bin/ori")),
            None
        );
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn default_native_target_uses_the_compiled_architecture() {
        let expected_arch = match std::env::consts::ARCH {
            "x86" => "i686",
            arch => arch,
        };
        let target = default_native_target_triple();

        assert!(
            target.starts_with(&format!("{expected_arch}-")),
            "target `{target}` does not match host architecture `{expected_arch}`"
        );
    }

    #[test]
    fn native_codegen_rejects_cross_target_requests() {
        assert!(validate_native_codegen_target(
            "x86_64-unknown-linux-gnu",
            "x86_64-unknown-linux-gnu"
        )
        .is_ok());
        let error =
            validate_native_codegen_target("wasm32-unknown-unknown", "x86_64-unknown-linux-gnu")
                .expect_err("cross-target native codegen must fail closed");
        assert!(error.contains("native.target_unsupported"));
        assert!(error.contains("wasm32-unknown-unknown"));
    }

    #[test]
    fn shared_link_args_prefer_a_runtime_cdylib_when_present() {
        let root =
            std::env::temp_dir().join(format!("ori_runtime_link_args_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).expect("create test directory");
        let (static_name, cdylib_name) = if cfg!(windows) {
            ("ori_runtime.lib", "ori_runtime.dll")
        } else if cfg!(target_os = "macos") {
            ("libori_runtime.a", "libori_runtime.dylib")
        } else {
            ("libori_runtime.a", "libori_runtime.so")
        };
        let static_lib = root.join(static_name);
        let cdylib = root.join(cdylib_name);
        std::fs::write(&static_lib, b"static").expect("write static runtime");
        std::fs::write(&cdylib, b"shared").expect("write shared runtime");

        let link = NativeRuntimeLink {
            runtime_lib: static_lib,
            native_static_libs: vec!["-no-pie".to_string(), "-lpthread".to_string()],
        };
        let args = link.link_args_for(true);

        assert_eq!(args.first(), Some(&cdylib));
        assert!(!args
            .iter()
            .any(|arg| arg == std::path::Path::new("-no-pie")));
        assert!(args
            .iter()
            .any(|arg| arg == std::path::Path::new("-lpthread")));

        let _ = std::fs::remove_dir_all(root);
    }
}
