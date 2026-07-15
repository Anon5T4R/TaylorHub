import { useEffect, useMemo, useState } from "react";
import { getVersion } from "@tauri-apps/api/app";
import { openUrl } from "@tauri-apps/plugin-opener";
import { CATALOG, compareVersions, extensionRoutes, type CatalogApp } from "./catalog";
import {
  addCustomRepo,
  applyAssociations,
  detectApps,
  getIcon,
  getLatestRelease,
  getOs,
  githubClientConfigured,
  githubDevicePoll,
  githubDeviceStart,
  githubTokenStatus,
  inTauri,
  installApp,
  installCustomApp,
  launchApp,
  listCustomRepos,
  onProgress,
  clearRecents,
  HUB_REPO,
  openRecent,
  readDispatch,
  readRecents,
  recreateShortcuts,
  removeCustomRepo,
  removeRecent,
  setGithubToken,
  setRecentPinned,
  uninstallApp,
  updateSelf,
  type CustomApp,
  type RecentEntry,
  type AssocEntry,
  type InstalledInfo,
  type Progress,
  type ReleaseInfo,
} from "./lib/hub";
import "./App.css";

type Tab = "apps" | "recentes" | "arquivos";

function extOf(path: string): string {
  const name = path.split(/[\\/]/).pop() ?? "";
  const dot = name.lastIndexOf(".");
  return dot > 0 ? name.slice(dot + 1).toLowerCase() : "";
}

function fileName(path: string): string {
  return path.split(/[\\/]/).pop() ?? path;
}

function dirName(path: string): string {
  const i = Math.max(path.lastIndexOf("\\"), path.lastIndexOf("/"));
  return i > 0 ? path.slice(0, i) : "";
}

function timeAgo(ts: number): string {
  if (!ts) return "";
  const s = Math.floor(Date.now() / 1000) - ts;
  if (s < 60) return "agora";
  if (s < 3600) return `${Math.floor(s / 60)} min atrás`;
  if (s < 86400) return `${Math.floor(s / 3600)} h atrás`;
  if (s < 172800) return "ontem";
  return new Date(ts * 1000).toLocaleDateString();
}

function fmtBytes(n: number): string {
  if (n >= 1_000_000) return `${(n / 1_000_000).toFixed(0)} MB`;
  return `${(n / 1_000).toFixed(0)} KB`;
}

/** Relógio curto (HH:MM) da última verificação de atualizações. */
function shortClock(ms: number): string {
  return new Date(ms).toLocaleTimeString([], { hour: "2-digit", minute: "2-digit" });
}

const CUSTOM_CATEGORY = "Meus repositórios";

/** Ordem das seções da grade; categorias novas caem no fim. */
const CATEGORY_ORDER = [
  "Escritório",
  "Dados e conhecimento",
  "Desenvolvimento",
  "Comunicação",
  "Inteligência artificial",
  "Mídia",
  "Segurança",
  CUSTOM_CATEGORY,
];

/** Card genérico pra repositório do usuário, no mesmo formato do catálogo. */
function customAsCatalog(c: CustomApp): CatalogApp {
  return {
    id: c.id,
    name: c.name,
    description: `Repositório do usuário — ${c.repo}`,
    kind: "app",
    repo: c.repo,
    assets: { win: c.winAsset, linux: c.linuxAsset },
    silentArgs: ["/S"],
    exe: c.exe || `${c.name}.exe`,
    extensions: [],
    category: CUSTOM_CATEGORY,
    accent: "#64748b",
  };
}

/** Agrupa os cards por categoria, na ordem fixa das seções. */
function groupByCategory(apps: CatalogApp[]): [string, CatalogApp[]][] {
  const by = new Map<string, CatalogApp[]>();
  for (const a of apps) {
    const c = a.category || "Outros";
    const list = by.get(c) ?? [];
    list.push(a);
    by.set(c, list);
  }
  const known = CATEGORY_ORDER.filter((c) => by.has(c));
  const extra = [...by.keys()].filter((c) => !CATEGORY_ORDER.includes(c));
  return [...known, ...extra].map((c) => [c, by.get(c)!]);
}

