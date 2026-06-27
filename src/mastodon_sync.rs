use std::fmt;

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub struct SyncMode {
    pub write_files: bool,
    pub full_sync: bool,
}

impl SyncMode {
    pub fn new(write_files: bool, full_sync: bool) -> Self {
        Self {
            write_files,
            full_sync,
        }
    }
}

impl fmt::Display for SyncMode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match (self.write_files, self.full_sync) {
            (true, true) => write!(f, "write/full"),
            (true, false) => write!(f, "write/incremental"),
            (false, true) => write!(f, "dry-run/full"),
            (false, false) => write!(f, "dry-run/incremental"),
        }
    }
}

pub struct SyncSummary {
    pub fetched: usize,
    pub written: usize,
    pub updated: usize,
    pub dry_run: usize,
    pub skipped_existing: usize,
    pub skipped_visibility: usize,
}

impl SyncSummary {
    pub fn new(fetched: usize) -> Self {
        Self {
            fetched,
            written: 0,
            updated: 0,
            dry_run: 0,
            skipped_existing: 0,
            skipped_visibility: 0,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn describes_sync_modes() {
        assert_eq!(
            SyncMode::new(false, false).to_string(),
            "dry-run/incremental"
        );
        assert_eq!(SyncMode::new(true, false).to_string(), "write/incremental");
        assert_eq!(SyncMode::new(false, true).to_string(), "dry-run/full");
        assert_eq!(SyncMode::new(true, true).to_string(), "write/full");
    }

    #[test]
    fn initializes_sync_summary_counters() {
        let summary = SyncSummary::new(3);

        assert_eq!(summary.fetched, 3);
        assert_eq!(summary.written, 0);
        assert_eq!(summary.updated, 0);
        assert_eq!(summary.dry_run, 0);
        assert_eq!(summary.skipped_existing, 0);
        assert_eq!(summary.skipped_visibility, 0);
    }
}
