"use client";

import { useEffect, useMemo, useState } from "react";
import { CheckCircle2, Trophy, XCircle } from "lucide-react";

type AgentSummary = {
  fixture?: string;
  agent: string;
  model?: string;
  total: number;
  success: number;
  conventional_valid: number;
  avg_ms: number;
  min_ms: number;
  p95_ms?: number;
  max_ms?: number;
  fixtures_won?: number;
};

type BenchSample = {
  fixture: string;
  agent: string;
  model?: string;
  iteration: number;
  duration_ms: number;
  success: boolean;
  conventional_valid: boolean;
  message?: string;
  error?: string;
  diff?: string;
};

type BenchReport = {
  schema_version?: number;
  generated_at?: string;
  seshat_version?: string;
  iterations: number;
  agents: string[];
  agent_selection: string;
  fixtures: string[];
  temp_root: string | null;
  summaries: AgentSummary[];
  overall: AgentSummary[];
  samples: BenchSample[];
  show_samples: number;
};

const benchJsonPath = "/data/bench.json";
const dashboardWidthClass = "mx-auto w-[calc(100%_-_48px)] max-w-[1180px] max-[540px]:w-[calc(100%_-_32px)]";
const accentCardClass =
  "bench-accent-fx relative overflow-hidden rounded-sm border border-white/[0.095] bg-[#0d1316]/80 shadow-[0_22px_72px_rgba(0,0,0,0.22),inset_0_1px_0_rgba(242,245,239,0.05)]";
const panelClass = `${accentCardClass} mt-5 p-[clamp(20px,3vw,28px)] backdrop-blur-lg max-[540px]:p-4`;

const kickerClass = "text-[0.78rem] font-extrabold uppercase tracking-[0.12em] text-[#67e480]";
const valueClass = "mt-2 block wrap-break-words text-[1.08rem] font-extrabold text-[#f2f5ef]";

function isBenchReport(value: unknown): value is BenchReport {
  if (!value || typeof value !== "object") {
    return false;
  }

  const report = value as Partial<BenchReport>;
  return (
    Array.isArray(report.overall) &&
    Array.isArray(report.samples) &&
    Array.isArray(report.agents) &&
    Array.isArray(report.fixtures)
  );
}

function formatNumber(value: number | undefined, digits = 0) {
  if (typeof value !== "number" || Number.isNaN(value)) {
    return "-";
  }

  return new Intl.NumberFormat("pt-BR", {
    maximumFractionDigits: digits,
    minimumFractionDigits: digits
  }).format(value);
}

function formatDuration(value: number | undefined) {
  if (typeof value !== "number" || Number.isNaN(value)) {
    return "-";
  }

  return `${formatNumber(value, value >= 100 ? 0 : 1)} ms`;
}

function formatDate(value: string) {
  const date = new Date(value);
  if (Number.isNaN(date.getTime())) {
    return value;
  }

  return new Intl.DateTimeFormat("pt-BR", {
    dateStyle: "medium",
    timeStyle: "short"
  }).format(date);
}

function successRate(summary: AgentSummary) {
  if (!summary.total) {
    return 0;
  }

  return Math.round((summary.success / summary.total) * 100);
}

function validRate(summary: AgentSummary) {
  if (!summary.total) {
    return 0;
  }

  return Math.round((summary.conventional_valid / summary.total) * 100);
}

function fixtureKey(summary: AgentSummary) {
  return `${summary.agent}${summary.model ? `:${summary.model}` : ""}`;
}

