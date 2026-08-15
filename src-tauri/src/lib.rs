// Decisões sobre pacote de sistema no Linux (pacman/apt) — puras e testadas,
// sem `cfg`, pra o `cargo test` exercitá-las também no Windows. Ver pkg.rs.
mod pkg;

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

/// Apara o valor CRU de `InstallLocation`.
///
/// O NSIS do Tauri grava o caminho **entre aspas** — medido no registro desta
/// máquina em 2026-07-20, nos 28 apps da suíte. Sem aparar, o `join` monta
/// `"C:\...\LocalZip"\LocalZip.exe`, que nunca existe: a detecção então caía
/// **sempre** no fallback do `DisplayIcon` e funcionava por acidente, pelo
/// caminho errado.
fn clean_install_location(raw: &str) -> String {
    raw.trim().trim_matches('"').trim().to_string()
}

/// Apara o valor CRU de `DisplayIcon`.
///
/// Duas formas convivem no registro desta máquina:
/// - NSIS do Tauri (a suíte): `"C:\...\localzip.exe"` — entre aspas e **sem**
///   o sufixo `,0`;
/// - electron-builder (LocalMind): `C:\...\LocalMind.exe,0` — sem aspas e com
///   o índice do ícone.
///
/// Por isso o corte na vírgula só vale quando o caminho **não** está entre
/// aspas: dentro das aspas a vírgula é parte do caminho (pasta com vírgula no
/// nome é legal no Windows), e quem termina o caminho é o fecha-aspas.
fn clean_display_icon(raw: &str) -> String {
    let s = raw.trim();
    match s.strip_prefix('"') {
        Some(rest) => rest.split('"').next().unwrap_or("").to_string(),
        None => s.split(',').next().unwrap_or("").trim().to_string(),
    }
}

