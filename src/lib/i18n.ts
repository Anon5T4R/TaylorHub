import { useSyncExternalStore } from "react";

/**
 * i18n leve da UI (mesmo padrão do LocalCode/LocalTranslate). `pt` é a fonte da
 * verdade das chaves; `en`/`es` como `Record<MessageKey, string>` fazem o
 * compilador recusar chave faltando ou sobrando. Locale num store externo pra
 * `t()` rodar fora de componente. O App remonta na troca (key={locale}).
 *
 * NÃO traduzir: a marca (TaylorHub/Taylor é intencional), os NOMES dos apps do
 * catálogo (LocalOffice, LocalCode…) e os dados do catálogo (repo/assets/exe).
 * As categorias e as DESCRIÇÕES dos cards são exibidas → localizadas aqui por
 * chave (`cat.*` e `desc.<id>`); o `catalog.ts` segue com o texto pt canônico
 * como fallback (apps de fora do catálogo usam o dele).
 */

export type Locale = "pt" | "en" | "es";

export const LOCALE_LABELS: Record<Locale, string> = {
  pt: "Português",
  en: "English",
  es: "Español",
};

const LOCALE_KEY = "taylorhub.locale";

const pt = {
  // Tempo relativo
  "time.now": "agora",
  "time.minAgo": "{n} min atrás",
  "time.hourAgo": "{n} h atrás",
  "time.yesterday": "ontem",

  // Cabeçalho / abas
  "hdr.sub": "suíte Local — instalar, atualizar, abrir",
  "tab.apps": "Apps",
  "tab.recent": "Recentes",
  "tab.files": "Associações",
  "gh.loggedTitle": "Logado no GitHub — 5.000 requisições/hora",
  "gh.loginTitle": "Login opcional no GitHub (evita o erro 403 de rate limit)",
  "gh.logged": "GitHub ✓",
  "gh.login": "GitHub",
  "refresh.title": "Consulta o GitHub agora, ignorando os caches (releases e ícones)",
  "refresh.checking": "Verificando…",
  "refresh.check": "Verificar atualizações",
  "refresh.when": "Verificado {clock}",

  // Banners
  "banner.browser": "Rodando no navegador (preview) — instalar/detectar só funciona no app Tauri.",
  "banner.updatePre": "Nova versão do Hub disponível: v{latest} (você está na v{current}).",
  "banner.updateBtn": "Atualizar o Hub",

  // Grade de apps
  "linux.recreate": "Recriar atalhos do menu",
  "repo.placeholder": "github.com/dono/repo — adicionar app de fora do catálogo",
  "repo.adding": "Adicionando…",
  "repo.add": "Adicionar repositório",
  "card.debBadge": "via .deb",
  "card.updateBadge": "v{v} disponível",
  "card.notInstalled": "não instalado",
  "card.lastVersion": " — última: v{v}",
  "card.installing": "Instalando…",
  "card.preparing": "Preparando…",
  "act.install": "Instalar",
  "act.update": "Atualizar",
  "act.open": "Abrir",
  "act.uninstall": "Desinstalar",
  "act.removeFromList": "Remover da lista",
  "act.removeFromListTitle": "Tirar da lista do Hub (não desinstala)",
  "card.debHint": "Instalado pelo gerenciador de pacotes — atualize/remova com o apt.",

  // Atualizar tudo (fila em série)
  "queue.updateAll": "Atualizar tudo ({n})",
  "confirm.updateAll":
    "Atualizar {n} apps de uma vez?\n\nApps abertos podem falhar ao atualizar — feche-os antes de continuar.",
  "queue.running": "Atualizando {done}/{total}: {name}…",
  "queue.doneOk": "Tudo atualizado — {n} apps.",
  "queue.doneWithFail": "{ok} atualizados, {fail} falharam: {names} (veja o erro no card).",

  // Recentes
  "rec.empty":
    "Nada por aqui ainda. Os arquivos que você abrir com duplo-clique (via associações do Hub) aparecem nesta lista — e dá pra fixar os favoritos com a ☆.",
  "rec.pinned": "★ Fixados",
  "rec.recent": "Recentes",
  "rec.noApp": "sem app associado",
  "rec.unpin": "Desafixar",
  "rec.pin": "Fixar",
  "rec.remove": "Remover da lista",
  "rec.clear": "Limpar recentes",

  // Associações
  "assoc.hint":
    'Escolha qual app abre cada tipo de arquivo. O Hub registra a associação e despacha o arquivo pro app certo ao clicar. Extensões que já têm dono no sistema (ex.: .md, .pdf) podem exigir confirmar no diálogo "Como você quer abrir?" do Windows uma vez.',
  "assoc.ext": "Extensão",
  "assoc.openWith": "Abrir com",
  "assoc.notInstalledOpt": " (não instalado)",
  "assoc.apply": "Aplicar associações",

  // Limpeza profunda
  "clean.title": "Limpeza profunda",
  "clean.hint":
    "Pra quem instalou e desinstalou apps pelo Hub antes destas correções existirem: acha e remove o que ficou pra trás — rotas de arquivo apontando pra um app que não existe mais, o cache de instaladores baixados e pastas de dados de apps já desinstalados.",
  "clean.scan": "Verificar",
  "clean.scanning": "Verificando…",
  "clean.nothing": "Nada pra limpar — tudo certo.",
  "clean.orphanTitle": "Rotas de arquivo órfãs ({n})",
  "clean.orphanHint":
    "Extensões que ainda apontam pro Hub como padrão do Windows, mas o app dono foi desinstalado.",
  "clean.orphanClean": "Remover rotas órfãs",
  "clean.downloadsTitle": "Cache de instaladores baixados",
  "clean.downloadsInfo": "{n} arquivo(s), {size}",
  "clean.downloadsClean": "Limpar cache de downloads",
  "clean.dirsTitle": "Pastas de dados esquecidas",
  "clean.dirsHint": "Pastas em AppData de apps que não estão mais instalados.",
  "clean.dirRemove": "Remover",
  "clean.doneOrphan": "{n} rota(s) removida(s).",
  "clean.doneDownloads": "{size} liberados.",
  "clean.doneDir": "{size} liberados.",

  // Modal do GitHub
  "ghm.title": "Conectar ao GitHub (opcional)",
  "ghm.p1": "Sem login, o GitHub limita as consultas anônimas a",
  "ghm.p1Strong": "60/hora por IP",
  "ghm.p2": "— abrir o Hub várias vezes seguidas dá o erro 403. Conectado, o limite vira",
  "ghm.p2Strong": "5.000/hora",
  "ghm.p3": ". A credencial fica no",
  "ghm.p3Strong": "cofre do sistema",
  "ghm.p4": "(nunca em arquivo) e só é usada pra consultar e baixar releases.",
  "ghm.login": "🌐 Entrar com o navegador",
  "ghm.opening": "Abrindo o navegador…",
  "ghm.flowCode": "Digite este código na página do GitHub que abriu:",
  "ghm.flowWait": "Aguardando você autorizar no navegador…",
  "ghm.flowReopen": "reabrir a página",
  "ghm.connected": "✓ Conectado.",
  "ghm.pasteGenerated": "Cole o token gerado:",
  "ghm.pasteManual": "Ou cole um token criado manualmente (sem nenhum escopo marcado):",
  "ghm.tokenPlaceholder": "ghp_… ou github_pat_…",
  "ghm.saveToken": "Salvar token",
  "ghm.disconnect": "Desconectar",
  "common.close": "Fechar",

  // Rodapé
  "foot.text":
    "Taylor — 100% offline depois de instalado; o Hub só acessa o GitHub pra baixar releases. Sem telemetria.",

  // Mensagens dinâmicas / confirmações
  "msg.releasesFail":
    "Não consegui consultar as releases no GitHub — sem isso não aparecem atualizações. ({reason})",
  "msg.queryingRelease": "Consultando a release…",
  "msg.added": "{name} adicionado.",
  "confirm.removeCustom": "Remover {name} da lista do Hub?\n(o app instalado NÃO é desinstalado)",
  "msg.generateHint": 'Clique em "Generate token" na página que abriu, copie e cole aqui embaixo.',
  "msg.connectedRate": "Conectado — {n} requisições/hora.",
  "msg.validating": "Validando…",
  "msg.tokenSaved": "Token salvo no cofre do sistema — {n} requisições/hora.",
  "msg.tokenRemoved": "Token removido.",
  "confirm.uninstall": "Desinstalar {name}?",
  "confirm.updateSelf": "Atualizar o TaylorHub de v{current} para v{latest}?\n\n{how}",
  "confirm.updateWin": "O Hub vai baixar a nova versão, fechar e se reinstalar sozinho.",
  "confirm.updateLinux": "O Hub vai substituir o próprio AppImage; depois é só reabrir.",
  "msg.downloadingUpdate": "Baixando atualização…",
  "msg.updatedRestart": "Atualizado! Feche e reabra o Hub.",
  "msg.updatedInstalling": "Instalando — o Hub vai fechar e reabrir atualizado…",
  "msg.error": "Erro: {e}",
  "msg.noAppForFile": "Nenhum app instalado atende .{ext} — veja a aba Associações.",
  "confirm.clearRecents": "Limpar a lista de recentes? (os fixados ficam)",
  "msg.noShortcut": "Nenhum app instalado (AppImage) pra criar atalho.",
  "msg.shortcutWarn": "Atalhos com avisos: {w}",
  "msg.shortcutOk": "Atalhos de menu recriados ({n} apps).",
  "msg.applying": "Aplicando…",
  "msg.noAppToAssoc": "Nenhum app instalado pra associar — instale primeiro na aba Apps.",
  "msg.assocWarn": "Aplicado com avisos: {w}",
  "msg.assocOk": "Associações aplicadas ({n} extensões).",

  // Categorias
  "cat.Escritório": "Escritório",
  "cat.Dados e conhecimento": "Dados e conhecimento",
  "cat.Desenvolvimento": "Desenvolvimento",
  "cat.Comunicação": "Comunicação",
  "cat.Inteligência artificial": "Inteligência artificial",
  "cat.Mídia": "Mídia",
  "cat.Segurança": "Segurança",
  "cat.Sistema": "Sistema",
  "cat.Automação": "Automação",
  "cat.Meus repositórios": "Meus repositórios",
  "cat.Outros": "Outros",
  "custom.repoDesc": "Repositório do usuário — {repo}",

  // Descrições dos apps (card)
  "desc.writer": "Documentos (Word) — DOCX/ODT/MD, acadêmico ABNT/APA, IA local",
  "desc.sheets": "Planilha (Excel) — XLSX/CSV, fórmulas, IA que edita células",
  "desc.slides": "Apresentações (PowerPoint) — canvas, PPTX, IA gera o deck",
  "desc.code": "Editor de código — LSP, debugger, git, terminal, IA agente",
  "desc.data": "Banco de dados visual (Airtable) — tabelas tipadas, kanban, IA",
  "desc.pdf": "Editor de PDF — organizar páginas, anotar, assinar, formulários, IA",
  "desc.agenda": "Calendário, tarefas e lembretes (Outlook sem e-mail) — recorrência, .ics, IA local",
  "desc.draw": "Diagramas e fluxogramas (Visio/draw.io) — canvas Excalidraw, conectores, IA gera o fluxograma",
  "desc.zim": "Wikipédia e bibliotecas offline (Kiwix) — lê arquivos .zim",
  "desc.scribe": "Transcrição de áudio offline (whisper) — timestamps, SRT/VTT, resumo e ata por IA",
  "desc.translate": "Tradutor offline (pt · es · en) — Marian/OPUS-MT no candle, detecção de idioma, histórico",
  "desc.media": "Converter, comprimir e cortar vídeo/áudio (ffmpeg) — presets, GIF, faixas, lote",
  "desc.image": "Visualizador, anotador e captura de tela — EXIF, lote, atalho global",
  "desc.player": "Player de vídeo/áudio minimalista (mpv) — legendas, faixas, capítulos, velocidade, playlist",
  "desc.taylorchat": "Mensageiro P2P offline — sem servidor, cifrado ponta a ponta, IA local",
  "desc.taylormind": "Mapas mentais (XMind) — IA gera/expande o mapa",
  "desc.localai": "Roda modelos GGUF locais (LM Studio) — CPU e iGPU no talo",
  "desc.openobsidian": "Notas e base de conhecimento (Obsidian) — grafo, mermaid, IA",
  "desc.keys": "Gerenciador de senhas (Bitwarden) — vault .tkeys cifrado, gerador, sem nuvem",
  "desc.files": "Gerenciador de arquivos — abas, 3 visões, copiar/mover com progresso, lixeira",
  "desc.zip": "Compactador — abre/extrai zip, tar e tar.gz sem sair do lugar; cria zip/tar.gz",
  "desc.converter": "Conversor universal — vídeo, áudio, imagem e documentos, em lote (ffmpeg + pandoc)",
  "desc.terminal": "Terminal — abas, perfis (PowerShell/cmd/Git Bash/WSL), busca, PTY nativo",
  "desc.calc": "Calculadora — padrão, científica, programador (HEX/BIN) e conversor de unidades",
  "desc.feed": "Leitor RSS/Atom — sem algoritmo nem conta; modo leitura offline, OPML",
  "desc.automation": "Automação de fluxos (n8n/Zapier local) — nós de HTTP/comando/arquivos/JS, .tflow",
  "desc.clip": "Histórico de área de transferência — busca, fixados, popup por atalho global",
  "desc.monitor": "Monitor de sistema — CPU/memória/rede/discos ao vivo, processos com encerrar",

  // Idioma
  "lang.title": "Idioma",

  // Tema
  "theme.title": "Tema",
  "theme.system": "Automático",
  "theme.light": "Claro",
  "theme.dark": "Escuro",
  "theme.nature": "Natureza",
  "theme.darkblue": "Azul escuro",
  "theme.calmgreen": "Verde calmo",
  "theme.pastelpink": "Rosa pastel",
  "theme.punkprincess": "PunkPrincess",
} as const;

