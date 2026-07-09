use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use tauri::{Emitter, Manager};

// ---------------------------------------------------------------------------
// Diretório de config do Hub (usável ANTES do Tauri subir — o dispatcher precisa)
// ---------------------------------------------------------------------------

fn config_dir() -> PathBuf {
    #[cfg(windows)]
    {
        PathBuf::from(std::env::var("APPDATA").unwrap_or_else(|_| ".".into())).join("TaylorHub")
    }
    #[cfg(not(windows))]
    {
        let base = std::env::var("XDG_CONFIG_HOME")
            .map(PathBuf::from)
            .unwrap_or_else(|_| {
                PathBuf::from(std::env::var("HOME").unwrap_or_else(|_| ".".into())).join(".config")
            });
        base.join("taylorhub")
    }
}

fn ensure_dir(dir: &Path) -> Result<(), String> {
    fs::create_dir_all(dir).map_err(|e| format!("Falha ao criar '{}': {}", dir.display(), e))
}

fn read_json<T: serde::de::DeserializeOwned>(path: &Path) -> Option<T> {
    let text = fs::read_to_string(path).ok()?;
    serde_json::from_str(&text).ok()
}

fn write_json<T: Serialize>(path: &Path, value: &T) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        ensure_dir(parent)?;
    }
    let text = serde_json::to_string_pretty(value).map_err(|e| e.to_string())?;
    fs::write(path, text).map_err(|e| format!("Falha ao salvar '{}': {}", path.display(), e))
}

// ---------------------------------------------------------------------------
// Detecção de apps instalados
// ---------------------------------------------------------------------------

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DetectSpec {
    id: String,
    /// DisplayName esperado no registro (Windows) — normalmente o productName.
    name: String,
    /// Nome do executável (ex.: "LocalOffice.exe").
    exe: String,
}

#[derive(Serialize, Clone, Default)]
#[serde(rename_all = "camelCase")]
pub struct InstalledInfo {
    id: String,
    installed: bool,
    version: String,
    location: String,
    exe: String,
    /// De onde veio a detecção: "registry" (Windows), "hub" (installed.json),
    /// "appimage" (achado em ~/Applications), "deb" (dpkg). Vazio = não instalado.
    source: String,
}

/// Extrai "0.14.6" de nomes tipo "LocalOffice_0.14.6_amd64.AppImage" / "OpenObsidian-0.7.1.AppImage".
/// Exige pelo menos dois grupos numéricos ("2" sozinho não é versão).
#[cfg_attr(windows, allow(dead_code))] // usado no scan de AppImages (Linux) e nos testes
fn version_from_filename(name: &str) -> Option<String> {
    fn validate(cur: &mut String) -> Option<String> {
        let v = cur.trim_matches('.').to_string();
        cur.clear();
        let parts: Vec<&str> = v.split('.').collect();
        if parts.len() >= 2
            && parts.iter().all(|p| !p.is_empty() && p.chars().all(|c| c.is_ascii_digit()))
        {
            Some(v)
        } else {
            None
        }
    }
    let mut cur = String::new();
    for c in name.chars() {
        if c.is_ascii_digit() || c == '.' {
            cur.push(c);
        } else if let Some(v) = validate(&mut cur) {
            return Some(v);
        }
    }
    validate(&mut cur)
}

/// Registro de instalações feitas pelo Hub no Linux (AppImages não têm registro).
#[cfg(not(windows))]
#[derive(Serialize, Deserialize, Default)]
struct LinuxInstalls(std::collections::HashMap<String, LinuxInstall>);

#[cfg(not(windows))]
#[derive(Serialize, Deserialize, Clone)]
struct LinuxInstall {
    version: String,
    path: String,
}

#[cfg(not(windows))]
fn linux_installs_path() -> PathBuf {
    config_dir().join("installed.json")
}

#[cfg(windows)]
fn detect_one(spec: &DetectSpec) -> InstalledInfo {
    use winreg::enums::{HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE};
    use winreg::RegKey;

    let hives: [(winreg::HKEY, &str); 3] = [
        (HKEY_CURRENT_USER, r"Software\Microsoft\Windows\CurrentVersion\Uninstall"),
        (HKEY_LOCAL_MACHINE, r"Software\Microsoft\Windows\CurrentVersion\Uninstall"),
        (HKEY_LOCAL_MACHINE, r"Software\WOW6432Node\Microsoft\Windows\CurrentVersion\Uninstall"),
    ];
    for (hive, path) in hives {
        let root = RegKey::predef(hive);
        let Ok(unin) = root.open_subkey(path) else { continue };
        for key in unin.enum_keys().flatten() {
            let Ok(sub) = unin.open_subkey(&key) else { continue };
            let dn: String = sub.get_value("DisplayName").unwrap_or_default();
            // NSIS do Tauri usa o productName puro; electron-builder às vezes acrescenta espaço+versão.
            if dn != spec.name && !dn.starts_with(&format!("{} ", spec.name)) {
                continue;
            }
            let version: String = sub.get_value("DisplayVersion").unwrap_or_default();
            let mut location: String = sub.get_value("InstallLocation").unwrap_or_default();
            let mut exe_path = if location.is_empty() {
                String::new()
            } else {
                Path::new(&location).join(&spec.exe).to_string_lossy().to_string()
            };
            if exe_path.is_empty() || !Path::new(&exe_path).exists() {
                // Fallback: DisplayIcon costuma apontar pro exe ("C:\...\App.exe,0").
                let icon: String = sub.get_value("DisplayIcon").unwrap_or_default();
                let icon = icon.split(',').next().unwrap_or("").trim_matches('"').to_string();
                if !icon.is_empty() && Path::new(&icon).exists() {
                    if location.is_empty() {
                        location = Path::new(&icon)
                            .parent()
                            .map(|p| p.to_string_lossy().to_string())
                            .unwrap_or_default();
                    }
                    exe_path = icon;
                }
            }
            return InstalledInfo {
                id: spec.id.clone(),
                installed: true,
                version,
                location,
                exe: exe_path,
                source: "registry".into(),
            };
        }
    }
    InstalledInfo { id: spec.id.clone(), ..Default::default() }
}

/// Varre ~/Applications por um AppImage cujo nome começa com o nome do app
/// (pega instalação feita por fora do Hub). Retorna (versão-do-nome, caminho).
#[cfg(not(windows))]
fn scan_appimages(name: &str) -> Option<(String, String)> {
    let home = std::env::var("HOME").ok()?;
    let dir = PathBuf::from(home).join("Applications");
    let prefix = name.to_lowercase();
    for entry in fs::read_dir(dir).ok()?.flatten() {
        let fname = entry.file_name().to_string_lossy().to_string();
        let lower = fname.to_lowercase();
        if lower.ends_with(".appimage") && lower.starts_with(&prefix) {
            let version = version_from_filename(&fname).unwrap_or_default();
            return Some((version, entry.path().to_string_lossy().to_string()));
        }
    }
    None
}

