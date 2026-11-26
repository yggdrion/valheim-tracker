//! Valheim Tracker Sync: watches the Valheim characters folder and uploads changed .fch files raw to a
//! valheim-tracker server (POST /upload?name=X, X-Token header), plus a heartbeat every 30 s for unchanged ones.
//! Same protocol as fchsync.sh. It never parses a save and never writes to the game folder.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use tauri::{AppHandle, Emitter, Manager, State};
use tauri_plugin_autostart::{MacosLauncher, ManagerExt};
use tauri_plugin_dialog::DialogExt;
use tauri_plugin_opener::OpenerExt;

const TICK: Duration = Duration::from_secs(2);
const HEARTBEAT: Duration = Duration::from_secs(30);
const SETTLE: Duration = Duration::from_millis(700); // the game writes .fch.new, then renames

#[derive(Serialize, Deserialize, Clone, Default)]
#[serde(default)]
struct Settings {
    server: String,
    token: String,
    /// empty = detect automatically
    dir: String,
    /// characters NOT to sync; new characters are synced by default
    excluded: Vec<String>,
    paused: bool,
}

#[derive(Serialize, Clone, Default)]
struct CharState {
    name: String,
    selected: bool,
    /// idle | uploaded | rejected | unreachable | error | paused | off
    status: String,
    message: String,
    last_upload: Option<u64>,
    last_contact: Option<u64>,
    modified: Option<u64>,
}

struct Track {
    sig: Option<(u64, u64)>,
    beat: Option<Instant>,
    st: CharState,
}

struct Inner {
    settings: Settings,
    config: PathBuf,
    tracks: HashMap<String, Track>,
    dir: Option<PathBuf>,
    candidates: Vec<String>,
    server: String, // last server check, shown in the UI
    force: bool,
}

type Shared = Arc<Mutex<Inner>>;

#[derive(Serialize, Clone)]
struct Status {
    settings: Settings,
    dir: String,
    dir_auto: bool,
    candidates: Vec<String>,
    server: String,
    chars: Vec<CharState>,
}

fn now() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0)
}

// ---- finding the characters ------------------------------------------------------------------

fn char_name(p: &Path) -> Option<String> {
    let f = p.file_name()?.to_str()?;
    let stem = f.strip_suffix(".fch")?; // skips .fch.old / .fch.new
    if stem.is_empty() || stem.contains("_backup_") {
        return None;
    }
    Some(stem.to_string())
}

fn list_chars(dir: &Path) -> Vec<(String, PathBuf)> {
    let mut v: Vec<_> = fs::read_dir(dir)
        .map(|rd| rd.flatten().filter_map(|e| char_name(&e.path()).map(|n| (n, e.path()))).collect())
        .unwrap_or_default();
    v.sort_by(|a, b| a.0.to_lowercase().cmp(&b.0.to_lowercase()));
    v
}

fn steam_userdata(steam: &Path, out: &mut Vec<PathBuf>) {
    if let Ok(rd) = fs::read_dir(steam.join("userdata")) {
        for e in rd.flatten() {
            out.push(e.path().join("892970").join("remote").join("characters"));
        }
    }
}

/// Folders with at least one character, most likely first (local saves, then Steam Cloud, then Proton).
fn candidates() -> Vec<PathBuf> {
    let home = dirs::home_dir().unwrap_or_default();
    let mut v: Vec<PathBuf> = Vec::new();
    #[cfg(target_os = "windows")]
    {
        let ll = home.join("AppData").join("LocalLow").join("IronGate").join("Valheim");
        v.push(ll.join("characters_local"));
        v.push(ll.join("characters"));
        for base in [r"C:\Program Files (x86)\Steam", r"C:\Program Files\Steam"] {
            steam_userdata(Path::new(base), &mut v);
        }
    }
    #[cfg(target_os = "macos")]
    {
        let sup = home.join("Library").join("Application Support");
        v.push(sup.join("unity.IronGate.Valheim").join("characters_local"));
        v.push(sup.join("unity.IronGate.Valheim").join("characters"));
        steam_userdata(&sup.join("Steam"), &mut v);
    }
    #[cfg(target_os = "linux")]
    {
        let u = home.join(".config/unity3d/IronGate/Valheim");
        v.push(u.join("characters_local"));
        v.push(u.join("characters"));
        for s in [".local/share/Steam", ".steam/steam", ".var/app/com.valvesoftware.Steam/.local/share/Steam"] {
            steam_userdata(&home.join(s), &mut v);
        }
        v.push(home.join(".local/share/Steam/steamapps/compatdata/892970/pfx/drive_c/users/steamuser/AppData/LocalLow/IronGate/Valheim/characters_local"));
    }
    // only folders with characters, each once (~/.steam/steam is often a link), the most recently played first
    let mut seen = std::collections::HashSet::new();
    let mut found: Vec<(u64, PathBuf)> = v
        .into_iter()
        .filter(|p| seen.insert(fs::canonicalize(p).unwrap_or_else(|_| p.clone())))
        .filter_map(|p| {
            let newest = list_chars(&p).iter().filter_map(|(_, f)| sig(f)).map(|s| s.0).max()?;
            Some((newest, p))
        })
        .collect();
    found.sort_by(|a, b| b.0.cmp(&a.0));
    found.into_iter().map(|(_, p)| p).collect()
}