export type MessageKey = keyof typeof pt;

const en: Record<MessageKey, string> = {
  "time.now": "now",
  "time.minAgo": "{n} min ago",
  "time.hourAgo": "{n} h ago",
  "time.yesterday": "yesterday",

  "hdr.sub": "Local suite — install, update, open",
  "tab.apps": "Apps",
  "tab.recent": "Recent",
  "tab.files": "Associations",
  "gh.loggedTitle": "Logged into GitHub — 5,000 requests/hour",
  "gh.loginTitle": "Optional GitHub login (avoids the 403 rate-limit error)",
  "gh.logged": "GitHub ✓",
  "gh.login": "GitHub",
  "refresh.title": "Query GitHub now, ignoring the caches (releases and icons)",
  "refresh.checking": "Checking…",
  "refresh.check": "Check for updates",
  "refresh.when": "Checked {clock}",

  "banner.browser": "Running in the browser (preview) — installing/detecting only works in the Tauri app.",
  "banner.updatePre": "New Hub version available: v{latest} (you're on v{current}).",
  "banner.updateBtn": "Update the Hub",

  "linux.recreate": "Recreate menu shortcuts",
  "repo.placeholder": "github.com/owner/repo — add an app from outside the catalog",
  "repo.adding": "Adding…",
  "repo.add": "Add repository",
  "card.debBadge": "via .deb",
  "card.updateBadge": "v{v} available",
  "card.notInstalled": "not installed",
  "card.lastVersion": " — latest: v{v}",
  "card.installing": "Installing…",
  "card.preparing": "Preparing…",
  "act.install": "Install",
  "act.update": "Update",
  "act.open": "Open",
  "act.uninstall": "Uninstall",
  "act.removeFromList": "Remove from list",
  "act.removeFromListTitle": "Remove from the Hub list (doesn't uninstall)",
  "card.debHint": "Installed by the package manager — update/remove it with apt.",

  "queue.updateAll": "Update all ({n})",
  "confirm.updateAll":
    "Update {n} apps at once?\n\nApps that are open may fail to update — close them before continuing.",
  "queue.running": "Updating {done}/{total}: {name}…",
  "queue.doneOk": "All up to date — {n} apps.",
  "queue.doneWithFail": "{ok} updated, {fail} failed: {names} (see the error on the card).",

  "rec.empty":
    "Nothing here yet. Files you open by double-clicking (via the Hub's associations) show up in this list — and you can pin favorites with the ☆.",
  "rec.pinned": "★ Pinned",
  "rec.recent": "Recent",
  "rec.noApp": "no associated app",
  "rec.unpin": "Unpin",
  "rec.pin": "Pin",
  "rec.remove": "Remove from list",
  "rec.clear": "Clear recents",

  "assoc.hint":
    'Choose which app opens each file type. The Hub registers the association and dispatches the file to the right app on click. Extensions that already have an owner in the system (e.g. .md, .pdf) may need confirming once in Windows\' "How do you want to open this?" dialog.',
  "assoc.ext": "Extension",
  "assoc.openWith": "Open with",
  "assoc.notInstalledOpt": " (not installed)",
  "assoc.apply": "Apply associations",

  "clean.title": "Deep clean",
  "clean.hint":
    "For anyone who installed and uninstalled apps through the Hub before these fixes existed: finds and removes what got left behind — file routes still pointing at an app that no longer exists, the downloaded-installer cache, and data folders from apps you already uninstalled.",
  "clean.scan": "Scan",
  "clean.scanning": "Scanning…",
  "clean.nothing": "Nothing to clean — all good.",
  "clean.orphanTitle": "Orphaned file routes ({n})",
  "clean.orphanHint":
    "Extensions still pointing at the Hub as the Windows default, but the owning app was uninstalled.",
  "clean.orphanClean": "Remove orphaned routes",
  "clean.downloadsTitle": "Downloaded installer cache",
  "clean.downloadsInfo": "{n} file(s), {size}",
  "clean.downloadsClean": "Clear downloads cache",
  "clean.dirsTitle": "Forgotten data folders",
  "clean.dirsHint": "AppData folders from apps that are no longer installed.",
  "clean.dirRemove": "Remove",
  "clean.doneOrphan": "{n} route(s) removed.",
  "clean.doneDownloads": "{size} freed.",
  "clean.doneDir": "{size} freed.",

  "ghm.title": "Connect to GitHub (optional)",
  "ghm.p1": "Without login, GitHub limits anonymous queries to",
  "ghm.p1Strong": "60/hour per IP",
  "ghm.p2": "— opening the Hub several times in a row triggers the 403 error. Logged in, the limit becomes",
  "ghm.p2Strong": "5,000/hour",
  "ghm.p3": ". The credential stays in the",
  "ghm.p3Strong": "system vault",
  "ghm.p4": "(never in a file) and is only used to query and download releases.",
  "ghm.login": "🌐 Sign in with the browser",
  "ghm.opening": "Opening the browser…",
  "ghm.flowCode": "Enter this code on the GitHub page that opened:",
  "ghm.flowWait": "Waiting for you to authorize in the browser…",
  "ghm.flowReopen": "reopen the page",
  "ghm.connected": "✓ Connected.",
  "ghm.pasteGenerated": "Paste the generated token:",
  "ghm.pasteManual": "Or paste a token created manually (with no scope checked):",
  "ghm.tokenPlaceholder": "ghp_… or github_pat_…",
  "ghm.saveToken": "Save token",
  "ghm.disconnect": "Disconnect",
  "common.close": "Close",

  "foot.text":
    "Taylor — 100% offline once installed; the Hub only accesses GitHub to download releases. No telemetry.",

  "msg.releasesFail":
    "Couldn't query the releases on GitHub — without that, updates won't show up. ({reason})",
  "msg.queryingRelease": "Querying the release…",
  "msg.added": "{name} added.",
  "confirm.removeCustom": "Remove {name} from the Hub list?\n(the installed app is NOT uninstalled)",
  "msg.generateHint": 'Click "Generate token" on the page that opened, copy it and paste it below.',
  "msg.connectedRate": "Connected — {n} requests/hour.",
  "msg.validating": "Validating…",
  "msg.tokenSaved": "Token saved in the system vault — {n} requests/hour.",
  "msg.tokenRemoved": "Token removed.",
  "confirm.uninstall": "Uninstall {name}?",
  "confirm.updateSelf": "Update TaylorHub from v{current} to v{latest}?\n\n{how}",
  "confirm.updateWin": "The Hub will download the new version, close and reinstall itself.",
  "confirm.updateLinux": "The Hub will replace its own AppImage; then just reopen it.",
  "msg.downloadingUpdate": "Downloading update…",
  "msg.updatedRestart": "Updated! Close and reopen the Hub.",
  "msg.updatedInstalling": "Installing — the Hub will close and reopen updated…",
  "msg.error": "Error: {e}",
  "msg.noAppForFile": "No installed app handles .{ext} — see the Associations tab.",
  "confirm.clearRecents": "Clear the recents list? (pinned ones stay)",
  "msg.noShortcut": "No installed app (AppImage) to create a shortcut for.",
  "msg.shortcutWarn": "Shortcuts with warnings: {w}",
  "msg.shortcutOk": "Menu shortcuts recreated ({n} apps).",
  "msg.applying": "Applying…",
  "msg.noAppToAssoc": "No installed app to associate — install one first in the Apps tab.",
  "msg.assocWarn": "Applied with warnings: {w}",
  "msg.assocOk": "Associations applied ({n} extensions).",

  "cat.Escritório": "Office",
  "cat.Dados e conhecimento": "Data & knowledge",
  "cat.Desenvolvimento": "Development",
  "cat.Comunicação": "Communication",
  "cat.Inteligência artificial": "Artificial intelligence",
  "cat.Mídia": "Media",
  "cat.Segurança": "Security",
  "cat.Sistema": "System",
  "cat.Automação": "Automation",
  "cat.Meus repositórios": "My repositories",
  "cat.Outros": "Other",
  "custom.repoDesc": "User repository — {repo}",

  "desc.writer": "Documents (Word) — DOCX/ODT/MD, academic ABNT/APA, local AI",
  "desc.sheets": "Spreadsheet (Excel) — XLSX/CSV, formulas, AI that edits cells",
  "desc.slides": "Presentations (PowerPoint) — canvas, PPTX, AI builds the deck",
  "desc.code": "Code editor — LSP, debugger, git, terminal, agentic AI",
  "desc.data": "Visual database (Airtable) — typed tables, kanban, AI",
  "desc.pdf": "PDF editor — organize pages, annotate, sign, forms, AI",
  "desc.agenda": "Calendar, tasks and reminders (Outlook without email) — recurrence, .ics, local AI",
  "desc.draw": "Diagrams and flowcharts (Visio/draw.io) — Excalidraw canvas, connectors, AI builds the flowchart",
  "desc.zim": "Wikipedia and offline libraries (Kiwix) — reads .zim files",
  "desc.scribe": "Offline audio transcription (whisper) — timestamps, SRT/VTT, AI summary and minutes",
  "desc.translate": "Offline translator (pt · es · en) — Marian/OPUS-MT on candle, language detection, history",
  "desc.media": "Convert, compress and trim video/audio (ffmpeg) — presets, GIF, tracks, batch",
  "desc.image": "Viewer, annotator and screen capture — EXIF, batch, global shortcut",
  "desc.player": "Minimalist video/audio player (mpv) — subtitles, tracks, chapters, speed, playlist",
  "desc.taylorchat": "Offline P2P messenger — serverless, end-to-end encrypted, local AI",
  "desc.taylormind": "Mind maps (XMind) — AI builds/expands the map",
  "desc.localai": "Runs local GGUF models (LM Studio) — CPU and iGPU maxed out",
  "desc.openobsidian": "Notes and knowledge base (Obsidian) — graph, mermaid, AI",
  "desc.keys": "Password manager (Bitwarden) — encrypted .tkeys vault, generator, no cloud",
  "desc.files": "File manager — tabs, 3 views, copy/move with progress, trash",
  "desc.zip": "Archiver — open/extract zip, tar and tar.gz in place; create zip/tar.gz",
  "desc.converter": "Universal converter — video, audio, image and documents, in batch (ffmpeg + pandoc)",
  "desc.terminal": "Terminal — tabs, profiles (PowerShell/cmd/Git Bash/WSL), search, native PTY",
  "desc.calc": "Calculator — standard, scientific, programmer (HEX/BIN) and unit converter",
  "desc.feed": "RSS/Atom reader — no algorithm, no account; offline reader mode, OPML",
  "desc.automation": "Flow automation (local n8n/Zapier) — HTTP/command/files/JS nodes, .tflow",
  "desc.clip": "Clipboard history — search, pinned items, global-shortcut popup",
  "desc.monitor": "System monitor — live CPU/memory/network/disks, processes with end task",

  "lang.title": "Language",

  "theme.title": "Theme",
  "theme.system": "Automatic",
  "theme.light": "Light",
  "theme.dark": "Dark",
  "theme.nature": "Nature",
  "theme.darkblue": "Dark blue",
  "theme.calmgreen": "Calm green",
  "theme.pastelpink": "Pastel pink",
  "theme.punkprincess": "PunkPrincess",
};