/// Procura o app no dpkg (instalado via .deb). Casa o nome do pacote
/// normalizado ("open-obsidian" ≈ "OpenObsidian"). Retorna (versão, exe).
#[cfg(not(windows))]
fn dpkg_detect(name: &str) -> Option<(String, String)> {
    fn norm(s: &str) -> String {
        s.chars().filter(|c| c.is_ascii_alphanumeric()).collect::<String>().to_lowercase()
    }
    let out = Command::new("dpkg-query")
        .args(["-W", "-f", "${Package}\\t${Version}\\n"])
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    let target = norm(name);
    let text = String::from_utf8_lossy(&out.stdout);
    for line in text.lines() {
        let mut cols = line.split('\t');
        let pkg = cols.next()?;
        let raw_ver = cols.next().unwrap_or("");
        if norm(pkg) != target {
            continue;
        }
        // "1:0.7.1-1" → sem epoch, sem revision debian.
        let ver = raw_ver.split(':').next_back().unwrap_or(raw_ver);
        let ver = ver.split('-').next().unwrap_or(ver).to_string();
        let exe = Command::new("dpkg")
            .args(["-L", pkg])
            .output()
            .ok()
            .and_then(|o| {
                String::from_utf8_lossy(&o.stdout)
                    .lines()
                    .find(|l| l.starts_with("/usr/bin/"))
                    .map(|s| s.to_string())
            })
            .unwrap_or_default();
        return Some((ver, exe));
    }
    None
}

#[cfg(not(windows))]
fn detect_one(spec: &DetectSpec) -> InstalledInfo {
    // 1) Instalado pelo Hub (installed.json)
    let installs: LinuxInstalls = read_json(&linux_installs_path()).unwrap_or_default();
    if let Some(found) = installs.0.get(&spec.id) {
        if Path::new(&found.path).exists() {
            return InstalledInfo {
                id: spec.id.clone(),
                installed: true,
                version: found.version.clone(),
                location: Path::new(&found.path)
                    .parent()
                    .map(|p| p.to_string_lossy().to_string())
                    .unwrap_or_default(),
                exe: found.path.clone(),
                source: "hub".into(),
            };
        }
    }
    // 2) AppImage instalado por fora, em ~/Applications
    if let Some((version, path)) = scan_appimages(&spec.name) {
        return InstalledInfo {
            id: spec.id.clone(),
            installed: true,
            version,
            location: Path::new(&path)
                .parent()
                .map(|p| p.to_string_lossy().to_string())
                .unwrap_or_default(),
            exe: path,
            source: "appimage".into(),
        };
    }
    // 3) .deb via dpkg (alternativa pra quem não curte AppImage — o Hub só
    //    mostra/abre; atualizar/remover fica com o apt)
    if let Some((version, exe)) = dpkg_detect(&spec.name) {
        return InstalledInfo {
            id: spec.id.clone(),
            installed: true,
            version,
            location: String::new(),
            exe,
            source: "deb".into(),
        };
    }
    let _ = &spec.exe; // exe do catálogo só é usado no Windows
    InstalledInfo { id: spec.id.clone(), ..Default::default() }
}

#[tauri::command]
fn detect_apps(specs: Vec<DetectSpec>) -> Vec<InstalledInfo> {
    specs.iter().map(detect_one).collect()
}

#[tauri::command]
fn get_os() -> String {
    std::env::consts::OS.to_string()
}

// ---------------------------------------------------------------------------
// GitHub releases
// ---------------------------------------------------------------------------

#[derive(Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ReleaseInfo {
    tag: String,
    version: String,
    assets: Vec<AssetInfo>,
}

#[derive(Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct AssetInfo {
    name: String,
    url: String,
    size: u64,
}

// Cache local das consultas de release: a API anônima do GitHub permite só
// 60 req/hora por IP, e cada abertura do Hub custa 1 req por app do catálogo.
// Sem cache, abrir o Hub ~6x na mesma hora já vira 403 pra tudo.

const RELEASE_CACHE_TTL_SECS: u64 = 30 * 60;

#[derive(Serialize, Deserialize, Default)]
struct ReleaseCache(std::collections::HashMap<String, CachedRelease>);

#[derive(Serialize, Deserialize, Clone)]
struct CachedRelease {
    fetched_at: u64,
    release: ReleaseInfo,
}

/// Serializa leitura+escrita do arquivo de cache (o frontend consulta todos
/// os repos em paralelo; sem isso, gravações concorrentes se perdem).
static RELEASE_CACHE_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

fn release_cache_path() -> PathBuf {
    config_dir().join("releases_cache.json")
}

fn now_secs() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

fn cache_is_fresh(entry: &CachedRelease, now: u64) -> bool {
    now.saturating_sub(entry.fetched_at) < RELEASE_CACHE_TTL_SECS
}

fn release_cache_get(repo: &str) -> Option<CachedRelease> {
    let _guard = RELEASE_CACHE_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let cache: ReleaseCache = read_json(&release_cache_path())?;
    cache.0.get(repo).cloned()
}

fn release_cache_put(repo: &str, release: &ReleaseInfo) {
    let _guard = RELEASE_CACHE_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let mut cache: ReleaseCache = read_json(&release_cache_path()).unwrap_or_default();
    cache.0.insert(
        repo.to_string(),
        CachedRelease { fetched_at: now_secs(), release: release.clone() },
    );
    // Cache é otimização: falha ao gravar não pode derrubar a consulta.
    let _ = write_json(&release_cache_path(), &cache);
}

fn http_client() -> Result<reqwest::Client, String> {
    reqwest::Client::builder()
        .user_agent("TaylorHub")
        .build()
        .map_err(|e| e.to_string())
}

/// Consulta a release mais recente, com cache local (TTL de 30 min).
/// `force` pula a checagem de TTL (botão "Verificar atualizações"), mas o
/// resultado ainda alimenta o cache e a falha ainda cai no fallback.
/// Se a API falhar (ex.: 403 de rate limit), devolve o cache mesmo vencido —
/// dado velho é melhor que erro na tela.
async fn fetch_latest(repo: &str, force: bool) -> Result<ReleaseInfo, String> {
    let cached = release_cache_get(repo);
    if !force {
        if let Some(entry) = &cached {
            if cache_is_fresh(entry, now_secs()) {
                return Ok(entry.release.clone());
            }
        }
    }
    match fetch_latest_remote(repo).await {
        Ok(release) => {
            release_cache_put(repo, &release);
            Ok(release)
        }
        Err(err) => cached.map(|entry| entry.release).ok_or(err),
    }
}

