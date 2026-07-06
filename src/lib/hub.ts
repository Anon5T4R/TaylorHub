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

export async function getLatestRelease(repo: string): Promise<ReleaseInfo> {
  return invoke<ReleaseInfo>("get_latest_release", { repo });
}

export async function installApp(app: CatalogApp, os: string): Promise<InstalledInfo> {
  return invoke<InstalledInfo>("install_app", {
    spec: {
      id: app.id,
      name: app.name,
      repo: app.repo,
      assetPattern: os === "windows" ? app.assets.win : app.assets.linux,
      silentArgs: app.silentArgs,
      exe: app.exe,
    },
  });
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