fn resolve_dir(s: &Settings, cands: &[PathBuf]) -> Option<PathBuf> {
    if !s.dir.is_empty() {
        let p = PathBuf::from(&s.dir);
        return p.is_dir().then_some(p);
    }
    cands.first().cloned()
}

fn sig(p: &Path) -> Option<(u64, u64)> {
    let m = fs::metadata(p).ok()?;
    let t = m.modified().ok()?.duration_since(UNIX_EPOCH).ok()?.as_millis() as u64;
    Some((t, m.len()))
}

// ---- talking to the tracker ------------------------------------------------------------------

fn urlenc(s: &str) -> String {
    s.bytes()
        .map(|b| match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => (b as char).to_string(),
            _ => format!("%{b:02X}"),
        })
        .collect()
}

/// (HTTP status, message); status 0 = server unreachable.
fn post(server: &str, token: &str, path: &str, body: &[u8]) -> (u16, String) {
    let agent = ureq::AgentBuilder::new().timeout(Duration::from_secs(15)).build();
    let url = format!("{}{}", server.trim().trim_end_matches('/'), path);
    let req = agent.post(&url).set("X-Token", token).set("Content-Type", "application/octet-stream");
    let msg = |text: String| {
        serde_json::from_str::<serde_json::Value>(&text)
            .ok()
            .and_then(|v| v.get("error").and_then(|e| e.as_str()).map(str::to_string))
            .unwrap_or(text)
    };
    match req.send_bytes(body) {
        Ok(r) => (r.status(), String::new()),
        Err(ureq::Error::Status(c, r)) => (c, msg(r.into_string().unwrap_or_default())),
        Err(e) => (0, e.to_string()),
    }
}

fn check_server(s: &Settings) -> String {
    if s.server.trim().is_empty() || s.token.is_empty() {
        return "Enter the server address and token.".into();
    }
    // a heartbeat for a name nobody has: 404 means the token was accepted
    match post(&s.server, &s.token, "/heartbeat?name=__probe__", &[]) {
        (404, _) | (200, _) => "Connected: server reachable, token accepted.".into(),
        (403, _) => "The server rejects this token.".into(),
        (0, e) => format!("Cannot reach the server ({e})."),
        (c, m) => format!("Unexpected answer from the server: {c} {m}"),
    }
}

// ---- the sync loop ---------------------------------------------------------------------------

