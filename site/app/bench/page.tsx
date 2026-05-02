import type { Metadata } from "next";
import Link from "next/link";
import { ArrowLeft, BarChart3, FileJson, Terminal } from "lucide-react";
import { BenchDashboard } from "./bench-dashboard";

export const metadata: Metadata = {
  title: "Bench | Seshat",
  description: "Carregue e visualize o JSON gerado por seshat bench agents."
};

export default function BenchPage() {
  return (
    <main className="benchPage min-h-dvh text-[#f2f5ef]">
      <header className="benchTopbar supports-[backdrop-filter]:backdrop-blur-xl" aria-label="Navegacao do bench">
        <Link className="docsBrand transition-colors hover:text-[#67e480]" href="/">
          <Terminal aria-hidden="true" size={20} />
          <span>Seshat</span>
        </Link>
        <nav className="text-sm">
          <Link className="transition-colors hover:text-[#67e480]" href="/">
            <ArrowLeft aria-hidden="true" size={16} />
            Site
          </Link>
          <Link className="transition-colors hover:text-[#67e480]" href="/docs">
            <FileJson aria-hidden="true" size={16} />
            Docs
          </Link>
        </nav>
      </header>

      <section className="benchHero" aria-labelledby="bench-title">
        <div className="benchHeroCopy min-w-0">
          <p className="sectionKicker">
            <BarChart3 aria-hidden="true" size={16} />
            Benchmark de agentes
          </p>
          <h1 id="bench-title">Benchmark analytics.</h1>
          <div className="benchHeroStats" aria-label="Parametros do bench">
            <span>codex</span>
            <span>claude</span>
            <span>ollama</span>
          </div>
        </div>
        <div className="benchHeroVisual">
          <div className="benchHeroChart" aria-hidden="true">
            <span style={{ height: "36%" }} />
            <span style={{ height: "62%" }} />
            <span style={{ height: "45%" }} />
            <span style={{ height: "78%" }} />
            <span style={{ height: "56%" }} />
            <span style={{ height: "88%" }} />
            <span style={{ height: "72%" }} />
          </div>
          <pre className="min-w-0 whitespace-pre overflow-x-auto">{`seshat bench agents \\
  --agents codex,claude,ollama \\
  --fixtures rust,python,typescript \\
  --iterations 5 \\
  --model seshat-commit \\
  --format text \\
  --pt-br \\
  --keep-temp \\
  --show-samples 3 \\
  --report bench.html \\
  --json bench.json \\
  --codex-bin codex \\
  --codex-home ~/.codex \\
  --codex-model gpt-5.3-codex \\
  --claude-bin claude \\
  --claude-config-dir ~/.claude \\
  --claude-model claude-sonnet-4-6 \\
  --ollama-model juniormartinxo/seshat-commit \\
  --profile amjr`}</pre>
        </div>
      </section>

      <BenchDashboard />
    </main>
  );
}
