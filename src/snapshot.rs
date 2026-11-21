//! ALA snapshot tree scanning.
//!
//! Layout: `<archive>/<date>/<repo>-<arch>/<pkgname>-<ver>-<rel>-<arch>.pkg.tar.*`
//! plus `.sig` files. Dates are `YYYY-MM-DD` directory names.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use crate::ArchiveError;

/// `<repo>-<arch>` pair.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct RepoArch {
    /// Repository.
    pub repo: String,
    /// Architecture.
    pub arch: String,
}

impl RepoArch {
    /// Parse `extra-x86_64`.
    pub fn parse(dirname: &str) -> Option<Self> {
        let (repo, arch) = dirname.split_once('-')?;
        if repo.is_empty() || arch.is_empty() {
            return None;
        }
        Some(Self { repo: repo.into(), arch: arch.into() })
    }
}

/// A package file inside a snapshot.
#[derive(Debug, Clone)]
pub struct PackageFile {
    /// Date of the snapshot (`YYYY-MM-DD`).
    pub date: String,
    /// Repository.
    pub repo: String,
    /// Architecture.
    pub arch: String,
    /// Package base (pkgname minus split suffix heuristics — set by caller
    /// when PKGBUILD/pkgbase is known; defaults to pkgname).
    pub pkgbase: String,
    /// Package name.
    pub pkgname: String,
    /// Full version `epoch:pkgver-pkgrel` (best-effort parse).
    pub version: String,
    /// Absolute path to the `.pkg.tar.*` file.
    pub path: PathBuf,
    /// Whether a `.sig` sibling exists.
    pub has_sig: bool,
}

impl PackageFile {
    /// Sortable version key — parsed via simple component split.
    pub fn version_key(&self) -> Vec<String> {
        version_sort_key(&self.version)
    }
}

/// Index of all package files across snapshots.
#[derive(Debug, Default)]
pub struct SnapshotIndex {
    /// All package files.
    pub packages: Vec<PackageFile>,
    /// Snapshot dates seen (sorted).
    pub dates: Vec<String>,
}

impl SnapshotIndex {
    /// Packages in a repo+arch.
    pub fn in_repo_arch(&self, repo: &str, arch: &str) -> Vec<&PackageFile> {
        self.packages
            .iter()
            .filter(|p| p.repo == repo && p.arch == arch)
            .collect()
    }
}

/// Split `name-ver-rel-arch.pkg.tar.zst` filename into components.
///
/// Package filenames are `<pkgname>-<epoch:>pkgver>-<pkgrel>-<arch>.pkg.tar.*`;
/// pkgver itself may contain `-`, so we split from the right: arch, then
/// pkgrel (digit-start), then pkgver, then pkgname.
pub fn parse_pkg_filename(filename: &str) -> Option<(String, String, String)> {
    let stem = filename
        .strip_suffix(".pkg.tar.zst")
        .or_else(|| filename.strip_suffix(".pkg.tar.xz"))
        .or_else(|| filename.strip_suffix(".pkg.tar.gz"))
        .unwrap_or(filename);
    // Strip .sig if present.
    let stem = stem.strip_suffix(".sig").unwrap_or(stem);
    let parts: Vec<&str> = stem.rsplitn(4, '-').collect();
    if parts.len() < 3 {
        return None;
    }
    // parts[0]=arch, parts[1]=pkgrel, parts[2..]=pkgname-pkgver
    let arch = parts[0].to_string();
    let pkgrel = parts[1].to_string();
    let rest = parts[2..].iter().rev().cloned().collect::<Vec<_>>().join("-");
    // rest = "pkgname-pkgver" possibly with epoch: in version.
    // pkgver may contain '-'; find the LAST '-' before pkgrel? We already
    // consumed pkgrel+arch; remaining "name-ver" split on FIRST '-' after
    // name? pkgname can't contain '-', so first '-' separates name from ver.
    let (pkgname, pkgver) = match rest.split_once('-') {
        Some((n, v)) => (n.to_string(), v.to_string()),
        None => return None,
    };
    Some((pkgname, format!("{pkgver}-{pkgrel}"), arch))
}

/// Version sort key: epoch, then dot/dash-separated numeric-aware segments.
pub fn version_sort_key(version: &str) -> Vec<String> {
    let (epoch, rest) = match version.split_once(':') {
        Some((e, r)) => (e.to_string(), r),
        None => ("0".to_string(), version),
    };
    let mut key = vec![format!("{:020}", epoch.parse::<u64>().unwrap_or(0))];
    for seg in rest.split(|c| c == '.' || c == '-' || c == '_') {
        if seg.is_empty() {
            continue;
        }
        if seg.chars().all(|c| c.is_ascii_digit()) {
            key.push(format!("{:020}", seg.parse::<u64>().unwrap_or(0)));
        } else {
            key.push(seg.to_string());
        }
    }
    key
}

