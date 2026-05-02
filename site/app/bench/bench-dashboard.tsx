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

  const visibleSamples = useMemo(() => filteredSamples.slice(0, 12), [filteredSamples]);
  const winner = report?.overall[0];

  return (
    <section className="benchDashboard pb-24">
      {!report ? (
        <div className="benchEmpty mt-5">
          <h2>Gere o JSON de bench.</h2>
          <p>
            A pagina espera o arquivo em <code>site/public/data/bench.json</code>.
          </p>
          {error ? <p className="benchError">Detalhe: {error}</p> : null}
        </div>
      ) : (
        <>
          <div className="benchMeta">
            <article>
              <span>Gerado em</span>
              <strong>{report.generated_at ? formatDate(report.generated_at) : "nao informado"}</strong>
            </article>
            <article>
              <span>Versao</span>
              <strong>{report.seshat_version ? `seshat ${report.seshat_version}` : "nao informado"}</strong>
            </article>
            <article>
              <span>Iteracoes</span>
              <strong>{report.iterations}</strong>
            </article>
          </div>

          <div className="benchSummaryGrid">
            <article className="benchWinner">
              <Trophy aria-hidden="true" size={22} />
              <span>Melhor geral</span>
              <strong>{winner?.agent ?? "-"}</strong>
              <p>{winner?.model ?? "modelo nao informado"}</p>
            </article>
            <article>
              <span>Agentes</span>
              <strong>{report.agents.length}</strong>
              <p>{report.agents.join(", ")}</p>
            </article>
            <article>
              <span>Fixtures</span>
              <strong>{report.fixtures.length}</strong>
              <p>{report.fixtures.join(", ")}</p>
            </article>
            <article>
              <span>Amostras</span>
              <strong>{report.samples.length}</strong>
              <p>show_samples: {report.show_samples}</p>
            </article>
          </div>

          <div className="benchPanel">
            <div className="benchPanelHeader">
              <div>
                <p className="sectionKicker">Ranking</p>
                <h2>Desempenho geral por agente.</h2>
              </div>
            </div>
            <div className="benchRanking">
              {report.overall.map((summary, index) => {
                const width = bestAvgMs ? Math.max(6, (summary.avg_ms / bestAvgMs) * 100) : 0;

                return (
                  <article className="benchRankCard" key={`${summary.agent}-${summary.model ?? "default"}`}>
                    <div className="benchRankHead">
                      <span>#{index + 1}</span>
                      <div>
                        <strong>{summary.agent}</strong>
                        <p>{summary.model ?? "modelo nao informado"}</p>
                      </div>
                    </div>
                    <div className="benchBar" aria-label={`Media ${formatDuration(summary.avg_ms)}`}>
                      <span style={{ width: `${width}%` }} />
                    </div>
                    <dl>
                      <div>
                        <dt>Sucesso</dt>
                        <dd>{summary.success}/{summary.total} ({successRate(summary)}%)</dd>
                      </div>
                      <div>
                        <dt>CC valido</dt>
                        <dd>{summary.conventional_valid}/{summary.total} ({validRate(summary)}%)</dd>
                      </div>
                      <div>
                        <dt>Media</dt>
                        <dd>{formatDuration(summary.avg_ms)}</dd>
                      </div>
                      <div>
                        <dt>P95</dt>
                        <dd>{formatDuration(summary.p95_ms)}</dd>
                      </div>
                      <div>
                        <dt>Fixtures vencidas</dt>
                        <dd>{summary.fixtures_won ?? 0}</dd>
                      </div>
                    </dl>
                  </article>
                );
              })}
            </div>
          </div>

          <div className="benchPanel">
            <div className="benchPanelHeader">
              <div>
                <p className="sectionKicker">Fixtures</p>
                <h2>Resumo por fixture e agente.</h2>
              </div>
            </div>
            <div className="benchTableWrap">
              <table className="benchTable">
                <thead>
                  <tr>
                    <th>Fixture</th>
                    <th>Agente</th>
                    <th>Modelo</th>
                    <th>Sucesso</th>
                    <th>CC valido</th>
                    <th>Media</th>
                    <th>P95</th>
                    <th>Wins</th>
                  </tr>
                </thead>
                <tbody>
                  {(report.summaries.length ? report.summaries : report.overall).map((summary) => {
                    const winSource = fixtureWins.get(fixtureKey(summary));
                    return (
                      <tr
                        key={`${summary.fixture ?? "geral"}-${summary.agent}-${summary.model ?? "default"}-${summary.avg_ms}`}
                      >
                        <td>{summary.fixture ?? "geral"}</td>
                        <td>{summary.agent}</td>
                        <td>{summary.model ?? "-"}</td>
                        <td>{summary.success}/{summary.total}</td>
                        <td>{summary.conventional_valid}/{summary.total}</td>
                        <td>{formatDuration(summary.avg_ms)}</td>
                        <td>{formatDuration(summary.p95_ms)}</td>
                        <td>{winSource?.fixtures_won ?? summary.fixtures_won ?? 0}</td>
                      </tr>
                    );
                  })}
                </tbody>
              </table>
            </div>
          </div>

          <div className="benchPanel">
            <div className="benchPanelHeader">
              <div>
                <p className="sectionKicker">Samples</p>
                <h2>Execuções individuais</h2>
              </div>
              <span>{visibleSamples.length} de {filteredSamples.length}</span>
            </div>
            <div className="benchTabs" role="tablist" aria-label="Samples por agente">
              {sampleAgents.map((agent) => (
                <button
                  aria-selected={agent === selectedSampleAgent}
                  className="transition-colors"
                  key={agent}
                  onClick={() => setSelectedSampleAgent(agent)}
                  role="tab"
                  type="button"
                >
                  {agent}
                </button>
              ))}
            </div>
            <div className="flex flex-col gap-4">
              {visibleSamples.map((sample) => (
                <article
                  className="benchSample"
                  key={`${sample.fixture}-${sample.agent}-${sample.iteration}-${sample.duration_ms}`}
                >
                  <header>
                    <div>
                      <strong>{sample.fixture}</strong>
                      <span>{sample.agent}{sample.model ? ` / ${sample.model}` : ""}</span>
                    </div>
                    {sample.success ? (
                      <CheckCircle2 className="sampleOk" aria-hidden="true" size={20} />
                    ) : (
                      <XCircle className="sampleFail" aria-hidden="true" size={20} />
                    )}
                  </header>
                  <p>{sample.message ?? sample.error ?? "sem mensagem"}</p>
                  <dl>
                    <div>
                      <dt>Iteracao</dt>
                      <dd>{sample.iteration}</dd>
                    </div>
                    <div>
                      <dt>Duracao</dt>
                      <dd>{formatDuration(sample.duration_ms)}</dd>
                    </div>
                    <div>
                      <dt>CC valido</dt>
                      <dd>{sample.conventional_valid ? "sim" : "nao"}</dd>
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
