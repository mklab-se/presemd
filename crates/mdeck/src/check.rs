use std::collections::BTreeSet;
use std::fmt;

/// Category of a check warning, for grouping and filtering.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum CheckCategory {
    DiagramRouting,
}

impl fmt::Display for CheckCategory {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CheckCategory::DiagramRouting => write!(f, "architecture"),
        }
    }
}

/// A single warning produced during presentation checking.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct CheckWarning {
    /// 1-indexed slide number.
    pub slide: usize,
    pub category: CheckCategory,
    pub message: String,
}

impl fmt::Display for CheckWarning {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "  slide {}: [{}] {}",
            self.slide, self.category, self.message
        )
    }
}

/// Collects deduplicated, sorted warnings from a presentation check pass.
#[derive(Debug, Clone, Default)]
pub struct CheckReport {
    warnings: BTreeSet<CheckWarning>,
}

impl CheckReport {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn add(&mut self, warning: CheckWarning) {
        self.warnings.insert(warning);
    }

    pub fn has_warnings(&self) -> bool {
        !self.warnings.is_empty()
    }

    pub fn warning_count(&self) -> usize {
        self.warnings.len()
    }

    pub fn warnings(&self) -> impl Iterator<Item = &CheckWarning> {
        self.warnings.iter()
    }

    /// Print a brief one-liner summary to stderr (for GUI mode).
    pub fn print_brief(&self) {
        if self.has_warnings() {
            eprintln!(
                "warning: {} diagram routing issue(s) found (run with --check for details)",
                self.warning_count()
            );
        }
    }

    /// Print detailed per-warning output to stderr (for --check mode).
    pub fn print_detailed(&self) {
        for w in self.warnings() {
            eprintln!("{w}");
        }
        eprintln!();
        eprintln!("{} warning(s) found.", self.warning_count());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn warning(slide: usize, msg: &str) -> CheckWarning {
        CheckWarning {
            slide,
            category: CheckCategory::DiagramRouting,
            message: msg.to_string(),
        }
    }

    #[test]
    fn dedup_identical_warnings() {
        let mut report = CheckReport::new();
        report.add(warning(1, "Could not route A -> B"));
        report.add(warning(1, "Could not route A -> B"));
        assert_eq!(report.warning_count(), 1);
    }

    #[test]
    fn sorted_by_slide_then_message() {
        let mut report = CheckReport::new();
        report.add(warning(3, "Z warning"));
        report.add(warning(1, "A warning"));
        report.add(warning(3, "A warning"));
        let slides: Vec<(usize, &str)> = report
            .warnings()
            .map(|w| (w.slide, w.message.as_str()))
            .collect();
        assert_eq!(
            slides,
            vec![(1, "A warning"), (3, "A warning"), (3, "Z warning"),]
        );
    }

    #[test]
    fn add_from_multiple_sources_deduplicates() {
        let mut report = CheckReport::new();
        report.add(warning(1, "msg1"));
        report.add(warning(2, "msg2"));
        report.add(warning(2, "msg2")); // duplicate
        report.add(warning(3, "msg3"));
        assert_eq!(report.warning_count(), 3);
    }

    #[test]
    fn empty_report_has_no_warnings() {
        let report = CheckReport::new();
        assert!(!report.has_warnings());
        assert_eq!(report.warning_count(), 0);
    }
}