async fn fetch_latest_remote(repo: &str) -> Result<ReleaseInfo, String> {
    let url = format!("https://api.github.com/repos/{}/releases/latest", repo);
    let resp = http_client()?
        .get(&url)
        .header("Accept", "application/vnd.github+json")
        .send()
        .await
        .map_err(|e| format!("Falha de rede em {}: {}", repo, e))?;
    let status = resp.status();
    if !status.is_success() {
        let hint = if status == 403 || status == 429 {
            " (provável rate limit da API — espere ~1h ou tente de novo mais tarde)"
        } else {
            ""
        };
        return Err(format!("GitHub respondeu {} para {}{}", status, repo, hint));
    }
    let body: serde_json::Value = resp.json().await.map_err(|e| e.to_string())?;
    let tag = body["tag_name"].as_str().unwrap_or("").to_string();
    let assets = body["assets"]
        .as_array()
        .map(|arr| {
            arr.iter()
                .map(|a| AssetInfo {
                    name: a["name"].as_str().unwrap_or("").to_string(),
                    url: a["browser_download_url"].as_str().unwrap_or("").to_string(),
                    size: a["size"].as_u64().unwrap_or(0),
                })
                .collect()
        })
        .unwrap_or_default();
    let version = tag.trim_start_matches('v').to_string();
    Ok(ReleaseInfo { tag, version, assets })
}

#[tauri::command]
async fn get_latest_release(repo: String, force: Option<bool>) -> Result<ReleaseInfo, String> {
    fetch_latest(&repo, force.unwrap_or(false)).await
}

// ---------------------------------------------------------------------------
// Cache local dos ícones dos cards
// ---------------------------------------------------------------------------

// Ícone baixado uma vez pra config_dir()/icons/<id>.png e servido de lá como
// data URL — abrir o Hub não depende de internet pra mostrar os ícones.
// Só re-baixa com force=true (botão "Verificar atualizações").

