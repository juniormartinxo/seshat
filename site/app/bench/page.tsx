import type { Metadata } from "next";
import Link from "next/link";
import { ArrowLeft, BarChart3, FileJson, Terminal } from "lucide-react";
import { BenchDashboard } from "./bench-dashboard";
import { ThemeToggle } from "../theme-toggle";

export const metadata: Metadata = {
  title: "Bench | Seshat",
  description: "Carregue e visualize o JSON gerado por seshat bench agents."
};

export default function BenchPage() {
  return (
    <main className="bench-page-fx relative min-h-dvh overflow-x-hidden overflow-y-auto text-[#f2f5ef]">
      <header
        className="sticky top-0 z-20 flex items-center justify-between gap-6 border-b border-white/10 bg-[#141e24]/55 px-[clamp(18px,5vw,64px)] py-4 backdrop-blur-[18px] supports-[backdrop-filter]:backdrop-blur-xl max-[900px]:flex-col max-[900px]:items-start"
        aria-label="Navegacao do bench"
      >
        <Link className="inline-flex items-center gap-2.5 font-black text-[#f2f5ef] transition-colors hover:text-[#67e480]" href="/">
          <Terminal className="text-[#67e480]" aria-hidden="true" size={20} />
          <span>Seshat</span>
        </Link>
        <nav className="inline-flex items-center gap-[clamp(12px,3vw,24px)] text-sm text-[#a7b0aa] max-[900px]:flex-col max-[900px]:items-start">
          <Link className="inline-flex items-center gap-2 transition-colors hover:text-[#67e480]" href="/">
            <ArrowLeft aria-hidden="true" size={16} />
            Site
          </Link>
          <Link className="inline-flex items-center gap-2 transition-colors hover:text-[#67e480]" href="/docs">
            <FileJson aria-hidden="true" size={16} />
            Docs
          </Link>
          <ThemeToggle />
        </nav>
      </header>

      <section
        className="bench-hero-fx relative mx-auto mt-[clamp(24px,4vw,46px)] grid min-h-[clamp(470px,64svh,610px)] w-[calc(100%_-_48px)] max-w-[1180px] grid-cols-[minmax(0,0.95fr)_minmax(360px,1.05fr)] items-center gap-[clamp(30px,5vw,76px)] overflow-hidden rounded-lg border border-white/10 p-[clamp(34px,6vw,68px)] shadow-[0_42px_120px_rgba(0,0,0,0.36),inset_0_1px_0_rgba(242,245,239,0.08)] backdrop-blur-xl max-[900px]:grid-cols-1 max-[540px]:w-[calc(100%_-_32px)] max-[540px]:p-4"
        aria-labelledby="bench-title"
      >
        <div className="relative z-[1] min-w-0">
          <p className="inline-flex items-center gap-2 text-[0.82rem] font-extrabold uppercase tracking-[0.12em] text-[#67e480]">
            <BarChart3 aria-hidden="true" size={16} />
            Benchmark de agentes
          </p>
          <h1
            className="mt-5 max-w-[620px] text-[clamp(2.55rem,5.8vw,4.65rem)] font-black leading-[0.98] tracking-normal text-balance"
            id="bench-title"
          >
            Benchmark analytics.
          </h1>
          <div className="mt-8 flex flex-wrap gap-2.5" aria-label="Parametros do bench">
            {["codex", "claude", "ollama"].map((agent) => (
              <span
                className="inline-flex min-h-8 items-center rounded-full border border-white/15 bg-white/[0.045] px-3 text-[0.82rem] font-extrabold text-[#d9e2df]"
                key={agent}
              >
                {agent}
              </span>
            ))}
          </div>
        </div>
        <div className="relative z-[1] grid">
          <pre className="m-0 min-w-0 overflow-x-auto whitespace-pre-wrap rounded-lg border border-[#8bc6bd]/30 bg-[linear-gradient(135deg,rgba(139,198,189,0.09),rgba(255,180,84,0.035)),rgba(5,9,11,0.82)] px-[22px] py-5 font-mono text-[0.84rem] leading-[1.56] text-[#edf3f0] shadow-[0_22px_70px_rgba(0,0,0,0.24)] [overflow-wrap:anywhere] [word-break:break-all] max-[540px]:p-4 max-[540px]:text-[0.78rem]">{`seshat bench agents \\
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
