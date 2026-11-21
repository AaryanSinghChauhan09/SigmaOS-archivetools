//! Integration tests for sigmaos-archivetools.

use std::path::Path;

use archivetools::{plan_cleanup, scan_snapshot, version_sort_key};

fn make_snapshot(root: &Path, date: &str, repo_arch: &str, pkgs: &[(&str, &str)]) {
    let d = root.join(date).join(repo_arch);
    std::fs::create_dir_all(&d).unwrap();
    for (stem, content) in pkgs {
        std::fs::write(d.join(format!("{stem}.pkg.tar.zst")), content).unwrap();
    }
}

#[test]
fn test_scan_and_cleanup() {
    let dir = tempfile::tempdir().unwrap();
    // foo 1.0 in two dates; bar 2.0 only in old date.
    make_snapshot(dir.path(), "2025-01-01", "extra-x86_64", &[
        ("foo-1.0-1-x86_64", "a"), ("bar-2.0-1-x86_64", "b"),
    ]);
    make_snapshot(dir.path(), "2025-02-01", "extra-x86_64", &[
        ("foo-1.0-1-x86_64", "a"), ("baz-3.0-1-x86_64", "c"),
    ]);
    let idx = scan_snapshot(dir.path()).unwrap();
    assert_eq!(idx.packages.len(), 4);
    let plan = plan_cleanup(&idx, 1);
    // foo old copy deleted; bar old copy kept (only version); baz kept.
    assert!(plan.delete.iter().any(|p| p.to_string_lossy().contains("2025-01-01") && p.to_string_lossy().contains("foo")));
    assert!(plan.keep.iter().any(|p| p.to_string_lossy().contains("bar")));
}

#[test]
fn test_filename_parse_via_scan() {
    let dir = tempfile::tempdir().unwrap();
    make_snapshot(dir.path(), "2025-01-01", "core-any", &[("base-1-1-any", "x")]);
    let idx = scan_snapshot(dir.path()).unwrap();
    assert_eq!(idx.packages[0].pkgname, "base");
    assert_eq!(idx.packages[0].arch, "any");
}

#[test]
fn test_version_key_orders() {
    assert!(version_sort_key("1.10-1") > version_sort_key("1.9-1"));
    assert!(version_sort_key("2:1.0-1") > version_sort_key("1:9.9-9"));
}

#[test]
fn test_invalid_root() {
    let dir = tempfile::tempdir().unwrap();
    assert!(scan_snapshot(dir.path()).is_err());
}
