// Copyright 3Bian Limited and Contributors
// SPDX-License-Identifier: MIT

use crate::compilation_database::ComparisonReport;

pub fn format_comparison_report(report: &ComparisonReport) -> String {
    let mut description = format!(
        "Reference: {}; generated: {}; missing: {}; extra: {}; different: {}; ambiguous: {}; unresolved variants: {}\n",
        report.reference_count,
        report.generated_count,
        report.missing.len(),
        report.extra.len(),
        report.different.len(),
        report.ambiguous.len(),
        report.conditional.len()
    );
    for entry in &report.missing {
        description += &format!("MISSING {} -> {:?}\n", entry.file, entry.output)
    }
    for entry in &report.extra {
        description += &format!("EXTRA {} -> {:?}\n", entry.file, entry.output)
    }
    for difference in &report.different {
        description += &format!("DIFFERENT {} -> {:?}\n", difference.file, difference.output);
        for change in &difference.changes {
            description += &format!("  {change}\n")
        }
    }
    for ambiguity in &report.ambiguous {
        description += &format!("AMBIGUOUS {ambiguity}\n")
    }
    for condition in &report.conditional {
        description += &format!("UNRESOLVED {condition}\n")
    }
    description
}