fn icon_path(id: &str) -> Option<PathBuf> {
    // id vem do catálogo, mas sanitiza mesmo assim (vira nome de arquivo).
    let safe: String = id
        .chars()
        .filter(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_'))
        .collect();
    if safe.is_empty() {
        return None;
    }
    Some(config_dir().join("icons").join(format!("{safe}.png")))
}

fn icon_data_url(bytes: &[u8]) -> String {
    use base64::{engine::general_purpose::STANDARD, Engine as _};
    format!("data:image/png;base64,{}", STANDARD.encode(bytes))
}

#[tauri::command]
async fn get_icon(id: String, url: String, force: Option<bool>) -> Result<String, String> {
    let path = icon_path(&id).ok_or("id de app inválido")?;
    let cached = fs::read(&path).ok();
    if !force.unwrap_or(false) {
        if let Some(bytes) = &cached {
            return Ok(icon_data_url(bytes));
        }
    }
    let fetched: Result<Vec<u8>, String> = async {
        let resp = http_client()?
            .get(&url)
            .send()
            .await
            .map_err(|e| format!("Falha de rede no ícone de {}: {}", id, e))?;
        if !resp.status().is_success() {
            return Err(format!("GitHub respondeu {} para o ícone de {}", resp.status(), id));
        }
        Ok(resp.bytes().await.map_err(|e| e.to_string())?.to_vec())
    }
    .await;
    match fetched {
        Ok(bytes) => {
            // Cache é otimização: falha ao gravar não pode derrubar o ícone.
            if let Some(dir) = path.parent() {
                let _ = fs::create_dir_all(dir);
            }
            let _ = fs::write(&path, &bytes);
            Ok(icon_data_url(&bytes))
        }
        // Download falhou (offline/rate limit): cache velho é melhor que letra.
        Err(err) => cached.map(|b| icon_data_url(&b)).ok_or(err),
    }
}

// ---------------------------------------------------------------------------
// Download + instalação
// ---------------------------------------------------------------------------

/// Glob simples, case-insensitive: '*' casa qualquer coisa; sem '?'.
fn glob_match(pattern: &str, name: &str) -> bool {
    let pattern = pattern.to_lowercase();
    let name = name.to_lowercase();
    let parts: Vec<&str> = pattern.split('*').collect();
    let mut pos = 0usize;
    for (i, part) in parts.iter().enumerate() {
        if part.is_empty() {
            continue;
        }
        if i == 0 {
            if !name.starts_with(part) {
                return false;
            }
            pos = part.len();
        } else if let Some(found) = name[pos..].find(part) {
            pos += found + part.len();
        } else {
            return false;
        }
    }
    // Sem '*' no final → precisa terminar exatamente onde parou.
    if !pattern.ends_with('*') && !parts.last().map(|p| p.is_empty()).unwrap_or(true) {
        return name.len() == pos;
    }
    true
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(windows, allow(dead_code))] // current_path/icon_url são Linux-only
pub struct InstallSpec {
    id: String,
    name: String,
    repo: String,
    /// Glob do asset por plataforma (já resolvido pelo frontend p/ o SO atual).
    asset_pattern: String,
    /// Args de instalação silenciosa no Windows (ex.: ["/S"]).
    silent_args: Vec<String>,
    exe: String,
    /// Linux: caminho do AppImage já instalado (update sobrescreve NO MESMO lugar).
    #[serde(default)]
    current_path: Option<String>,
    /// PNG do ícone (raw do GitHub) pro atalho .desktop no Linux.
    #[serde(default)]
    icon_url: Option<String>,
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct Progress {
    id: String,
    phase: String, // "download" | "install"
    done: u64,
    total: u64,
}

async fn download_asset(
    app: &tauri::AppHandle,
    id: &str,
    asset: &AssetInfo,
) -> Result<PathBuf, String> {
    use futures_util::StreamExt;
    let dir = config_dir().join("downloads");
    ensure_dir(&dir)?;
    let dest = dir.join(&asset.name);
    let resp = http_client()?
        .get(&asset.url)
        .send()
        .await
        .map_err(|e| format!("Falha ao baixar {}: {}", asset.name, e))?;
    if !resp.status().is_success() {
        return Err(format!("Download de {} falhou: {}", asset.name, resp.status()));
    }
    let total = resp.content_length().unwrap_or(asset.size);
    let mut file = fs::File::create(&dest)
        .map_err(|e| format!("Falha ao criar '{}': {}", dest.display(), e))?;
    let mut stream = resp.bytes_stream();
    let mut done: u64 = 0;
    let mut last_emit: u64 = 0;
    while let Some(chunk) = stream.next().await {
        let chunk = chunk.map_err(|e| format!("Download interrompido: {}", e))?;
        use std::io::Write;
        file.write_all(&chunk).map_err(|e| e.to_string())?;
        done += chunk.len() as u64;
        // Não afogar o front: emite a cada ~512KB.
        if done - last_emit > 512 * 1024 || done == total {
            last_emit = done;
            let _ = app.emit(
                "hub-progress",
                Progress { id: id.to_string(), phase: "download".into(), done, total },
            );
        }
    }
    Ok(dest)
}

#[cfg(windows)]
fn install_payload(spec: &InstallSpec, payload: &Path) -> Result<(), String> {
    let status = Command::new(payload)
        .args(&spec.silent_args)
        .status()
        .map_err(|e| format!("Falha ao executar instalador: {}", e))?;
    if !status.success() {
        return Err(format!(
            "Instalador de {} saiu com código {:?}",
            spec.name,
            status.code()
        ));
    }
    Ok(())
}

#[cfg(not(windows))]
fn linux_icon_path(id: &str) -> Option<PathBuf> {
    let home = std::env::var("HOME").ok()?;
    Some(PathBuf::from(home).join(format!(".local/share/icons/taylor-{}.png", id)))
}

/// Cria/atualiza a entrada de menu (~/.local/share/applications/taylor-<id>.desktop).
#[cfg(not(windows))]
fn write_desktop_entry(id: &str, name: &str, exec: &str) -> Result<(), String> {
    let home = PathBuf::from(std::env::var("HOME").map_err(|_| "HOME não definido".to_string())?);
    let desktop_dir = home.join(".local/share/applications");
    ensure_dir(&desktop_dir)?;
    let icon_line = linux_icon_path(id)
        .filter(|p| p.exists())
        .map(|p| format!("Icon={}\n", p.display()))
        .unwrap_or_default();
    let desktop = format!(
        "[Desktop Entry]\nType=Application\nName={}\nExec=\"{}\" %f\n{}Terminal=false\nCategories=Office;\n",
        name, exec, icon_line
    );
    fs::write(desktop_dir.join(format!("taylor-{}.desktop", id)), desktop)
        .map_err(|e| e.to_string())?;
    let _ = Command::new("update-desktop-database").arg(&desktop_dir).status();
    Ok(())
}

#[cfg(not(windows))]
fn install_payload(spec: &InstallSpec, payload: &Path, icon: Option<&[u8]>) -> Result<PathBuf, String> {
    // Linux: AppImage → mesmo caminho do já instalado (update in-place) ou
    // ~/Applications/<Name>.AppImage + .desktop com ícone + installed.json
    use std::os::unix::fs::PermissionsExt;
    let home = PathBuf::from(std::env::var("HOME").map_err(|_| "HOME não definido".to_string())?);
    let dest = match &spec.current_path {
        Some(p) if !p.is_empty() && Path::new(p).parent().map(|d| d.exists()).unwrap_or(false) => {
            PathBuf::from(p)
        }
        _ => {
            let apps_dir = home.join("Applications");
            ensure_dir(&apps_dir)?;
            apps_dir.join(format!("{}.AppImage", spec.name))
        }
    };
    fs::copy(payload, &dest).map_err(|e| format!("Falha ao copiar AppImage: {}", e))?;
    let mut perms = fs::metadata(&dest).map_err(|e| e.to_string())?.permissions();
    perms.set_mode(0o755);
    fs::set_permissions(&dest, perms).map_err(|e| e.to_string())?;

    // Ícone do menu (best-effort)
    if let (Some(bytes), Some(icon_path)) = (icon, linux_icon_path(&spec.id)) {
        if let Some(parent) = icon_path.parent() {
            let _ = ensure_dir(parent);
        }
        let _ = fs::write(&icon_path, bytes);
    }

    write_desktop_entry(&spec.id, &spec.name, &dest.to_string_lossy())?;
    Ok(dest)
}

#[tauri::command]
async fn install_app(app: tauri::AppHandle, spec: InstallSpec) -> Result<InstalledInfo, String> {
    let release = fetch_latest(&spec.repo, false).await?;
    let asset = release
        .assets
        .iter()
        .find(|a| glob_match(&spec.asset_pattern, &a.name))
        .cloned()
        .ok_or_else(|| {
            format!(
                "Nenhum asset da release {} de {} casa com '{}'",
                release.tag, spec.repo, spec.asset_pattern
            )
        })?;

    let payload = download_asset(&app, &spec.id, &asset).await?;

    let _ = app.emit(
        "hub-progress",
        Progress { id: spec.id.clone(), phase: "install".into(), done: 0, total: 0 },
    );

    // Ícone pro atalho .desktop (Linux, best-effort — 404/offline não travam o install).
    #[cfg(not(windows))]
    let icon_bytes: Option<Vec<u8>> = match spec.icon_url.as_deref() {
        Some(url) if !url.is_empty() => match http_client()?.get(url).send().await {
            Ok(resp) if resp.status().is_success() => resp.bytes().await.ok().map(|b| b.to_vec()),
            _ => None,
        },
        _ => None,
    };

    // Instalação roda processo bloqueante → thread própria.
    let version = release.version.clone();
    let info = tauri::async_runtime::spawn_blocking(move || -> Result<InstalledInfo, String> {
        #[cfg(windows)]
        install_payload(&spec, &payload)?;
        #[cfg(not(windows))]
        {
            let dest = install_payload(&spec, &payload, icon_bytes.as_deref())?;
            // Registrar no installed.json (Linux não tem registro).
            let path = linux_installs_path();
            let mut installs: LinuxInstalls = read_json(&path).unwrap_or_default();
            installs.0.insert(
                spec.id.clone(),
                LinuxInstall { version: version.clone(), path: dest.to_string_lossy().to_string() },
            );
            write_json(&path, &installs)?;
        }
        let _ = fs::remove_file(&payload);

        let detected = detect_one(&DetectSpec {
            id: spec.id.clone(),
            name: spec.name.clone(),
            exe: spec.exe.clone(),
        });
        if detected.installed {
            Ok(detected)
        } else {
            // Instalador terminou mas não achamos o registro — reporta mesmo assim.
            Ok(InstalledInfo {
                id: spec.id.clone(),
                installed: true,
                version,
                location: String::new(),
                exe: String::new(),
                source: String::new(),
            })
        }
    })
    .await
    .map_err(|e| format!("Falha na thread de instalação: {}", e))??;

    Ok(info)
}

// ---------------------------------------------------------------------------
// Auto-update do próprio Hub (sempre disparado pelo usuário, nunca sozinho)
// ---------------------------------------------------------------------------

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SelfUpdateSpec {
    repo: String,
    asset_pattern: String,
}

/// Baixa a versão nova do Hub e aplica.
/// Windows: agenda o instalador silencioso (com 2s de atraso pro exe liberar) e
/// fecha o Hub — retorna "closing". Linux: sobrescreve o próprio AppImage
/// ($APPIMAGE) com rename atômico — retorna "restart" (usuário reabre).
#[tauri::command]
async fn update_self(app: tauri::AppHandle, spec: SelfUpdateSpec) -> Result<String, String> {
    let release = fetch_latest(&spec.repo, false).await?;
    let asset = release
        .assets
        .iter()
        .find(|a| glob_match(&spec.asset_pattern, &a.name))
        .cloned()
        .ok_or_else(|| format!("Nenhum asset casa com '{}'", spec.asset_pattern))?;
    let payload = download_asset(&app, "hub", &asset).await?;

    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        let hub = std::env::current_exe()
            .map_err(|e| format!("current_exe: {}", e))?
            .to_string_lossy()
            .to_string();
        // O instalador não sobrescreve exe em uso: dispara com atraso e fecha o Hub.
        // Duas lições pagas aqui:
        // 1. NÃO usar `timeout` como delay: exige stdin de console e, vindo de app
        //    GUI, falha na hora — o instalador rodava com o Hub aberto e abortava.
        // 2. NÃO passar linha de cmd com aspas internas via `Command::args`: o Rust
        //    escapa `"` como `\"` (convenção do runtime C), mas o cmd.exe NÃO
        //    entende `\"` — a linha inteira virava lixo e nada rodava.
        // Solução: gravar um .cmd em disco (conteúdo controlado byte a byte) e
        // executá-lo. O relançamento é INCONDICIONAL: se o instalador falhar, o
        // usuário fica com a versão antiga aberta em vez de ficar sem Hub, e a
        // saída do instalador vai pra taylorhub-update.log pra diagnóstico.
        let bat = std::env::temp_dir().join("taylorhub-update.cmd");
        let log = std::env::temp_dir().join("taylorhub-update.log");
        let script = format!(
            "@echo off\r\n\
             rem gerado pelo TaylorHub: instala a atualizacao e reabre o app\r\n\
             ping -n 4 127.0.0.1 >nul\r\n\
             \"{inst}\" /S >\"{log}\" 2>&1\r\n\
             start \"\" \"{hub}\"\r\n\
             del \"%~f0\"\r\n",
            inst = payload.display(),
            log = log.display(),
            hub = hub
        );
        fs::write(&bat, script).map_err(|e| format!("Falha ao gravar o script de update: {}", e))?;
        Command::new("cmd")
            .arg("/C")
            // raw_arg: caminho verbatim, sem o escaping incompatível com o cmd
            .raw_arg(format!("\"{}\"", bat.display()))
            .creation_flags(CREATE_NO_WINDOW)
            .spawn()
            .map_err(|e| format!("Falha ao agendar o instalador: {}", e))?;
        let handle = app.clone();
        std::thread::spawn(move || {
            std::thread::sleep(std::time::Duration::from_millis(600));
            handle.exit(0);
        });
        Ok("closing".into())
    }
    #[cfg(not(windows))]
    {
        use std::os::unix::fs::PermissionsExt;
        let target = std::env::var("APPIMAGE")
            .map_err(|_| "O Hub não está rodando como AppImage".to_string())?;
        let tmp = format!("{}.new", target);
        fs::copy(&payload, &tmp).map_err(|e| format!("Falha ao copiar: {}", e))?;
        let mut perms = fs::metadata(&tmp).map_err(|e| e.to_string())?.permissions();
        perms.set_mode(0o755);
        fs::set_permissions(&tmp, perms).map_err(|e| e.to_string())?;
        fs::rename(&tmp, &target).map_err(|e| format!("Falha ao trocar o AppImage: {}", e))?;
        let _ = fs::remove_file(&payload);
        Ok("restart".into())
    }
}

// ---------------------------------------------------------------------------
// Desinstalar / atalhos de menu
// ---------------------------------------------------------------------------

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(windows, allow(dead_code))]
pub struct UninstallSpec {
    id: String,
    name: String,
    exe: String,
    source: String,
}