export function BenchDashboard() {
  const [report, setReport] = useState<BenchReport | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [selectedFixture, setSelectedFixture] = useState("");
  const [selectedSampleAgent, setSelectedSampleAgent] = useState("");

  const loadReport = async () => {
    setError(null);

    try {
      const response = await fetch(benchJsonPath, { cache: "no-store" });
      if (!response.ok) {
        throw new Error(`HTTP ${response.status}`);
      }

      const json = (await response.json()) as unknown;
      if (!isBenchReport(json)) {
        throw new Error("campos obrigatorios ausentes");
      }

      setReport(json);
    } catch (err) {
      setReport(null);
      setError(err instanceof Error ? err.message : "falha desconhecida");
    }
  };

  useEffect(() => {
    void loadReport();
  }, []);

  const bestAvgMs = useMemo(() => {
    if (!report?.overall.length) {
      return 0;
    }

    return Math.max(...report.overall.map((summary) => summary.avg_ms || 0));
  }, [report]);

  const fixtureWins = useMemo(() => {
    const wins = new Map<string, AgentSummary>();

    report?.summaries.forEach((summary) => {
      const key = fixtureKey(summary);
      const current = wins.get(key);

      if (!current || (summary.fixtures_won ?? 0) > (current.fixtures_won ?? 0)) {
        wins.set(key, summary);
      }
    });

    return wins;
  }, [report]);

  const sampleAgents = useMemo(() => {
    if (!report) {
      return [];
    }

    const agentsWithSamples = new Set(report.samples.map((sample) => sample.agent));
    const orderedAgents = report.agents.filter((agent) => agentsWithSamples.has(agent));
    const extraAgents = [...agentsWithSamples].filter((agent) => !orderedAgents.includes(agent));
    return [...orderedAgents, ...extraAgents];
  }, [report]);

  const fixtureSummaries = useMemo(() => {
    if (!report) {
      return [];
    }

    return report.summaries.length ? report.summaries : report.overall;
  }, [report]);

  const fixtureTabs = useMemo(() => {
    if (!report?.summaries.length) {
      return [];
    }

    const summarizedFixtures = new Set(
      report.summaries
        .map((summary) => summary.fixture)
        .filter((fixture): fixture is string => Boolean(fixture))
    );
    const orderedFixtures = report.fixtures.filter((fixture) => summarizedFixtures.has(fixture));
    const extraFixtures = [...summarizedFixtures].filter((fixture) => !orderedFixtures.includes(fixture));

    return [...orderedFixtures, ...extraFixtures];
  }, [report]);

  useEffect(() => {
    if (!fixtureTabs.length) {
      if (selectedFixture) {
        setSelectedFixture("");
      }
      return;
    }

    if (!fixtureTabs.includes(selectedFixture)) {
      setSelectedFixture(fixtureTabs[0]);
    }
  }, [fixtureTabs, selectedFixture]);

  useEffect(() => {
    if (!sampleAgents.length) {
      if (selectedSampleAgent) {
        setSelectedSampleAgent("");
      }
      return;
    }

    if (!sampleAgents.includes(selectedSampleAgent)) {
      setSelectedSampleAgent(sampleAgents[0]);
    }
  }, [sampleAgents, selectedSampleAgent]);

  const filteredSamples = useMemo(() => {
    if (!report) {
      return [];
    }

    return selectedSampleAgent
      ? report.samples.filter((sample) => sample.agent === selectedSampleAgent)
      : report.samples;
  }, [report, selectedSampleAgent]);

  const activeFixture = selectedFixture || fixtureTabs[0] || "";
  const filteredFixtureSummaries = useMemo(() => {
    if (!activeFixture || !report?.summaries.length) {
      return fixtureSummaries;
    }

    return fixtureSummaries.filter((summary) => summary.fixture === activeFixture);
  }, [activeFixture, fixtureSummaries, report?.summaries.length]);

  const visibleSamples = useMemo(() => filteredSamples.slice(0, 12), [filteredSamples]);
  const winner = report?.overall[0];

  return (
    <section className={`${dashboardWidthClass} relative z-[1] pb-[92px]`}>
      {!report ? (
        <div className={`${accentCardClass} mt-5 p-[clamp(24px,5vw,42px)]`}>
          <h2 className="m-0 text-[1.65rem] font-black">Gere o JSON de bench.</h2>
          <p className="mt-3 max-w-[680px] leading-[1.65] text-[#a7b0aa]">
            A pagina espera o arquivo em <code>site/public/data/bench.json</code>.
          </p>
          {error ? <p className="mt-1.5 text-[0.92rem] text-[#ffaaa6]">Detalhe: {error}</p> : null}
        </div>
      ) : (
        <>
          <div className="mt-5 grid grid-cols-3 gap-3.5 max-[900px]:grid-cols-1">
            <article className={`${accentCardClass} p-[18px]`}>
              <span className="text-mono text-xs">Gerado em</span>
              <strong className={valueClass}>
                {report.generated_at ? formatDate(report.generated_at) : "nao informado"}
              </strong>
            </article>
            <article className={`${accentCardClass} p-[18px]`}>
              <span className="text-mono text-xs">Versao</span>
              <strong className={valueClass}>
                {report.seshat_version ? `seshat ${report.seshat_version}` : "nao informado"}
              </strong>
            </article>
            <article className={`${accentCardClass} p-[18px]`}>
              <span className="text-mono text-xs">Iteracoes</span>
              <strong className={valueClass}>{report.iterations}</strong>
            </article>
          </div>

          <div className="mt-3.5 grid grid-cols-[1.2fr_repeat(3,minmax(0,1fr))] gap-3.5 max-[900px]:grid-cols-1">
            <article className={`${accentCardClass} p-[18px]`}>
              <span className="text-mono text-xs">Agentes</span>
              <strong className={valueClass}>{report.agents.length}</strong>
              <p className="mt-2.5 leading-normal text-[#a7b0aa]">{report.agents.join(", ")}</p>
            </article>
            <article className={`${accentCardClass} p-[18px]`}>
              <span className="text-mono text-xs">Fixtures</span>
              <strong className={valueClass}>{report.fixtures.length}</strong>
              <p className="mt-2.5 leading-normal text-[#a7b0aa]">{report.fixtures.join(", ")}</p>
            </article>
            <article className={`${accentCardClass} p-[18px]`}>
              <span className="text-mono text-xs">Amostras</span>
              <strong className={valueClass}>{report.samples.length}</strong>
              <p className="mt-2.5 leading-normal text-[#a7b0aa]">show_samples: {report.show_samples}</p>
            </article>
          </div>

          <div className={panelClass}>
            <div className="mb-[18px] flex items-end justify-between gap-6 max-[900px]:flex-col max-[900px]:items-start">
              <div>
                <p className={kickerClass}>Ranking</p>
                <h2 className="mt-2 text-[clamp(1.7rem,3.4vw,2.7rem)] font-black leading-none">
                  Desempenho geral por agente.
                </h2>
              </div>
            </div>
            <div className="grid grid-cols-3 gap-3.5 max-[900px]:grid-cols-1">
            <article
              className={`${accentCardClass} border-[#8bc6bd]/35 bg-[linear-gradient(135deg,rgba(139,198,189,0.18),rgba(255,180,84,0.06)),rgba(13,25,22,0.86)] p-[18px]`}
            >
              <Trophy className="text-[#8bc6bd]" aria-hidden="true" size={22} />
              <span className="text-mono text-xs">Melhor geral</span>
              <strong className="mt-2 block wrap-break-words text-[2.2rem] font-black text-[#f2f5ef]">
                {winner?.agent ?? "-"}
              </strong>
              <p className="mt-2.5 leading-normal text-[#a7b0aa]">{winner?.model ?? "modelo nao informado"}</p>
            </article>
              {report.overall.map((summary, index) => {
                const width = bestAvgMs ? Math.max(6, (summary.avg_ms / bestAvgMs) * 100) : 0;

                return (
                  <article
                    className="rounded-sm border border-white/10 bg-[linear-gradient(180deg,rgba(139,198,189,0.056),rgba(139,198,189,0)),rgba(8,13,15,0.74)] p-[18px]"
                    key={`${summary.agent}-${summary.model ?? "default"}`}
                  >
                    <div className="flex items-center gap-3.5">
                      <span className="inline-flex size-[34px] items-center justify-center rounded-full bg-[#8bc6bd]/15 font-black text-[#8bc6bd]">
                        #{index + 1}
                      </span>
                      <div className="min-w-0">
                        <strong className="block text-[1.15rem] text-[#f2f5ef]">{summary.agent}</strong>
                        <p className="mt-1 text-[0.9rem] text-[#a7b0aa]">{summary.model ?? "modelo nao informado"}</p>
                      </div>
                    </div>
                    <div
                      className="my-[18px] h-2.5 overflow-hidden rounded-full bg-white/10"
                      aria-label={`Media ${formatDuration(summary.avg_ms)}`}
                    >
                      <span
                        className="block h-full rounded-full bg-[linear-gradient(90deg,#8bc6bd,#ffb454,#51d7ff)]"
                        style={{ width: `${width}%` }}
                      />
                    </div>
                    <dl className="grid grid-cols-2 gap-3 max-[540px]:grid-cols-1">
                      <div>
                        <dt className="text-mono text-xs">Sucesso</dt>
                        <dd className="mt-1 font-extrabold text-[#f2f5ef]">
                          {summary.success}/{summary.total} ({successRate(summary)}%)
                        </dd>
                      </div>
                      <div>
                        <dt className="text-mono text-xs">CC valido</dt>
                        <dd className="mt-1 font-extrabold text-[#f2f5ef]">
                          {summary.conventional_valid}/{summary.total} ({validRate(summary)}%)
                        </dd>
                      </div>
                      <div>
                        <dt className="text-mono text-xs">Media</dt>
                        <dd className="mt-1 font-extrabold text-[#f2f5ef]">{formatDuration(summary.avg_ms)}</dd>
                      </div>
                      <div>
                        <dt className="text-mono text-xs">P95</dt>
                        <dd className="mt-1 font-extrabold text-[#f2f5ef]">{formatDuration(summary.p95_ms)}</dd>
                      </div>
                      <div>
                        <dt className="text-mono text-xs">Fixtures vencidas</dt>
                        <dd className="mt-1 font-extrabold text-[#f2f5ef]">{summary.fixtures_won ?? 0}</dd>
                      </div>
                    </dl>
                  </article>
                );
              })}
            </div>
          </div>

          <div className={panelClass}>
            <div className="mb-[18px] flex items-end justify-between gap-6 max-[900px]:flex-col max-[900px]:items-start">
              <div>
                <p className={kickerClass}>Fixtures</p>
                <h2 className="mt-2 text-[clamp(1.7rem,3.4vw,2.7rem)] font-black leading-none">
                  Resumo por fixture e agente.
                </h2>
              </div>
            </div>
            {fixtureTabs.length ? (
              <div
                className="mb-[18px] flex flex-wrap gap-2 border-b border-white/10 pb-3.5"
                role="tablist"
                aria-label="Fixtures por linguagem"
              >
                {fixtureTabs.map((fixture) => (
                  <button
                    aria-selected={fixture === activeFixture}
                    className="min-h-9 cursor-pointer rounded-sm border border-white/15 bg-white/[0.045] px-3.5 text-[0.9rem] font-extrabold text-[#a7b0aa] aria-selected:border-[#8bc6bd]/40 aria-selected:bg-[linear-gradient(90deg,rgba(139,198,189,0.22),rgba(255,180,84,0.1))] aria-selected:text-[#f2f5ef]"
                    key={fixture}
                    onClick={() => setSelectedFixture(fixture)}
                    role="tab"
                    type="button"
                  >
                    {fixture}
                  </button>
                ))}
              </div>
            ) : null}
            <div className="overflow-x-auto">
              <table className="w-full min-w-[760px] border-collapse">
                <thead>
                  <tr>
                    {["Fixture", "Agente", "Modelo", "Sucesso", "CC valido", "Media", "P95", "Wins"].map((heading) => (
                      <th
                        className="border-b border-white/10 p-3 text-left align-top text-[0.78rem] font-extrabold uppercase tracking-[0.08em] text-[#a7b0aa]"
                        key={heading}
                      >
                        {heading}
                      </th>
                    ))}
                  </tr>
                </thead>
                <tbody>
                  {filteredFixtureSummaries.map((summary) => {
                    const winSource = fixtureWins.get(fixtureKey(summary));
                    return (
                      <tr
                        className="odd:bg-white/[0.025] hover:bg-[#51d7ff]/5"
                        key={`${summary.fixture ?? "geral"}-${summary.agent}-${summary.model ?? "default"}-${summary.avg_ms}`}
                      >
                        <td className="border-b border-white/10 p-3 align-top text-[#d8dfda]">{summary.fixture ?? "geral"}</td>
                        <td className="border-b border-white/10 p-3 align-top text-[#d8dfda]">{summary.agent}</td>
                        <td className="border-b border-white/10 p-3 align-top text-[#d8dfda]">{summary.model ?? "-"}</td>
                        <td className="border-b border-white/10 p-3 align-top text-[#d8dfda]">
                          {summary.success}/{summary.total}
                        </td>
                        <td className="border-b border-white/10 p-3 align-top text-[#d8dfda]">
                          {summary.conventional_valid}/{summary.total}
                        </td>
                        <td className="border-b border-white/10 p-3 align-top text-[#d8dfda]">
                          {formatDuration(summary.avg_ms)}
                        </td>
                        <td className="border-b border-white/10 p-3 align-top text-[#d8dfda]">
                          {formatDuration(summary.p95_ms)}
                        </td>
                        <td className="border-b border-white/10 p-3 align-top text-[#d8dfda]">
                          {winSource?.fixtures_won ?? summary.fixtures_won ?? 0}
                        </td>
                      </tr>
                    );
                  })}
                </tbody>
              </table>
            </div>
          </div>

          <div className={panelClass}>
            <div className="mb-[18px] flex items-end justify-between gap-6 max-[900px]:flex-col max-[900px]:items-start">
              <div>
                <p className={kickerClass}>Samples</p>
                <h2 className="mt-2 text-[clamp(1.7rem,3.4vw,2.7rem)] font-black leading-none">
                  Execuções individuais
                </h2>
              </div>
              <span className="whitespace-nowrap text-[#a7b0aa]">
                {visibleSamples.length} de {filteredSamples.length}
              </span>
            </div>
            <div
              className="mb-[18px] flex flex-wrap gap-2 border-b border-white/10 pb-3.5"
              role="tablist"
              aria-label="Samples por agente"
            >
              {sampleAgents.map((agent) => (
                <button
                  aria-selected={agent === selectedSampleAgent}
                  className="min-h-9 cursor-pointer rounded-sm border border-white/15 bg-white/[0.045] px-3.5 text-[0.9rem] font-extrabold text-[#a7b0aa] aria-selected:border-[#8bc6bd]/40 aria-selected:bg-[linear-gradient(90deg,rgba(139,198,189,0.22),rgba(255,180,84,0.1))] aria-selected:text-[#f2f5ef]"
                  key={agent}
                  onClick={() => setSelectedSampleAgent(agent)}
                  role="tab"
                  type="button"
                >
                  {agent}
                </button>
              ))}
            </div>
            <div className="grid grid-cols-2 gap-3.5 max-[900px]:grid-cols-1">
              {visibleSamples.map((sample) => (
                <article
                  className="grid gap-3.5 rounded-sm border border-white/10 bg-[linear-gradient(180deg,rgba(139,198,189,0.048),rgba(139,198,189,0)),rgba(8,13,15,0.74)] p-[18px]"
                  key={`${sample.fixture}-${sample.agent}-${sample.iteration}-${sample.duration_ms}`}
                >
                  <header className="flex items-start justify-between gap-4">
                    <div className="min-w-0">
                      <strong className="block text-[1.18rem] text-[#f2f5ef]">{sample.fixture}</strong>
                      <div className="flex flex-row align-middle items-end gap-2">
                      <span className="text-xs rounded-sm bg-gray-950 px-2 py-0.5 font-mono">
                        {sample.agent}
                        </span>
                        <span className="text-xs rounded-sm bg-gray-950 px-2 py-0.5 font-mono ">
                        {sample.model ? ` / ${sample.model}` : ""}
                      </span>
                      </div>
                    </div>
                    {sample.success ? (
                      <CheckCircle2 className="shrink-0 text-[#67e480]" aria-hidden="true" size={20} />
                    ) : (
                      <XCircle className="shrink-0 text-[#ff6b6b]" aria-hidden="true" size={20} />
                    )}
                  </header>
                  <code className="block rounded-sm border border-white/10 bg-[#030709]/40 px-3.5 py-3 font-mono text-xs italic leading-[1.58] text-amber-200">
                    {sample.message ?? sample.error ?? "sem mensagem"}
                  </code>
                  <dl className="grid grid-cols-3 gap-3 max-[540px]:grid-cols-1">
                    <div>
                      <dt className="text-mono text-xs">Iteracao</dt>
                      <dd className="mt-1 font-extrabold text-[#f2f5ef]">{sample.iteration}</dd>
                    </div>
                    <div>
                      <dt className="text-mono text-xs">Duracao</dt>
                      <dd className="mt-1 font-extrabold text-[#f2f5ef]">{formatDuration(sample.duration_ms)}</dd>
                    </div>
                    <div>
                      <dt className="text-mono text-xs">CC valido</dt>
                      <dd className="mt-1 font-extrabold text-[#f2f5ef]">
                        {sample.conventional_valid ? "sim" : "nao"}
                      </dd>
                    </div>
                  </dl>
                </article>
              ))}
            </div>
          </div>
        </>
      )}
    </section>
  );
}
