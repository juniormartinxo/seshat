use seshat::bench::*;
use std::path::PathBuf;

fn main() {
    let codex_model = Some("gpt-5".to_string());
    let claude_model = Some("claude-sonnet-4-6".to_string());
    let ollama_model = Some("juniormartinxo/seshat-commit".to_string());

    let report = AgentBenchReport {
        iterations: 5,
        agents: vec!["codex".into(), "claude".into(), "ollama".into()],
        agent_selection: AgentSelection::Explicit,
        fixtures: vec!["rust".into(), "python".into(), "typescript".into()],
        temp_root: Some(PathBuf::from("/tmp/seshat-bench-demo")),
        summaries: vec![
            AgentBenchSummary { fixture: "rust".into(),       agent: "codex".into(),  model: codex_model.clone(),  total: 5, success: 5, conventional_valid: 5, avg_ms: 3210.0, min_ms: 3050.0, p95_ms: 3450.0, max_ms: 3450.0 },
            AgentBenchSummary { fixture: "rust".into(),       agent: "claude".into(), model: claude_model.clone(), total: 5, success: 5, conventional_valid: 5, avg_ms: 5503.0, min_ms: 4614.0, p95_ms: 7122.0, max_ms: 7122.0 },
            AgentBenchSummary { fixture: "rust".into(),       agent: "ollama".into(), model: ollama_model.clone(), total: 5, success: 5, conventional_valid: 4, avg_ms: 322.0,  min_ms: 238.0,  p95_ms: 633.0,  max_ms: 633.0 },
            AgentBenchSummary { fixture: "python".into(),     agent: "codex".into(),  model: codex_model.clone(),  total: 5, success: 5, conventional_valid: 4, avg_ms: 3110.0, min_ms: 2950.0, p95_ms: 3300.0, max_ms: 3300.0 },
            AgentBenchSummary { fixture: "python".into(),     agent: "claude".into(), model: claude_model.clone(), total: 5, success: 5, conventional_valid: 5, avg_ms: 5857.0, min_ms: 5115.0, p95_ms: 7168.0, max_ms: 7168.0 },
            AgentBenchSummary { fixture: "python".into(),     agent: "ollama".into(), model: ollama_model.clone(), total: 5, success: 5, conventional_valid: 5, avg_ms: 279.0,  min_ms: 249.0,  p95_ms: 348.0,  max_ms: 348.0 },
            AgentBenchSummary { fixture: "typescript".into(), agent: "codex".into(),  model: codex_model.clone(),  total: 5, success: 5, conventional_valid: 5, avg_ms: 3050.0, min_ms: 2900.0, p95_ms: 3200.0, max_ms: 3200.0 },
            AgentBenchSummary { fixture: "typescript".into(), agent: "claude".into(), model: claude_model.clone(), total: 5, success: 4, conventional_valid: 4, avg_ms: 5638.0, min_ms: 4815.0, p95_ms: 7774.0, max_ms: 7774.0 },
            AgentBenchSummary { fixture: "typescript".into(), agent: "ollama".into(), model: ollama_model.clone(), total: 5, success: 5, conventional_valid: 5, avg_ms: 293.0,  min_ms: 231.0,  p95_ms: 441.0,  max_ms: 441.0 },
        ],
        overall: vec![
            AgentBenchOverallSummary { agent: "ollama".into(), model: ollama_model.clone(), total: 15, success: 15, conventional_valid: 14, avg_ms: 298.0,  min_ms: 231.0,  p95_ms: 633.0,  max_ms: 633.0,  fixtures_won: 3 },
            AgentBenchOverallSummary { agent: "codex".into(),  model: codex_model.clone(),  total: 15, success: 15, conventional_valid: 14, avg_ms: 3123.0, min_ms: 2900.0, p95_ms: 3450.0, max_ms: 3450.0, fixtures_won: 0 },
            AgentBenchOverallSummary { agent: "claude".into(), model: claude_model.clone(), total: 15, success: 14, conventional_valid: 14, avg_ms: 5666.0, min_ms: 4614.0, p95_ms: 7774.0, max_ms: 7774.0, fixtures_won: 0 },
        ],
        samples: vec![
            // Rust
            AgentBenchSample { fixture: "rust".into(), agent: "codex".into(),  model: codex_model.clone(),  iteration: 1, duration_ms: 3120.0, success: true,  conventional_valid: true,  message: Some("feat(calculator): adicionar testes para função calculate_total".into()), error: None, diff: rust_diff() },
            AgentBenchSample { fixture: "rust".into(), agent: "claude".into(), model: claude_model.clone(), iteration: 1, duration_ms: 5065.0, success: true,  conventional_valid: true,  message: Some("feat(calculator): adicionar função calculate_total".into()), error: None, diff: String::new() },
            AgentBenchSample { fixture: "rust".into(), agent: "ollama".into(), model: ollama_model.clone(), iteration: 1, duration_ms: 633.0,  success: true,  conventional_valid: true,  message: Some("feat(calculator): adiciona função para calcular total de itens".into()), error: None, diff: String::new() },
            AgentBenchSample { fixture: "rust".into(), agent: "codex".into(),  model: codex_model.clone(),  iteration: 2, duration_ms: 3050.0, success: true,  conventional_valid: true,  message: Some("feat: incluir testes para somatório de items".into()), error: None, diff: String::new() },
            AgentBenchSample { fixture: "rust".into(), agent: "claude".into(), model: claude_model.clone(), iteration: 2, duration_ms: 7122.0, success: true,  conventional_valid: true,  message: Some("feat(calculator): adiciona função de soma de itens".into()), error: None, diff: String::new() },
            AgentBenchSample { fixture: "rust".into(), agent: "ollama".into(), model: ollama_model.clone(), iteration: 2, duration_ms: 239.0,  success: true,  conventional_valid: false, message: Some("adiciona testes para a função".into()), error: None, diff: String::new() },
            AgentBenchSample { fixture: "rust".into(), agent: "codex".into(),  model: codex_model.clone(),  iteration: 3, duration_ms: 3200.0, success: true,  conventional_valid: true,  message: Some("test(lib): adicionar suite para calculate_total".into()), error: None, diff: String::new() },
            AgentBenchSample { fixture: "rust".into(), agent: "claude".into(), model: claude_model.clone(), iteration: 3, duration_ms: 4614.0, success: true,  conventional_valid: true,  message: Some("feat(calculator): adicionar função calculate_total".into()), error: None, diff: String::new() },
            AgentBenchSample { fixture: "rust".into(), agent: "ollama".into(), model: ollama_model.clone(), iteration: 3, duration_ms: 238.0,  success: true,  conventional_valid: true,  message: Some("feat(calculator): adiciona função para calcular total de itens".into()), error: None, diff: String::new() },
            // Python
            AgentBenchSample { fixture: "python".into(), agent: "codex".into(),  model: codex_model.clone(),  iteration: 1, duration_ms: 3110.0, success: true,  conventional_valid: true,  message: Some("feat: adicionar testes para módulo de cálculo".into()), error: None, diff: python_diff() },
            AgentBenchSample { fixture: "python".into(), agent: "claude".into(), model: claude_model.clone(), iteration: 1, duration_ms: 5368.0, success: true,  conventional_valid: true,  message: Some("feat(calculator): adicionar função calculate_total com teste".into()), error: None, diff: String::new() },
            AgentBenchSample { fixture: "python".into(), agent: "ollama".into(), model: ollama_model.clone(), iteration: 1, duration_ms: 348.0,  success: true,  conventional_valid: true,  message: Some("feat(calculator): adicionar função de cálculo total e teste".into()), error: None, diff: String::new() },
            AgentBenchSample { fixture: "python".into(), agent: "codex".into(),  model: codex_model.clone(),  iteration: 2, duration_ms: 2950.0, success: true,  conventional_valid: false, message: Some("update calculator".into()), error: None, diff: String::new() },
            AgentBenchSample { fixture: "python".into(), agent: "claude".into(), model: claude_model.clone(), iteration: 2, duration_ms: 5418.0, success: true,  conventional_valid: true,  message: Some("feat(calculator): adiciona função calculate_total com teste".into()), error: None, diff: String::new() },
            AgentBenchSample { fixture: "python".into(), agent: "ollama".into(), model: ollama_model.clone(), iteration: 2, duration_ms: 254.0,  success: true,  conventional_valid: true,  message: Some("feat(calculator): adicionar função de cálculo total e teste".into()), error: None, diff: String::new() },
            // TypeScript
            AgentBenchSample { fixture: "typescript".into(), agent: "codex".into(),  model: codex_model.clone(),  iteration: 1, duration_ms: 3050.0, success: true,  conventional_valid: true,  message: Some("feat: adicionar testes para utilitários de strings".into()), error: None, diff: ts_diff() },
            AgentBenchSample { fixture: "typescript".into(), agent: "claude".into(), model: claude_model.clone(), iteration: 1, duration_ms: 4815.0, success: false, conventional_valid: false, message: None, error: Some("Claude CLI: timeout de 30s atingido sem resposta do servidor.".into()), diff: String::new() },
            AgentBenchSample { fixture: "typescript".into(), agent: "ollama".into(), model: ollama_model.clone(), iteration: 1, duration_ms: 441.0,  success: true,  conventional_valid: true,  message: Some("feat(calculator): adicionar função de cálculo total".into()), error: None, diff: String::new() },
            AgentBenchSample { fixture: "typescript".into(), agent: "codex".into(),  model: codex_model.clone(),  iteration: 2, duration_ms: 3100.0, success: true,  conventional_valid: true,  message: Some("test(utils): cobrir upper com casos básicos".into()), error: None, diff: String::new() },
            AgentBenchSample { fixture: "typescript".into(), agent: "claude".into(), model: claude_model.clone(), iteration: 2, duration_ms: 5170.0, success: true,  conventional_valid: true,  message: Some("test(utils): adicionar testes para upper".into()), error: None, diff: String::new() },
            AgentBenchSample { fixture: "typescript".into(), agent: "ollama".into(), model: ollama_model.clone(), iteration: 2, duration_ms: 237.0,  success: true,  conventional_valid: true,  message: Some("feat(calculator): adicionar função calculateTotal".into()), error: None, diff: String::new() },
        ],
        show_samples: 2,
        override_notes: vec![
            "codex: home=/home/junior/.config/cloak/profiles/amjr/codex, model=gpt-5".into(),
            "claude: config_dir=/home/junior/.config/cloak/profiles/amjr/claude, model=claude-sonnet-4-6".into(),
            "ollama: model=juniormartinxo/seshat-commit".into(),
        ],
    };

    let html = generate_html_report(&report, ReportLanguage::Portuguese);
    let path = "/tmp/seshat-bench-demo.html";
    std::fs::write(path, html).expect("write html");
    println!(
        "Demo HTML escrito em {path} ({} KB)",
        std::fs::metadata(path).unwrap().len() / 1024
    );
    println!();
    println!("Para abrir:");
    println!("  wslview {path}");
    println!("  # ou:");
    println!("  explorer.exe $(wslpath -w {path})");
}

