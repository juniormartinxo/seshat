"use client";

import { useEffect, useMemo, useRef, useState } from "react";
import { CheckCircle2, FileUp, RefreshCw, Trophy, XCircle } from "lucide-react";

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
  schema_version: number;
  generated_at: string;
  seshat_version: string;
  iterations: number;
  agents: string[];
  agent_selection: string;
  fixtures: string[];
  temp_root: string | null;
  summaries: AgentSummary[];
  overall: AgentSummary[];
  samples: BenchSample[];
  show_samples: number;
  override_notes?: string[];
};

const defaultJsonPath = "/data/seshat-bench-report.json";
const fallbackJsonPaths = [defaultJsonPath, "/data/bench.json"];

function isBenchReport(value: unknown): value is BenchReport {
  if (!value || typeof value !== "object") {
    return false;
  }

  const report = value as Partial<BenchReport>;
  return (
    typeof report.schema_version === "number" &&
    report.schema_version >= 1 &&
    typeof report.generated_at === "string" &&
    typeof report.seshat_version === "string" &&
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
  const [source, setSource] = useState(defaultJsonPath);
  const [status, setStatus] = useState(`Buscando ${defaultJsonPath}...`);
  const [error, setError] = useState<string | null>(null);
  const inputRef = useRef<HTMLInputElement>(null);

  const loadReport = async (paths = fallbackJsonPaths) => {
    setStatus(`Carregando ${paths.join(" ou ")}...`);
    setError(null);
    const errors: string[] = [];

    for (const path of paths) {
      const response = await fetch(path, { cache: "no-store" });
      if (!response.ok) {
        errors.push(`${path}: HTTP ${response.status}`);
        continue;
      }

      const json = (await response.json()) as unknown;
      if (!isBenchReport(json)) {
        errors.push(`${path}: schema_version ou campos obrigatorios ausentes`);
        continue;
      }

      setReport(json);
      setSource(path);
      setStatus(`Relatorio carregado de ${path}`);
      return;
    }

    setReport(null);
    setStatus("Nenhum bench publicado em /data.");
    setError(errors.join(" | ") || "falha desconhecida");
  };

  const loadFile = async (file: File) => {
    setError(null);
    setStatus(`Lendo ${file.name}...`);

    try {
      const json = JSON.parse(await file.text()) as unknown;
      if (!isBenchReport(json)) {
        throw new Error("schema_version ou campos obrigatorios ausentes");
      }

      setReport(json);
      setSource(file.name);
      setStatus(`Relatorio carregado de ${file.name}`);
    } catch (err) {
      setReport(null);
      setStatus("Nao foi possivel carregar o arquivo.");
      setError(err instanceof Error ? err.message : "JSON invalido");
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

  const visibleSamples = useMemo(() => report?.samples.slice(0, 12) ?? [], [report]);
  const winner = report?.overall[0];

  return (
    <section className="benchDashboard">
      <div className="benchLoader">
        <div>
          <p className="benchStatus">{status}</p>
          {error ? <p className="benchError">Detalhe: {error}</p> : null}
        </div>
        <div className="benchActions">
          <button className="button secondary" type="button" onClick={() => void loadReport()}>
            <RefreshCw aria-hidden="true" size={17} />
            Recarregar
          </button>
          <button className="button primary" type="button" onClick={() => inputRef.current?.click()}>
            <FileUp aria-hidden="true" size={17} />
            Carregar JSON
          </button>
          <input
            ref={inputRef}
            type="file"
            accept="application/json,.json"
            onChange={(event) => {
              const file = event.target.files?.[0];
              if (file) {
                void loadFile(file);
              }
            }}
          />
        </div>
      </div>

      {!report ? (
        <div className="benchEmpty">
          <h2>Publique ou carregue um JSON de bench.</h2>
          <p>
            Gere o arquivo em <code>site/public/data/seshat-bench-report.json</code> para a
            pagina carregar automaticamente, ou use o botao acima para inspecionar um JSON local.
          </p>
        </div>
      ) : (
        <>
          <div className="benchMeta">
            <article>
              <span>Gerado em</span>
              <strong>{formatDate(report.generated_at)}</strong>
            </article>
            <article>
              <span>Versao</span>
              <strong>seshat {report.seshat_version}</strong>
            </article>
            <article>
              <span>Iteracoes</span>
              <strong>{report.iterations}</strong>
            </article>
            <article>
              <span>Fonte</span>
              <strong>{source}</strong>
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
                <h2>Execucoes individuais.</h2>
              </div>
              <span>{visibleSamples.length} de {report.samples.length}</span>
            </div>
            <div className="benchSamples">
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

          {report.override_notes?.length ? (
            <div className="benchPanel">
              <p className="sectionKicker">Notas</p>
              <ul className="benchNotes">
                {report.override_notes.map((note) => (
                  <li key={note}>{note}</li>
                ))}
              </ul>
            </div>
          ) : null}
        </>
      )}
    </section>
  );
}