/** Ícone real do app (cache local via get_icon); cai pra letra inicial se ainda não baixado. */
function AppAvatar({ app, src }: { app: CatalogApp; src?: string }) {
  const [broken, setBroken] = useState(false);
  if (src && !broken) {
    return (
      <img
        className="avatar avatar-img"
        src={src}
        alt=""
        onError={() => setBroken(true)}
      />
    );
  }
  return (
    <span className="avatar" style={{ background: app.accent }}>
      {app.name[0]}
    </span>
  );
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
  const [shortcutMsg, setShortcutMsg] = useState<string>("");
  const [hubUpdate, setHubUpdate] = useState<{ current: string; latest: string } | null>(null);
  const [hubUpdateMsg, setHubUpdateMsg] = useState<string>("");
  const [recents, setRecents] = useState<RecentEntry[]>([]);
  const [recentsMsg, setRecentsMsg] = useState<string>("");
  const [checking, setChecking] = useState(false);
  const [checkedAt, setCheckedAt] = useState<number | null>(null);
  const [icons, setIcons] = useState<Record<string, string>>({});

  // repositórios do usuário + login opcional no GitHub + ordem dos cards
  const [customApps, setCustomApps] = useState<CustomApp[]>([]);
  const [repoInput, setRepoInput] = useState("");
  const [repoMsg, setRepoMsg] = useState("");
  const [repoBusy, setRepoBusy] = useState(false);
  const [ghLogged, setGhLogged] = useState(false);
  const [ghOpen, setGhOpen] = useState(false);
  const [ghToken, setGhToken] = useState("");
  const [ghMsg, setGhMsg] = useState("");
  const [ghConfigured, setGhConfigured] = useState(false);
  const [ghFlow, setGhFlow] = useState<{ userCode: string; verificationUri: string } | null>(null);
  const [ghBusy, setGhBusy] = useState(false);
  const [ghManual, setGhManual] = useState(false);

  const routeOptions = useMemo(() => extensionRoutes(), []);

  const sections = useMemo(
    () => groupByCategory([...CATALOG, ...customApps.map(customAsCatalog)]),
    [customApps],
  );

  const refreshInstalled = async (customs: CustomApp[] = customApps) => {
    const specs = [
      ...CATALOG.map((a) => ({ id: a.id, name: a.name, exe: a.exe })),
      ...customs.map((c) => ({ id: c.id, name: c.name, exe: c.exe || `${c.name}.exe` })),
    ];
    const infos = await detectApps(specs);
    const map: Record<string, InstalledInfo> = {};
    for (const info of infos) map[info.id] = info;
    setInstalled(map);
    return map;
  };

  // Ícones dos cards: servidos do cache local em disco (data URL) — abrir o Hub
  // não gasta rede com isso. `force` re-baixa do GitHub (botão "Verificar
  // atualizações"). Sem cache e sem rede, o card fica com a letra inicial.
  const loadIcons = async (force = false) => {
    await Promise.allSettled(
      CATALOG.filter((app) => app.iconUrl).map(async (app) => {
        const src = await getIcon(app, force);
        setIcons((prev) => ({ ...prev, [app.id]: src }));
      }),
    );
  };

  // Consulta a última release de cada app + do próprio Hub. `force` fura o cache
  // (TTL de 30 min): o carregamento inicial usa o cache; o botão "Verificar
  // atualizações" força a consulta pra não ficar preso numa versão vencida.
  const loadReleases = async (force = false, customs: CustomApp[] = customApps) => {
    setChecking(true);
    try {
      const targets = [
        ...CATALOG.map((a) => ({ id: a.id, repo: a.repo })),
        ...customs.map((c) => ({ id: c.id, repo: c.repo })),
      ];
      const results = await Promise.allSettled(
        targets.map(async (app) => {
          const rel = await getLatestRelease(app.repo, force);
          setLatest((prev) => ({ ...prev, [app.id]: rel }));
        }),
      );
      if (results.every((r) => r.status === "rejected")) {
        const first = results[0] as PromiseRejectedResult;
        setErrors((prev) => ({
          ...prev,
          _global: `Não consegui consultar as releases no GitHub — sem isso não aparecem atualizações. (${first.reason})`,
        }));
      } else {
        setErrors((prev) => {
          if (!prev._global) return prev;
          const next = { ...prev };
          delete next._global;
          return next;
        });
      }
      // Update do PRÓPRIO Hub: só avisa; aplicar exige clique do usuário.
      try {
        const current = await getVersion();
        const rel = await getLatestRelease(HUB_REPO, force);
        setHubUpdate(compareVersions(rel.version, current) > 0 ? { current, latest: rel.version } : null);
      } catch {
        /* sem internet/release — segue o baile */
      }
      setCheckedAt(Date.now());
    } finally {
      setChecking(false);
    }
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
        const customs = await listCustomRepos();
        setCustomApps(customs);
        githubTokenStatus().then(setGhLogged).catch(() => {});
        githubClientConfigured().then(setGhConfigured).catch(() => {});
        await refreshInstalled(customs);
        setRecents(await readRecents());
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
        // Carregamento inicial: usa os caches (rápido e sem gastar rate-limit).
        // Ícones em paralelo — não precisam esperar as releases.
        void loadIcons(false);
        await loadReleases(false, customs);
      } catch (e) {
        setErrors((prev) => ({ ...prev, _global: String(e) }));
      }
    })();
    return () => unlisten?.();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [tauri]);

  const isCustom = (id: string) => id.startsWith("custom-");

  const doInstall = async (app: CatalogApp) => {
    setBusy((b) => ({ ...b, [app.id]: true }));
    setErrors((e) => ({ ...e, [app.id]: "" }));
    try {
      if (isCustom(app.id)) {
        const info = await installCustomApp(app.id);
        setInstalled((prev) => ({ ...prev, [app.id]: info }));
        // nome/exe reais são descobertos na instalação — recarrega a lista
        setCustomApps(await listCustomRepos());
      } else {
        // Linux: se já existe um AppImage (mesmo instalado por fora), atualiza NO MESMO caminho.
        const current = installed[app.id];
        const currentPath =
          os !== "windows" && current?.installed && current.source !== "deb"
            ? current.exe
            : undefined;
        const info = await installApp(app, os, currentPath);
        setInstalled((prev) => ({ ...prev, [app.id]: info }));
      }
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

  const doAddRepo = async () => {
    const input = repoInput.trim();
    if (!input || repoBusy) return;
    setRepoBusy(true);
    setRepoMsg("Consultando a release…");
    try {
      const c = await addCustomRepo(input);
      const customs = [...customApps, c];
      setCustomApps(customs);
      setRepoInput("");
      setRepoMsg(`${c.name} adicionado.`);
      void refreshInstalled(customs);
      getLatestRelease(c.repo).then((rel) =>
        setLatest((prev) => ({ ...prev, [c.id]: rel })),
      );
    } catch (e) {
      setRepoMsg(String(e));
    } finally {
      setRepoBusy(false);
    }
  };

  const doRemoveCustom = async (app: CatalogApp) => {
    if (!window.confirm(`Remover ${app.name} da lista do Hub?\n(o app instalado NÃO é desinstalado)`)) {
      return;
    }
    await removeCustomRepo(app.id);
    setCustomApps((list) => list.filter((c) => c.id !== app.id));
  };

  /** Login pelo navegador: device flow quando o build tem OAuth App; senão
   *  abre a página de criação de token já preenchida (colar uma vez). */
  const browserLogin = async () => {
    setGhMsg("");
    if (!ghConfigured) {
      await openUrl(
        "https://github.com/settings/tokens/new?description=TaylorHub%20(leitura%20p%C3%BAblica)&scopes=",
      ).catch(() => {});
      setGhManual(true);
      setGhMsg("Clique em \"Generate token\" na página que abriu, copie e cole aqui embaixo.");
      return;
    }
    setGhBusy(true);
    try {
      const s = await githubDeviceStart();
      setGhFlow({ userCode: s.userCode, verificationUri: s.verificationUri });
      await openUrl(s.verificationUri).catch(() => {});
      const limit = await githubDevicePoll(s.deviceCode, s.interval, s.expiresIn);
      setGhLogged(true);
      setGhFlow(null);
      setGhMsg(`Conectado — ${limit.toLocaleString("pt-BR")} requisições/hora.`);
    } catch (e) {
      setGhFlow(null);
      setGhMsg(String(e));
    } finally {
      setGhBusy(false);
    }
  };

  const saveGhToken = async () => {
    setGhMsg("Validando…");
    try {
      const limit = await setGithubToken(ghToken);
      setGhLogged(true);
      setGhToken("");
      setGhMsg(`Token salvo no cofre do sistema — ${limit.toLocaleString("pt-BR")} requisições/hora.`);
    } catch (e) {
      setGhMsg(String(e));
    }
  };

  const clearGhToken = async () => {
    try {
      await setGithubToken("");
      setGhLogged(false);
      setGhMsg("Token removido.");
    } catch (e) {
      setGhMsg(String(e));
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

  const doUninstall = async (app: CatalogApp) => {
    const info = installed[app.id];
    if (!info?.installed) return;
    if (!window.confirm(`Desinstalar ${app.name}?`)) return;
    setBusy((b) => ({ ...b, [app.id]: true }));
    setErrors((e) => ({ ...e, [app.id]: "" }));
    try {
      await uninstallApp(app, info);
      await refreshInstalled();
    } catch (e) {
      setErrors((prev) => ({ ...prev, [app.id]: String(e) }));
    } finally {
      setBusy((b) => ({ ...b, [app.id]: false }));
    }
  };

  const doUpdateSelf = async () => {
    if (!hubUpdate) return;
    const ok = window.confirm(
      `Atualizar o TaylorHub de v${hubUpdate.current} para v${hubUpdate.latest}?\n\n` +
        (os === "windows"
          ? "O Hub vai baixar a nova versão, fechar e se reinstalar sozinho."
          : "O Hub vai substituir o próprio AppImage; depois é só reabrir."),
    );
    if (!ok) return;
    setHubUpdateMsg("Baixando atualização…");
    try {
      const result = await updateSelf(os);
      setHubUpdateMsg(
        result === "restart"
          ? "Atualizado! Feche e reabra o Hub."
          : "Instalando — o Hub vai fechar e reabrir atualizado…",
      );
    } catch (e) {
      setHubUpdateMsg(`Erro: ${e}`);
    }
  };

  /** App que abre este arquivo hoje (rota da extensão) — precisa estar instalado. */
  const appForFile = (path: string): { app: CatalogApp; exe: string } | null => {
    const ext = extOf(path);
    const appId = routes[ext];
    const app = CATALOG.find((a) => a.id === appId);
    const info = installed[appId ?? ""];
    if (!app || !info?.installed || !info.exe) return null;
    return { app, exe: info.exe };
  };

  const doOpenRecent = async (entry: RecentEntry) => {
    const target = appForFile(entry.path);
    if (!target) {
      setRecentsMsg(`Nenhum app instalado atende .${extOf(entry.path)} — veja a aba Associações.`);
      return;
    }
    setRecentsMsg("");
    try {
      await openRecent(entry.path, target.exe);
      setRecents(await readRecents());
    } catch (e) {
      setRecentsMsg(String(e));
    }
  };

  const doTogglePin = async (entry: RecentEntry) => {
    setRecents(await setRecentPinned(entry.path, !entry.pinned));
  };

  const doRemoveRecent = async (entry: RecentEntry) => {
    setRecents(await removeRecent(entry.path));
  };

  const doClearRecents = async () => {
    if (!window.confirm("Limpar a lista de recentes? (os fixados ficam)")) return;
    setRecents(await clearRecents());
  };

  const doRecreateShortcuts = async () => {
    const entries = CATALOG.filter(
      (a) => installed[a.id]?.installed && installed[a.id].source !== "deb" && installed[a.id].exe,
    ).map((a) => ({ id: a.id, name: a.name, exe: installed[a.id].exe }));
    if (!entries.length) {
      setShortcutMsg("Nenhum app instalado (AppImage) pra criar atalho.");
      return;
    }
    try {
      const warnings = await recreateShortcuts(entries);
      setShortcutMsg(
        warnings.length
          ? `Atalhos com avisos: ${warnings.join("; ")}`
          : `Atalhos de menu recriados (${entries.length} apps).`,
      );
    } catch (e) {
      setShortcutMsg(`Erro: ${e}`);
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
            TaylorHub <span className="hub-sub">suíte Local — instalar, atualizar, abrir</span>
          </h1>
        </div>
        <nav className="hub-tabs">
          <button className={tab === "apps" ? "active" : ""} onClick={() => setTab("apps")}>
            Apps
          </button>
          <button
            className={tab === "recentes" ? "active" : ""}
            onClick={() => {
              setTab("recentes");
              if (tauri) readRecents().then(setRecents);
            }}
          >
            Recentes
          </button>
          <button
            className={tab === "arquivos" ? "active" : ""}
            onClick={() => setTab("arquivos")}
          >
            Associações
          </button>
        </nav>
        {tauri && (
          <div className="hub-refresh">
            <button
              className="gh-btn"
              onClick={() => {
                setGhMsg("");
                setGhOpen(true);
              }}
              title={
                ghLogged
                  ? "Logado no GitHub — 5.000 requisições/hora"
                  : "Login opcional no GitHub (evita o erro 403 de rate limit)"
              }
            >
              {ghLogged ? "GitHub ✓" : "GitHub"}
            </button>
            <button
              className="refresh-btn"
              onClick={() => {
                void loadIcons(true);
                void loadReleases(true);
              }}
              disabled={checking}
              title="Consulta o GitHub agora, ignorando os caches (releases e ícones)"
            >
              <span className={checking ? "spin" : ""}>↻</span>
              {checking ? "Verificando…" : "Verificar atualizações"}
            </button>
            {checkedAt && !checking && (
              <span className="refresh-when">Verificado {shortClock(checkedAt)}</span>
            )}
          </div>
        )}
      </header>

      {!tauri && (
        <div className="hub-banner">
          Rodando no navegador (preview) — instalar/detectar só funciona no app Tauri.
        </div>
      )}
      {errors._global && <div className="hub-banner error">{errors._global}</div>}
      {hubUpdate && (
        <div className="hub-banner update">
          <span>
            Nova versão do Hub disponível: v{hubUpdate.latest} (você está na v{hubUpdate.current}).
          </span>
          <button className="primary" onClick={doUpdateSelf}>
            Atualizar o Hub
          </button>
          {hubUpdateMsg && <span className="assoc-msg">{hubUpdateMsg}</span>}
        </div>
      )}

      {tab === "apps" && (
        <main className="hub-grid-wrap">
          {os === "linux" && (
            <div className="linux-bar">
              <button onClick={doRecreateShortcuts}>Recriar atalhos do menu</button>
              {shortcutMsg && <span className="assoc-msg">{shortcutMsg}</span>}
            </div>
          )}
          <div className="repo-bar">
            <input
              value={repoInput}
              onChange={(e) => setRepoInput(e.target.value)}
              onKeyDown={(e) => {
                if (e.key === "Enter") void doAddRepo();
              }}
              placeholder="github.com/dono/repo — adicionar app de fora do catálogo"
              spellCheck={false}
            />
            <button className="primary" onClick={doAddRepo} disabled={!tauri || repoBusy || !repoInput.trim()}>
              {repoBusy ? "Adicionando…" : "Adicionar repositório"}
            </button>
            {repoMsg && <span className="assoc-msg">{repoMsg}</span>}
          </div>
          {sections.map(([category, apps]) => (
          <section className="hub-section" key={category}>
          <h3 className="cat-title">{category}</h3>
          <div className="hub-grid">
          {apps.map((app) => {
            const info = installed[app.id];
            const rel = latest[app.id];
            const isDeb = info?.source === "deb";
            const hasUpdate =
              info?.installed &&
              !isDeb &&
              rel &&
              !!info.version &&
              compareVersions(rel.version, info.version) > 0;
            const prog = progress[app.id];
            const isBusy = !!busy[app.id];
            return (
              <div className="card" key={app.id} style={{ borderTopColor: app.accent }}>
                <div className="card-head">
                  <AppAvatar app={app} src={icons[app.id] ?? (tauri ? undefined : app.iconUrl)} />
                  <div>
                    <h2>{app.name}</h2>
                    <div className="version">
                      {info?.installed ? (
                        <>
                          v{info.version || "?"}
                          {isDeb && <span className="badge deb">via .deb</span>}
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
                  {info?.installed && !isDeb && (
                    <button
                      className="danger"
                      disabled={isBusy}
                      onClick={() => doUninstall(app)}
                    >
                      Desinstalar
                    </button>
                  )}
                  {isCustom(app.id) && (
                    <button
                      disabled={isBusy}
                      title="Tirar da lista do Hub (não desinstala)"
                      onClick={() => doRemoveCustom(app)}
                    >
                      Remover da lista
                    </button>
                  )}
                </div>
                {isDeb && (
                  <div className="deb-hint">
                    Instalado pelo gerenciador de pacotes — atualize/remova com o apt.
                  </div>
                )}
              </div>
            );
          })}
          </div>
          </section>
          ))}
        </main>
      )}

      {tab === "recentes" && (
        <main className="hub-recents">
          {recentsMsg && <div className="hub-banner error">{recentsMsg}</div>}
          {recents.length === 0 && (
            <p className="hint">
              Nada por aqui ainda. Os arquivos que você abrir com duplo-clique (via associações do
              Hub) aparecem nesta lista — e dá pra fixar os favoritos com a ☆.
            </p>
          )}
          {(["fixados", "recentes"] as const).map((section) => {
            const list = recents
              .filter((e) => (section === "fixados" ? e.pinned : !e.pinned))
              .sort((a, b) => b.ts - a.ts);
            if (!list.length) return null;
            return (
              <section key={section}>
                <h3>{section === "fixados" ? "★ Fixados" : "Recentes"}</h3>
                <ul className="recent-list">
                  {list.map((entry) => {
                    const target = appForFile(entry.path);
                    return (
                      <li key={entry.path}>
                        <span
                          className="recent-dot"
                          style={{ background: target?.app.accent ?? "var(--muted)" }}
                          title={target?.app.name ?? "sem app associado"}
                        />
                        <button
                          className="recent-name"
                          title={entry.path}
                          onClick={() => doOpenRecent(entry)}
                        >
                          {fileName(entry.path)}
                        </button>
                        <span className="recent-dir">{dirName(entry.path)}</span>
                        <span className="recent-time">{timeAgo(entry.ts)}</span>
                        <button
                          className="recent-act"
                          title={entry.pinned ? "Desafixar" : "Fixar"}
                          onClick={() => doTogglePin(entry)}
                        >
                          {entry.pinned ? "★" : "☆"}
                        </button>
                        <button
                          className="recent-act"
                          title="Remover da lista"
                          onClick={() => doRemoveRecent(entry)}
                        >
                          ✕
                        </button>
                      </li>
                    );
                  })}
                </ul>
              </section>
            );
          })}
          {recents.some((e) => !e.pinned) && (
            <button className="clear-recents" onClick={doClearRecents}>
              Limpar recentes
            </button>
          )}
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

      {ghOpen && (
        <div className="modal-overlay" onClick={() => !ghBusy && setGhOpen(false)}>
          <div className="modal" onClick={(e) => e.stopPropagation()}>
            <h3>Conectar ao GitHub (opcional)</h3>
            <p>
              Sem login, o GitHub limita as consultas anônimas a <strong>60/hora por IP</strong> —
              abrir o Hub várias vezes seguidas dá o erro 403. Conectado, o limite vira{" "}
              <strong>5.000/hora</strong>. A credencial fica no <strong>cofre do sistema</strong>{" "}
              (nunca em arquivo) e só é usada pra consultar e baixar releases.
            </p>

            {!ghLogged && !ghFlow && (
              <button className="primary gh-big" onClick={browserLogin} disabled={ghBusy}>
                {ghBusy ? "Abrindo o navegador…" : "🌐 Entrar com o navegador"}
              </button>
            )}

            {ghFlow && (
              <div className="gh-flow">
                <p>Digite este código na página do GitHub que abriu:</p>
                <div className="gh-code">{ghFlow.userCode}</div>
                <p className="gh-wait">
                  Aguardando você autorizar no navegador…{" "}
                  <a
                    href={ghFlow.verificationUri}
                    onClick={(e) => {
                      e.preventDefault();
                      openUrl(ghFlow.verificationUri).catch(() => {});
                    }}
                  >
                    reabrir a página
                  </a>
                </p>
              </div>
            )}

            {ghLogged && <p className="assoc-msg">✓ Conectado.</p>}

            {(ghManual || !ghConfigured) && !ghLogged && (
              <>
                <p className="gh-manual-hint">
                  {ghManual
                    ? "Cole o token gerado:"
                    : "Ou cole um token criado manualmente (sem nenhum escopo marcado):"}
                </p>
                <input
                  type="password"
                  value={ghToken}
                  onChange={(e) => setGhToken(e.target.value)}
                  placeholder="ghp_… ou github_pat_…"
                  spellCheck={false}
                />
              </>
            )}

            <div className="modal-actions">
              {!ghLogged && ghToken.trim() && (
                <button className="primary" onClick={saveGhToken}>
                  Salvar token
                </button>
              )}
              {ghLogged && (
                <button className="danger" onClick={clearGhToken}>
                  Desconectar
                </button>
              )}
              <button disabled={ghBusy} onClick={() => setGhOpen(false)}>
                Fechar
              </button>
            </div>
            {ghMsg && <p className="assoc-msg">{ghMsg}</p>}
          </div>
        </div>
      )}

      <footer className="hub-footer">
        Taylor — 100% offline depois de instalado; o Hub só acessa o GitHub pra baixar
        releases. Sem telemetria.
      </footer>
    </div>
  );
}
