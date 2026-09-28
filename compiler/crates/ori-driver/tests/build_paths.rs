use std::path::Path;

#[allow(dead_code)]
#[path = "../build.rs"]
mod driver_build;

#[test]
fn manifest_remapping_matches_last_prefix_and_preserves_unmapped_development() {
    let manifest = "/work/tree/compiler/crates/ori-driver";
    assert_eq!(driver_build::remap_manifest_dir(manifest, ""), manifest);
    assert_eq!(
        Path::new(&driver_build::remap_manifest_dir(
            manifest,
            "--remap-path-prefix=/work=/first\x1f--remap-path-prefix\x1f/work/tree=/ori-source"
        )),
        Path::new("/ori-source/compiler/crates/ori-driver")
    );
    assert_eq!(
        driver_build::remap_manifest_dir(manifest, "--remap-path-prefix=/work/trees=/other"),
        manifest
    );
    assert_eq!(
        Path::new(&driver_build::remap_manifest_dir(
            "/work=a/crate",
            "--remap-path-prefix=/work=a=/source"
        )),
        Path::new("/source/crate")
    );
    assert_eq!(
        driver_build::remap_manifest_dir(manifest, "--remap-path-prefix=/work/tree="),
        Path::new("compiler/crates/ori-driver").to_string_lossy()
    );
}
