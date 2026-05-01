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
    <main className="benchPage">
      <header className="benchTopbar" aria-label="Navegacao do bench">
        <Link className="docsBrand" href="/">
          <Terminal aria-hidden="true" size={20} />
          <span>Seshat</span>
        </Link>
        <nav>
          <Link href="/">
            <ArrowLeft aria-hidden="true" size={16} />
            Site
          </Link>
          <Link href="/docs">
            <FileJson aria-hidden="true" size={16} />
            Docs
          </Link>
        </nav>
      </header>

      <section className="benchHero">
        <div>
          <p className="sectionKicker">
            <BarChart3 aria-hidden="true" size={16} />
            Benchmark de agentes
          </p>
          <h1>Carregue o resultado do bench.</h1>
          <p>
            A pagina le o schema JSON v1 gerado por <code>seshat bench agents --json</code> e
            transforma o ranking em uma visao navegavel para comparar agents, fixtures e mensagens.
          </p>
        </div>
        <pre>{`cd /home/junior/apps/jm/seshat-rs/site/public/data

seshat bench agents \\
  --agents codex,claude,ollama \\
  --fixtures rust,python,typescript \\
  --iterations 5 \\
  --pt-br \\
  --json`}</pre>
      </section>

      <BenchDashboard />
    </main>
  );
}