/// Resolve `(exe, location)` a partir dos valores **crus** do registro.
///
/// Pura de propósito: os casos difíceis (aspas, `,0`, `InstallLocation`
/// obsoleto depois de reinstalar) são exercitáveis sem mexer no registro da
/// máquina. Mesmo desenho do `siblings.rs` do LocalFiles.
///
/// Ordem: `InstallLocation` + nome do exe (o caso normal) → `DisplayIcon` (pra
/// instalador que não grava `InstallLocation`, como o electron-builder).
fn exe_and_location_from_registry_values<F>(
    install_location: &str,
    display_icon: &str,
    exe_name: &str,
    exists: F,
) -> (String, String)
where
    F: Fn(&Path) -> bool,
{
    let mut location = clean_install_location(install_location);
    if !location.is_empty() {
        let cand = Path::new(&location).join(exe_name);
        if exists(&cand) {
            return (cand.to_string_lossy().to_string(), location);
        }
    }
    let icon = clean_display_icon(display_icon);
    if !icon.is_empty() && exists(Path::new(&icon)) {
        if location.is_empty() {
            location =
                Path::new(&icon).parent().map(|p| p.to_string_lossy().to_string()).unwrap_or_default();
        }
        return (icon, location);
    }
    (String::new(), location)
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
            let raw_location: String = sub.get_value("InstallLocation").unwrap_or_default();
            let raw_icon: String = sub.get_value("DisplayIcon").unwrap_or_default();
            let (exe_path, location) = exe_and_location_from_registry_values(
                &raw_location,
                &raw_icon,
                &spec.exe,
                |p| p.exists(),
            );
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

/// Existe este programa no PATH? (`which`, sem depender de shell)
#[cfg(not(windows))]
fn has_bin(prog: &str) -> bool {
    Command::new("which")
        .arg(prog)
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

/// O gerenciador de pacotes desta máquina. A ESCOLHA é pura e testada
/// (`pkg::pick_manager`); aqui só se olha o que existe no disco.
#[cfg(not(windows))]
fn current_manager() -> pkg::PkgManager {
    pkg::pick_manager(has_bin("pacman"), has_bin("apt-get"))
}

/// Qual gerenciador o front deve considerar ao escolher o asset da release.
/// Vazio = nenhum; aí o caminho é o AppImage, como sempre foi.
#[tauri::command]
fn linux_pkg_manager() -> String {
    #[cfg(not(windows))]
    {
        current_manager().as_str().to_string()
    }
    #[cfg(windows)]
    {
        String::new()
    }
}

/// Procura o app no pacman (instalado via `.pkg.tar.zst`). Casa pelo nome
/// normalizado, a MESMA regra do dpkg — o pacote do Arch nasce do repack do
/// `.deb` e preserva o `Package:`, então uma regra serve as duas distros.
/// Retorna (versão, exe).
#[cfg(not(windows))]
fn pacman_detect(name: &str) -> Option<(String, String)> {
    let target = pkg::norm_name(name);
    // `pacman -Qq` lista só os nomes — barato. Perguntar direto por um nome
    // chutado não serviria: o nome do pacote ("taylor-hub") não é o nome do
    // catálogo ("TaylorHub"), e é justamente a normalização que os liga.
    let out = Command::new("pacman").args(["-Qq"]).output().ok()?;
    if !out.status.success() {
        return None;
    }
    let nomes = String::from_utf8_lossy(&out.stdout);
    let encontrado = nomes.lines().map(|l| l.trim()).find(|l| pkg::norm_name(l) == target)?;

    let q = Command::new("pacman").args(["-Q", encontrado]).output().ok()?;
    let version = pkg::parse_pacman_query(&String::from_utf8_lossy(&q.stdout))?;

    // O executável: primeiro arquivo do pacote em /usr/bin. Mesmo critério do
    // dpkg logo abaixo.
    let exe = Command::new("pacman")
        .args(["-Ql", encontrado])
        .output()
        .ok()
        .and_then(|o| {
            String::from_utf8_lossy(&o.stdout)
                .lines()
                // "taylor-hub /usr/bin/hub" → segunda coluna
                .filter_map(|l| l.split_whitespace().nth(1))
                .find(|p| p.starts_with("/usr/bin/") && !p.ends_with('/'))
                .map(|s| s.to_string())
        })
        .unwrap_or_default();
    Some((version, exe))
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
    // 3) Pacote de SISTEMA. Desde a v0.24 o Hub não só detecta: ele instala e
    //    remove por `pkexec` (ver `pkg.rs`). A ordem entre pacman e dpkg segue a
    //    mesma regra do `pick_manager` — pacman primeiro, porque máquina com os
    //    dois é Arch.
    if let Some((version, exe)) = pacman_detect(&spec.name) {
        return InstalledInfo {
            id: spec.id.clone(),
            installed: true,
            version,
            location: String::new(),
            exe,
            source: "pacman".into(),
        };
    }
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

// ---------------------------------------------------------------------------
// Token do GitHub (opcional) — sobe o rate limit de 60/h (anônimo) pra 5000/h
// ---------------------------------------------------------------------------

const KEYRING_SERVICE: &str = "TaylorHub";
const KEYRING_USER: &str = "github-token";

/// Token guardado no cofre do SO (DPAPI/Secret Service) — nunca em arquivo.
fn github_token() -> Option<String> {
    let entry = keyring::Entry::new(KEYRING_SERVICE, KEYRING_USER).ok()?;
    entry.get_password().ok().filter(|t| !t.is_empty())
}

/// Anexa o Authorization quando há token. O reqwest remove o header sozinho
/// em redirects pra outro host (ex.: download que pula pro S3 do GitHub).
fn with_auth(req: reqwest::RequestBuilder) -> reqwest::RequestBuilder {
    match github_token() {
        Some(t) => req.header("Authorization", format!("Bearer {t}")),
        None => req,
    }
}

#[tauri::command]
fn github_token_status() -> bool {
    github_token().is_some()
}

/// OAuth App "TaylorHub" (conta Anon5T4R) com device flow habilitado.
/// Client ID é público por natureza — não há secret no device flow.
/// Vazio = build sem login pelo navegador; o front cai no fallback de abrir
/// a página de criação de token pré-preenchida.
const GITHUB_CLIENT_ID: &str = "Ov23lirGYsjc99nYBvh3";

#[tauri::command]
fn github_client_configured() -> bool {
    !GITHUB_CLIENT_ID.is_empty()
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DeviceStart {
    user_code: String,
    verification_uri: String,
    device_code: String,
    interval: u64,
    expires_in: u64,
}

/// Passo 1 do device flow: pede o código que o usuário digita no navegador.
#[tauri::command]
async fn github_device_start() -> Result<DeviceStart, String> {
    if GITHUB_CLIENT_ID.is_empty() {
        return Err("Login pelo navegador não configurado neste build".into());
    }
    let resp = http_client()?
        .post("https://github.com/login/device/code")
        .header("Accept", "application/json")
        .form(&[("client_id", GITHUB_CLIENT_ID), ("scope", "")])
        .send()
        .await
        .map_err(|e| format!("Falha de rede: {e}"))?;
    let v: serde_json::Value = resp.json().await.map_err(|e| e.to_string())?;
    let get = |k: &str| v[k].as_str().unwrap_or("").to_string();
    let device_code = get("device_code");
    if device_code.is_empty() {
        return Err(format!("GitHub não iniciou o login: {v}"));
    }
    Ok(DeviceStart {
        user_code: get("user_code"),
        verification_uri: get("verification_uri"),
        device_code,
        interval: v["interval"].as_u64().unwrap_or(5),
        expires_in: v["expires_in"].as_u64().unwrap_or(900),
    })
}

/// Passo 2: espera o usuário autorizar no navegador; guarda o token no cofre.
/// Devolve o limite de requisições/hora da conta.
#[tauri::command]
async fn github_device_poll(
    device_code: String,
    interval: u64,
    expires_in: u64,
) -> Result<u64, String> {
    let deadline = now_secs() + expires_in.clamp(60, 1800);
    let mut wait = interval.max(5);
    loop {
        if now_secs() > deadline {
            return Err("Tempo esgotado — tente de novo".into());
        }
        tokio::time::sleep(std::time::Duration::from_secs(wait)).await;
        let resp = http_client()?
            .post("https://github.com/login/oauth/access_token")
            .header("Accept", "application/json")
            .form(&[
                ("client_id", GITHUB_CLIENT_ID),
                ("device_code", device_code.as_str()),
                ("grant_type", "urn:ietf:params:oauth:grant-type:device_code"),
            ])
            .send()
            .await
            .map_err(|e| format!("Falha de rede: {e}"))?;
        let v: serde_json::Value = resp.json().await.map_err(|e| e.to_string())?;
        if let Some(tok) = v["access_token"].as_str() {
            keyring::Entry::new(KEYRING_SERVICE, KEYRING_USER)
                .and_then(|e| e.set_password(tok))
                .map_err(|e| format!("Falha ao guardar no cofre do sistema: {e}"))?;
            // limite real da conta (melhor esforço — o token já está salvo)
            let limit = async {
                let r = http_client()
                    .ok()?
                    .get("https://api.github.com/rate_limit")
                    .header("Accept", "application/vnd.github+json")
                    .header("Authorization", format!("Bearer {tok}"))
                    .send()
                    .await
                    .ok()?;
                let b: serde_json::Value = r.json().await.ok()?;
                b["resources"]["core"]["limit"].as_u64()
            }
            .await
            .unwrap_or(5000);
            return Ok(limit);
        }
        match v["error"].as_str() {
            Some("authorization_pending") | None => {}
            Some("slow_down") => wait += 5,
            Some("expired_token") => return Err("O código expirou — tente de novo".into()),
            Some("access_denied") => return Err("Login cancelado no GitHub".into()),
            Some(e) => return Err(format!("GitHub: {e}")),
        }
    }
}

/// Salva o token (validando em /rate_limit) ou remove (string vazia).
/// Devolve o limite de requisições/hora que o token concede.
#[tauri::command]
async fn set_github_token(token: String) -> Result<u64, String> {
    let token = token.trim().to_string();
    let entry = keyring::Entry::new(KEYRING_SERVICE, KEYRING_USER)
        .map_err(|e| format!("Cofre do sistema indisponível: {e}"))?;
    if token.is_empty() {
        let _ = entry.delete_credential();
        return Ok(0);
    }
    let resp = http_client()?
        .get("https://api.github.com/rate_limit")
        .header("Accept", "application/vnd.github+json")
        .header("Authorization", format!("Bearer {token}"))
        .send()
        .await
        .map_err(|e| format!("Falha de rede: {e}"))?;
    if !resp.status().is_success() {
        return Err(format!("O GitHub recusou o token ({})", resp.status()));
    }
    let body: serde_json::Value = resp.json().await.map_err(|e| e.to_string())?;
    let limit = body["resources"]["core"]["limit"].as_u64().unwrap_or(5000);
    entry
        .set_password(&token)
        .map_err(|e| format!("Falha ao guardar no cofre do sistema: {e}"))?;
    Ok(limit)
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
    let resp = with_auth(
        http_client()?
            .get(&url)
            .header("Accept", "application/vnd.github+json"),
    )
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
    // `strip_prefix` e nao `trim_start_matches`: o segundo remove TODOS os `v`
    // iniciais, entao uma tag `vv1.0` viraria `1.0` em vez de `v1.0`.
    let version = tag.strip_prefix('v').unwrap_or(&tag).to_string();
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

fn downloads_dir() -> PathBuf {
    config_dir().join("downloads")
}

/// Limpa o cache de downloads: além do payload já usado (removido por quem
/// chamou), tira qualquer outro arquivo "esquecido" — instalação anterior
/// interrompida, ou o auto-update do próprio Hub, que fecha o processo antes
/// de conseguir rodar o `remove_file` (corrigido em `update_self`, mas isso
/// aqui pega o que já tinha acumulado / o que ainda escapar). Nada nessa
/// pasta deveria sobreviver mais que alguns minutos, então qualquer arquivo
/// com mais de 10 min é considerado lixo.
fn sweep_stale_downloads(skip: &Path) {
    let Ok(rd) = fs::read_dir(downloads_dir()) else { return };
    let cutoff = std::time::SystemTime::now() - std::time::Duration::from_secs(600);
    for entry in rd.flatten() {
        let path = entry.path();
        if path == skip {
            continue;
        }
        let stale = entry.metadata().and_then(|m| m.modified()).map(|t| t < cutoff).unwrap_or(true);
        if stale {
            let _ = fs::remove_file(&path);
        }
    }
}

async fn download_asset(
    app: &tauri::AppHandle,
    id: &str,
    asset: &AssetInfo,
) -> Result<PathBuf, String> {
    use futures_util::StreamExt;
    let dir = downloads_dir();
    ensure_dir(&dir)?;
    let dest = dir.join(&asset.name);
    let resp = with_auth(http_client()?.get(&asset.url))
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

/// Roda um comando de gerenciador de pacotes (já montado por `pkg.rs`).
///
/// Sem `pkexec` na máquina não há como pedir privilégio por janela — e aí a
/// regra da casa vale: **dizer o que não deu e como fazer à mão** em vez de
/// falhar com "erro ao instalar". Polkit não é garantido (servidor, WM
/// minimalista), então este caminho não é hipótese remota.
#[cfg(not(windows))]
fn run_pkg_cmd(cmd: &[String]) -> Result<(), String> {
    let manual = pkg::as_manual_command(cmd);
    if !has_bin("pkexec") {
        return Err(format!(
            "Esta máquina não tem `pkexec` (polkit), então o Hub não consegue pedir a senha \
             de administrador. Rode você mesmo num terminal:\n\n{}",
            manual
        ));
    }
    let status = Command::new(&cmd[0])
        .args(&cmd[1..])
        .status()
        .map_err(|e| format!("Falha ao executar `{}`: {}", cmd[0], e))?;
    if status.success() {
        return Ok(());
    }
    // 126/127 do pkexec = o usuário cancelou o diálogo ou a autorização falhou.
    // Vale distinguir: "cancelei" não é erro do app, e mandar o usuário
    // investigar um erro que ele mesmo causou é ruído.
    match status.code() {
        Some(126) | Some(127) => Err("Autorização cancelada ou negada.".to_string()),
        code => Err(format!(
            "O gerenciador de pacotes saiu com código {:?}. Pra ver a mensagem dele, rode:\n\n{}",
            code, manual
        )),
    }
}

/// Onde o payload baixado foi parar. `Managed` = quem instalou foi o
/// gerenciador de pacotes, e não há caminho NOSSO pra anotar no
/// `installed.json` — quem responde "está instalado?" a partir daí é o
/// `pacman_detect`/`dpkg_detect`, que leem o próprio gerenciador. Anotar um
/// caminho inventado ali seria criar uma segunda fonte da verdade que
/// envelhece sozinha (o usuário remove por fora e o Hub segue jurando que está
/// instalado).
#[cfg(not(windows))]
enum Installed {
    AppImage(PathBuf),
    Managed,
}

#[cfg(not(windows))]
fn install_payload(
    spec: &InstallSpec,
    payload: &Path,
    icon: Option<&[u8]>,
) -> Result<Installed, String> {
    // Pacote de SISTEMA: entrega pro gerenciador e sai da frente. Ele cuida do
    // binário, do `.desktop` e do ícone nos caminhos do sistema — por isso não
    // se escreve nada em `~/.local` aqui.
    let nome = payload.file_name().map(|s| s.to_string_lossy().to_lowercase()).unwrap_or_default();
    if nome.ends_with(".pkg.tar.zst") || nome.ends_with(".deb") {
        let m = current_manager();
        let cmd = pkg::install_cmd(m, &payload.to_string_lossy()).ok_or_else(|| {
            format!(
                "Baixei um pacote de sistema ({}) mas esta máquina não tem pacman nem apt.",
                nome
            )
        })?;
        run_pkg_cmd(&cmd)?;
        return Ok(Installed::Managed);
    }

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
    Ok(Installed::AppImage(dest))
}

#[tauri::command]
async fn install_app(app: tauri::AppHandle, spec: InstallSpec) -> Result<InstalledInfo, String> {
    let release = fetch_latest(&spec.repo, false).await?;

    // No Linux, PREFERIR o pacote nativo da distro quando a release o tiver —
    // e cair no que o catálogo pediu (o AppImage) quando não tiver. A escolha
    // mora aqui, e não no front, por dois motivos: é aqui que se conhece a
    // lista real de assets da release (o front só tem o glob), e é aqui que se
    // sabe qual gerenciador a máquina tem. Ordem e porquê em `pkg::asset_globs_in_order`.
    #[cfg(not(windows))]
    let globs = pkg::asset_globs_in_order(current_manager(), &spec.asset_pattern);
    #[cfg(windows)]
    let globs = vec![spec.asset_pattern.clone()];

    let asset = globs
        .iter()
        .find_map(|g| release.assets.iter().find(|a| glob_match(g, &a.name)))
        .cloned()
        .ok_or_else(|| {
            format!(
                "Nenhum asset da release {} de {} casa com {}",
                release.tag,
                spec.repo,
                globs.iter().map(|g| format!("'{}'", g)).collect::<Vec<_>>().join(" nem ")
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
            match install_payload(&spec, &payload, icon_bytes.as_deref())? {
                // AppImage é nosso: só o `installed.json` sabe dele.
                Installed::AppImage(dest) => {
                    let path = linux_installs_path();
                    let mut installs: LinuxInstalls = read_json(&path).unwrap_or_default();
                    installs.0.insert(
                        spec.id.clone(),
                        LinuxInstall {
                            version: version.clone(),
                            path: dest.to_string_lossy().to_string(),
                        },
                    );
                    write_json(&path, &installs)?;
                }
                // Pacote de sistema: NÃO entra no `installed.json`. Quem sabe
                // é o gerenciador, e ele é a fonte da verdade — inclusive
                // quando o usuário remover por fora do Hub (`pacman -R` no
                // terminal), caso em que uma anotação nossa continuaria
                // jurando que está instalado.
                Installed::Managed => {}
            }
        }
        let _ = fs::remove_file(&payload);
        sweep_stale_downloads(&payload);

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
// Repositórios do usuário — apps de fora do catálogo, pelo link do GitHub
// ---------------------------------------------------------------------------

#[derive(Serialize, Deserialize, Clone, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct CustomApp {
    id: String,
    /// Começa como o nome do repo; vira o DisplayName real do registro
    /// depois da primeira instalação (diff de chaves de Uninstall).
    name: String,
    repo: String,
    win_asset: String,
    linux_asset: String,
    /// Nome do executável (descoberto no diff pós-instalação).
    exe: String,
}

fn custom_apps_path() -> PathBuf {
    config_dir().join("custom_apps.json")
}

fn load_custom_apps() -> Vec<CustomApp> {
    read_json(&custom_apps_path()).unwrap_or_default()
}

fn save_custom_apps(list: &Vec<CustomApp>) -> Result<(), String> {
    write_json(&custom_apps_path(), list)
}

/// Aceita "owner/repo", URL completa do GitHub ou variações com .git no fim.
fn parse_repo(input: &str) -> Option<String> {
    let s = input.trim();
    let s = s
        .strip_prefix("https://")
        .or_else(|| s.strip_prefix("http://"))
        .unwrap_or(s);
    let s = s.strip_prefix("www.").unwrap_or(s);
    let s = s.strip_prefix("github.com/").unwrap_or(s);
    let s = s.trim_matches('/').trim_end_matches(".git");
    let parts: Vec<&str> = s.split('/').collect();
    if parts.len() != 2 {
        return None;
    }
    let ok = |p: &str| {
        !p.is_empty()
            && p.chars()
                .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.'))
    };
    (ok(parts[0]) && ok(parts[1])).then(|| format!("{}/{}", parts[0], parts[1]))
}

/// Detecta os assets instaláveis da release (glob mais específico primeiro).
fn guess_assets(assets: &[AssetInfo]) -> (String, String) {
    let pick = |patterns: &[&str]| -> String {
        patterns
            .iter()
            .find(|p| assets.iter().any(|a| glob_match(p, &a.name)))
            .map(|p| p.to_string())
            .unwrap_or_default()
    };
    let win = pick(&["*x64-setup.exe", "*setup*.exe", "*install*.exe", "*.exe"]);
    let linux = pick(&["*amd64.appimage", "*x86_64.appimage", "*.appimage"]);
    (win, linux)
}

#[tauri::command]
async fn add_custom_repo(input: String) -> Result<CustomApp, String> {
    let repo = parse_repo(&input)
        .ok_or("Endereço inválido — cole o link do repositório (github.com/dono/repo)")?;
    let mut list = load_custom_apps();
    if list.iter().any(|c| c.repo.eq_ignore_ascii_case(&repo)) {
        return Err("Esse repositório já está na lista".into());
    }
    let release = fetch_latest(&repo, true).await?;
    let (win_asset, linux_asset) = guess_assets(&release.assets);
    if win_asset.is_empty() && linux_asset.is_empty() {
        return Err(format!(
            "A release {} de {} não tem instalador .exe nem AppImage nos assets",
            release.tag, repo
        ));
    }
    let slug: String = repo
        .to_lowercase()
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '-' })
        .collect();
    let app = CustomApp {
        id: format!("custom-{slug}"),
        name: repo.split('/').nth(1).unwrap_or(&repo).to_string(),
        repo,
        win_asset,
        linux_asset,
        exe: String::new(),
    };
    list.push(app.clone());
    save_custom_apps(&list)?;
    Ok(app)
}

#[tauri::command]
fn list_custom_repos() -> Vec<CustomApp> {
    load_custom_apps()
}

/// Tira da lista do Hub (NÃO desinstala o app).
#[tauri::command]
fn remove_custom_repo(id: String) -> Result<(), String> {
    let mut list = load_custom_apps();
    list.retain(|c| c.id != id);
    save_custom_apps(&list)
}

/// Chaves de Uninstall existentes, com prefixo do hive (pro diff pós-instalação).
#[cfg(windows)]
fn uninstall_keys() -> std::collections::HashSet<String> {
    use winreg::enums::{HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE};
    use winreg::RegKey;
    let hives: [(winreg::HKEY, &str, &str); 3] = [
        (HKEY_CURRENT_USER, r"Software\Microsoft\Windows\CurrentVersion\Uninstall", "hkcu"),
        (HKEY_LOCAL_MACHINE, r"Software\Microsoft\Windows\CurrentVersion\Uninstall", "hklm"),
        (
            HKEY_LOCAL_MACHINE,
            r"Software\WOW6432Node\Microsoft\Windows\CurrentVersion\Uninstall",
            "wow",
        ),
    ];
    let mut out = std::collections::HashSet::new();
    for (hive, path, tag) in hives {
        let root = RegKey::predef(hive);
        if let Ok(unin) = root.open_subkey(path) {
            for key in unin.enum_keys().flatten() {
                out.insert(format!("{tag}\\{key}"));
            }
        }
    }
    out
}

/// Lê (DisplayName, DisplayVersion, exe, InstallLocation) de uma chave taggeada.
#[cfg(windows)]
fn read_uninstall_entry(tagged: &str) -> Option<(String, String, String, String)> {
    use winreg::enums::{HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE};
    use winreg::RegKey;
    let (tag, key) = tagged.split_once('\\')?;
    let (hive, path) = match tag {
        "hkcu" => (HKEY_CURRENT_USER, r"Software\Microsoft\Windows\CurrentVersion\Uninstall"),
        "hklm" => (HKEY_LOCAL_MACHINE, r"Software\Microsoft\Windows\CurrentVersion\Uninstall"),
        _ => (
            HKEY_LOCAL_MACHINE,
            r"Software\WOW6432Node\Microsoft\Windows\CurrentVersion\Uninstall",
        ),
    };
    let sub = RegKey::predef(hive).open_subkey(path).ok()?.open_subkey(key).ok()?;
    let name: String = sub.get_value("DisplayName").unwrap_or_default();
    if name.is_empty() {
        return None;
    }
    let version: String = sub.get_value("DisplayVersion").unwrap_or_default();
    let raw_location: String = sub.get_value("InstallLocation").unwrap_or_default();
    let mut location = clean_install_location(&raw_location);
    let raw_icon: String = sub.get_value("DisplayIcon").unwrap_or_default();
    let icon = clean_display_icon(&raw_icon);
    let mut exe = String::new();
    if !icon.is_empty() && icon.to_lowercase().ends_with(".exe") && Path::new(&icon).exists() {
        exe = icon.clone();
        if location.is_empty() {
            location = Path::new(&icon)
                .parent()
                .map(|p| p.to_string_lossy().to_string())
                .unwrap_or_default();
        }
    } else if !location.is_empty() {
        // Melhor esforço: o primeiro .exe da pasta que não é o desinstalador.
        if let Ok(rd) = fs::read_dir(&location) {
            for e in rd.flatten() {
                let n = e.file_name().to_string_lossy().to_lowercase();
                if n.ends_with(".exe") && !n.contains("unin") {
                    exe = e.path().to_string_lossy().to_string();
                    break;
                }
            }
        }
    }
    Some((name, version, exe, location))
}

#[tauri::command]
async fn install_custom_app(app: tauri::AppHandle, id: String) -> Result<InstalledInfo, String> {
    let mut list = load_custom_apps();
    let idx = list
        .iter()
        .position(|c| c.id == id)
        .ok_or("Repositório não está na lista")?;
    let c = list[idx].clone();

    let pattern = if cfg!(windows) { c.win_asset.clone() } else { c.linux_asset.clone() };
    if pattern.is_empty() {
        return Err("A release desse repositório não tem asset pra esta plataforma".into());
    }
    let release = fetch_latest(&c.repo, false).await?;
    let asset = release
        .assets
        .iter()
        .find(|a| glob_match(&pattern, &a.name))
        .cloned()
        .ok_or_else(|| format!("Nenhum asset da release {} casa com '{}'", release.tag, pattern))?;
    let payload = download_asset(&app, &c.id, &asset).await?;
    let _ = app.emit(
        "hub-progress",
        Progress { id: c.id.clone(), phase: "install".into(), done: 0, total: 0 },
    );

    let version = release.version.clone();
    let (info, updated) = tauri::async_runtime::spawn_blocking(
        move || -> Result<(InstalledInfo, CustomApp), String> {
            let mut c = c;
            let spec = InstallSpec {
                id: c.id.clone(),
                name: c.name.clone(),
                repo: c.repo.clone(),
                asset_pattern: String::new(),
                silent_args: vec!["/S".into()],
                exe: c.exe.clone(),
                current_path: None,
                icon_url: None,
            };
            #[cfg(windows)]
            {
                let before = uninstall_keys();
                install_payload(&spec, &payload)?;
                let _ = fs::remove_file(&payload);
                sweep_stale_downloads(&payload);
                // O que apareceu de novo no registro é o app recém-instalado.
                let after = uninstall_keys();
                let found = after
                    .difference(&before)
                    .filter_map(|k| read_uninstall_entry(k))
                    .next();
                let (name, ver, exe, location) = match found {
                    Some(f) => f,
                    None => {
                        // Instalador só atualizou uma chave existente: busca por nome.
                        let d = detect_one(&DetectSpec {
                            id: c.id.clone(),
                            name: c.name.clone(),
                            exe: c.exe.clone(),
                        });
                        if d.installed {
                            (c.name.clone(), d.version, d.exe, d.location)
                        } else {
                            (c.name.clone(), version.clone(), String::new(), String::new())
                        }
                    }
                };
                c.name = name;
                if let Some(f) = Path::new(&exe).file_name() {
                    c.exe = f.to_string_lossy().to_string();
                }
                Ok((
                    InstalledInfo {
                        id: c.id.clone(),
                        installed: true,
                        version: if ver.is_empty() { version } else { ver },
                        location,
                        exe,
                        source: "registry".into(),
                    },
                    c,
                ))
            }
            #[cfg(not(windows))]
            {
                // Repositório do usuário no Linux é AppImage por construção: o
                // `guess_assets` só procura `*.appimage`. O `match` existe pra
                // essa premissa ser explícita — se um dia o `guess_assets`
                // aprender `.deb`/`.pkg.tar.zst`, este braço é o lembrete de
                // que o caminho do card fora do catálogo também precisa saber
                // o que fazer com um pacote de sistema (que não tem caminho
                // nosso pra anotar no `installed.json`).
                let dest = match install_payload(&spec, &payload, None)? {
                    Installed::AppImage(p) => p,
                    Installed::Managed => {
                        return Err(
                            "Repositórios adicionados por link ainda só instalam AppImage no \
                             Linux. Instale este pacote pelo gerenciador da sua distro."
                                .into(),
                        )
                    }
                };
                let path = linux_installs_path();
                let mut installs: LinuxInstalls = read_json(&path).unwrap_or_default();
                installs.0.insert(
                    c.id.clone(),
                    LinuxInstall {
                        version: version.clone(),
                        path: dest.to_string_lossy().to_string(),
                    },
                );
                write_json(&path, &installs)?;
                let _ = fs::remove_file(&payload);
                sweep_stale_downloads(&payload);
                Ok((
                    InstalledInfo {
                        id: c.id.clone(),
                        installed: true,
                        version,
                        location: dest
                            .parent()
                            .map(|p| p.to_string_lossy().to_string())
                            .unwrap_or_default(),
                        exe: dest.to_string_lossy().to_string(),
                        source: "hub".into(),
                    },
                    c,
                ))
            }
        },
    )
    .await
    .map_err(|e| format!("Falha na thread de instalação: {e}"))??;

    list[idx] = updated;
    save_custom_apps(&list)?;
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

    // No Linux, o MESMO critério do `install_app`: pacote nativo primeiro,
    // AppImage como queda. Sem isto o Hub instalado por pacman só tinha o
    // AppImage pra baixar — e trocar o binário por baixo do gerenciador não é
    // opção, então a atualização simplesmente não acontecia.
    #[cfg(not(windows))]
    let globs = pkg::asset_globs_in_order(current_manager(), &spec.asset_pattern);
    #[cfg(windows)]
    let globs = vec![spec.asset_pattern.clone()];

    let asset = globs
        .iter()
        .find_map(|g| release.assets.iter().find(|a| glob_match(g, &a.name)))
        .cloned()
        .ok_or_else(|| {
            format!(
                "Nenhum asset da release {} casa com {}",
                release.tag,
                globs.iter().map(|g| format!("'{}'", g)).collect::<Vec<_>>().join(" nem ")
            )
        })?;
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
        // O `del` do instalador baixado é NECESSÁRIO aqui, não cosmético: como o
        // Hub sai (`exit`) antes do script rodar, o `remove_file` normal do
        // `download_asset` nunca executa pra esse download — sem essa linha o
        // instalador de CADA update do Hub ficava pra sempre em downloads/
        // (achado real: 20+ instaladores antigos, 1+ GB acumulado).
        let bat = std::env::temp_dir().join("taylorhub-update.cmd");
        let log = std::env::temp_dir().join("taylorhub-update.log");
        let script = format!(
            "@echo off\r\n\
             rem gerado pelo TaylorHub: instala a atualizacao e reabre o app\r\n\
             ping -n 4 127.0.0.1 >nul\r\n\
             \"{inst}\" /S >\"{log}\" 2>&1\r\n\
             del \"{inst}\"\r\n\
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
        // Instalado como PACOTE DE SISTEMA: quem troca o binário é o próprio
        // gerenciador, por `pkexec` — a mesma porta que o Hub já usa pra
        // instalar os outros apps.
        //
        // Antes daqui a atualização era RECUSADA neste caso, e o motivo estava
        // certo: sobrescrever o arquivo por baixo do pacman/dpkg deixaria o
        // gerenciador achando que ainda é a versão antiga. O erro era a
        // conclusão — a saída não é recusar, é passar pelo gerenciador, que é
        // exatamente o que mantém a informação dele correta.
        let nome_payload = payload
            .file_name()
            .map(|s| s.to_string_lossy().to_lowercase())
            .unwrap_or_default();
        if nome_payload.ends_with(".pkg.tar.zst") || nome_payload.ends_with(".deb") {
            let m = current_manager();
            let cmd = pkg::install_cmd(m, &payload.to_string_lossy()).ok_or_else(|| {
                format!("Baixei um pacote de sistema ({}) mas esta máquina não tem pacman nem apt.", nome_payload)
            })?;
            run_pkg_cmd(&cmd)?;
            let _ = fs::remove_file(&payload);
            return Ok("restart".into());
        }

        // Sem `$APPIMAGE` o Hub não está rodando de um AppImage nem de pacote
        // de sistema (o caso acima), então não há o que atualizar no lugar.
        let target = std::env::var("APPIMAGE").map_err(|_| {
            let m = current_manager();
            if m == pkg::PkgManager::None {
                "O Hub não está rodando como AppImage, então não há o que atualizar no lugar."
                    .to_string()
            } else {
                format!(
                    "O Hub foi instalado como pacote de sistema. Baixe a versão nova da página \
                     de releases e instale com `sudo {} <arquivo>` — atualizar por baixo do \
                     gerenciador deixaria ele com a informação errada.",
                    match m {
                        pkg::PkgManager::Pacman => "pacman -U",
                        _ => "apt install ./",
                    }
                )
            }
        })?;
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

/// O nome do PACOTE deste app no gerenciador — descoberto perguntando ao
/// próprio gerenciador, nunca montado a partir do nome do app.
///
/// É a peça de segurança da desinstalação. O que vai pro `pacman -R` tem que
/// ser um nome que o gerenciador JÁ LISTA como instalado; derivar
/// "TaylorHub" → "taylor-hub" por regra seria adivinhar, e adivinhar do lado
/// errado aqui significa remover software que não é nosso. Por isso a busca é
/// "ache, entre os pacotes instalados, aquele cujo nome normalizado bate" — se
/// não achar, não há remoção, e o erro diz isso.
#[cfg(not(windows))]
fn installed_pkg_name(m: pkg::PkgManager, app_name: &str) -> Option<String> {
    let alvo = pkg::norm_name(app_name);
    let out = match m {
        pkg::PkgManager::Pacman => Command::new("pacman").args(["-Qq"]).output().ok()?,
        pkg::PkgManager::Apt => Command::new("dpkg-query")
            .args(["-W", "-f", "${Package}\\n"])
            .output()
            .ok()?,
        pkg::PkgManager::None => return None,
    };
    if !out.status.success() {
        return None;
    }
    String::from_utf8_lossy(&out.stdout)
        .lines()
        .map(|l| l.trim())
        .find(|l| !l.is_empty() && pkg::norm_name(l) == alvo)
        .map(|s| s.to_string())
}

#[cfg(not(windows))]
fn uninstall_os(spec: &UninstallSpec) -> Result<(), String> {
    // Instalado como pacote de sistema: quem remove é o gerenciador, por
    // `pkexec`. Até a v0.23 o Hub só sabia dizer "remova você mesmo" aqui.
    if spec.source == "deb" || spec.source == "pacman" {
        let m = current_manager();
        if m == pkg::PkgManager::None {
            return Err(format!(
                "{} está instalado como pacote de sistema, mas não achei pacman nem apt \
                 nesta máquina pra removê-lo.",
                spec.name
            ));
        }
        let nome = installed_pkg_name(m, &spec.name).ok_or_else(|| {
            format!(
                "Não achei um pacote instalado com o nome de {} no {}. \
                 Por segurança o Hub não remove um pacote que ele não conseguiu confirmar.",
                spec.name,
                m.as_str()
            )
        })?;
        let cmd = pkg::remove_cmd(m, &nome)
            .ok_or_else(|| format!("Nome de pacote recusado por segurança: {:?}", nome))?;
        return run_pkg_cmd(&cmd);
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
    tauri::async_runtime::spawn_blocking(move || {
        uninstall_os(&spec)?;
        // Best-effort: o app já foi removido com sucesso: uma falha aqui não
        // deve virar erro pro usuário, só deixar pra próxima Limpeza profunda.
        prune_dispatch_for_app(&spec.id);
        Ok(())
    })
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
///
/// `remove` traz os ids que NÃO devem ter entrada nossa — na prática, os apps
/// instalados como pacote de sistema, que já trazem o `.desktop` deles. Sem
/// isso o menu fica com duas entradas do mesmo app, e a nossa aparece com
/// ícone genérico. Limpar aqui é o que conserta quem já ficou com a duplicata
/// de versões anteriores; só parar de criar não desfaz o que existe.
#[tauri::command]
fn recreate_shortcuts(entries: Vec<ShortcutEntry>, remove: Vec<String>) -> Vec<String> {
    #[cfg(windows)]
    {
        let _ = (entries, remove);
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
        for id in &remove {
            let _ = remove_desktop_entry(id);
        }
        warnings
    }
}

/// Apaga a entrada de menu que o Hub criou para `id`, se existir.
#[cfg(not(windows))]
fn remove_desktop_entry(id: &str) -> Result<(), String> {
    let home = PathBuf::from(std::env::var("HOME").map_err(|_| "HOME não definido".to_string())?);
    let dir = home.join(".local/share/applications");
    let f = dir.join(format!("taylor-{}.desktop", id));
    if f.exists() {
        fs::remove_file(&f).map_err(|e| e.to_string())?;
        let _ = Command::new("update-desktop-database").arg(&dir).status();
    }
    Ok(())
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

/// Divide as entradas em (mantidas, removidas) pelo predicado — usado tanto
/// pra podar as rotas de um app desinstalado quanto pra achar órfãs em geral.
fn partition_dispatch(
    entries: Vec<AssocEntry>,
    mut drop_if: impl FnMut(&AssocEntry) -> bool,
) -> (Vec<AssocEntry>, Vec<AssocEntry>) {
    let mut kept = Vec::new();
    let mut removed = Vec::new();
    for e in entries {
        if drop_if(&e) {
            removed.push(e);
        } else {
            kept.push(e);
        }
    }
    (kept, removed)
}

/// Rota órfã: o app dono não está mais onde o dispatch.json diz que estava
/// (desinstalado por fora do fluxo que já poda, ou o `dispatch.json` é de
/// antes dessa poda existir). `exists` é injetado pra dar pra testar sem tocar
/// o disco de verdade.
fn orphan_routes(
    entries: Vec<AssocEntry>,
    exists: impl Fn(&Path) -> bool,
) -> (Vec<AssocEntry>, Vec<AssocEntry>) {
    partition_dispatch(entries, |e| e.exe.is_empty() || !exists(Path::new(&e.exe)))
}

/// Poda as rotas do app da lista + a chave de registro que elas criaram.
/// Chamado depois de um `uninstall_os` bem-sucedido — antes dessa poda, a
/// extensão continuava apontando pro Hub como handler padrão do Windows pra
/// sempre, mesmo com o app removido (achado real: `.py`/`.go` do LocalCode).
fn prune_dispatch_for_app(app_id: &str) {
    let entries: Vec<AssocEntry> = read_json(&dispatch_path()).unwrap_or_default();
    let (kept, removed) = partition_dispatch(entries, |e| e.app_id == app_id);
    if removed.is_empty() {
        return;
    }
    let _ = write_json(&dispatch_path(), &kept);
    let exts: Vec<String> = removed.into_iter().map(|e| e.ext).collect();
    let _ = remove_progids_os(&exts);
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

/// Tira do registro a `ProgID` que o Hub criou pra cada extensão da lista.
/// Só apaga `.ext` se ele ainda apontar pro `Taylor.ext` do Hub — se o usuário
/// reassociou por fora (Explorer, outro instalador), essa troca não é nossa
/// pra desfazer.
#[cfg(windows)]
fn remove_progids_os(exts: &[String]) -> Vec<String> {
    use winreg::enums::HKEY_CURRENT_USER;
    use winreg::RegKey;
    let mut warnings = Vec::new();
    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    let Ok(classes) = hkcu.open_subkey(r"Software\Classes") else { return warnings };
    for ext in exts {
        let progid = format!("Taylor.{}", ext);
        let ext_key = format!(".{}", ext);
        if let Ok(extk) = classes.open_subkey(&ext_key) {
            let current: String = extk.get_value("").unwrap_or_default();
            if current == progid {
                if let Err(e) = classes.delete_subkey_all(&ext_key) {
                    warnings.push(format!(".{}: {}", ext, e));
                }
            }
        }
        if let Err(e) = classes.delete_subkey_all(&progid) {
            if e.kind() != std::io::ErrorKind::NotFound {
                warnings.push(format!("{}: {}", progid, e));
            }
        }
    }
    warnings
}

/// Linux: as associações são globais por MIME (xdg-mime), sem registro por
/// extensão pra apagar — quem chama já regenera o `.desktop`/mime.xml com a
/// lista atualizada via `apply_assoc_os`, o que já deixa de ofertar os tipos
/// removidos como padrão.
#[cfg(not(windows))]
fn remove_progids_os(_exts: &[String]) -> Vec<String> {
    Vec::new()
}

#[tauri::command]
fn apply_associations(entries: Vec<AssocEntry>) -> Result<Vec<String>, String> {
    // Extensão que EXISTIA na rotina salva e não está na nova lista (usuário
    // trocou a rota, ou o app dela foi desinstalado) — sem isso, o `.ext`
    // ficava apontando pro Hub pra sempre mesmo perdendo a rota.
    let old: Vec<AssocEntry> = read_json(&dispatch_path()).unwrap_or_default();
    let new_exts: std::collections::HashSet<&str> = entries.iter().map(|e| e.ext.as_str()).collect();
    let removed_exts: Vec<String> =
        old.into_iter().map(|e| e.ext).filter(|ext| !new_exts.contains(ext.as_str())).collect();
    if !removed_exts.is_empty() {
        let _ = remove_progids_os(&removed_exts);
    }
    write_json(&dispatch_path(), &entries)?;
    Ok(apply_assoc_os(&entries))
}

// ---------------------------------------------------------------------------
// Limpeza profunda — pra quem instalou/desinstalou apps ANTES dessas correções
// existirem e ficou com rota órfã, `ProgID` pendurada ou pasta de dados de um
// app que já não está mais lá. `uninstall_app`/`apply_associations` evitam
// que a bagunça se repita daqui pra frente; isto aqui varre e resolve a que
// já existe.
// ---------------------------------------------------------------------------

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CacheInfo {
    count: u64,
    bytes: u64,
}

fn dir_files_info(dir: &Path) -> CacheInfo {
    let mut count = 0u64;
    let mut bytes = 0u64;
    if let Ok(rd) = fs::read_dir(dir) {
        for entry in rd.flatten() {
            if let Ok(meta) = entry.metadata() {
                if meta.is_file() {
                    count += 1;
                    bytes += meta.len();
                }
            }
        }
    }
    CacheInfo { count, bytes }
}

/// Cache de instaladores baixados (`downloads/`). Em uso normal fica vazio —
/// tudo que sobrevive ali é resto de um download anterior.
#[tauri::command]
fn scan_downloads_cache() -> CacheInfo {
    dir_files_info(&downloads_dir())
}

#[tauri::command]
fn clean_downloads_cache() -> u64 {
    let mut freed = 0u64;
    if let Ok(rd) = fs::read_dir(downloads_dir()) {
        for entry in rd.flatten() {
            if let Ok(meta) = entry.metadata() {
                if meta.is_file() {
                    freed += meta.len();
                    let _ = fs::remove_file(entry.path());
                }
            }
        }
    }
    freed
}

/// Rotas do dispatch.json cujo app não existe mais no caminho gravado —
/// sobrevivência de uninstalls feitos antes da poda automática existir.
#[tauri::command]
fn scan_orphan_routes() -> Vec<AssocEntry> {
    let entries: Vec<AssocEntry> = read_json(&dispatch_path()).unwrap_or_default();
    orphan_routes(entries, |p| p.exists()).1
}

/// Remove as rotas órfãs (dispatch.json) + a `ProgID`/`.ext` que elas tinham
/// no registro. Devolve as entradas removidas, pra UI relatar o que sumiu.
#[tauri::command]
fn clean_orphan_routes() -> Vec<AssocEntry> {
    let entries: Vec<AssocEntry> = read_json(&dispatch_path()).unwrap_or_default();
    let (kept, removed) = orphan_routes(entries, |p| p.exists());
    if removed.is_empty() {
        return removed;
    }
    let _ = write_json(&dispatch_path(), &kept);
    let exts: Vec<String> = removed.iter().map(|e| e.ext.clone()).collect();
    let _ = remove_progids_os(&exts);
    removed
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LeftoverDir {
    path: String,
    bytes: u64,
}

fn dir_size(path: &Path) -> u64 {
    let mut total = 0u64;
    let Ok(rd) = fs::read_dir(path) else { return 0 };
    for entry in rd.flatten() {
        let p = entry.path();
        match entry.metadata() {
            Ok(meta) if meta.is_dir() => total += dir_size(&p),
            Ok(meta) => total += meta.len(),
            Err(_) => {}
        }
    }
    total
}

/// O frontend manda os candidatos (nome de cada app do catálogo que não está
/// instalado agora, nos formatos de pasta que a suíte usa — Tauri e
/// electron-builder); aqui só confere quais existem de verdade e o tamanho.
#[tauri::command]
fn scan_leftover_dirs(paths: Vec<String>) -> Vec<LeftoverDir> {
    paths
        .into_iter()
        .filter_map(|p| {
            let path = PathBuf::from(&p);
            if path.is_dir() {
                Some(LeftoverDir { bytes: dir_size(&path), path: p })
            } else {
                None
            }
        })
        .collect()
}

/// Só compara texto (case-insensitive) contra as bases — o Windows não
/// distingue maiúsculas no caminho, e comparar por `Path` component a
/// component teria o mesmo custo por pouco ganho aqui.
fn path_is_within_bases(path: &Path, bases: &[String]) -> bool {
    let target = path.to_string_lossy().to_lowercase();
    bases.iter().any(|base| {
        let base = base.to_lowercase();
        !base.is_empty() && target.starts_with(&base) && target.len() > base.len()
    })
}

/// Trava de segurança pro `delete_leftover_dir`: só deixa apagar dentro do
/// AppData do usuário (Local ou Roaming), nunca um caminho arbitrário — o
/// candidato vem do frontend, mas quem decide se é seguro apagar é aqui.
fn is_safe_to_delete(path: &Path) -> bool {
    let bases: Vec<String> =
        ["LOCALAPPDATA", "APPDATA"].iter().filter_map(|v| std::env::var(v).ok()).collect();
    path_is_within_bases(path, &bases)
}

#[tauri::command]
fn delete_leftover_dir(path: String) -> Result<u64, String> {
    let p = PathBuf::from(&path);
    if !is_safe_to_delete(&p) {
        return Err(format!("Caminho fora do AppData do usuário, recusado: {}", path));
    }
    if !p.is_dir() {
        return Ok(0);
    }
    let bytes = dir_size(&p);
    fs::remove_dir_all(&p).map_err(|e| format!("Falha ao remover '{}': {}", path, e))?;
    Ok(bytes)
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
    // ── Contorno da tela branca do webkit: REMOVIDO, e o porquê importa ──────
    //
    // Este bloco desligava o renderer DMABUF, desligava o compositing e forçava
    // XWayland, porque o webkit2gtk pintava a janela inteira de branco em
    // Arch/GNOME. Era mitigação às cegas — o comentário dizia "branco é pior que
    // lento" — e custava a aceleração do WebView.
    //
    // A CAUSA foi encontrada em 26/07/2026 e é de EMPACOTAMENTO, não de código:
    // o AppDir do AppImage levava `libwayland-*` do Ubuntu do CI, que brigavam
    // com o Mesa do host e derrubavam o EGL (`EGL_BAD_PARAMETER`). Corrigido em
    // `Anon5T4R/linux-packaging`: as libs que falam com driver/compositor agora
    // vêm do host, e o pacote nativo (pacman/apt) usa o webkit do sistema.
    // Tratar o sintoma deixou de fazer sentido.
    //
    // Remover o forçamento NÃO tira a saída de emergência: estas variáveis são
    // lidas pelo próprio webkitgtk, não por este código. Se a tela branca voltar
    // em alguma combinação de driver, rodar com
    // `WEBKIT_DISABLE_DMABUF_RENDERER=1` continua funcionando — e aí é sinal de
    // que sobrou lib de host em algum AppDir, que é onde se deve olhar.

    // Dispatcher: roda ANTES do Tauri (não abre janela, não engata single-instance).
    let args: Vec<String> = std::env::args().collect();
    if args.len() >= 3 && args[1] == "--open" {
        if dispatch_open(&args[2]) {
            return;
        }
        // Não conseguiu despachar (rota/app faltando) → abre o Hub normalmente.
    }

    let mut builder = tauri::Builder::default()
        .on_window_event(|window, event| {
            // Bug do tao <= 0.35 no GNOME/Wayland: botões da titlebar (min/
            // max/fechar) mortos até um resize (tauri#13440, tauri#11856). O
            // toggle de `resizable` em cada foco força o GTK a revalidar as
            // decorações, restaurando o estado original em seguida. Remover
            // quando o tauri puxar o tao 0.36 (via wry 0.56).
            #[cfg(target_os = "linux")]
            if let tauri::WindowEvent::Focused(true) = event {
                let r = window.is_resizable().unwrap_or(true);
                let _ = window.set_resizable(!r);
                let _ = window.set_resizable(r);
            }
            #[cfg(not(target_os = "linux"))]
            let _ = (window, event);
        });
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
            linux_pkg_manager,
            get_os,
            get_latest_release,
            get_icon,
            install_app,
            uninstall_app,
            update_self,
            github_token_status,
            set_github_token,
            github_client_configured,
            github_device_start,
            github_device_poll,
            add_custom_repo,
            list_custom_repos,
            remove_custom_repo,
            install_custom_app,
            recreate_shortcuts,
            launch_app,
            apply_associations,
            read_dispatch,
            read_recents,
            set_recent_pinned,
            remove_recent,
            clear_recents,
            open_recent,
            scan_downloads_cache,
            clean_downloads_cache,
            scan_orphan_routes,
            clean_orphan_routes,
            scan_leftover_dirs,
            delete_leftover_dir
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

#[cfg(test)]
mod tests {
    use super::{
        cache_is_fresh, clean_display_icon, clean_install_location,
        exe_and_location_from_registry_values, fetch_latest, glob_match, guess_assets,
        orphan_routes, parse_repo, partition_dispatch, path_is_within_bases, version_from_filename,
        AssetInfo, AssocEntry, CachedRelease, ReleaseCache, ReleaseInfo, RELEASE_CACHE_TTL_SECS,
    };
    use std::path::{Path, PathBuf};

    /// `exists` de mentira: só os caminhos listados "existem".
    fn only(paths: &[&str]) -> impl Fn(&Path) -> bool {
        let owned: Vec<PathBuf> = paths.iter().map(PathBuf::from).collect();
        move |p: &Path| owned.iter().any(|x| x == p)
    }

    /// Caminho montado com o separador DA PLATAFORMA.
    ///
    /// Literal `r"C:\..."` em teste passa no Windows e quebra no job Ubuntu do
    /// CI: no Linux isso é UM componente só, então `join`/`parent` não fazem o
    /// esperado. A lógica testada aqui (tirar aspas, cortar no `,`, juntar,
    /// subir um nível) é neutra de plataforma e merece rodar nas duas.
    fn p(parts: &[&str]) -> String {
        let mut b = PathBuf::new();
        for x in parts {
            b.push(x);
        }
        b.to_string_lossy().into_owned()
    }

    /// O caso que estava QUEBRADO: `InstallLocation` entre aspas.
    ///
    /// Sem aparar, o `join` monta um caminho que nunca existe e a detecção cai
    /// no `DisplayIcon` — funcionava, mas pelo caminho errado, e o `location`
    /// devolvido pra UI saía com as aspas dentro.
    #[test]
    fn install_location_entre_aspas_e_aparado() {
        let dir = p(&["base", "LocalZip"]);
        let exe = p(&["base", "LocalZip", "LocalZip.exe"]);
        let (achado, loc) = exe_and_location_from_registry_values(
            &format!("\"{dir}\""),
            "",
            "LocalZip.exe",
            only(&[&exe]),
        );
        assert_eq!(achado, exe, "as aspas do InstallLocation têm que sair");
        assert_eq!(loc, dir, "o location que vai pra UI não pode levar aspas");
    }

    #[test]
    fn sem_install_location_cai_no_display_icon() {
        let exe = p(&["prog", "LocalMind", "LocalMind.exe"]);
        let dir = p(&["prog", "LocalMind"]);
        let (achado, loc) = exe_and_location_from_registry_values(
            "",
            &format!("{exe},0"),
            "LocalMind.exe",
            only(&[&exe]),
        );
        assert_eq!(achado, exe);
        assert_eq!(loc, dir, "sem InstallLocation, o location vem da pasta do ícone");
    }

    #[test]
    fn install_location_obsoleto_nao_ganha_do_display_icon() {
        // Usuário reinstalou noutro lugar: a chave aponta pra pasta que não
        // existe mais. Aceitar sem conferir daria "programa não encontrado".
        let antigo = p(&["antigo", "LocalZip"]);
        let novo = p(&["novo", "LocalZip.exe"]);
        let (achado, _) = exe_and_location_from_registry_values(
            &antigo,
            &format!("{novo},0"),
            "LocalZip.exe",
            only(&[&novo]),
        );
        assert_eq!(achado, novo);
    }

    #[test]
    fn nada_encontrado_devolve_exe_vazio_e_nao_um_caminho_falso() {
        let (exe, _) = exe_and_location_from_registry_values(
            &p(&["x"]),
            &p(&["y", "a.exe"]),
            "a.exe",
            |_| false,
        );
        assert_eq!(exe, "");
        let (exe, loc) = exe_and_location_from_registry_values("", "", "a.exe", |_| true);
        assert_eq!((exe.as_str(), loc.as_str()), ("", ""));
        let (exe, loc) = exe_and_location_from_registry_values("  ", "  ", "a.exe", |_| true);
        assert_eq!((exe.as_str(), loc.as_str()), ("", ""));
    }

    #[test]
    fn display_icon_so_com_o_indice_nao_vira_caminho_vazio() {
        let (exe, _) = exe_and_location_from_registry_values("", ",0", "a.exe", |_| true);
        assert_eq!(exe, "");
    }

    #[test]
    fn virgula_dentro_das_aspas_e_parte_do_caminho() {
        // Pasta com vírgula no nome é legal no Windows. Cortar na vírgula
        // ANTES de olhar as aspas mutilaria o caminho.
        assert_eq!(clean_display_icon("\"C:\\Rock, Paper\\app.exe\""), "C:\\Rock, Paper\\app.exe");
        // Sem aspas, a vírgula é o índice do ícone (formato electron-builder).
        assert_eq!(clean_display_icon("C:\\App\\app.exe,0"), "C:\\App\\app.exe");
        assert_eq!(clean_install_location("  \"C:\\App\"  "), "C:\\App");
        assert_eq!(clean_install_location(""), "");
    }

    // Só no Windows: aqui os literais com barra invertida SÃO o objeto do
    // teste (é o texto que o registro do Windows guarda), então neutralizá-los
    // perderia o sentido.
    #[cfg(windows)]
    #[test]
    fn valores_reais_do_registro_desta_maquina() {
        // COPIADOS do registro em 2026-07-20 (HKCU\...\Uninstall). Foi por
        // escrever o teste de cabeça que o bug passou despercebido: os dois
        // detalhes que só o dado real mostra são o `InstallLocation` ENTRE
        // ASPAS e o `DisplayIcon` entre aspas e SEM o `,0`.
        let loc = r#""C:\Users\Hades\AppData\Local\LocalZip""#;
        let icon = r#""C:\Users\Hades\AppData\Local\LocalZip\localzip.exe""#;
        let real = r"C:\Users\Hades\AppData\Local\LocalZip\LocalZip.exe";
        let (exe, location) =
            exe_and_location_from_registry_values(loc, icon, "LocalZip.exe", only(&[real]));
        assert_eq!(exe, real, "o InstallLocation real tem que resolver sozinho");
        assert_eq!(location, r"C:\Users\Hades\AppData\Local\LocalZip");

        // E o formato do electron-builder (LocalMind, também copiado do
        // registro real): sem InstallLocation e com `,0`.
        let mind = r"C:\Users\Hades\AppData\Local\Programs\LocalMind\LocalMind.exe";
        let (exe, location) = exe_and_location_from_registry_values(
            "",
            r"C:\Users\Hades\AppData\Local\Programs\LocalMind\LocalMind.exe,0",
            "LocalMind.exe",
            only(&[mind]),
        );
        assert_eq!(exe, mind);
        assert_eq!(location, r"C:\Users\Hades\AppData\Local\Programs\LocalMind");
    }

    #[test]
    fn parse_repo_aceita_url_e_owner_repo() {
        for input in [
            "Anon5T4R/LocalZIM",
            "https://github.com/Anon5T4R/LocalZIM",
            "http://www.github.com/Anon5T4R/LocalZIM/",
            "github.com/Anon5T4R/LocalZIM.git",
        ] {
            assert_eq!(parse_repo(input).as_deref(), Some("Anon5T4R/LocalZIM"), "{input}");
        }
        for input in ["", "LocalZIM", "a/b/c", "github.com/só/inválido!", "https://gitlab.com"] {
            assert_eq!(parse_repo(input), None, "{input}");
        }
    }

    #[test]
    fn guess_assets_prefere_o_glob_mais_especifico() {
        let mk = |names: &[&str]| -> Vec<AssetInfo> {
            names
                .iter()
                .map(|n| AssetInfo { name: n.to_string(), url: String::new(), size: 0 })
                .collect()
        };
        let assets = mk(&["App_1.0.0_x64-setup.exe", "App_1.0.0_amd64.AppImage", "app.tar.gz"]);
        let (win, linux) = guess_assets(&assets);
        assert_eq!(win, "*x64-setup.exe");
        assert_eq!(linux, "*amd64.appimage");

        let (win, linux) = guess_assets(&mk(&["Foo-Setup-2.1.exe"]));
        assert_eq!(win, "*setup*.exe");
        assert_eq!(linux, "");

        let (win, linux) = guess_assets(&mk(&["portable.zip"]));
        assert_eq!(win, "");
        assert_eq!(linux, "");
    }

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

    fn entry(ext: &str, app_id: &str, exe: &str) -> AssocEntry {
        AssocEntry { ext: ext.into(), app_id: app_id.into(), app_name: app_id.into(), exe: exe.into() }
    }

    #[test]
    fn poda_por_app_ignora_as_rotas_de_outros_apps() {
        let entries = vec![
            entry("py", "code", "x"),
            entry("go", "code", "y"),
            entry("md", "writer", "z"),
        ];
        let (kept, removed) = partition_dispatch(entries, |e| e.app_id == "code");
        assert_eq!(kept.len(), 1, "só a rota do writer sobrevive");
        assert_eq!(kept[0].app_id, "writer");
        assert_eq!(removed.len(), 2, "as duas rotas do code somem");
        assert!(removed.iter().all(|e| e.app_id == "code"));
    }

    #[test]
    fn orfa_e_a_que_o_exe_sumiu_do_disco() {
        // Caso real: LocalCode foi desinstalado, mas o dispatch.json ainda
        // apontava pro exe dele — enquanto LocalOffice segue instalado.
        let writer_exe = "C:/LocalOffice/writer.exe";
        let entries = vec![entry("py", "code", "C:/LocalCode/code.exe"), entry("md", "writer", writer_exe)];
        let (kept, removed) = orphan_routes(entries, only(&[writer_exe]));
        assert_eq!(kept.len(), 1, "a rota do app que ainda existe fica");
        assert_eq!(kept[0].ext, "md");
        assert_eq!(removed.len(), 1, "a do exe que sumiu vai pra fora");
        assert_eq!(removed[0].ext, "py");
    }

    #[test]
    fn rota_com_exe_vazio_tambem_e_orfa() {
        let entries = vec![entry("tdraw", "draw", "")];
        let (kept, removed) = orphan_routes(entries, |_| true);
        assert!(kept.is_empty());
        assert_eq!(removed.len(), 1);
    }

    #[test]
    fn leftover_dir_so_e_seguro_dentro_do_appdata() {
        let bases = vec![r"C:\Users\Hades\AppData\Local".to_string()];
        assert!(
            path_is_within_bases(Path::new(r"C:\Users\Hades\AppData\Local\LocalCode"), &bases),
            "pasta de app dentro do LOCALAPPDATA é o caso de uso normal"
        );
        assert!(
            path_is_within_bases(Path::new(r"c:\users\hades\appdata\local\LocalCode"), &bases),
            "o Windows não distingue maiúsculas no caminho"
        );
        assert!(
            !path_is_within_bases(Path::new(r"C:\Users\Hades\AppData\Local"), &bases),
            "a raiz do LOCALAPPDATA em si nunca pode ser o alvo"
        );
        assert!(
            !path_is_within_bases(Path::new(r"C:\Windows\System32"), &bases),
            "caminho fora do AppData nunca é seguro, mesmo vindo de outro comando"
        );
    }
}
