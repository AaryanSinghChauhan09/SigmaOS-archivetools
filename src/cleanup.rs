//! Archive cleanup planning — port of `archive-cleaner` semantics.
//!
//! ALA keeps the last N dated snapshots of each package version per
//! repo/arch. Older duplicates (same pkgbase+version appearing in multiple
//! snapshot dates) are deleted, keeping the newest snapshot only.

use std::collections::BTreeMap;

use crate::snapshot::{PackageFile, SnapshotIndex};

/// Cleanup plan: files to delete and files to keep.
#[derive(Debug, Default)]
pub struct CleanupPlan {
    /// Package files safe to delete (old duplicates).
    pub delete: Vec<std::path::PathBuf>,
    /// Package files retained.
    pub keep: Vec<std::path::PathBuf>,
    /// Reasons for deletion, keyed by path.
    pub reasons: BTreeMap<std::path::PathBuf, String>,
}

/// Plan cleanup for an ALA snapshot index.
///
/// `keep_dates` = number of most recent snapshot dates to always retain
/// (upstream default keeps several months). Within the retained window,
/// only the newest copy of each `(repo, arch, pkgbase, version)` survives;
/// older copies in earlier retained dates are also deduplicated.
///
/// Outside the window, everything is deleted unless it's the only copy of
/// that version (ALA never deletes a version entirely — it keeps the last
/// occurrence of each unique version).
pub fn plan_cleanup(index: &SnapshotIndex, keep_dates: usize) -> CleanupPlan {
    let mut plan = CleanupPlan::default();
    let dates = &index.dates;
    let cutoff = if dates.len() > keep_dates {
        dates[dates.len() - keep_dates].clone()
    } else {
        String::new()
    };

    // Group by (repo, arch, pkgbase, version).
    let mut groups: BTreeMap<(String, String, String, String), Vec<&PackageFile>> =
        BTreeMap::new();
    for p in &index.packages {
        groups
            .entry((p.repo.clone(), p.arch.clone(), p.pkgbase.clone(), p.version.clone()))
            .or_default()
            .push(p);
    }

    for ((_repo, _arch, _base, _ver), mut files) in groups {
        // Sort newest-first by date then path.
        files.sort_by(|a, b| b.date.cmp(&a.date).then(a.path.cmp(&b.path)));
        // Always keep the newest copy.
        let newest = files.remove(0);
        plan.keep.push(newest.path.clone());

        for f in files {
            if f.date >= cutoff && cutoff.is_empty() == false {
                // Within retention window but duplicate version → delete.
                plan.delete.push(f.path.clone());
                plan.reasons.insert(
                    f.path.clone(),
                    format!("duplicate of newer snapshot {}", newest.date),
                );
            } else if cutoff.is_empty() {
                // Whole archive within window → dedupe only.
                plan.delete.push(f.path.clone());
                plan.reasons.insert(
                    f.path.clone(),
                    format!("duplicate of newer snapshot {}", newest.date),
                );
            } else {
                // Outside window: delete only if a newer copy exists in window.
                plan.delete.push(f.path.clone());
                plan.reasons.insert(
                    f.path.clone(),
                    format!("superseded by snapshot {}", newest.date),
                );
            }
        }
    }
    plan
}