fn rust_diff() -> String {
    "diff --git a/src/calculator.rs b/src/calculator.rs\n\
     new file mode 100644\n\
     index 0000000..f3d1d88\n\
     --- /dev/null\n\
     +++ b/src/calculator.rs\n\
     @@ -0,0 +1,13 @@\n\
     +pub fn calculate_total(items: &[u32]) -> u32 {\n\
     +    items.iter().sum()\n\
     +}\n\
     +\n\
     +#[cfg(test)]\n\
     +mod tests {\n\
     +    use super::*;\n\
     +\n\
     +    #[test]\n\
     +    fn sums_items() {\n\
     +        assert_eq!(calculate_total(&[2, 3, 5]), 10);\n\
     +    }\n\
     +}"
    .into()
}

fn python_diff() -> String {
    "diff --git a/src/calculator.py b/src/calculator.py\n\
     new file mode 100644\n\
     index 0000000..2cda04f\n\
     --- /dev/null\n\
     +++ b/src/calculator.py\n\
     @@ -0,0 +1,9 @@\n\
     +from __future__ import annotations\n\
     +\n\
     +\n\
     +def calculate_total(items: list[int]) -> int:\n\
     +    return sum(items)\n\
     +\n\
     +\n\
     +def test_calculate_total() -> None:\n\
     +    assert calculate_total([2, 3, 5]) == 10"
        .into()
}

fn ts_diff() -> String {
    "diff --git a/src/calculator.ts b/src/calculator.ts\n\
     new file mode 100644\n\
     index 0000000..29dc5e5\n\
     --- /dev/null\n\
     +++ b/src/calculator.ts\n\
     @@ -0,0 +1,7 @@\n\
     +export function calculateTotal(items: number[]): number {\n\
     +  return items.reduce((total, item) => total + item, 0);\n\
     +}\n\
     +\n\
     +if (calculateTotal([2, 3, 5]) !== 10) {\n\
     +  throw new Error(\"unexpected total\");\n\
     +}"
    .into()
}
