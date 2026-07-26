import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import type { CatalogApp } from "../catalog";

export function inTauri(): boolean {
  return typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;
}

export interface InstalledInfo {
  id: string;
  installed: boolean;
  version: string;
  location: string;
  exe: string;
  /** "registry" (Windows) | "hub" | "appimage" (achado em ~/Applications) | "deb" (dpkg) */
  source: string;
}

export interface ReleaseInfo {
  tag: string;
  version: string;
  assets: { name: string; url: string; size: number }[];
}

export interface Progress {
  id: string;
  phase: "download" | "install";
  done: number;
  total: number;
}

export interface AssocEntry {
  ext: string;
  appId: string;
  appName: string;
  exe: string;
}

export interface CustomApp {
  id: string;
  name: string;
  repo: string;
  winAsset: string;
  linuxAsset: string;
  exe: string;
}

export async function getOs(): Promise<string> {
  return invoke<string>("get_os");
}

export async function detectApps(
  apps: { id: string; name: string; exe: string }[],
): Promise<InstalledInfo[]> {
  return invoke<InstalledInfo[]>("detect_apps", {
    specs: apps.map((a) => ({ id: a.id, name: a.name, exe: a.exe })),
  });
}

// ---------- repositórios do usuário ----------

export async function listCustomRepos(): Promise<CustomApp[]> {
  return invoke<CustomApp[]>("list_custom_repos");
}

export async function addCustomRepo(input: string): Promise<CustomApp> {
  return invoke<CustomApp>("add_custom_repo", { input });
}

export async function removeCustomRepo(id: string): Promise<void> {
  return invoke<void>("remove_custom_repo", { id });
}

export async function installCustomApp(id: string): Promise<InstalledInfo> {
  return invoke<InstalledInfo>("install_custom_app", { id });
}

// ---------- token do GitHub (opcional) ----------

export async function githubTokenStatus(): Promise<boolean> {
  return invoke<boolean>("github_token_status");
}

/** Salva (valida antes) ou remove (string vazia). Devolve o limite de req/h. */
export async function setGithubToken(token: string): Promise<number> {
  return invoke<number>("set_github_token", { token });
}

/** Build tem OAuth App configurado? (login pelo navegador via device flow) */
export async function githubClientConfigured(): Promise<boolean> {
  return invoke<boolean>("github_client_configured");
}

export interface DeviceStart {
  userCode: string;
  verificationUri: string;
  deviceCode: string;
  interval: number;
  expiresIn: number;
}

export async function githubDeviceStart(): Promise<DeviceStart> {
  return invoke<DeviceStart>("github_device_start");
}

/** Bloqueia até o usuário autorizar no navegador; devolve o limite de req/h. */
export async function githubDevicePoll(
  deviceCode: string,
  interval: number,
  expiresIn: number,
): Promise<number> {
  return invoke<number>("github_device_poll", { deviceCode, interval, expiresIn });
}

export async function getLatestRelease(repo: string, force = false): Promise<ReleaseInfo> {
  return invoke<ReleaseInfo>("get_latest_release", { repo, force });
}

/** Ícone do card como data URL, servido do cache local; `force` re-baixa do GitHub. */
export async function getIcon(app: CatalogApp, force = false): Promise<string> {
  return invoke<string>("get_icon", { id: app.id, url: app.iconUrl ?? "", force });
}

export async function installApp(
  app: CatalogApp,
  os: string,
  currentPath?: string,
): Promise<InstalledInfo> {
  return invoke<InstalledInfo>("install_app", {
    spec: {
      id: app.id,
      name: app.name,
      repo: app.repo,
      assetPattern: os === "windows" ? app.assets.win : app.assets.linux,
      silentArgs: app.silentArgs,
      exe: app.exe,
      currentPath: currentPath ?? null,
      iconUrl: app.iconUrl ?? null,
    },
  });
}

/**
 * O gerenciador de pacotes desta máquina ("pacman", "apt" ou "" quando não há
 * nenhum conhecido — aí o caminho é o AppImage).
 *
 * Serve só pra AVISAR o usuário: quem escolhe o asset é o Rust, no
 * `install_app`, que é onde se conhece a lista real de assets da release.
 * Decidir aqui obrigaria o front a adivinhar se a release tem pacote nativo.
 */
export async function linuxPkgManager(): Promise<string> {
  return invoke<string>("linux_pkg_manager");
}

export async function uninstallApp(app: CatalogApp, info: InstalledInfo): Promise<void> {
  return invoke("uninstall_app", {
    spec: { id: app.id, name: app.name, exe: info.exe, source: info.source },
  });
}

export const HUB_REPO = "Anon5T4R/TaylorHub";

/** Atualiza o próprio Hub. Retorna "closing" (Windows, o app fecha sozinho) ou "restart" (Linux). */
export async function updateSelf(os: string): Promise<string> {
  return invoke<string>("update_self", {
    spec: {
      repo: HUB_REPO,
      assetPattern: os === "windows" ? "*_x64-setup.exe" : "*_amd64.AppImage",
    },
  });
}

export async function recreateShortcuts(
  entries: { id: string; name: string; exe: string }[],
  remove: string[] = [],
): Promise<string[]> {
  return invoke<string[]>("recreate_shortcuts", { entries, remove });
}

export async function launchApp(exe: string, file?: string): Promise<void> {
  return invoke("launch_app", { exe, file: file ?? null });
}

export async function applyAssociations(entries: AssocEntry[]): Promise<string[]> {
  return invoke<string[]>("apply_associations", { entries });
}

