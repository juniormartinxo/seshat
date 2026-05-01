import {
  ArrowRight,
  Bot,
  BookOpen,
  Cpu,
  Download,
  ExternalLink,
  GitBranch,
  GitCommitHorizontal,
  Play,
  ShieldCheck,
  Terminal,
  Workflow
} from "lucide-react";
import Link from "next/link";

const commands = [
  { prompt: "cargo install --git https://github.com/juniormartinxo/seshat", tone: "muted" },
  { prompt: "ollama pull juniormartinxo/seshat-commit", tone: "cyan" },
  { prompt: "seshat config --provider ollama --model juniormartinxo/seshat-commit", tone: "green" },
  { prompt: "git add src/main.rs && seshat commit --yes", tone: "amber" }
];

const terminalRows = [
  { label: "[info]", text: "Seshat Flow", tone: "cyan" },
  { label: "[ok]", text: "rustfmt (lint)", tone: "green" },
  { label: "[ok]", text: "cargo-test (test) created_e2e_test", tone: "green" },
  { label: "[ok]", text: "clippy (typecheck)", tone: "green" },
  { label: "[ok]", text: "fix(tooling): foca teste criado", tone: "amber" }
];

const features = [
  {
    icon: GitCommitHorizontal,
    title: "Commit com contexto",
    text: "Le o diff staged, gera Conventional Commit e confirma usando o provider configurado."
  },
  {
    icon: Workflow,
    title: "Flow por arquivo",
    text: "Separa o trabalho em commits pequenos, com locks por arquivo para fluxos paralelos."
  },
  {
    icon: ShieldCheck,
    title: "Checks antes do commit",
    text: "Roda lint, teste e typecheck com escopo no arquivo atual sempre que possivel."
  },
  {
    icon: Bot,
    title: "IA local ou externa",
    text: "Funciona com Ollama, Codex, Claude e outros providers configurados."
  }
];

const steps = [
  {
    title: "Instale",
    command: "cargo install --git https://github.com/juniormartinxo/seshat"
  },
  {
    title: "Configure",
    command: "seshat config --provider ollama --model juniormartinxo/seshat-commit"
  },
  {
    title: "Commit",
    command: "seshat commit --yes"
  }
];

export default function Home() {
  return (
    <main className="marketingPage">
      <header className="topbar" aria-label="Navegacao principal">
        <a className="brand" href="#top" aria-label="Seshat">
          <Terminal size={20} aria-hidden="true" />
          <span>Seshat</span>
        </a>
        <nav>
          <a href="#install">Instalar</a>
          <a href="#model">Modelo</a>
          <Link href="/docs">Docs</Link>
          <a href="https://github.com/juniormartinxo/seshat" target="_blank" rel="noreferrer">
            GitHub
          </a>
        </nav>
      </header>

      <section id="top" className="hero">
        <div className="heroScene" aria-hidden="true">
          <div className="scanline" />
          <div className="codeRail leftRail">
            {terminalRows.map((row) => (
              <span key={`${row.label}-${row.text}`} className={row.tone}>
                {row.label} {row.text}
              </span>
            ))}
          </div>
          <div className="codeRail rightRail">
            {commands.map((row) => (
              <span key={row.prompt} className={row.tone}>
                &gt; {row.prompt}
              </span>
            ))}
          </div>
        </div>

        <div className="heroContent">
          <div className="eyebrow">
            <Cpu size={16} aria-hidden="true" />
            CLI Rust para commits assistidos por IA
          </div>
          <h1>Seshat</h1>
          <p>
            Automatize commits com Conventional Commits, checks por arquivo e um modelo local
            pronto para rodar no Ollama.
          </p>
          <div className="heroActions">
            <a className="button primary" href="#install">
              <Download size={18} aria-hidden="true" />
              Instalar
            </a>
            <Link className="button secondary" href="/docs">
              <BookOpen size={18} aria-hidden="true" />
              Documentacao
            </Link>
            <a
              className="button secondary"
              href="https://ollama.com/juniormartinxo/seshat-commit"
              target="_blank"
              rel="noreferrer"
            >
              <Bot size={18} aria-hidden="true" />
              Modelo Ollama
            </a>
          </div>
        </div>
      </section>

      <section className="quickStart" id="install">
        <div className="sectionHeader">
          <p className="sectionKicker">Uso rapido</p>
          <h2>Do zero ao commit em tres comandos.</h2>
        </div>
        <div className="steps">
          {steps.map((step, index) => (
            <article className="step" key={step.title}>
              <span className="stepNumber">{index + 1}</span>
              <h3>{step.title}</h3>
              <code>{step.command}</code>
            </article>
          ))}
        </div>
      </section>

      <section className="featureBand">
        <div className="featureGrid">
          {features.map((feature) => {
            const Icon = feature.icon;
            return (
              <article className="feature" key={feature.title}>
                <Icon size={22} aria-hidden="true" />
                <h3>{feature.title}</h3>
                <p>{feature.text}</p>
              </article>
            );
          })}
        </div>
      </section>

      <section className="terminalBand" aria-label="Exemplo do terminal">
        <div className="terminalCopy">
          <p className="sectionKicker">Flow</p>
          <h2>Commits pequenos, checks no escopo certo.</h2>
          <p>
            O `flow` pega arquivos alterados, roda checks relevantes e cria commits atomicos sem
            misturar mudancas independentes.
          </p>
          <a className="inlineLink" href="https://github.com/juniormartinxo/seshat" target="_blank" rel="noreferrer">
            Ver codigo no GitHub
            <ExternalLink size={16} aria-hidden="true" />
          </a>
        </div>
        <div className="terminalWindow">
          <div className="terminalChrome">
            <span />
            <span />
            <span />
          </div>
          <pre>{`> seshat flow 3 --yes
Seshat Flow
  Files: 3
  Language: PT-BR
  Provider: ollama

[success] rustfmt (lint)
[success] cargo-test (test)
running 1 test
test created_e2e_test ... ok
[success] clippy (typecheck)
[ok] Sucesso: fix(tooling): foca teste criado`}</pre>
        </div>
      </section>

      <section className="modelBand" id="model">
        <div>
          <p className="sectionKicker">Modelo local</p>
          <h2>`seshat-commit` no Ollama.</h2>
          <p>
            Um modelo treinado para receber `git diff` e devolver uma mensagem de commit direta,
            em PT-BR e no formato Conventional Commits.
          </p>
        </div>
        <div className="modelActions">
          <a className="button primary" href="https://ollama.com/juniormartinxo/seshat-commit" target="_blank" rel="noreferrer">
            <Play size={18} aria-hidden="true" />
            Baixar modelo
          </a>
          <Link className="button secondary" href="/docs/seshat-commit">
            <BookOpen size={18} aria-hidden="true" />
            Como usar
          </Link>
        </div>
      </section>

      <footer>
        <span>Seshat</span>
        <div>
          <a href="https://github.com/juniormartinxo/seshat" target="_blank" rel="noreferrer">
            <GitBranch size={18} aria-hidden="true" />
            GitHub
          </a>
          <Link href="/docs">
            Docs
            <BookOpen size={16} aria-hidden="true" />
          </Link>
          <a href="#top">
            Topo
            <ArrowRight size={16} aria-hidden="true" />
          </a>
        </div>
      </footer>
    </main>
  );
}
