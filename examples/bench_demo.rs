use seshat::bench::*;
fn main() {
    use std::path::PathBuf;
    let report = AgentBenchReport {
        iterations: 3,
        agents: vec!["codex".into(), "claude".into(), "ollama".into()],
        agent_selection: AgentSelection::Explicit,
        fixtures: vec!["rust".into(), "python".into(), "typescript".into()],
        temp_root: Some(PathBuf::from("/tmp/seshat-bench-demo")),
        summaries: vec![
            AgentBenchSummary { fixture: "rust".into(), agent: "codex".into(), total: 3, success: 3, conventional_valid: 3, avg_ms: 3210.0, min_ms: 3050.0, p95_ms: 3450.0, max_ms: 3450.0 },
            AgentBenchSummary { fixture: "rust".into(), agent: "claude".into(), total: 3, success: 3, conventional_valid: 3, avg_ms: 4200.0, min_ms: 4180.0, p95_ms: 4310.0, max_ms: 4310.0 },
            AgentBenchSummary { fixture: "rust".into(), agent: "ollama".into(), total: 3, success: 3, conventional_valid: 2, avg_ms: 1180.0, min_ms: 1100.0, p95_ms: 1340.0, max_ms: 1340.0 },
            AgentBenchSummary { fixture: "python".into(), agent: "codex".into(), total: 3, success: 3, conventional_valid: 2, avg_ms: 3110.0, min_ms: 2950.0, p95_ms: 3300.0, max_ms: 3300.0 },
            AgentBenchSummary { fixture: "python".into(), agent: "claude".into(), total: 3, success: 3, conventional_valid: 3, avg_ms: 4400.0, min_ms: 4200.0, p95_ms: 4600.0, max_ms: 4600.0 },
            AgentBenchSummary { fixture: "python".into(), agent: "ollama".into(), total: 3, success: 3, conventional_valid: 1, avg_ms: 1240.0, min_ms: 1180.0, p95_ms: 1400.0, max_ms: 1400.0 },
            AgentBenchSummary { fixture: "typescript".into(), agent: "codex".into(), total: 3, success: 3, conventional_valid: 3, avg_ms: 3050.0, min_ms: 2900.0, p95_ms: 3200.0, max_ms: 3200.0 },
            AgentBenchSummary { fixture: "typescript".into(), agent: "claude".into(), total: 3, success: 2, conventional_valid: 2, avg_ms: 4150.0, min_ms: 4000.0, p95_ms: 4280.0, max_ms: 4280.0 },
            AgentBenchSummary { fixture: "typescript".into(), agent: "ollama".into(), total: 3, success: 3, conventional_valid: 3, avg_ms: 1290.0, min_ms: 1200.0, p95_ms: 1410.0, max_ms: 1410.0 },
        ],
        overall: vec![
            AgentBenchOverallSummary { agent: "claude".into(), total: 9, success: 8, conventional_valid: 8, avg_ms: 4220.0, min_ms: 4000.0, p95_ms: 4600.0, max_ms: 4600.0, fixtures_won: 2 },
            AgentBenchOverallSummary { agent: "codex".into(), total: 9, success: 9, conventional_valid: 8, avg_ms: 3120.0, min_ms: 2900.0, p95_ms: 3450.0, max_ms: 3450.0, fixtures_won: 1 },
            AgentBenchOverallSummary { agent: "ollama".into(), total: 9, success: 9, conventional_valid: 6, avg_ms: 1240.0, min_ms: 1100.0, p95_ms: 1410.0, max_ms: 1410.0, fixtures_won: 0 },
        ],
        samples: vec![
            AgentBenchSample { fixture: "rust".into(), agent: "codex".into(), iteration: 1, duration_ms: 3120.0, success: true, conventional_valid: true, message: Some("feat: adicionar testes para função calculate_total".into()), error: None, diff: "diff --git a/src/lib.rs b/src/lib.rs\n--- a/src/lib.rs\n+++ b/src/lib.rs\n@@ -1,5 +1,12 @@\n pub fn calculate_total(items: &[u32]) -> u32 {\n     items.iter().sum()\n }\n+\n+#[cfg(test)]\n+mod tests {\n+    use super::*;\n+\n+    #[test]\n+    fn sums_items() {\n+        assert_eq!(calculate_total(&[2, 3, 5]), 10);\n+    }\n+}\n".into() },
            AgentBenchSample { fixture: "rust".into(), agent: "claude".into(), iteration: 1, duration_ms: 4250.0, success: true, conventional_valid: true, message: Some("test(lib): cobrir calculate_total com testes unitários".into()), error: None, diff: "".into() },
            AgentBenchSample { fixture: "rust".into(), agent: "ollama".into(), iteration: 1, duration_ms: 1180.0, success: true, conventional_valid: true, message: Some("feat(lib): adicionar testes unitários para calculate_total".into()), error: None, diff: "".into() },
            AgentBenchSample { fixture: "rust".into(), agent: "codex".into(), iteration: 2, duration_ms: 3050.0, success: true, conventional_valid: true, message: Some("feat: incluir testes para somatório de items".into()), error: None, diff: "".into() },
            AgentBenchSample { fixture: "rust".into(), agent: "claude".into(), iteration: 2, duration_ms: 4310.0, success: true, conventional_valid: true, message: Some("test: validar calculate_total com casos de soma".into()), error: None, diff: "".into() },
            AgentBenchSample { fixture: "rust".into(), agent: "ollama".into(), iteration: 2, duration_ms: 1250.0, success: true, conventional_valid: false, message: Some("adiciona testes para a função".into()), error: None, diff: "".into() },
            AgentBenchSample { fixture: "python".into(), agent: "codex".into(), iteration: 1, duration_ms: 3110.0, success: true, conventional_valid: true, message: Some("feat: adicionar testes para módulo de cálculo".into()), error: None, diff: "diff --git a/calculator.py b/calculator.py\n--- a/calculator.py\n+++ b/calculator.py\n@@ -1,3 +1,9 @@\n def total(items):\n     return sum(items)\n+\n+\n+def test_total():\n+    assert total([2, 3, 5]) == 10\n+    assert total([]) == 0\n".into() },
            AgentBenchSample { fixture: "python".into(), agent: "claude".into(), iteration: 1, duration_ms: 4400.0, success: true, conventional_valid: true, message: Some("test: adicionar suite de testes para função total".into()), error: None, diff: "".into() },
            AgentBenchSample { fixture: "python".into(), agent: "ollama".into(), iteration: 1, duration_ms: 1240.0, success: false, conventional_valid: false, message: None, error: Some("connection refused".into()), diff: "".into() },
            AgentBenchSample { fixture: "typescript".into(), agent: "codex".into(), iteration: 1, duration_ms: 3050.0, success: true, conventional_valid: true, message: Some("feat: adicionar testes para utilitários de strings".into()), error: None, diff: "diff --git a/src/utils.ts b/src/utils.ts\n--- a/src/utils.ts\n+++ b/src/utils.ts\n@@ -1,3 +1,9 @@\n export const upper = (s: string) => s.toUpperCase();\n+\n+describe('upper', () => {\n+  it('uppercases', () => {\n+    expect(upper('hi')).toBe('HI');\n+  });\n+});\n".into() },
            AgentBenchSample { fixture: "typescript".into(), agent: "claude".into(), iteration: 1, duration_ms: 4150.0, success: true, conventional_valid: true, message: Some("test(utils): cobrir upper com casos básicos".into()), error: None, diff: "".into() },
            AgentBenchSample { fixture: "typescript".into(), agent: "ollama".into(), iteration: 1, duration_ms: 1290.0, success: true, conventional_valid: true, message: Some("test(utils): adicionar testes para upper".into()), error: None, diff: "".into() },
        ],
        show_samples: 2,
        override_notes: Vec::new(),
    };
    print_report(&report, ReportLanguage::Portuguese);
}