fn sync_once(sh: &Shared) {
    let (settings, force) = {
        let mut g = sh.lock().unwrap();
        let f = g.force;
        g.force = false;
        (g.settings.clone(), f)
    };
    let cands = candidates();
    let dir = resolve_dir(&settings, &cands);
    let files = dir.as_deref().map(list_chars).unwrap_or_default();
    {
        let mut g = sh.lock().unwrap();
        g.candidates = cands.iter().map(|p| p.display().to_string()).collect();
        g.dir = dir.clone();
        g.tracks.retain(|k, _| files.iter().any(|(n, _)| n == k));
        for (n, p) in &files {
            let t = g.tracks.entry(n.clone()).or_insert_with(|| Track {
                sig: None,
                beat: None,
                st: CharState { name: n.clone(), status: "idle".into(), ..Default::default() },
            });
            t.st.selected = !settings.excluded.contains(n);
            t.st.modified = sig(p).map(|s| s.0 / 1000);
            if force {
                t.sig = None;
            }
            if !t.st.selected {
                t.st.status = "off".into();
                t.st.message.clear();
                t.sig = None;
            } else if settings.paused {
                t.st.status = "paused".into();
            } else if t.st.status == "off" || t.st.status == "paused" {
                t.st.status = "idle".into();
            }
        }
    }
    if settings.paused || settings.server.trim().is_empty() || settings.token.is_empty() {
        return;
    }
    for (name, path) in files {
        let (selected, prev, beat) = {
            let g = sh.lock().unwrap();
            let t = &g.tracks[&name];
            (t.st.selected, t.sig, t.beat)
        };
        if !selected {
            continue;
        }
        let s = sig(&path);
        let q = urlenc(&name);
        if s.is_some() && s == prev {
            if beat.map_or(true, |b| b.elapsed() >= HEARTBEAT) {
                let (code, m) = post(&settings.server, &settings.token, &format!("/heartbeat?name={q}"), &[]);
                let mut g = sh.lock().unwrap();
                let t = g.tracks.get_mut(&name).unwrap();
                match code {
                    200 => {
                        t.beat = Some(Instant::now());
                        t.st.last_contact = Some(now());
                    }
                    404 => t.sig = None, // server forgot it or analyses differently now: upload again
                    0 => {
                        t.st.status = "unreachable".into();
                        t.st.message = m;
                    }
                    _ => {
                        t.beat = Some(Instant::now());
                        t.st.status = "error".into();
                        t.st.message = format!("{code} {m}");
                    }
                }
            }
            continue;
        }
        thread::sleep(SETTLE);
        if sig(&path) != s {
            continue; // still being written
        }
        let body = match fs::read(&path) {
            Ok(b) => b,
            Err(e) => {
                let mut g = sh.lock().unwrap();
                let t = g.tracks.get_mut(&name).unwrap();
                t.st.status = "error".into();
                t.st.message = format!("cannot read the file: {e}");
                continue;
            }
        };
        let (code, m) = post(&settings.server, &settings.token, &format!("/upload?name={q}"), &body);
        let mut g = sh.lock().unwrap();
        let t = g.tracks.get_mut(&name).unwrap();
        match code {
            200 => {
                t.sig = s;
                t.beat = Some(Instant::now());
                t.st.status = "uploaded".into();
                t.st.message.clear();
                t.st.last_upload = Some(now());
                t.st.last_contact = Some(now());
            }
            400 => {
                t.sig = s; // skip until the file changes
                t.beat = Some(Instant::now());
                t.st.status = "rejected".into();
                t.st.message = m;
            }
            0 => {
                t.st.status = "unreachable".into();
                t.st.message = m;
            }
            403 => {
                t.st.status = "error".into();
                t.st.message = "the server rejects the token".into();
            }
            _ => {
                t.st.status = "error".into();
                t.st.message = format!("{code} {m}");
            }
        }
    }
}

fn status(sh: &Shared) -> Status {
    let g = sh.lock().unwrap();
    let mut chars: Vec<CharState> = g.tracks.values().map(|t| t.st.clone()).collect();
    chars.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
    Status {
        settings: g.settings.clone(),
        dir: g.dir.as_ref().map(|p| p.display().to_string()).unwrap_or_default(),
        dir_auto: g.settings.dir.is_empty(),
        candidates: g.candidates.clone(),
        server: g.server.clone(),
        chars,
    }
}

fn save(g: &Inner) -> Result<(), String> {
    if let Some(p) = g.config.parent() {
        fs::create_dir_all(p).map_err(|e| e.to_string())?;
    }
    fs::write(&g.config, serde_json::to_vec_pretty(&g.settings).unwrap()).map_err(|e| e.to_string())
}

// ---- commands for the window -----------------------------------------------------------------

#[tauri::command]
fn get_status(sh: State<Shared>) -> Status {
    status(&sh)
}

#[tauri::command]
async fn save_server(server: String, token: String, sh: State<'_, Shared>) -> Result<Status, String> {
    let s = {
        let mut g = sh.lock().unwrap();
        g.settings.server = server.trim().trim_end_matches('/').to_string();
        g.settings.token = token.trim().to_string();
        g.force = true;
        save(&g)?;
        g.settings.clone()
    };
    let msg = check_server(&s);
    sh.lock().unwrap().server = msg;
    Ok(status(&sh))
}

