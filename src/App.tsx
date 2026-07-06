import { useEffect, useMemo, useState } from "react";
import { CATALOG, compareVersions, extensionRoutes, type CatalogApp } from "./catalog";
import {
  applyAssociations,
  detectApps,
  getLatestRelease,
  getOs,
  inTauri,
  installApp,
  launchApp,
  onProgress,
  readDispatch,
  type AssocEntry,
  type InstalledInfo,
  type Progress,
  type ReleaseInfo,
} from "./lib/hub";
import "./App.css";

type Tab = "apps" | "arquivos";

function fmtBytes(n: number): string {
  if (n >= 1_000_000) return `${(n / 1_000_000).toFixed(0)} MB`;
  return `${(n / 1_000).toFixed(0)} KB`;
}

export default function App() {
  const tauri = inTauri();
  const [tab, setTab] = useState<Tab>("apps");
  const [os, setOs] = useState<string>("windows");
  const [installed, setInstalled] = useState<Record<string, InstalledInfo>>({});
  const [latest, setLatest] = useState<Record<string, ReleaseInfo>>({});
  const [progress, setProgress] = useState<Record<string, Progress>>({});
  const [busy, setBusy] = useState<Record<string, boolean>>({});
  const [errors, setErrors] = useState<Record<string, string>>({});
  const [routes, setRoutes] = useState<Record<string, string>>({});
  const [assocMsg, setAssocMsg] = useState<string>("");

  const routeOptions = useMemo(() => extensionRoutes(), []);

  const refreshInstalled = async () => {
    const infos = await detectApps(CATALOG);
    const map: Record<string, InstalledInfo> = {};
    for (const info of infos) map[info.id] = info;
    setInstalled(map);
    return map;
  };

  useEffect(() => {
    // Rotas default: primeiro app do catálogo que declara a extensão.
    const defaults: Record<string, string> = {};
    for (const [ext, apps] of routeOptions) defaults[ext] = apps[0].id;
    setRoutes(defaults);

    if (!tauri) return;
    let unlisten: (() => void) | undefined;
    (async () => {
      try {
        setOs(await getOs());
        await refreshInstalled();
        // Rotas salvas anteriormente sobrescrevem os defaults.
        const saved = await readDispatch();
        if (saved.length) {
          setRoutes((prev) => {
            const next = { ...prev };
            for (const e of saved) if (next[e.ext]) next[e.ext] = e.appId;
            return next;
          });
        }
        unlisten = await onProgress((p) =>
          setProgress((prev) => ({ ...prev, [p.id]: p })),
        );
        // Última versão de cada app (tolera falha individual, ex. sem internet).
        await Promise.allSettled(
          CATALOG.map(async (app) => {
            const rel = await getLatestRelease(app.repo);
            setLatest((prev) => ({ ...prev, [app.id]: rel }));
          }),
        );
      } catch (e) {
        setErrors((prev) => ({ ...prev, _global: String(e) }));
      }
    })();
    return () => unlisten?.();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [tauri]);

  const doInstall = async (app: CatalogApp) => {
    setBusy((b) => ({ ...b, [app.id]: true }));
    setErrors((e) => ({ ...e, [app.id]: "" }));
    try {
      const info = await installApp(app, os);
      setInstalled((prev) => ({ ...prev, [app.id]: info }));
    } catch (e) {
      setErrors((prev) => ({ ...prev, [app.id]: String(e) }));
    } finally {
      setBusy((b) => ({ ...b, [app.id]: false }));
      setProgress((p) => {
        const next = { ...p };
        delete next[app.id];
        return next;
      });
    }
  };

  const doLaunch = async (app: CatalogApp) => {
    const info = installed[app.id];
    if (!info?.exe) return;
    try {
      await launchApp(info.exe);
    } catch (e) {
      setErrors((prev) => ({ ...prev, [app.id]: String(e) }));
    }
  };

  const doApplyAssociations = async () => {
    setAssocMsg("Aplicando…");
    const entries: AssocEntry[] = [];
    for (const [ext, appId] of Object.entries(routes)) {
      const app = CATALOG.find((a) => a.id === appId);
      const info = installed[appId];
      if (!app || !info?.installed || !info.exe) continue;
      entries.push({ ext, appId, appName: app.name, exe: info.exe });
    }
    if (!entries.length) {
      setAssocMsg("Nenhum app instalado pra associar — instale primeiro na aba Apps.");
      return;
    }
    try {
      const warnings = await applyAssociations(entries);
      setAssocMsg(
        warnings.length
          ? `Aplicado com avisos: ${warnings.join("; ")}`
          : `Associações aplicadas (${entries.length} extensões).`,
      );
    } catch (e) {
      setAssocMsg(`Erro: ${e}`);
    }
  };

  return (
    <div className="hub">
      <header className="hub-header">
        <div className="hub-brand">
          <span className="hub-logo">◱</span>
          <h1>
            TaylorHub <span className="hub-sub">suíte Taylor — instalar, atualizar, abrir</span>
          </h1>
        </div>
        <nav className="hub-tabs">
          <button className={tab === "apps" ? "active" : ""} onClick={() => setTab("apps")}>
            Apps
          </button>
          <button
            className={tab === "arquivos" ? "active" : ""}
            onClick={() => setTab("arquivos")}
          >
            Arquivos
          </button>
        </nav>
      </header>

      {!tauri && (
        <div className="hub-banner">
          Rodando no navegador (preview) — instalar/detectar só funciona no app Tauri.
        </div>
      )}
      {errors._global && <div className="hub-banner error">{errors._global}</div>}

      {tab === "apps" && (
        <main className="hub-grid">
          {CATALOG.map((app) => {
            const info = installed[app.id];
            const rel = latest[app.id];
            const hasUpdate =
              info?.installed && rel && compareVersions(rel.version, info.version) > 0;
            const prog = progress[app.id];
            const isBusy = !!busy[app.id];
            return (
              <div className="card" key={app.id} style={{ borderTopColor: app.accent }}>
                <div className="card-head">
                  <span className="avatar" style={{ background: app.accent }}>
                    {app.name[0]}
                  </span>
                  <div>
                    <h2>{app.name}</h2>
                    <div className="version">
                      {info?.installed ? (
                        <>
                          v{info.version || "?"}
                          {hasUpdate && <span className="badge">v{rel!.version} disponível</span>}
                        </>
                      ) : (
                        <span className="muted">
                          não instalado{rel ? ` — última: v${rel.version}` : ""}
                        </span>
                      )}
                    </div>
                  </div>
                </div>
                <p className="desc">{app.description}</p>
                {isBusy && (
                  <div className="progress">
                    {prog?.phase === "download" && prog.total > 0 ? (
                      <>
                        <div className="bar">
                          <div
                            className="fill"
                            style={{ width: `${(100 * prog.done) / prog.total}%` }}
                          />
                        </div>
                        <span>
                          {fmtBytes(prog.done)} / {fmtBytes(prog.total)}
                        </span>
                      </>
                    ) : (
                      <span>{prog?.phase === "install" ? "Instalando…" : "Preparando…"}</span>
                    )}
                  </div>
                )}
                {errors[app.id] && <div className="err">{errors[app.id]}</div>}
                <div className="actions">
                  {!info?.installed && (
                    <button className="primary" disabled={!tauri || isBusy} onClick={() => doInstall(app)}>
                      Instalar
                    </button>
                  )}
                  {hasUpdate && (
                    <button className="primary" disabled={isBusy} onClick={() => doInstall(app)}>
                      Atualizar
                    </button>
                  )}
                  {info?.installed && (
                    <button disabled={isBusy || !info.exe} onClick={() => doLaunch(app)}>
                      Abrir
                    </button>
                  )}
                </div>
              </div>
            );
          })}
        </main>
      )}

      {tab === "arquivos" && (
        <main className="hub-files">
          <p className="hint">
            Escolha qual app abre cada tipo de arquivo. O Hub registra a associação e despacha o
            arquivo pro app certo ao clicar. Extensões que já têm dono no sistema (ex.: .md, .pdf)
            podem exigir confirmar no diálogo "Como você quer abrir?" do Windows uma vez.
          </p>
          <table>
            <thead>
              <tr>
                <th>Extensão</th>
                <th>Abrir com</th>
              </tr>
            </thead>
            <tbody>
              {[...routeOptions.entries()].map(([ext, apps]) => (
                <tr key={ext}>
                  <td>
                    <code>.{ext}</code>
                  </td>
                  <td>
                    <select
                      value={routes[ext] ?? apps[0].id}
                      onChange={(e) => setRoutes((r) => ({ ...r, [ext]: e.target.value }))}
                    >
                      {apps.map((a) => (
                        <option key={a.id} value={a.id}>
                          {a.name}
                          {installed[a.id]?.installed ? "" : " (não instalado)"}
                        </option>
                      ))}
                    </select>
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
          <div className="files-actions">
            <button className="primary" disabled={!tauri} onClick={doApplyAssociations}>
              Aplicar associações
            </button>
            {assocMsg && <span className="assoc-msg">{assocMsg}</span>}
          </div>
        </main>
      )}

      <footer className="hub-footer">
        Taylor — 100% offline depois de instalado; o Hub só acessa o GitHub pra baixar
        releases. Sem telemetria.
      </footer>
    </div>
  );
}