/// Scan an ALA archive root (`<archive>/<date>/<repo>-<arch>/...`).
pub fn scan_snapshot(root: &Path) -> Result<SnapshotIndex, ArchiveError> {
    let mut idx = SnapshotIndex::default();
    let mut dates = std::collections::BTreeSet::new();
    for date_entry in std::fs::read_dir(root)? {
        let date_entry = date_entry?;
        let date_dir = date_entry.path();
        if !date_dir.is_dir() {
            continue;
        }
        let date = date_entry.file_name().to_string_lossy().to_string();
        if !is_date_dir(&date) {
            continue;
        }
        dates.insert(date.clone());
        for ra_entry in std::fs::read_dir(&date_dir)? {
            let ra_entry = ra_entry?;
            let ra_dir = ra_entry.path();
            if !ra_dir.is_dir() {
                continue;
            }
            let ra_name = ra_entry.file_name().to_string_lossy().to_string();
            let Some(ra) = RepoArch::parse(&ra_name) else {
                continue;
            };
            for pkg_entry in std::fs::read_dir(&ra_dir)? {
                let pkg_entry = pkg_entry?;
                let path = pkg_entry.path();
                if !path.is_file() {
                    continue;
                }
                let fname = pkg_entry.file_name().to_string_lossy().to_string();
                if fname.ends_with(".sig") {
                    continue; // signatures handled via has_sig
                }
                if !is_pkg_file(&fname) {
                    continue;
                }
                let Some((pkgname, version, arch)) = parse_pkg_filename(&fname) else {
                    continue;
                };
                let pkgbase = pkgname.clone();
                let has_sig = path.with_file_name(format!("{fname}.sig")).exists();
                idx.packages.push(PackageFile {
                    date: date.clone(),
                    repo: ra.repo.clone(),
                    arch: ra.arch.clone(),
                    pkgbase,
                    pkgname,
                    version,
                    path,
                    has_sig,
                });
            }
        }
    }
    idx.dates = dates.into_iter().collect();
    if idx.packages.is_empty() {
        return Err(ArchiveError::InvalidRoot(format!(
            "{}: no package files found",
            root.display()
        )));
    }
    Ok(idx)
}

fn is_date_dir(name: &str) -> bool {
    name.len() == 10
        && name.chars().enumerate().all(|(i, c)| {
            if i == 4 || i == 7 {
                c == '-'
            } else {
                c.is_ascii_digit()
            }
        })
}

fn is_pkg_file(name: &str) -> bool {
    name.ends_with(".pkg.tar.zst") || name.ends_with(".pkg.tar.xz") || name.ends_with(".pkg.tar.gz")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_pkg_filename() {
        let (n, v, a) = parse_pkg_filename("foo-1.2.3-4-x86_64.pkg.tar.zst").unwrap();
        assert_eq!(n, "foo");
        assert_eq!(v, "1.2.3-4");
        assert_eq!(a, "x86_64");
    }

    #[test]
    fn test_parse_pkg_filename_epoch() {
        let (n, v, _) = parse_pkg_filename("bar-2:1.0-1-any.pkg.tar.zst").unwrap();
        assert_eq!(n, "bar");
        assert_eq!(v, "2:1.0-1");
    }

    #[test]
    fn test_version_sort_key() {
        let a = version_sort_key("1.10-1");
        let b = version_sort_key("1.9-1");
        assert!(a > b);
    }

    #[test]
    fn test_scan_snapshot() {
        let dir = tempfile::tempdir().unwrap();
        let d = dir.path().join("2025-01-01/extra-x86_64");
        std::fs::create_dir_all(&d).unwrap();
        std::fs::write(
            d.join("foo-1.0-1-x86_64.pkg.tar.zst"),
            b"pkg",
        )
        .unwrap();
        std::fs::write(d.join("foo-1.0-1-x86_64.pkg.tar.zst.sig"), b"sig").unwrap();
        let idx = scan_snapshot(dir.path()).unwrap();
        assert_eq!(idx.packages.len(), 1);
        assert_eq!(idx.packages[0].pkgname, "foo");
        assert!(idx.packages[0].has_sig);
        assert_eq!(idx.dates, vec!["2025-01-01"]);
    }
}
