// Catálogo da suíte Local. App novo = uma entrada aqui (e/ou no catalog.json remoto).
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
  /** Seção da grade de apps (Escritório, Desenvolvimento…). */
  category: string;
  accent: string; // cor do card/avatar
  /** PNG do ícone (raw do GitHub) — card (via cache local em disco) e atalho .desktop do Linux. */
  iconUrl?: string;
}

export const CATALOG: CatalogApp[] = [
  {
    id: "writer",
    category: "Escritório",
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
    category: "Escritório",
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
    category: "Escritório",
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
    category: "Desenvolvimento",
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
    id: "data",
    category: "Dados e conhecimento",
    name: "LocalData",
    description: "Banco de dados visual (Airtable) — tabelas tipadas, kanban, IA",
    kind: "app",
    repo: "Anon5T4R/LocalData",
    assets: { win: "*_x64-setup.exe", linux: "*_amd64.AppImage" },
    silentArgs: ["/S"],
    exe: "LocalData.exe",
    // db/sqlite: o LocalData abre qualquer SQLite (só adiciona os metadados dele)
    extensions: ["tbase", "db", "sqlite"],
    accent: "#a855f7",
    iconUrl: "https://raw.githubusercontent.com/Anon5T4R/LocalData/main/src-tauri/icons/128x128.png",
  },
  {
    id: "pdf",
    category: "Escritório",
    name: "LocalPDF",
    description: "Editor de PDF — organizar páginas, anotar, assinar, formulários, IA",
    kind: "app",
    repo: "Anon5T4R/LocalPDF",
    assets: { win: "*_x64-setup.exe", linux: "*_amd64.AppImage" },
    silentArgs: ["/S"],
    exe: "LocalPDF.exe",
    extensions: ["pdf"],
    accent: "#dc2626",
    iconUrl: "https://raw.githubusercontent.com/Anon5T4R/LocalPDF/main/src-tauri/icons/128x128.png",
  },
  {
    id: "agenda",
    category: "Escritório",
    name: "LocalAgenda",
    description: "Calendário, tarefas e lembretes (Outlook sem e-mail) — recorrência, .ics, IA local",
    kind: "app",
    repo: "Anon5T4R/LocalAgenda",
    assets: { win: "*_x64-setup.exe", linux: "*_amd64.AppImage" },
    silentArgs: ["/S"],
    exe: "LocalAgenda.exe",
    extensions: ["ics"],
    accent: "#2563eb",
    iconUrl: "https://raw.githubusercontent.com/Anon5T4R/LocalAgenda/main/src-tauri/icons/128x128.png",
  },
  {
    id: "draw",
    category: "Escritório",
    name: "LocalDraw",
    description: "Diagramas e fluxogramas (Visio/draw.io) — canvas Excalidraw, conectores, IA gera o fluxograma",
    kind: "app",
    repo: "Anon5T4R/LocalDraw",
    assets: { win: "*_x64-setup.exe", linux: "*_amd64.AppImage" },
    silentArgs: ["/S"],
    exe: "LocalDraw.exe",
    extensions: ["tdraw"],
    accent: "#0284c7",
    iconUrl: "https://raw.githubusercontent.com/Anon5T4R/LocalDraw/main/src-tauri/icons/128x128.png",
  },
  {
    id: "zim",
    category: "Dados e conhecimento",
    name: "LocalZIM",
    description: "Wikipédia e bibliotecas offline (Kiwix) — lê arquivos .zim",
    kind: "app",
    repo: "Anon5T4R/LocalZIM",
    assets: { win: "*_x64-setup.exe", linux: "*_amd64.AppImage" },
    silentArgs: ["/S"],
    exe: "LocalZIM.exe",
    extensions: ["zim"],
    accent: "#8b5cf6",
    iconUrl: "https://raw.githubusercontent.com/Anon5T4R/LocalZIM/main/src-tauri/icons/128x128.png",
  },
  {
    id: "scribe",
    category: "Inteligência artificial",
    name: "LocalScribe",
    description: "Transcrição de áudio offline (whisper) — timestamps, SRT/VTT, resumo e ata por IA",
    kind: "app",
    repo: "Anon5T4R/LocalScribe",
    assets: { win: "*_x64-setup.exe", linux: "*_amd64.AppImage" },
    silentArgs: ["/S"],
    exe: "LocalScribe.exe",
    extensions: [],
    accent: "#7c3aed",
    iconUrl: "https://raw.githubusercontent.com/Anon5T4R/LocalScribe/main/src-tauri/icons/128x128.png",
  },
  {
    id: "translate",
    category: "Inteligência artificial",
    name: "LocalTranslate",
    description: "Tradutor offline (pt · es · en) — Marian/OPUS-MT no candle, detecção de idioma, histórico",
    kind: "app",
    repo: "Anon5T4R/LocalTranslate",
    assets: { win: "*_x64-setup.exe", linux: "*_amd64.AppImage" },
    silentArgs: ["/S"],
    exe: "LocalTranslate.exe",
    extensions: [],
    accent: "#4f46e5",
    iconUrl: "https://raw.githubusercontent.com/Anon5T4R/LocalTranslate/main/src-tauri/icons/128x128.png",
  },
  {
    id: "media",
    category: "Mídia",
    name: "LocalMedia",
    description: "Converter, comprimir e cortar vídeo/áudio (ffmpeg) — presets, GIF, faixas, lote",
    kind: "app",
    repo: "Anon5T4R/LocalMedia",
    assets: { win: "*_x64-setup.exe", linux: "*_amd64.AppImage" },
    silentArgs: ["/S"],
    exe: "LocalMedia.exe",
    extensions: [],
    accent: "#ea580c",
    iconUrl: "https://raw.githubusercontent.com/Anon5T4R/LocalMedia/main/src-tauri/icons/128x128.png",
  },
  {
    id: "image",
    category: "Mídia",
    // Decisão do plano: NÃO associar extensões de imagem por padrão (não
    // roubar o visualizador do SO). Quem quiser associa por fora.
    name: "LocalImage",
    description: "Visualizador, anotador e captura de tela — EXIF, lote, atalho global",
    kind: "app",
    repo: "Anon5T4R/LocalImage",
    assets: { win: "*_x64-setup.exe", linux: "*_amd64.AppImage" },
    silentArgs: ["/S"],
    exe: "LocalImage.exe",
    extensions: [],
    accent: "#0d9488",
    iconUrl: "https://raw.githubusercontent.com/Anon5T4R/LocalImage/main/src-tauri/icons/128x128.png",
  },
  {
    id: "player",
    category: "Mídia",
    name: "LocalPlayer",
    description: "Player de vídeo/áudio minimalista (mpv) — legendas, faixas, capítulos, velocidade, playlist",
    kind: "app",
    repo: "Anon5T4R/LocalPlayer",
    assets: { win: "*_x64-setup.exe", linux: "*_amd64.AppImage" },
    silentArgs: ["/S"],
    exe: "LocalPlayer.exe",
    extensions: ["mp4", "mkv", "webm", "avi", "mov", "mp3", "flac", "m4a", "opus"],
    accent: "#e11d48",
    iconUrl: "https://raw.githubusercontent.com/Anon5T4R/LocalPlayer/main/src-tauri/icons/128x128.png",
  },
  {
    id: "taylorchat",
    category: "Comunicação",
    name: "TaylorChat",
    description: "Mensageiro P2P offline — sem servidor, cifrado ponta a ponta, IA local",
    kind: "app",
    repo: "Anon5T4R/TaylorChat",
    assets: { win: "*_x64-setup.exe", linux: "*_amd64.AppImage" },
    silentArgs: ["/S"],
    exe: "TaylorChat.exe",
    extensions: [],
    accent: "#14b8a6",
    iconUrl: "https://raw.githubusercontent.com/Anon5T4R/TaylorChat/main/src-tauri/icons/128x128.png",
  },
  {
    // id mantém "taylormind" por compatibilidade (chave de installed.json/dispatch).
    id: "taylormind",
    category: "Dados e conhecimento",
    name: "LocalMind",
    description: "Mapas mentais (XMind) — IA gera/expande o mapa",
    kind: "app",
    repo: "Anon5T4R/LocalMind",
    assets: { win: "LocalMind.Setup.*.exe", linux: "LocalMind*.AppImage" },
    silentArgs: ["/S"],
    exe: "LocalMind.exe",
    extensions: ["tmind"],
    accent: "#ea580c",
    iconUrl: "https://raw.githubusercontent.com/Anon5T4R/LocalMind/main/build/icon.png",
  },
  {
    id: "localai",
    category: "Inteligência artificial",
    name: "LocalAI Studio",
    description: "Roda modelos GGUF locais (LM Studio) — CPU e iGPU no talo",
    kind: "app",
    repo: "Anon5T4R/LocalAI",
    assets: { win: "*_x64-setup.exe", linux: "*_amd64.AppImage" },
    silentArgs: ["/S"],
    exe: "LocalAI Studio.exe",
    extensions: [],
    accent: "#e11d48",
    iconUrl: "https://raw.githubusercontent.com/Anon5T4R/LocalAI/main/src-tauri/icons/128x128.png",
  },
  {
    id: "openobsidian",
    category: "Dados e conhecimento",
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
  {
    id: "keys",
    category: "Segurança",
    name: "LocalKeys",
    description: "Gerenciador de senhas (Bitwarden) — vault .tkeys cifrado, gerador, sem nuvem",
    kind: "app",
    repo: "Anon5T4R/LocalKeys",
    assets: { win: "*_x64-setup.exe", linux: "*_amd64.AppImage" },
    silentArgs: ["/S"],
    exe: "LocalKeys.exe",
    extensions: ["tkeys"],
    accent: "#4338ca",
    iconUrl: "https://raw.githubusercontent.com/Anon5T4R/LocalKeys/main/src-tauri/icons/128x128.png",
  },
  {
    id: "files",
    category: "Sistema",
    name: "LocalFiles",
    description: "Gerenciador de arquivos — abas, 3 visões, copiar/mover com progresso, lixeira",
    kind: "app",
    repo: "Anon5T4R/LocalFiles",
    assets: { win: "*_x64-setup.exe", linux: "*_amd64.AppImage" },
    silentArgs: ["/S"],
    exe: "LocalFiles.exe",
    extensions: [],
    accent: "#d97706",
    iconUrl: "https://raw.githubusercontent.com/Anon5T4R/LocalFiles/main/src-tauri/icons/128x128.png",
  },
  {
    id: "zip",
    category: "Sistema",
    name: "LocalZip",
    description: "Compactador — abre/extrai zip, tar e tar.gz sem sair do lugar; cria zip/tar.gz",
    kind: "app",
    repo: "Anon5T4R/LocalZip",
    assets: { win: "*_x64-setup.exe", linux: "*_amd64.AppImage" },
    silentArgs: ["/S"],
    exe: "LocalZip.exe",
    extensions: ["zip", "tar", "gz", "tgz"],
    accent: "#b45309",
    iconUrl: "https://raw.githubusercontent.com/Anon5T4R/LocalZip/main/src-tauri/icons/128x128.png",
  },
  {
    id: "converter",
    category: "Sistema",
    name: "LocalConverter",
    description: "Conversor universal — vídeo, áudio, imagem e documentos, em lote (ffmpeg + pandoc)",
    kind: "app",
    repo: "Anon5T4R/LocalConverter",
    assets: { win: "*_x64-setup.exe", linux: "*_amd64.AppImage" },
    silentArgs: ["/S"],
    exe: "LocalConverter.exe",
    extensions: [],
    accent: "#059669",
    iconUrl: "https://raw.githubusercontent.com/Anon5T4R/LocalConverter/main/src-tauri/icons/128x128.png",
  },
  {
    id: "terminal",
    category: "Sistema",
    name: "LocalTerminal",
    description: "Terminal — abas, perfis (PowerShell/cmd/Git Bash/WSL), busca, PTY nativo",
    kind: "app",
    repo: "Anon5T4R/LocalTerminal",
    assets: { win: "*_x64-setup.exe", linux: "*_amd64.AppImage" },
    silentArgs: ["/S"],
    exe: "LocalTerminal.exe",
    extensions: [],
    accent: "#0f766e",
    iconUrl: "https://raw.githubusercontent.com/Anon5T4R/LocalTerminal/main/src-tauri/icons/128x128.png",
  },
  {
    id: "calc",
    category: "Sistema",
    name: "LocalCalc",
    description: "Calculadora — padrão, científica, programador (HEX/BIN) e conversor de unidades",
    kind: "app",
    repo: "Anon5T4R/LocalCalc",
    assets: { win: "*_x64-setup.exe", linux: "*_amd64.AppImage" },
    silentArgs: ["/S"],
    exe: "LocalCalc.exe",
    extensions: [],
    accent: "#7c3aed",
    iconUrl: "https://raw.githubusercontent.com/Anon5T4R/LocalCalc/main/src-tauri/icons/128x128.png",
  },
  {
    id: "feed",
    category: "Dados e conhecimento",
    name: "LocalFeed",
    description: "Leitor RSS/Atom — sem algoritmo nem conta; modo leitura offline, OPML",
    kind: "app",
    repo: "Anon5T4R/LocalFeed",
    assets: { win: "*_x64-setup.exe", linux: "*_amd64.AppImage" },
    silentArgs: ["/S"],
    exe: "LocalFeed.exe",
    extensions: ["opml"],
    accent: "#ea580c",
    iconUrl: "https://raw.githubusercontent.com/Anon5T4R/LocalFeed/main/src-tauri/icons/128x128.png",
  },
  {
    id: "automation",
    category: "Automação",
    name: "LocalAutomation",
    description: "Automação de fluxos (n8n/Zapier local) — nós de HTTP/comando/arquivos/JS, .tflow",
    kind: "app",
    repo: "Anon5T4R/LocalAutomation",
    assets: { win: "*_x64-setup.exe", linux: "*_amd64.AppImage" },
    silentArgs: ["/S"],
    exe: "LocalAutomation.exe",
    extensions: ["tflow"],
    accent: "#0891b2",
    iconUrl: "https://raw.githubusercontent.com/Anon5T4R/LocalAutomation/main/src-tauri/icons/128x128.png",
  },
  {
    id: "clip",
    category: "Sistema",
    name: "LocalClip",
    description: "Histórico de área de transferência — busca, fixados, popup por atalho global",
    kind: "app",
    repo: "Anon5T4R/LocalClip",
    assets: { win: "*_x64-setup.exe", linux: "*_amd64.AppImage" },
    silentArgs: ["/S"],
    exe: "LocalClip.exe",
    extensions: [],
    accent: "#059669",
    iconUrl: "https://raw.githubusercontent.com/Anon5T4R/LocalClip/main/src-tauri/icons/128x128.png",
  },
  {
    id: "monitor",
    category: "Sistema",
    name: "LocalMonitor",
    description: "Monitor de sistema — CPU/memória/rede/discos ao vivo, processos com encerrar",
    kind: "app",
    repo: "Anon5T4R/LocalMonitor",
    assets: { win: "*_x64-setup.exe", linux: "*_amd64.AppImage" },
    silentArgs: ["/S"],
    exe: "LocalMonitor.exe",
    extensions: [],
    accent: "#dc2626",
    iconUrl: "https://raw.githubusercontent.com/Anon5T4R/LocalMonitor/main/src-tauri/icons/128x128.png",
  },
  {
    id: "record",
    category: "Mídia",
    name: "LocalRecord",
    description: "Estúdio de captura de tela — tela + câmera + áudio, com anotação ao vivo",
    kind: "app",
    repo: "Anon5T4R/LocalRecord",
    assets: { win: "*_x64-setup.exe", linux: "*_amd64.AppImage" },
    silentArgs: ["/S"],
    exe: "LocalRecord.exe",
    // .trec (cenas) so existe a partir da v0.3 -- nao anunciar extensao que o app ainda nao abre.
    extensions: [],
    accent: "#8b5cf6",
    iconUrl: "https://raw.githubusercontent.com/Anon5T4R/LocalRecord/main/src-tauri/icons/128x128.png",
  },
  {
    id: "video",
    category: "Mídia",
    name: "LocalVideo",
    description: "Editor de vídeo — importe, corte, reordene e exporte sem recodificar",
    kind: "app",
    repo: "Anon5T4R/LocalVideo",
    assets: { win: "*_x64-setup.exe", linux: "*_amd64.AppImage" },
    silentArgs: ["/S"],
    exe: "LocalVideo.exe",
    // .tvproj e projeto NOSSO, nao formato do mundo -- associacao so faz sentido
    // depois que o app estiver maduro. Fica de fora da v0.1.
    extensions: [],
    accent: "#0ea5e9",
    iconUrl: "https://raw.githubusercontent.com/Anon5T4R/LocalVideo/main/src-tauri/icons/128x128.png",
  },
  {
    id: "paint",
    category: "Mídia",
    name: "LocalPaint",
    description: "Editor de imagem raster — camadas, pincel com pressão, filtros e seleção",
    kind: "app",
    repo: "Anon5T4R/LocalPaint",
    assets: { win: "*_x64-setup.exe", linux: "*_amd64.AppImage" },
    silentArgs: ["/S"],
    exe: "LocalPaint.exe",
    // .tpaint e formato NOSSO; o instalador ja registra a associacao — o Hub
    // nao precisa rotear (mesma decisao do LocalVideo com o .tvproj).
    extensions: [],
    accent: "#d97706",
    iconUrl: "https://raw.githubusercontent.com/Anon5T4R/LocalPaint/main/src-tauri/icons/128x128.png",
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

/**
 * Quebra uma versão em números + sufixo de pré-lançamento.
 *
 * O `parseInt(n, 10) || 0` que estava aqui engolia CALADO duas coisas, e as
 * duas só aparecem em repositório adicionado pelo usuário (os apps do catálogo
 * embutido sempre publicam `vX.Y.Z`, então nada disso mordia):
 *
 * - **Prefixo não-numérico.** Tag `release-1.2.0` virava `[0, 2, 0]`, ou seja
 *   lia como 0.2.0 — a versão publicada parecia MAIS VELHA que a instalada e o
 *   update nunca era oferecido. Prefixo é rótulo, não versão: sai fora.
 * - **Sufixo de pré-lançamento.** `1.0.0-rc1` virava exatamente `[1, 0, 0]` e
 *   comparava IGUAL a `1.0.0`, então quem estava num rc nunca recebia a
 *   estável. Regra do semver: `1.0.0-rc1` vem ANTES de `1.0.0`.
 *
 * Metadado de build (`+algo`) é descartado sem virar pré-lançamento — pelo
 * semver ele não conta pra precedência, e tratá-lo como sufixo faria o app
 * oferecer "update" pra mesma versão, pra sempre.
 */
function parseVersion(v: string): { nums: number[]; pre: string } {
  const semRotulo = v.trim().replace(/^[^0-9]*/, "");
  const semBuild = semRotulo.split("+")[0];
  const [corpo, ...resto] = semBuild.split("-");
  const nums = corpo.split(".").map((n) => {
    const m = /^\d+/.exec(n);
    return m ? parseInt(m[0], 10) : 0;
  });
  return { nums, pre: resto.join("-") };
}

export function compareVersions(a: string, b: string): number {
  const pa = parseVersion(a);
  const pb = parseVersion(b);
  for (let i = 0; i < Math.max(pa.nums.length, pb.nums.length); i++) {
    const d = (pa.nums[i] ?? 0) - (pb.nums[i] ?? 0);
    if (d !== 0) return d;
  }
  if (pa.pre === pb.pre) return 0;
  // Ausência de sufixo GANHA: a estável é mais nova que qualquer rc dela.
  if (!pa.pre) return 1;
  if (!pb.pre) return -1;
  return pa.pre < pb.pre ? -1 : 1;
}
