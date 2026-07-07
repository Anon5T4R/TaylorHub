// Catálogo da suíte Taylor. App novo = uma entrada aqui (e/ou no catalog.json remoto).
// A ordem importa: pra extensão disputada, o PRIMEIRO app da lista é a rota default.

export interface CatalogApp {
  id: string;
  name: string;
  description: string;
  kind: "app";
  repo: string; // owner/repo no GitHub
  assets: { win: string; linux: string }; // glob do asset por plataforma
  silentArgs: string[]; // args de instalação silenciosa (Windows)
  exe: string; // executável pós-instalação (Windows)
  extensions: string[]; // extensões que este app atende
  accent: string; // cor do card/avatar
  /** PNG do ícone (raw do GitHub) — usado no atalho .desktop do Linux. */
  iconUrl?: string;
}

export const CATALOG: CatalogApp[] = [
  {
    id: "writer",
    name: "LocalOffice",
    description: "Documentos (Word) — DOCX/ODT/MD, acadêmico ABNT/APA, IA local",
    kind: "app",
    repo: "Anon5T4R/LocalOffice",
    assets: { win: "*_x64-setup.exe", linux: "*_amd64.AppImage" },
    silentArgs: ["/S"],
    exe: "LocalOffice.exe",
    extensions: ["md", "markdown", "txt", "docx", "odt", "rtf", "html", "htm"],
    accent: "#2563eb",
    iconUrl: "https://raw.githubusercontent.com/Anon5T4R/LocalOffice/main/src-tauri/icons/128x128.png",
  },
  {
    id: "sheets",
    name: "LocalSheets",
    description: "Planilha (Excel) — XLSX/CSV, fórmulas, IA que edita células",
    kind: "app",
    repo: "Anon5T4R/LocalSheets",
    assets: { win: "*_x64-setup.exe", linux: "*_amd64.AppImage" },
    silentArgs: ["/S"],
    exe: "LocalSheets.exe",
    extensions: ["xlsx", "csv"],
    accent: "#16a34a",
    iconUrl: "https://raw.githubusercontent.com/Anon5T4R/LocalSheets/main/src-tauri/icons/128x128.png",
  },
  {
    id: "slides",
    name: "LocalSlides",
    description: "Apresentações (PowerPoint) — canvas, PPTX, IA gera o deck",
    kind: "app",
    repo: "Anon5T4R/LocalSlides",
    assets: { win: "*_x64-setup.exe", linux: "*_amd64.AppImage" },
    silentArgs: ["/S"],
    exe: "LocalSlides.exe",
    extensions: ["tslides", "pptx"],
    accent: "#0891b2",
    iconUrl: "https://raw.githubusercontent.com/Anon5T4R/LocalSlides/main/src-tauri/icons/128x128.png",
  },
  {
    id: "code",
    name: "LocalCode",
    description: "Editor de código — LSP, debugger, git, terminal, IA agente",
    kind: "app",
    repo: "Anon5T4R/LocalCode",
    assets: { win: "*_x64-setup.exe", linux: "*_amd64.AppImage" },
    silentArgs: ["/S"],
    exe: "LocalCode.exe",
    extensions: [
      "js", "ts", "tsx", "jsx", "rs", "json", "toml", "yaml", "yml", "css",
      "py", "go", "md", "html",
    ],
    accent: "#7c3aed",
    iconUrl: "https://raw.githubusercontent.com/Anon5T4R/LocalCode/master/src-tauri/icons/icon_128x128.png",
  },
  {
    id: "taylormind",
    name: "TaylorMind",
    description: "Mapas mentais (XMind) — IA gera/expande o mapa",
    kind: "app",
    repo: "Anon5T4R/TaylorMind",
    assets: { win: "TaylorMind.Setup.*.exe", linux: "TaylorMind*.AppImage" },
    silentArgs: ["/S"],
    exe: "TaylorMind.exe",
    extensions: ["tmind"],
    accent: "#ea580c",
    iconUrl: "https://raw.githubusercontent.com/Anon5T4R/TaylorMind/main/build/icon.png",
  },
  {
    id: "taylorai",
    name: "TaylorAI Studio",
    description: "Roda modelos GGUF locais (LM Studio) — CPU e iGPU no talo",
    kind: "app",
    repo: "Anon5T4R/taylorai-studio",
    assets: { win: "*_x64-setup.exe", linux: "*_amd64.AppImage" },
    silentArgs: ["/S"],
    exe: "TaylorAI Studio.exe",
    extensions: [],
    accent: "#e11d48",
    iconUrl: "https://raw.githubusercontent.com/Anon5T4R/taylorai-studio/main/src-tauri/icons/128x128.png",
  },
  {
    id: "openobsidian",
    name: "OpenObsidian",
    description: "Notas e base de conhecimento (Obsidian) — grafo, mermaid, IA",
    kind: "app",
    repo: "Anon5T4R/OpenObsidian",
    assets: { win: "OpenObsidian.Setup.*.exe", linux: "OpenObsidian*.AppImage" },
    silentArgs: ["/S"],
    exe: "OpenObsidian.exe",
    extensions: [],
    accent: "#9333ea",
    iconUrl: "https://raw.githubusercontent.com/Anon5T4R/OpenObsidian/master/resources/icon.png",
  },
];

/** Todas as extensões do catálogo, com os apps candidatos (ordem do catálogo). */
export function extensionRoutes(): Map<string, CatalogApp[]> {
  const map = new Map<string, CatalogApp[]>();
  for (const app of CATALOG) {
    for (const ext of app.extensions) {
      const list = map.get(ext) ?? [];
      list.push(app);
      map.set(ext, list);
    }
  }
  return map;
}

export function compareVersions(a: string, b: string): number {
  const pa = a.split(".").map((n) => parseInt(n, 10) || 0);
  const pb = b.split(".").map((n) => parseInt(n, 10) || 0);
  for (let i = 0; i < Math.max(pa.length, pb.length); i++) {
    const d = (pa[i] ?? 0) - (pb[i] ?? 0);
    if (d !== 0) return d;
  }
  return 0;
}