export async function readDispatch(): Promise<AssocEntry[]> {
  return invoke<AssocEntry[]>("read_dispatch");
}

// ---------- limpeza profunda ----------

export interface CacheInfo {
  count: number;
  bytes: number;
}

export interface LeftoverDir {
  path: string;
  bytes: number;
}

export async function scanDownloadsCache(): Promise<CacheInfo> {
  return invoke<CacheInfo>("scan_downloads_cache");
}

/** Devolve os bytes liberados. */
export async function cleanDownloadsCache(): Promise<number> {
  return invoke<number>("clean_downloads_cache");
}

export async function scanOrphanRoutes(): Promise<AssocEntry[]> {
  return invoke<AssocEntry[]>("scan_orphan_routes");
}

/** Remove as rotas órfãs + a ProgID delas no registro; devolve o que foi removido. */
export async function cleanOrphanRoutes(): Promise<AssocEntry[]> {
  return invoke<AssocEntry[]>("clean_orphan_routes");
}

export async function scanLeftoverDirs(paths: string[]): Promise<LeftoverDir[]> {
  return invoke<LeftoverDir[]>("scan_leftover_dirs", { paths });
}

/** Devolve os bytes liberados. */
export async function deleteLeftoverDir(path: string): Promise<number> {
  return invoke<number>("delete_leftover_dir", { path });
}

export function onProgress(cb: (p: Progress) => void): Promise<UnlistenFn> {
  return listen<Progress>("hub-progress", (e) => cb(e.payload));
}

export interface RecentEntry {
  path: string;
  ts: number;
  pinned: boolean;
}

export async function readRecents(): Promise<RecentEntry[]> {
  return invoke<RecentEntry[]>("read_recents");
}

export async function setRecentPinned(path: string, pinned: boolean): Promise<RecentEntry[]> {
  return invoke<RecentEntry[]>("set_recent_pinned", { path, pinned });
}

export async function removeRecent(path: string): Promise<RecentEntry[]> {
  return invoke<RecentEntry[]>("remove_recent", { path });
}

export async function clearRecents(): Promise<RecentEntry[]> {
  return invoke<RecentEntry[]>("clear_recents");
}

export async function openRecent(path: string, exe: string): Promise<void> {
  return invoke("open_recent", { path, exe });
}

// ---------- que formato vai ser instalado ----------

export type InstallFormat = "pacman" | "deb" | "appimage" | "exe" | "desconhecido";

/**
 * Mesma semântica do `glob_match` do Rust (src-tauri/src/lib.rs): sem regex,
 * `*` em qualquer posição, comparação em minúsculas, e sem `*` no fim o nome
 * precisa terminar exatamente onde o padrão parou.
 */
export function globMatch(pattern: string, name: string): boolean {
  const p = pattern.toLowerCase();
  const n = name.toLowerCase();
  const partes = p.split("*");
  let pos = 0;
  for (let i = 0; i < partes.length; i++) {
    const parte = partes[i];
    if (!parte) continue;
    if (i === 0) {
      if (!n.startsWith(parte)) return false;
      pos = parte.length;
    } else {
      const achou = n.indexOf(parte, pos);
      if (achou < 0) return false;
      pos = achou + parte.length;
    }
  }
  if (!p.endsWith("*") && partes[partes.length - 1] !== "") return n.length === pos;
  return true;
}

/**
 * Qual formato o Hub VAI instalar desta release nesta máquina.
 *
 * ESPELHA `pkg::asset_globs_in_order` do Rust, e é por isso que existe: quem
 * decide de verdade é o back (é lá que se conhece a lista real de assets), mas
 * o usuário precisa saber o que vai acontecer ANTES de clicar em Instalar. Se
 * as duas divergirem, o Hub promete uma coisa e faz outra — então as duas leem
 * exatamente a mesma entrada: os assets da release + o gerenciador da máquina.
 *
 * A ordem "nativo primeiro, AppImage como queda" não é preferência estética: o
 * pacote nativo usa o webkit2gtk DO SISTEMA, enquanto o AppImage carrega o
 * dele, do Ubuntu do CI — que é a origem da classe de bug que fazia a janela
 * abrir branca fora do Ubuntu.
 */
export function installFormat(
  assets: { name: string }[],
  os: string,
  pkgMgr: string,
  linuxFallback = "*_amd64.AppImage",
): InstallFormat {
  const tem = (glob: string) => assets.some((a) => globMatch(glob, a.name));

  if (os === "windows") return tem("*-setup.exe") || tem("*.exe") ? "exe" : "desconhecido";

  if (pkgMgr === "pacman" && tem("*.pkg.tar.zst")) return "pacman";
  if (pkgMgr === "apt" && tem("*_amd64.deb")) return "deb";
  if (tem(linuxFallback) || tem("*.AppImage")) return "appimage";
  return "desconhecido";
}

/**
 * O app está instalado pelo gerenciador de pacotes do sistema (pacman/apt)?
 *
 * Existe porque este teste estava ESCRITO À MÃO em quatro lugares, e quando o
 * pacman entrou (v0.24) três deles ficaram só com `!== "deb"`. O sintoma foi o
 * Hub criar um atalho `.desktop` duplicado — com ícone genérico — para apps que
 * já trazem o seu, vindo do pacote. Um lugar só é o que impede a próxima origem
 * de instalação de repetir a história.
 */
export function isSystemPackage(source: string | undefined): boolean {
  return source === "deb" || source === "pacman";
}
