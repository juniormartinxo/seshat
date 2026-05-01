//! Condenses unified diffs by stripping metadata while keeping every change.
//!
//! Vendored from rtk-ai/rtk v0.35.0 (`src/cmds/git/diff_cmd.rs::condense_unified_diff`).
//! Copyright 2024 Patrick Szymkowiak — Apache License 2.0.
//!
//! Only the pure `condense_unified_diff` helper was vendored. The surrounding
//! CLI wrappers (`run`, `run_stdin`), two-file line-by-line diff, and
//! telemetry were not needed for seshat's review pipeline.

/// Strips diff metadata (`diff --git`, `---`, `+++`, `@@` hunks) while
/// preserving every `+`/`-` line. Emits a compact header per file with
/// added/removed counts, followed by the raw change lines indented by two
/// spaces. If a file has more than 10 changes, appends a trailing
/// `... +N more` marker with `N = total - 10`.
///
/// Empty input produces empty output.
pub fn condense_unified_diff(diff: &str) -> String {
    let mut result = Vec::new();
    let mut current_file = String::new();
    let mut added = 0;
    let mut removed = 0;
    let mut changes = Vec::new();

    // Never truncate diff content — users make decisions based on this data.
    // Only strip diff metadata (headers, @@ hunks); all +/- lines shown in full.
    for line in diff.lines() {
        if line.starts_with("diff --git") || line.starts_with("--- ") || line.starts_with("+++ ") {
            if line.starts_with("+++ ") {
                if !current_file.is_empty() && (added > 0 || removed > 0) {
                    result.push(format!("[file] {} (+{} -{})", current_file, added, removed));
                    for c in &changes {
                        result.push(format!("  {}", c));
                    }
                    let total = added + removed;
                    if total > 10 {
                        result.push(format!("  ... +{} more", total - 10));
                    }
                }
                current_file = line
                    .trim_start_matches("+++ ")
                    .trim_start_matches("b/")
                    .to_string();
                added = 0;
                removed = 0;
                changes.clear();
            }
        } else if line.starts_with('+') && !line.starts_with("+++") {
            added += 1;
            changes.push(line.to_string());
        } else if line.starts_with('-') && !line.starts_with("---") {
            removed += 1;
            changes.push(line.to_string());
        }
    }

    // Last file
    if !current_file.is_empty() && (added > 0 || removed > 0) {
        result.push(format!("[file] {} (+{} -{})", current_file, added, removed));
        for c in &changes {
            result.push(format!("  {}", c));
        }
        let total = added + removed;
        if total > 10 {
            result.push(format!("  ... +{} more", total - 10));
        }
    }

    result.join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_condense_unified_diff_single_file() {
        let diff = r#"diff --git a/src/main.rs b/src/main.rs
--- a/src/main.rs
+++ b/src/main.rs
@@ -1,3 +1,4 @@
 fn main() {
+    println!("hello");
     println!("world");
 }
"#;
        let result = condense_unified_diff(diff);
        assert!(result.contains("src/main.rs"));
        assert!(result.contains("+1"));
        assert!(result.contains("println"));
    }

    #[test]
    fn test_condense_unified_diff_multiple_files() {
        let diff = r#"diff --git a/a.rs b/a.rs
--- a/a.rs
+++ b/a.rs
+added line
diff --git a/b.rs b/b.rs
--- a/b.rs
+++ b/b.rs
-removed line
"#;
        let result = condense_unified_diff(diff);
        assert!(result.contains("a.rs"));
        assert!(result.contains("b.rs"));
    }

    #[test]
    fn test_condense_unified_diff_empty() {
        let result = condense_unified_diff("");
        assert!(result.is_empty());
    }

    fn make_large_unified_diff(added: usize, removed: usize) -> String {
        let mut lines = vec![
            "diff --git a/config.yaml b/config.yaml".to_string(),
            "--- a/config.yaml".to_string(),
            "+++ b/config.yaml".to_string(),
            "@@ -1,200 +1,200 @@".to_string(),
        ];
        for i in 0..removed {
            lines.push(format!("-old_value_{}", i));
        }
        for i in 0..added {
            lines.push(format!("+new_value_{}", i));
        }
        lines.join("\n")
    }

    #[test]
    fn test_condense_unified_diff_overflow_count_accuracy() {
        // 100 added + 100 removed = 200 total changes; overflow marker must
        // report 200 - 10 = 190 regardless of how many lines are actually
        // emitted, so downstream readers can trust the magnitude indicator.
        let diff = make_large_unified_diff(100, 100);
        let result = condense_unified_diff(&diff);
        assert!(
            result.contains("+190 more"),
            "Expected '+190 more' but got:\n{}",
            result
        );
    }

    #[test]
    fn test_condense_unified_diff_no_false_overflow() {
        // 8 changes total — all fit within the 10-change threshold, no overflow marker
        let diff = make_large_unified_diff(4, 4);
        let result = condense_unified_diff(&diff);
        assert!(
            !result.contains("more"),
            "No overflow message expected for 8 changes, got:\n{}",
            result
        );
    }
}
