//! SigmaOS archivetools component.
//!
//! Rust implementation of `archlinux/archivetools`: tooling for the Arch
//! Linux Archive (ALA) — snapshot management, old-version cleanup, and
//! hardlink deduplication of package files.
//!
//! Upstream is `archive.sh` + `archive-cleaner` + systemd units; this port
//! implements the core logic: scanning dated snapshot trees, computing
//! which package versions to delete per repo/arch/pkgbase retention policy,
//! and planning hardlink dedup.

#![forbid(unsafe_code)]
#![warn(missing_docs)]

pub mod cleanup;
pub mod error;
pub mod snapshot;

pub use cleanup::{CleanupPlan, plan_cleanup};
pub use error::ArchiveError;
pub use snapshot::{PackageFile, RepoArch, SnapshotIndex, parse_pkg_filename, scan_snapshot, version_sort_key};