#[cfg(windows)]
fn uninstall_os(spec: &UninstallSpec) -> Result<(), String> {
    use winreg::enums::{HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE};
    use winreg::RegKey;
    let hives: [(winreg::HKEY, &str); 3] = [
        (HKEY_CURRENT_USER, r"Software\Microsoft\Windows\CurrentVersion\Uninstall"),
        (HKEY_LOCAL_MACHINE, r"Software\Microsoft\Windows\CurrentVersion\Uninstall"),
        (HKEY_LOCAL_MACHINE, r"Software\WOW6432Node\Microsoft\Windows\CurrentVersion\Uninstall"),
    ];
    for (hive, path) in hives {
        let root = RegKey::predef(hive);
        let Ok(unin) = root.open_subkey(path) else { continue };
        for key in unin.enum_keys().flatten() {
            let Ok(sub) = unin.open_subkey(&key) else { continue };
            let dn: String = sub.get_value("DisplayName").unwrap_or_default();
            if dn != spec.name && !dn.starts_with(&format!("{} ", spec.name)) {
                continue;
            }
            let quiet: String = sub.get_value("QuietUninstallString").unwrap_or_default();
            let normal: String = sub.get_value("UninstallString").unwrap_or_default();
            let (cmdline, add_silent) =
                if !quiet.is_empty() { (quiet, false) } else { (normal, true) };
            if cmdline.is_empty() {
                return Err(format!("{}: sem UninstallString no registro", spec.name));
            }
            // "C:\...\uninstall.exe" [args] → separa programa e args.
            let (prog, rest) = if let Some(stripped) = cmdline.strip_prefix('"') {
                let end = stripped.find('"').unwrap_or(stripped.len());
                (stripped[..end].to_string(), stripped[end + 1..].trim().to_string())
            } else {
                match cmdline.split_once(' ') {
                    Some((p, r)) => (p.to_string(), r.trim().to_string()),
                    None => (cmdline.clone(), String::new()),
                }
            };
            let mut cmd = Command::new(&prog);
            if !rest.is_empty() {
                for a in rest.split_whitespace() {
                    cmd.arg(a);
                }
            }
            if add_silent {
                cmd.arg("/S");
            }
            let status = cmd.status().map_err(|e| format!("Falha no desinstalador: {}", e))?;
            if !status.success() {
                return Err(format!("Desinstalador saiu com código {:?}", status.code()));
            }
            // NSIS costuma se re-executar de um temp e devolver na hora — dá um respiro
            // antes do frontend re-detectar.
            std::thread::sleep(std::time::Duration::from_millis(2500));
            return Ok(());
        }
    }
    Err(format!("{}: não encontrado no registro", spec.name))
}