#[tauri::command]
async fn test_server(sh: State<'_, Shared>) -> Result<Status, String> {
    let s = sh.lock().unwrap().settings.clone();
    let msg = check_server(&s);
    sh.lock().unwrap().server = msg;
    Ok(status(&sh))
}

#[tauri::command]
fn set_selected(name: String, on: bool, sh: State<Shared>) -> Result<Status, String> {
    {
        let mut g = sh.lock().unwrap();
        g.settings.excluded.retain(|n| n != &name);
        if !on {
            g.settings.excluded.push(name);
        }
        save(&g)?;
    }
    Ok(status(&sh))
}

#[tauri::command]
fn set_paused(paused: bool, sh: State<Shared>) -> Result<Status, String> {
    {
        let mut g = sh.lock().unwrap();
        g.settings.paused = paused;
        save(&g)?;
    }
    Ok(status(&sh))
}

#[tauri::command]
fn sync_now(sh: State<Shared>) {
    sh.lock().unwrap().force = true;
}

#[tauri::command]
fn set_dir(dir: String, sh: State<Shared>) -> Result<Status, String> {
    {
        let mut g = sh.lock().unwrap();
        g.settings.dir = dir;
        g.tracks.clear();
        save(&g)?;
    }
    Ok(status(&sh))
}

#[tauri::command]
async fn pick_dir(app: AppHandle, sh: State<'_, Shared>) -> Result<Status, String> {
    let start = sh.lock().unwrap().dir.clone();
    let mut d = app.dialog().file().set_title("Choose the Valheim characters folder");
    if let Some(s) = start {
        d = d.set_directory(s);
    }
    if let Some(p) = d.blocking_pick_folder() {
        let p = p.into_path().map_err(|e| e.to_string())?;
        let mut g = sh.lock().unwrap();
        g.settings.dir = p.display().to_string();
        g.tracks.clear();
        save(&g)?;
    }
    Ok(status(&sh))
}

#[tauri::command]
fn get_autostart(app: AppHandle) -> bool {
    app.autolaunch().is_enabled().unwrap_or(false)
}

#[tauri::command]
fn set_autostart(on: bool, app: AppHandle) -> Result<bool, String> {
    let a = app.autolaunch();
    if on { a.enable() } else { a.disable() }.map_err(|e| e.to_string())?;
    a.is_enabled().map_err(|e| e.to_string())
}

#[tauri::command]
fn open_hall(app: AppHandle, sh: State<Shared>) -> Result<(), String> {
    let url = sh.lock().unwrap().settings.server.clone();
    if url.is_empty() {
        return Err("no server set".into());
    }
    app.opener().open_url(url, None::<&str>).map_err(|e| e.to_string())
}

pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_autostart::init(MacosLauncher::LaunchAgent, None))
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .setup(|app| {
            let config = app.path().app_config_dir()?.join("settings.json");
            let settings: Settings =
                fs::read(&config).ok().and_then(|b| serde_json::from_slice(&b).ok()).unwrap_or_default();
            let sh: Shared = Arc::new(Mutex::new(Inner {
                settings,
                config,
                tracks: HashMap::new(),
                dir: None,
                candidates: Vec::new(),
                server: String::new(),
                force: false,
            }));
            app.manage(sh.clone());
            let handle = app.handle().clone();
            {
                let sh = sh.clone();
                thread::spawn(move || {
                    let s = sh.lock().unwrap().settings.clone();
                    let msg = check_server(&s);
                    sh.lock().unwrap().server = msg;
                });
            }
            thread::spawn(move || loop {
                sync_once(&sh);
                let _ = handle.emit("status", status(&sh));
                thread::sleep(TICK);
            });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            get_status, save_server, test_server, set_selected, set_paused, sync_now, set_dir, pick_dir,
            get_autostart, set_autostart, open_hall
        ])
        .run(tauri::generate_context!())
        .expect("error while running the app");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names() {
        assert_eq!(char_name(Path::new("/x/Roota.fch")).as_deref(), Some("Roota"));
        assert_eq!(char_name(Path::new("/x/roota.fch.old")), None);
        assert_eq!(char_name(Path::new("/x/roota_backup_auto-20261124.fch")), None);
        assert_eq!(char_name(Path::new("/x/notes.txt")), None);
    }

    #[test]
    fn encodes_names() {
        assert_eq!(urlenc("Ragnar Lodbrok"), "Ragnar%20Lodbrok");
        assert_eq!(urlenc("a.b-c_d"), "a.b-c_d");
    }
}
