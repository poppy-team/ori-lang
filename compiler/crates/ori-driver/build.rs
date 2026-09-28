fn main() {
    let manifest =
        std::env::var("CARGO_MANIFEST_DIR").expect("Cargo must provide CARGO_MANIFEST_DIR");
    let flags = std::env::var("CARGO_ENCODED_RUSTFLAGS").unwrap_or_default();
    let manifest = remap_manifest_dir(&manifest, &flags);
    println!("cargo:rustc-env=ORI_DRIVER_MANIFEST_DIR={manifest}");
    println!("cargo:rerun-if-env-changed=CARGO_ENCODED_RUSTFLAGS");
    println!("cargo:rerun-if-changed=build.rs");
}

pub fn remap_manifest_dir(manifest: &str, flags: &str) -> String {
    let mut remapped = manifest.to_owned();
    let mut flags = flags.split('\x1f');
    while let Some(flag) = flags.next() {
        let mapping = if flag == "--remap-path-prefix" {
            flags.next()
        } else {
            flag.strip_prefix("--remap-path-prefix=")
        };
        if let Some((from, to)) = mapping.and_then(|value| value.rsplit_once('=')) {
            if let Ok(suffix) = std::path::Path::new(manifest).strip_prefix(from) {
                remapped = std::path::Path::new(to)
                    .join(suffix)
                    .to_string_lossy()
                    .into_owned();
            }
        }
    }
    remapped
}