#[cfg(not(windows))]
fn uninstall_os(spec: &UninstallSpec) -> Result<(), String> {
    if spec.source == "deb" {
        return Err(format!(
            "{} foi instalado via .deb — remova com o gerenciador de pacotes (ex.: sudo apt remove).",
            spec.name
        ));
    }
    if !spec.exe.is_empty() && Path::new(&spec.exe).exists() {
        fs::remove_file(&spec.exe).map_err(|e| format!("Falha ao remover AppImage: {}", e))?;
    }
    if let Ok(home) = std::env::var("HOME") {
        let desktop = PathBuf::from(&home)
            .join(format!(".local/share/applications/taylor-{}.desktop", spec.id));
        let _ = fs::remove_file(desktop);
        if let Some(icon) = linux_icon_path(&spec.id) {
            let _ = fs::remove_file(icon);
        }
        let _ = Command::new("update-desktop-database")
            .arg(PathBuf::from(&home).join(".local/share/applications"))
            .status();
    }
    // Tira do installed.json
    let path = linux_installs_path();
    let mut installs: LinuxInstalls = read_json(&path).unwrap_or_default();
    installs.0.remove(&spec.id);
    write_json(&path, &installs)?;
    Ok(())
}

#[tauri::command]
async fn uninstall_app(spec: UninstallSpec) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || uninstall_os(&spec))
        .await
        .map_err(|e| format!("Falha na thread de desinstalação: {}", e))?
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(windows, allow(dead_code))]
pub struct ShortcutEntry {
    id: String,
    name: String,
    exe: String,
}

/// Linux: (re)cria as entradas de menu .desktop dos apps instalados.
/// No Windows é no-op (o instalador NSIS já cria os atalhos).
#[tauri::command]
fn recreate_shortcuts(entries: Vec<ShortcutEntry>) -> Vec<String> {
    #[cfg(windows)]
    {
        let _ = entries;
        Vec::new()
    }
    #[cfg(not(windows))]
    {
        let mut warnings = Vec::new();
        for e in &entries {
            if e.exe.is_empty() || !Path::new(&e.exe).exists() {
                warnings.push(format!("{}: executável não encontrado", e.name));
                continue;
            }
            if let Err(err) = write_desktop_entry(&e.id, &e.name, &e.exe) {
                warnings.push(format!("{}: {}", e.name, err));
            }
        }
        warnings
    }
}

// ---------------------------------------------------------------------------
// Abrir apps
// ---------------------------------------------------------------------------

fn spawn_detached(exe: &str, file: Option<&str>) -> Result<(), String> {
    let mut cmd = Command::new(exe);
    if let Some(f) = file {
        cmd.arg(f);
    }
    if let Some(dir) = Path::new(exe).parent() {
        cmd.current_dir(dir);
    }
    cmd.spawn().map_err(|e| format!("Falha ao abrir '{}': {}", exe, e))?;
    Ok(())
}

#[tauri::command]
fn launch_app(exe: String, file: Option<String>) -> Result<(), String> {
    spawn_detached(&exe, file.as_deref())
}

// ---------------------------------------------------------------------------
// Associações de arquivo + dispatcher
// ---------------------------------------------------------------------------

#[derive(Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct AssocEntry {
    ext: String,
    app_id: String,
    app_name: String,
    exe: String,
}

fn dispatch_path() -> PathBuf {
    config_dir().join("dispatch.json")
}

#[tauri::command]
fn read_dispatch() -> Vec<AssocEntry> {
    read_json(&dispatch_path()).unwrap_or_default()
}

/// MIME types conhecidos (Linux); extensão fora daqui vira application/x-taylor-<ext>.
#[cfg(not(windows))]
fn known_mime(ext: &str) -> Option<&'static str> {
    Some(match ext {
        "md" | "markdown" => "text/markdown",
        "txt" => "text/plain",
        "html" | "htm" => "text/html",
        "rtf" => "application/rtf",
        "docx" => "application/vnd.openxmlformats-officedocument.wordprocessingml.document",
        "odt" => "application/vnd.oasis.opendocument.text",
        "xlsx" => "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet",
        "csv" => "text/csv",
        "pptx" => "application/vnd.openxmlformats-officedocument.presentationml.presentation",
        "json" => "application/json",
        "js" => "text/javascript",
        "css" => "text/css",
        "yaml" | "yml" => "application/yaml",
        "toml" => "application/toml",
        _ => return None,
    })
}

fn hub_exe() -> String {
    // AppImage: o binário real fica num mount temporário; o caminho estável é $APPIMAGE.
    if let Ok(appimage) = std::env::var("APPIMAGE") {
        return appimage;
    }
    std::env::current_exe()
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_default()
}

#[cfg(windows)]
fn apply_assoc_os(entries: &[AssocEntry]) -> Vec<String> {
    use winreg::enums::HKEY_CURRENT_USER;
    use winreg::RegKey;
    let mut warnings = Vec::new();
    let hub = hub_exe();
    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    for entry in entries {
        let progid = format!("Taylor.{}", entry.ext);
        let res = (|| -> std::io::Result<()> {
            let (classes, _) = hkcu.create_subkey(r"Software\Classes")?;
            let (pk, _) = classes.create_subkey(&progid)?;
            pk.set_value("", &format!("{} ({})", entry.app_name, entry.ext))?;
            let (icon, _) = pk.create_subkey("DefaultIcon")?;
            icon.set_value("", &format!("{},0", entry.exe))?;
            let (cmd, _) = pk.create_subkey(r"shell\open\command")?;
            cmd.set_value("", &format!("\"{}\" --open \"%1\"", hub))?;
            let (extk, _) = classes.create_subkey(format!(".{}", entry.ext))?;
            extk.set_value("", &progid)?;
            Ok(())
        })();
        if let Err(e) = res {
            warnings.push(format!(".{}: {}", entry.ext, e));
        }
    }
    warnings
}

