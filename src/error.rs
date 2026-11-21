//! Error types for the archivetools component.

/// Errors that can occur while scanning or planning archive operations.
#[derive(Debug, thiserror::Error)]
pub enum ArchiveError {
    /// The archive root does not look like an ALA snapshot tree.
    #[error("invalid archive root: {0}")]
    InvalidRoot(String),

    /// I/O error.
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
}