const es: Record<MessageKey, string> = {
  "time.now": "ahora",
  "time.minAgo": "hace {n} min",
  "time.hourAgo": "hace {n} h",
  "time.yesterday": "ayer",

  "hdr.sub": "suite Local — instalar, actualizar, abrir",
  "tab.apps": "Apps",
  "tab.recent": "Recientes",
  "tab.files": "Asociaciones",
  "gh.loggedTitle": "Conectado a GitHub — 5.000 solicitudes/hora",
  "gh.loginTitle": "Inicio de sesión opcional en GitHub (evita el error 403 de rate limit)",
  "gh.logged": "GitHub ✓",
  "gh.login": "GitHub",
  "refresh.title": "Consulta GitHub ahora, ignorando las cachés (releases e iconos)",
  "refresh.checking": "Comprobando…",
  "refresh.check": "Buscar actualizaciones",
  "refresh.when": "Comprobado {clock}",

  "banner.browser": "Ejecutándose en el navegador (preview) — instalar/detectar solo funciona en la app Tauri.",
  "banner.updatePre": "Nueva versión del Hub disponible: v{latest} (estás en la v{current}).",
  "banner.updateBtn": "Actualizar el Hub",

  "linux.recreate": "Recrear accesos del menú",
  "repo.placeholder": "github.com/dueño/repo — añadir app fuera del catálogo",
  "repo.adding": "Añadiendo…",
  "repo.add": "Añadir repositorio",
  "card.debBadge": "vía .deb",
  "card.updateBadge": "v{v} disponible",
  "card.notInstalled": "no instalado",
  "card.lastVersion": " — última: v{v}",
  "card.installing": "Instalando…",
  "card.preparing": "Preparando…",
  "act.install": "Instalar",
  "act.update": "Actualizar",
  "act.open": "Abrir",
  "act.uninstall": "Desinstalar",
  "act.removeFromList": "Quitar de la lista",
  "act.removeFromListTitle": "Quitar de la lista del Hub (no desinstala)",
  "card.debHint": "Instalado por el gestor de paquetes — actualiza/quita con apt.",

  "queue.updateAll": "Actualizar todo ({n})",
  "confirm.updateAll":
    "¿Actualizar {n} apps de una vez?\n\nLas apps abiertas pueden fallar al actualizar — ciérralas antes de continuar.",
  "queue.running": "Actualizando {done}/{total}: {name}…",
  "queue.doneOk": "Todo actualizado — {n} apps.",
  "queue.doneWithFail": "{ok} actualizadas, {fail} fallaron: {names} (mira el error en la tarjeta).",

  "rec.empty":
    "Nada por aquí todavía. Los archivos que abras con doble clic (vía las asociaciones del Hub) aparecen en esta lista — y puedes fijar los favoritos con la ☆.",
  "rec.pinned": "★ Fijados",
  "rec.recent": "Recientes",
  "rec.noApp": "sin app asociada",
  "rec.unpin": "Desfijar",
  "rec.pin": "Fijar",
  "rec.remove": "Quitar de la lista",
  "rec.clear": "Limpiar recientes",

  "assoc.hint":
    'Elige qué app abre cada tipo de archivo. El Hub registra la asociación y envía el archivo a la app correcta al hacer clic. Las extensiones que ya tienen dueño en el sistema (ej.: .md, .pdf) pueden requerir confirmar una vez en el diálogo "¿Cómo quieres abrir esto?" de Windows.',
  "assoc.ext": "Extensión",
  "assoc.openWith": "Abrir con",
  "assoc.notInstalledOpt": " (no instalado)",
  "assoc.apply": "Aplicar asociaciones",

  "clean.title": "Limpieza profunda",
  "clean.hint":
    "Para quien instaló y desinstaló apps por el Hub antes de que existieran estas correcciones: encuentra y elimina lo que quedó atrás — rutas de archivo que aún apuntan a un app que ya no existe, la caché de instaladores descargados y carpetas de datos de apps ya desinstalados.",
  "clean.scan": "Verificar",
  "clean.scanning": "Verificando…",
  "clean.nothing": "Nada que limpiar — todo en orden.",
  "clean.orphanTitle": "Rutas de archivo huérfanas ({n})",
  "clean.orphanHint":
    "Extensiones que siguen apuntando al Hub como predeterminado de Windows, pero la app dueña fue desinstalada.",
  "clean.orphanClean": "Quitar rutas huérfanas",
  "clean.downloadsTitle": "Caché de instaladores descargados",
  "clean.downloadsInfo": "{n} archivo(s), {size}",
  "clean.downloadsClean": "Limpiar caché de descargas",
  "clean.dirsTitle": "Carpetas de datos olvidadas",
  "clean.dirsHint": "Carpetas en AppData de apps que ya no están instaladas.",
  "clean.dirRemove": "Quitar",
  "clean.doneOrphan": "{n} ruta(s) eliminada(s).",
  "clean.doneDownloads": "{size} liberados.",
  "clean.doneDir": "{size} liberados.",

  "ghm.title": "Conectar a GitHub (opcional)",
  "ghm.p1": "Sin inicio de sesión, GitHub limita las consultas anónimas a",
  "ghm.p1Strong": "60/hora por IP",
  "ghm.p2": "— abrir el Hub varias veces seguidas da el error 403. Conectado, el límite pasa a",
  "ghm.p2Strong": "5.000/hora",
  "ghm.p3": ". La credencial queda en el",
  "ghm.p3Strong": "cofre del sistema",
  "ghm.p4": "(nunca en un archivo) y solo se usa para consultar y descargar releases.",
  "ghm.login": "🌐 Entrar con el navegador",
  "ghm.opening": "Abriendo el navegador…",
  "ghm.flowCode": "Escribe este código en la página de GitHub que se abrió:",
  "ghm.flowWait": "Esperando a que autorices en el navegador…",
  "ghm.flowReopen": "reabrir la página",
  "ghm.connected": "✓ Conectado.",
  "ghm.pasteGenerated": "Pega el token generado:",
  "ghm.pasteManual": "O pega un token creado manualmente (sin ningún alcance marcado):",
  "ghm.tokenPlaceholder": "ghp_… o github_pat_…",
  "ghm.saveToken": "Guardar token",
  "ghm.disconnect": "Desconectar",
  "common.close": "Cerrar",

  "foot.text":
    "Taylor — 100% offline una vez instalado; el Hub solo accede a GitHub para descargar releases. Sin telemetría.",

  "msg.releasesFail":
    "No pude consultar las releases en GitHub — sin eso no aparecen actualizaciones. ({reason})",
  "msg.queryingRelease": "Consultando la release…",
  "msg.added": "{name} añadido.",
  "confirm.removeCustom": "¿Quitar {name} de la lista del Hub?\n(la app instalada NO se desinstala)",
  "msg.generateHint": 'Haz clic en "Generate token" en la página que se abrió, cópialo y pégalo aquí abajo.',
  "msg.connectedRate": "Conectado — {n} solicitudes/hora.",
  "msg.validating": "Validando…",
  "msg.tokenSaved": "Token guardado en el cofre del sistema — {n} solicitudes/hora.",
  "msg.tokenRemoved": "Token eliminado.",
  "confirm.uninstall": "¿Desinstalar {name}?",
  "confirm.updateSelf": "¿Actualizar TaylorHub de la v{current} a la v{latest}?\n\n{how}",
  "confirm.updateWin": "El Hub descargará la nueva versión, se cerrará y se reinstalará solo.",
  "confirm.updateLinux": "El Hub reemplazará su propio AppImage; luego solo vuelve a abrirlo.",
  "msg.downloadingUpdate": "Descargando actualización…",
  "msg.updatedRestart": "¡Actualizado! Cierra y vuelve a abrir el Hub.",
  "msg.updatedInstalling": "Instalando — el Hub se cerrará y se reabrirá actualizado…",
  "msg.error": "Error: {e}",
  "msg.noAppForFile": "Ninguna app instalada abre .{ext} — mira la pestaña Asociaciones.",
  "confirm.clearRecents": "¿Limpiar la lista de recientes? (los fijados se quedan)",
  "msg.noShortcut": "Ninguna app instalada (AppImage) para crear acceso.",
  "msg.shortcutWarn": "Accesos con avisos: {w}",
  "msg.shortcutOk": "Accesos del menú recreados ({n} apps).",
  "msg.applying": "Aplicando…",
  "msg.noAppToAssoc": "Ninguna app instalada para asociar — instala una primero en la pestaña Apps.",
  "msg.assocWarn": "Aplicado con avisos: {w}",
  "msg.assocOk": "Asociaciones aplicadas ({n} extensiones).",

  "cat.Escritório": "Oficina",
  "cat.Dados e conhecimento": "Datos y conocimiento",
  "cat.Desenvolvimento": "Desarrollo",
  "cat.Comunicação": "Comunicación",
  "cat.Inteligência artificial": "Inteligencia artificial",
  "cat.Mídia": "Medios",
  "cat.Segurança": "Seguridad",
  "cat.Sistema": "Sistema",
  "cat.Automação": "Automatización",
  "cat.Meus repositórios": "Mis repositorios",
  "cat.Outros": "Otros",
  "custom.repoDesc": "Repositorio del usuario — {repo}",

  "desc.writer": "Documentos (Word) — DOCX/ODT/MD, académico ABNT/APA, IA local",
  "desc.sheets": "Hoja de cálculo (Excel) — XLSX/CSV, fórmulas, IA que edita celdas",
  "desc.slides": "Presentaciones (PowerPoint) — lienzo, PPTX, la IA genera el deck",
  "desc.code": "Editor de código — LSP, depurador, git, terminal, IA agente",
  "desc.data": "Base de datos visual (Airtable) — tablas tipadas, kanban, IA",
  "desc.pdf": "Editor de PDF — organizar páginas, anotar, firmar, formularios, IA",
  "desc.agenda": "Calendario, tareas y recordatorios (Outlook sin correo) — recurrencia, .ics, IA local",
  "desc.draw": "Diagramas y diagramas de flujo (Visio/draw.io) — lienzo Excalidraw, conectores, la IA genera el diagrama",
  "desc.zim": "Wikipedia y bibliotecas offline (Kiwix) — lee archivos .zim",
  "desc.scribe": "Transcripción de audio offline (whisper) — timestamps, SRT/VTT, resumen y acta por IA",
  "desc.translate": "Traductor offline (pt · es · en) — Marian/OPUS-MT en candle, detección de idioma, historial",
  "desc.media": "Convertir, comprimir y recortar vídeo/audio (ffmpeg) — presets, GIF, pistas, lote",
  "desc.image": "Visor, anotador y captura de pantalla — EXIF, lote, atajo global",
  "desc.player": "Reproductor de vídeo/audio minimalista (mpv) — subtítulos, pistas, capítulos, velocidad, playlist",
  "desc.taylorchat": "Mensajero P2P offline — sin servidor, cifrado de extremo a extremo, IA local",
  "desc.taylormind": "Mapas mentales (XMind) — la IA genera/expande el mapa",
  "desc.localai": "Ejecuta modelos GGUF locales (LM Studio) — CPU e iGPU al máximo",
  "desc.openobsidian": "Notas y base de conocimiento (Obsidian) — grafo, mermaid, IA",
  "desc.keys": "Gestor de contraseñas (Bitwarden) — vault .tkeys cifrado, generador, sin nube",
  "desc.files": "Gestor de archivos — pestañas, 3 vistas, copiar/mover con progreso, papelera",
  "desc.zip": "Compresor — abre/extrae zip, tar y tar.gz sin moverse; crea zip/tar.gz",
  "desc.converter": "Conversor universal — vídeo, audio, imagen y documentos, por lotes (ffmpeg + pandoc)",
  "desc.terminal": "Terminal — pestañas, perfiles (PowerShell/cmd/Git Bash/WSL), búsqueda, PTY nativo",
  "desc.calc": "Calculadora — estándar, científica, programador (HEX/BIN) y conversor de unidades",
  "desc.feed": "Lector RSS/Atom — sin algoritmo ni cuenta; modo lectura offline, OPML",
  "desc.automation": "Automatización de flujos (n8n/Zapier local) — nodos HTTP/comando/archivos/JS, .tflow",
  "desc.clip": "Historial de portapapeles — búsqueda, fijados, popup con atajo global",
  "desc.monitor": "Monitor de sistema — CPU/memoria/red/discos en vivo, procesos con finalizar",

  "lang.title": "Idioma",

  "theme.title": "Tema",
  "theme.system": "Automático",
  "theme.light": "Claro",
  "theme.dark": "Oscuro",
  "theme.nature": "Naturaleza",
  "theme.darkblue": "Azul oscuro",
  "theme.calmgreen": "Verde tranquilo",
  "theme.pastelpink": "Rosa pastel",
  "theme.punkprincess": "PunkPrincess",
};