#[cfg(not(windows))]
fn apply_assoc_os(entries: &[AssocEntry]) -> Vec<String> {
    let mut warnings = Vec::new();
    let Ok(home) = std::env::var("HOME") else {
        return vec!["HOME não definido".into()];
    };
    let home = PathBuf::from(home);
    let share = home.join(".local/share");

    // 1. MIME custom pra extensões desconhecidas (.tslides, .tmind…)
    let custom: Vec<&AssocEntry> =
        entries.iter().filter(|e| known_mime(&e.ext).is_none()).collect();
    if !custom.is_empty() {
        let mut xml = String::from(
            "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<mime-info xmlns=\"http://www.freedesktop.org/standards/shared-mime-info\">\n",
        );
        for e in &custom {
            xml.push_str(&format!(
                "  <mime-type type=\"application/x-taylor-{ext}\">\n    <comment>{name} ({ext})</comment>\n    <glob pattern=\"*.{ext}\"/>\n  </mime-type>\n",
                ext = e.ext,
                name = e.app_name
            ));
        }
        xml.push_str("</mime-info>\n");
        let pkg_dir = share.join("mime/packages");
        if let Err(e) = ensure_dir(&pkg_dir)
            .and_then(|_| fs::write(pkg_dir.join("taylorhub.xml"), xml).map_err(|e| e.to_string()))
        {
            warnings.push(format!("mime xml: {}", e));
        }
        let _ = Command::new("update-mime-database").arg(share.join("mime")).status();
    }

    // 2. .desktop do dispatcher com todos os MimeTypes
    let mimes: Vec<String> = entries
        .iter()
        .map(|e| {
            known_mime(&e.ext)
                .map(|m| m.to_string())
                .unwrap_or_else(|| format!("application/x-taylor-{}", e.ext))
        })
        .collect();
    let desktop_dir = share.join("applications");
    let desktop = format!(
        "[Desktop Entry]\nType=Application\nName=TaylorHub (abrir arquivo)\nExec=\"{}\" --open %f\nNoDisplay=true\nTerminal=false\nMimeType={};\n",
        hub_exe(),
        mimes.join(";")
    );
    if let Err(e) = ensure_dir(&desktop_dir).and_then(|_| {
        fs::write(desktop_dir.join("taylorhub-open.desktop"), desktop).map_err(|e| e.to_string())
    }) {
        warnings.push(format!(".desktop: {}", e));
    }
    let _ = Command::new("update-desktop-database").arg(&desktop_dir).status();

    // 3. Definir como padrão por MIME
    for mime in &mimes {
        let ok = Command::new("xdg-mime")
            .args(["default", "taylorhub-open.desktop", mime])
            .status()
            .map(|s| s.success())
            .unwrap_or(false);
        if !ok {
            warnings.push(format!("xdg-mime falhou para {}", mime));
        }
    }
    warnings
}

#[tauri::command]
fn apply_associations(entries: Vec<AssocEntry>) -> Result<Vec<String>, String> {
    write_json(&dispatch_path(), &entries)?;
    Ok(apply_assoc_os(&entries))
}

// ---------------------------------------------------------------------------
// Recentes / favoritos (alimentado pelo dispatcher e pela UI)
// ---------------------------------------------------------------------------

#[derive(Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct RecentEntry {
    path: String,
    ts: u64,
    #[serde(default)]
    pinned: bool,
}

fn recents_path() -> PathBuf {
    config_dir().join("recents.json")
}

/// Registra um arquivo aberto: vai pro topo, preserva o "fixado",
/// mantém todos os fixados + até 40 não fixados.
fn log_recent(file: &str) {
    let mut items: Vec<RecentEntry> = read_json(&recents_path()).unwrap_or_default();
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let pinned = items.iter().find(|e| e.path == file).map(|e| e.pinned).unwrap_or(false);
    items.retain(|e| e.path != file);
    items.insert(0, RecentEntry { path: file.to_string(), ts: now, pinned });
    let mut unpinned = 0;
    items.retain(|e| {
        if e.pinned {
            true
        } else {
            unpinned += 1;
            unpinned <= 40
        }
    });
    let _ = write_json(&recents_path(), &items);
}

#[tauri::command]
fn read_recents() -> Vec<RecentEntry> {
    read_json(&recents_path()).unwrap_or_default()
}

#[tauri::command]
fn set_recent_pinned(path: String, pinned: bool) -> Result<Vec<RecentEntry>, String> {
    let mut items: Vec<RecentEntry> = read_json(&recents_path()).unwrap_or_default();
    for e in items.iter_mut() {
        if e.path == path {
            e.pinned = pinned;
        }
    }
    write_json(&recents_path(), &items)?;
    Ok(items)
}

#[tauri::command]
fn remove_recent(path: String) -> Result<Vec<RecentEntry>, String> {
    let mut items: Vec<RecentEntry> = read_json(&recents_path()).unwrap_or_default();
    items.retain(|e| e.path != path);
    write_json(&recents_path(), &items)?;
    Ok(items)
}

/// Limpa os não fixados; os fixados ficam.
#[tauri::command]
fn clear_recents() -> Result<Vec<RecentEntry>, String> {
    let mut items: Vec<RecentEntry> = read_json(&recents_path()).unwrap_or_default();
    items.retain(|e| e.pinned);
    write_json(&recents_path(), &items)?;
    Ok(items)
}

/// Abre um recente com o app resolvido pelo frontend (rotas + instalados)
/// e re-registra no topo da lista.
#[tauri::command]
fn open_recent(path: String, exe: String) -> Result<(), String> {
    if !Path::new(&path).exists() {
        return Err("O arquivo não existe mais nesse caminho.".into());
    }
    spawn_detached(&exe, Some(&path))?;
    log_recent(&path);
    Ok(())
}

/// Modo `taylorhub --open <arquivo>`: despacha pro app certo SEM abrir a UI.
/// Retorna true se conseguiu despachar (o processo então termina).
fn dispatch_open(file: &str) -> bool {
    let ext = Path::new(file)
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| e.to_lowercase())
        .unwrap_or_default();
    if ext.is_empty() {
        return false;
    }
    let entries: Vec<AssocEntry> = read_json(&dispatch_path()).unwrap_or_default();
    let Some(entry) = entries.iter().find(|e| e.ext == ext) else {
        return false;
    };
    if entry.exe.is_empty() || !Path::new(&entry.exe).exists() {
        return false;
    }
    let ok = spawn_detached(&entry.exe, Some(file)).is_ok();
    if ok {
        log_recent(file);
    }
    ok
}

