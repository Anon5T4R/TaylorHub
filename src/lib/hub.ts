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

export async function getOs(): Promise<string> {
  return invoke<string>("get_os");
}

export async function detectApps(apps: CatalogApp[]): Promise<InstalledInfo[]> {
  return invoke<InstalledInfo[]>("detect_apps", {
    specs: apps.map((a) => ({ id: a.id, name: a.name, exe: a.exe })),
  });
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
): Promise<string[]> {
  return invoke<string[]>("recreate_shortcuts", { entries });
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