const DICTS: Record<Locale, Record<MessageKey, string>> = { pt, en, es };

/** Palpite de locale pelo idioma do sistema (só no 1º uso). */
export function detectLocale(): Locale {
  const l = (typeof navigator !== "undefined" ? navigator.language : "pt").toLowerCase();
  if (l.startsWith("en")) return "en";
  if (l.startsWith("es")) return "es";
  return "pt";
}

function loadLocale(): Locale {
  const v = typeof localStorage !== "undefined" ? localStorage.getItem(LOCALE_KEY) : null;
  return v === "pt" || v === "en" || v === "es" ? v : detectLocale();
}

let current: Locale = loadLocale();
const listeners = new Set<() => void>();

export function getLocale(): Locale {
  return current;
}

export function setLocale(locale: Locale) {
  if (locale === current) return;
  current = locale;
  try {
    localStorage.setItem(LOCALE_KEY, locale);
  } catch {
    /* localStorage indisponível */
  }
  for (const l of listeners) l();
}

function subscribe(l: () => void) {
  listeners.add(l);
  return () => listeners.delete(l);
}

/** Inscreve o componente nas trocas de locale. */
export function useLocale(): Locale {
  return useSyncExternalStore(subscribe, getLocale);
}

/** BCP-47 do locale atual (pra Number/Date toLocaleString). */
export function localeTag(): string {
  return current === "pt" ? "pt-BR" : current === "es" ? "es-ES" : "en-US";
}

/** Traduz uma chave, interpolando placeholders `{param}`. */
export function t(key: MessageKey, params?: Record<string, string | number>): string {
  let msg: string = DICTS[current][key] ?? pt[key] ?? key;
  if (params) {
    for (const [k, v] of Object.entries(params)) {
      msg = msg.split(`{${k}}`).join(String(v));
    }
  }
  return msg;
}

/** Rótulo localizado de uma categoria do catálogo (a chave é o nome pt). */
export function catLabel(category: string): string {
  return t(`cat.${category}` as MessageKey);
}

/** Descrição localizada de um app do catálogo; cai no texto do catálogo se não houver chave. */
export function appDesc(id: string, fallback: string): string {
  const key = `desc.${id}` as MessageKey;
  const s = t(key);
  return s === key ? fallback : s;
}
