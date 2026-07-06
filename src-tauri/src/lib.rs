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
            };
        }
    }
    InstalledInfo { id: spec.id.clone(), ..Default::default() }
}

#[cfg(not(windows))]
fn detect_one(spec: &DetectSpec) -> InstalledInfo {
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
            };
        }
    }
    let _ = &spec.exe; // exe só é usado no Windows
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

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ReleaseInfo {
    tag: String,
    version: String,
    assets: Vec<AssetInfo>,
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct AssetInfo {
    name: String,
    url: String,
    size: u64,
}

fn http_client() -> Result<reqwest::Client, String> {
    reqwest::Client::builder()
        .user_agent("TaylorHub")
        .build()
        .map_err(|e| e.to_string())
}

async fn fetch_latest(repo: &str) -> Result<ReleaseInfo, String> {
    let url = format!("https://api.github.com/repos/{}/releases/latest", repo);
    let resp = http_client()?
        .get(&url)
        .header("Accept", "application/vnd.github+json")
        .send()
        .await
        .map_err(|e| format!("Falha de rede em {}: {}", repo, e))?;
    if !resp.status().is_success() {
        return Err(format!("GitHub respondeu {} para {}", resp.status(), repo));
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
async fn get_latest_release(repo: String) -> Result<ReleaseInfo, String> {
    fetch_latest(&repo).await
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
pub struct InstallSpec {
    id: String,
    name: String,
    repo: String,
    /// Glob do asset por plataforma (já resolvido pelo frontend p/ o SO atual).
    asset_pattern: String,
    /// Args de instalação silenciosa no Windows (ex.: ["/S"]).
    silent_args: Vec<String>,
    exe: String,
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
fn install_payload(spec: &InstallSpec, payload: &Path) -> Result<(), String> {
    // Linux: AppImage → ~/Applications/<Name>.AppImage + .desktop + installed.json
    use std::os::unix::fs::PermissionsExt;
    let home = PathBuf::from(std::env::var("HOME").map_err(|_| "HOME não definido".to_string())?);
    let apps_dir = home.join("Applications");
    ensure_dir(&apps_dir)?;
    let dest = apps_dir.join(format!("{}.AppImage", spec.name));
    fs::copy(payload, &dest).map_err(|e| format!("Falha ao copiar AppImage: {}", e))?;
    let mut perms = fs::metadata(&dest).map_err(|e| e.to_string())?.permissions();
    perms.set_mode(0o755);
    fs::set_permissions(&dest, perms).map_err(|e| e.to_string())?;

    // Entrada de menu
    let desktop_dir = home.join(".local/share/applications");
    ensure_dir(&desktop_dir)?;
    let desktop = format!(
        "[Desktop Entry]\nType=Application\nName={}\nExec=\"{}\" %f\nTerminal=false\nCategories=Office;\n",
        spec.name,
        dest.display()
    );
    fs::write(desktop_dir.join(format!("taylor-{}.desktop", spec.id)), desktop)
        .map_err(|e| e.to_string())?;
    let _ = Command::new("update-desktop-database").arg(&desktop_dir).status();
    Ok(())
}

#[tauri::command]
async fn install_app(app: tauri::AppHandle, spec: InstallSpec) -> Result<InstalledInfo, String> {
    let release = fetch_latest(&spec.repo).await?;
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

    // Instalação roda processo bloqueante → thread própria.
    let version = release.version.clone();
    let info = tauri::async_runtime::spawn_blocking(move || -> Result<InstalledInfo, String> {
        install_payload(&spec, &payload)?;
        let _ = fs::remove_file(&payload);

        #[cfg(not(windows))]
        {
            // Registrar no installed.json (Linux não tem registro).
            let path = linux_installs_path();
            let mut installs: LinuxInstalls = read_json(&path).unwrap_or_default();
            let home = std::env::var("HOME").unwrap_or_default();
            let dest = format!("{}/Applications/{}.AppImage", home, spec.name);
            installs
                .0
                .insert(spec.id.clone(), LinuxInstall { version: version.clone(), path: dest });
            write_json(&path, &installs)?;
        }

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
            })
        }
    })
    .await
    .map_err(|e| format!("Falha na thread de instalação: {}", e))??;

    Ok(info)
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
    spawn_detached(&entry.exe, Some(file)).is_ok()
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
            install_app,
            launch_app,
            apply_associations,
            read_dispatch
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

#[cfg(test)]
mod tests {
    use super::glob_match;

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