// ---------------------------------------------------------------------------
// Entry point
// ---------------------------------------------------------------------------

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // Dispatcher: roda ANTES do Tauri (não abre janela, não engata single-instance).
    let args: Vec<String> = std::env::args().collect();
    if args.len() >= 3 && args[1] == "--open" {
        if dispatch_open(&args[2]) {
            return;
        }
        // Não conseguiu despachar (rota/app faltando) → abre o Hub normalmente.
    }

    let mut builder = tauri::Builder::default();
    #[cfg(desktop)]
    {
        builder = builder.plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            if let Some(win) = app.get_webview_window("main") {
                let _ = win.set_focus();
            }
        }));
    }
    builder
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![
            detect_apps,
            get_os,
            get_latest_release,
            get_icon,
            install_app,
            uninstall_app,
            update_self,
            recreate_shortcuts,
            launch_app,
            apply_associations,
            read_dispatch,
            read_recents,
            set_recent_pinned,
            remove_recent,
            clear_recents,
            open_recent
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

#[cfg(test)]
mod tests {
    use super::{
        cache_is_fresh, fetch_latest, glob_match, version_from_filename, CachedRelease,
        ReleaseCache, ReleaseInfo, RELEASE_CACHE_TTL_SECS,
    };

    /// Valida o caminho real de rede (reqwest+rustls → api.github.com) na máquina.
    /// `--ignored` pra não travar CI offline; rodar com: cargo test -- --ignored
    #[test]
    #[ignore]
    fn github_fetch_works() {
        // force=true: o teste valida o caminho de REDE — sem furar o TTL ele
        // responderia do cache e não testaria nada.
        let rel = tauri::async_runtime::block_on(fetch_latest("Anon5T4R/LocalOffice", true))
            .expect("fetch_latest falhou");
        assert!(rel.version.starts_with("0."), "versão inesperada: {}", rel.version);
        assert!(rel.assets.iter().any(|a| a.name.ends_with("-setup.exe")));
    }

    /// Fallback: API falha (aqui, 404 de repo inexistente) + cache vencido → devolve o cache.
    /// Mexe no releases_cache.json real da máquina; `--ignored` como o teste de rede.
    #[test]
    #[ignore]
    fn stale_cache_fallback_works() {
        let repo = "Anon5T4R/RepoQueNaoExiste12345";
        super::release_cache_put(
            repo,
            &ReleaseInfo { tag: "v9.9.9".into(), version: "9.9.9".into(), assets: vec![] },
        );
        // Vence a entrada na marra (fetched_at = 0).
        {
            let _guard =
                super::RELEASE_CACHE_LOCK.lock().unwrap_or_else(|e| e.into_inner());
            let path = super::release_cache_path();
            let mut cache: ReleaseCache = super::read_json(&path).unwrap();
            cache.0.get_mut(repo).unwrap().fetched_at = 0;
            super::write_json(&path, &cache).unwrap();
        }
        let rel = tauri::async_runtime::block_on(fetch_latest(repo, false))
            .expect("fallback pro cache vencido não funcionou");
        assert_eq!(rel.version, "9.9.9");
        // Limpa a entrada fake.
        {
            let _guard =
                super::RELEASE_CACHE_LOCK.lock().unwrap_or_else(|e| e.into_inner());
            let path = super::release_cache_path();
            let mut cache: ReleaseCache = super::read_json(&path).unwrap();
            cache.0.remove(repo);
            super::write_json(&path, &cache).unwrap();
        }
    }

    #[test]
    fn cache_freshness() {
        let entry = CachedRelease {
            fetched_at: 1000,
            release: ReleaseInfo { tag: "v1.0.0".into(), version: "1.0.0".into(), assets: vec![] },
        };
        assert!(cache_is_fresh(&entry, 1000));
        assert!(cache_is_fresh(&entry, 1000 + RELEASE_CACHE_TTL_SECS - 1));
        assert!(!cache_is_fresh(&entry, 1000 + RELEASE_CACHE_TTL_SECS));
        // Relógio andou pra trás (fetched_at no futuro) → trata como fresco, não estoura.
        assert!(cache_is_fresh(&entry, 500));
    }

    #[test]
    fn cache_roundtrip() {
        let mut cache = ReleaseCache::default();
        cache.0.insert(
            "Anon5T4R/LocalOffice".into(),
            CachedRelease {
                fetched_at: 42,
                release: ReleaseInfo {
                    tag: "v0.14.6".into(),
                    version: "0.14.6".into(),
                    assets: vec![super::AssetInfo {
                        name: "LocalOffice_0.14.6_x64-setup.exe".into(),
                        url: "https://example.com/x".into(),
                        size: 123,
                    }],
                },
            },
        );
        let text = serde_json::to_string(&cache).unwrap();
        let back: ReleaseCache = serde_json::from_str(&text).unwrap();
        let entry = &back.0["Anon5T4R/LocalOffice"];
        assert_eq!(entry.fetched_at, 42);
        assert_eq!(entry.release.version, "0.14.6");
        assert_eq!(entry.release.assets[0].size, 123);
    }

    #[test]
    fn version_from_names() {
        assert_eq!(version_from_filename("LocalOffice_0.14.6_amd64.AppImage").as_deref(), Some("0.14.6"));
        assert_eq!(version_from_filename("OpenObsidian-0.7.1.AppImage").as_deref(), Some("0.7.1"));
        assert_eq!(version_from_filename("TaylorMind-0.1.0.AppImage").as_deref(), Some("0.1.0"));
        assert_eq!(version_from_filename("LocalSheets.AppImage"), None);
        assert_eq!(version_from_filename("App2.AppImage"), None); // "2" sozinho não é versão
    }

    #[test]
    fn glob_basics() {
        assert!(glob_match("*_x64-setup.exe", "LocalOffice_0.14.6_x64-setup.exe"));
        assert!(glob_match("TaylorMind.Setup.*.exe", "TaylorMind.Setup.0.1.0.exe"));
        assert!(glob_match("*_amd64.AppImage", "LocalSheets_0.4.0_amd64.AppImage"));
        assert!(glob_match("OpenObsidian*.AppImage", "OpenObsidian-0.7.1.AppImage"));
        assert!(!glob_match("*_x64-setup.exe", "LocalOffice_0.14.6_amd64.AppImage"));
        assert!(!glob_match("TaylorMind.Setup.*.exe", "TaylorMind-0.1.0-portable.exe"));
        // Case-insensitive
        assert!(glob_match("*_X64-SETUP.EXE", "localoffice_0.1.0_x64-setup.exe"));
    }
}
