//! Valheim Tracker (desktop app): watches the Valheim characters folder and uploads changed .fch files raw to a
//! valheim-tracker server (POST /upload?name=X, X-Token header), plus a heartbeat every 30 s for unchanged ones.
//! It never parses a save and never writes to the game folder.

mod local;

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, MutexGuard};
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use local::{LocalHall, Mode};
use tauri::{AppHandle, Emitter, Manager, State};
use tauri_plugin_autostart::{MacosLauncher, ManagerExt};
use tauri_plugin_dialog::DialogExt;
use tauri_plugin_opener::OpenerExt;
use tauri_plugin_updater::UpdaterExt;
use tracker_core::store::IngestError;
use tracker_core::worlds::{list_worlds, worlds_root, WorldDir};

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
    /// world folder name for the Hall's map; empty = the newest
    world: String,
    /// "local" | "group"; empty = group when a server is set, else local
    view: String,
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
    /// signature of the last file handed to the local Hall
    ing: Option<(u64, u64)>,
    /// removed from the local Hall (deselected) since it was last selected
    purged: bool,
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
    local: LocalHall,
    worlds: Vec<String>,
    world: String,
    world_path: Option<PathBuf>,
    /// bumped by `clear_tracks` (another folder): work started for the old folder is dropped
    gen: u64,
}

type Shared = Arc<Mutex<Inner>>;

/// Locks, ignoring poison: a panic elsewhere must not take every command and the sync loop down with it.
fn lock(sh: &Shared) -> MutexGuard<'_, Inner> {
    sh.lock().unwrap_or_else(|e| e.into_inner())
}

#[derive(Serialize, Clone)]
struct Status {
    settings: Settings,
    dir: String,
    dir_auto: bool,
    candidates: Vec<String>,
    server: String,
    chars: Vec<CharState>,
    hall_url: String,
    /// "local" | "group": what the Hall pane shows
    view: String,
    worlds: Vec<String>,
    world: String,
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

/// `None` = the folder could not be read (distinct from "read, no characters").
fn list_chars(dir: &Path) -> Option<Vec<(String, PathBuf)>> {
    let rd = fs::read_dir(dir).ok()?;
    let mut v: Vec<_> = rd.flatten().filter_map(|e| char_name(&e.path()).map(|n| (n, e.path()))).collect();
    v.sort_by(|a, b| a.0.to_lowercase().cmp(&b.0.to_lowercase()));
    Some(v)
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
            let newest = list_chars(&p).unwrap_or_default().iter().filter_map(|(_, f)| sig(f)).map(|s| s.0).max()?;
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

/// "group" only with a server set; empty setting = group when a server is set.
fn effective_view(s: &Settings) -> &'static str {
    let has_server = !s.server.trim().is_empty();
    match s.view.as_str() {
        "local" => "local",
        _ if has_server => "group",
        _ => "local",
    }
}

fn apply_mode(g: &Inner) {
    let want = if effective_view(&g.settings) == "group" {
        Mode::Group { server: g.settings.server.trim().trim_end_matches('/').to_string() }
    } else {
        Mode::Local
    };
    if let Ok(mut m) = g.local.mode.write() {
        if *m != want {
            *m = want;
        }
    }
}

/// Lists the worlds next to the characters folder and points the Hall at the chosen one.
fn read_worlds(dir: Option<&Path>) -> Vec<WorldDir> {
    dir.and_then(worlds_root).map(|r| list_worlds(&r)).unwrap_or_default()
}

/// Cheap: no file reads, `list` comes from `read_worlds` made outside the lock.
fn refresh_worlds(g: &mut Inner, list: &[WorldDir]) {
    g.worlds = list.iter().map(|w| w.name.clone()).collect();
    let chosen = list.iter().find(|w| w.name == g.settings.world).or(list.first());
    g.world = chosen.map(|w| w.name.clone()).unwrap_or_default();
    let path = chosen.map(|w| w.path.clone());
    if path != g.world_path {
        g.local.hall.set_world_dir(path.clone());
        g.world_path = path;
    }
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
    let (settings, force, gen) = {
        let mut g = lock(sh);
        let f = g.force;
        g.force = false;
        (g.settings.clone(), f, g.gen)
    };
    let cands = candidates();
    let dir = resolve_dir(&settings, &cands);
    // None = could not read the folder: keep everything this tick, remove nothing
    let listing = dir.as_deref().and_then(list_chars);
    let files = listing.clone().unwrap_or_default();
    let world_list = read_worlds(dir.as_deref());
    let mut to_remove: Vec<String> = Vec::new();
    let local = lock(sh).local.clone();
    {
        let mut g = lock(sh);
        if g.gen != gen {
            return; // the folder changed while this tick was listing the old one
        }
        g.candidates = cands.iter().map(|p| p.display().to_string()).collect();
        g.dir = dir.clone();
        if listing.is_some() {
            g.tracks.retain(|k, _| {
                let keep = files.iter().any(|(n, _)| n == k);
                if !keep && !settings.paused {
                    to_remove.push(k.clone()); // the file is gone
                }
                keep
            });
        }
        refresh_worlds(&mut g, &world_list);
        apply_mode(&g);
        for (n, p) in &files {
            let t = g.tracks.entry(n.clone()).or_insert_with(|| Track {
                sig: None,
                ing: None,
                purged: false,
                beat: None,
                st: CharState { name: n.clone(), status: "idle".into(), ..Default::default() },
            });
            t.st.selected = !settings.excluded.contains(n);
            t.st.modified = sig(p).map(|s| s.0 / 1000);
            if force {
                t.sig = None;
                t.ing = None;
            }
            if t.st.selected {
                t.purged = false;
            } else {
                t.st.status = "off".into();
                t.st.message.clear();
                t.sig = None;
                t.ing = None;
                if !settings.paused && !t.purged {
                    to_remove.push(n.clone());
                    t.purged = true;
                }
            }
            if t.st.selected && settings.paused {
                t.st.status = "paused".into();
            } else if t.st.selected && (t.st.status == "off" || t.st.status == "paused") {
                t.st.status = "idle".into();
            }
        }
    }
    for n in &to_remove {
        local.hall.store().remove(n);
    }
    if settings.paused {
        return;
    }
    if listing.is_some() {
        reconcile(&local, &files, &settings.excluded);
    }
    let has_server = !settings.server.trim().is_empty() && !settings.token.is_empty();
    for (name, path) in files {
        let (selected, prev, ing, beat) = {
            let g = lock(sh);
            if g.gen != gen {
                return;
            }
            let Some(t) = g.tracks.get(&name) else { continue };
            (t.st.selected, t.sig, t.ing, t.beat)
        };
        if !selected {
            continue;
        }
        let s = sig(&path);
        let q = urlenc(&name);
        let need_ing = s.is_none() || s != ing;
        let need_up = has_server && (s.is_none() || s != prev);
        if !need_ing && !need_up {
            if has_server && beat.map_or(true, |b| b.elapsed() >= HEARTBEAT) {
                let (code, m) = post(&settings.server, &settings.token, &format!("/heartbeat?name={q}"), &[]);
                let mut g = lock(sh);
                let Some(t) = g.tracks.get_mut(&name) else { continue };
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
                let mut g = lock(sh);
                let Some(t) = g.tracks.get_mut(&name) else { continue };
                t.st.status = "error".into();
                t.st.message = format!("cannot read the file: {e}");
                continue;
            }
        };
        if need_ing {
            // the file's mtime is "last seen", so a character is online while its file changed recently
            let at = s.map_or(0.0, |s| s.0 as f64 / 1000.0);
            // only for the current folder; re-checked after, as the folder can change during the ingest
            let current = |g: &Inner| g.gen == gen && g.tracks.contains_key(&name);
            if !current(&lock(sh)) {
                return;
            }
            let r = local.hall.store().ingest(&name, &body, at);
            let mut g = lock(sh);
            if !current(&g) {
                drop(g);
                local.hall.store().remove(&name);
                return;
            }
            let Some(t) = g.tracks.get_mut(&name) else { continue };
            t.ing = s; // a bad file is retried when it changes
            if !has_server {
                match r {
                    Ok(()) => {
                        if t.st.status == "rejected" || t.st.status == "error" {
                            t.st.status = "idle".into();
                        }
                        t.st.message.clear();
                    }
                    Err(IngestError::Bad(m)) => {
                        t.st.status = "rejected".into();
                        t.st.message = m;
                    }
                }
            }
        }
        if !need_up {
            continue;
        }
        let (code, m) = post(&settings.server, &settings.token, &format!("/upload?name={q}"), &body);
        let mut g = lock(sh);
        let Some(t) = g.tracks.get_mut(&name) else { continue };
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

/// Removes from the local Hall every character that is not a selected file of the folder just listed: catches
/// what the track diffing cannot see (files gone while the app was closed or paused, a folder change racing a tick).
fn reconcile(local: &LocalHall, files: &[(String, PathBuf)], excluded: &[String]) {
    let store = local.hall.store();
    for n in store.names() {
        if !files.iter().any(|(f, _)| *f == n && !excluded.contains(f)) {
            store.remove(&n);
        }
    }
}

fn status(sh: &Shared) -> Status {
    let g = lock(sh);
    let mut chars: Vec<CharState> = g.tracks.values().map(|t| t.st.clone()).collect();
    chars.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
    Status {
        settings: g.settings.clone(),
        dir: g.dir.as_ref().map(|p| p.display().to_string()).unwrap_or_default(),
        dir_auto: g.settings.dir.is_empty(),
        candidates: g.candidates.clone(),
        server: g.server.clone(),
        chars,
        hall_url: g.local.url(),
        view: effective_view(&g.settings).to_string(),
        worlds: g.worlds.clone(),
        world: g.world.clone(),
    }
}

/// Forgets every track and the characters the local Hall got from them (another folder is coming).
/// Returns the names to remove from the store, to be removed after the lock is dropped.
fn clear_tracks(g: &mut Inner) -> Vec<String> {
    let names = if g.settings.paused { Vec::new() } else { g.tracks.keys().cloned().collect() };
    g.tracks.clear();
    g.gen += 1;
    names
}

fn remove_chars(local: &LocalHall, names: &[String]) {
    for n in names {
        local.hall.store().remove(n);
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
        let mut g = lock(&sh);
        g.settings.server = server.trim().trim_end_matches('/').to_string();
        g.settings.token = token.trim().to_string();
        g.force = true;
        apply_mode(&g);
        save(&g)?;
        g.settings.clone()
    };
    let msg = check_server(&s);
    lock(&sh).server = msg;
    Ok(status(&sh))
}

#[tauri::command]
async fn test_server(sh: State<'_, Shared>) -> Result<Status, String> {
    let s = lock(&sh).settings.clone();
    let msg = check_server(&s);
    lock(&sh).server = msg;
    Ok(status(&sh))
}

#[tauri::command]
fn set_selected(name: String, on: bool, sh: State<Shared>) -> Result<Status, String> {
    {
        let mut g = lock(&sh);
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
        let mut g = lock(&sh);
        g.settings.paused = paused;
        save(&g)?;
    }
    Ok(status(&sh))
}

#[tauri::command]
fn hall_url(sh: State<Shared>) -> String {
    lock(&sh).local.url()
}

#[tauri::command]
fn set_view(view: String, sh: State<Shared>) -> Result<Status, String> {
    {
        let mut g = lock(&sh);
        let has_server = !g.settings.server.trim().is_empty();
        g.settings.view = if view == "group" && has_server { "group".into() } else if view == "group" { String::new() } else { "local".into() };
        apply_mode(&g);
        save(&g)?;
    }
    Ok(status(&sh))
}

#[tauri::command]
fn set_world(name: String, sh: State<Shared>) -> Result<Status, String> {
    let dir = lock(&sh).dir.clone();
    let list = read_worlds(dir.as_deref());
    {
        let mut g = lock(&sh);
        g.settings.world = name;
        refresh_worlds(&mut g, &list);
        save(&g)?;
    }
    Ok(status(&sh))
}

#[tauri::command]
fn sync_now(sh: State<Shared>) {
    lock(&sh).force = true;
}

#[tauri::command]
fn set_dir(dir: String, sh: State<Shared>) -> Result<Status, String> {
    let (local, gone, saved) = {
        let mut g = lock(&sh);
        g.settings.dir = dir;
        let gone = clear_tracks(&mut g);
        (g.local.clone(), gone, save(&g))
    };
    remove_chars(&local, &gone);
    saved?;
    Ok(status(&sh))
}

#[tauri::command]
async fn pick_dir(app: AppHandle, sh: State<'_, Shared>) -> Result<Status, String> {
    let start = lock(&sh).dir.clone();
    let mut d = app.dialog().file().set_title("Choose the Valheim characters folder");
    if let Some(s) = start {
        d = d.set_directory(s);
    }
    if let Some(p) = d.blocking_pick_folder() {
        let p = p.into_path().map_err(|e| e.to_string())?;
        let (local, gone, saved) = {
            let mut g = lock(&sh);
            g.settings.dir = p.display().to_string();
            let gone = clear_tracks(&mut g);
            (g.local.clone(), gone, save(&g))
        };
        remove_chars(&local, &gone);
        saved?;
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
    let url = lock(&sh).settings.server.clone();
    if url.is_empty() {
        return Err("no server set".into());
    }
    app.opener().open_url(url, None::<&str>).map_err(|e| e.to_string())
}

// ---- self-update (signed releases on GitHub, see plugins.updater in tauri.conf.json) --------------

#[derive(Serialize, Clone)]
struct UpdateInfo {
    current: String,
    /// newest version if it is newer than the running one
    available: Option<String>,
}

#[tauri::command]
async fn check_update(app: AppHandle) -> Result<UpdateInfo, String> {
    let u = app.updater().map_err(|e| e.to_string())?.check().await.map_err(|e| e.to_string())?;
    Ok(UpdateInfo { current: app.package_info().version.to_string(), available: u.map(|u| u.version) })
}

#[tauri::command]
async fn install_update(app: AppHandle) -> Result<(), String> {
    let u = app.updater().map_err(|e| e.to_string())?.check().await.map_err(|e| e.to_string())?;
    let Some(u) = u else { return Ok(()) };
    u.download_and_install(|_, _| {}, || {}).await.map_err(|e| e.to_string())?;
    app.restart()
}

/// The window may only show the app's own pages and, in its iframe, the local Hall.
fn allowed_nav(url: &tauri::Url, hall_port: Option<u16>, dev: Option<&tauri::Url>) -> bool {
    let host = url.host_str().unwrap_or("");
    match url.scheme() {
        "tauri" => true,
        "about" => true,
        "http" | "https" if host == "tauri.localhost" => true,
        "http" if host == "127.0.0.1" && hall_port.is_some() && url.port() == hall_port => true,
        _ => dev.is_some_and(|d| d.origin() == url.origin()),
    }
}

fn nav_guard<R: tauri::Runtime>() -> tauri::plugin::TauriPlugin<R> {
    tauri::plugin::Builder::new("nav-guard")
        .on_navigation(|webview, url| {
            let app = webview.app_handle();
            let port = app.try_state::<Shared>().map(|sh| lock(&sh).local.port);
            let dev = if cfg!(debug_assertions) { app.config().build.dev_url.clone() } else { None };
            let ok = allowed_nav(url, port, dev.as_ref());
            if !ok {
                eprintln!("blocked navigation to {url}");
            }
            ok
        })
        .build()
}

pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_autostart::init(MacosLauncher::LaunchAgent, None))
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(nav_guard())
        .setup(|app| {
            let config = app.path().app_config_dir()?.join("settings.json");
            let settings: Settings =
                fs::read(&config).ok().and_then(|b| serde_json::from_slice(&b).ok()).unwrap_or_default();
            let data = app.path().app_data_dir()?;
            // everything in it is derived from the save files and re-ingested on the first tick (`ing` starts
            // empty): starting from nothing drops characters whose files went away while the app was closed
            let state = data.join("local-state");
            let _ = fs::remove_dir_all(&state);
            let port_file = data.join("hall-port");
            let preferred = fs::read_to_string(&port_file).ok().and_then(|s| s.trim().parse().ok()).unwrap_or(0);
            let local = local::start(state, preferred)?;
            if local.port != preferred {
                let _ = fs::write(&port_file, local.port.to_string());
            }
            let sh: Shared = Arc::new(Mutex::new(Inner {
                settings,
                config,
                tracks: HashMap::new(),
                dir: None,
                candidates: Vec::new(),
                server: String::new(),
                force: false,
                local,
                worlds: Vec::new(),
                world: String::new(),
                world_path: None,
                gen: 0,
            }));
            apply_mode(&lock(&sh));
            app.manage(sh.clone());
            let handle = app.handle().clone();
            {
                let sh = sh.clone();
                thread::spawn(move || {
                    let s = lock(&sh).settings.clone();
                    let msg = check_server(&s);
                    lock(&sh).server = msg;
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
            get_autostart, set_autostart, open_hall, hall_url, set_view, set_world, check_update, install_update
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

    fn tmp(tag: &str) -> PathBuf {
        let p = std::env::temp_dir().join(format!("vts-sync-{}-{tag}", std::process::id()));
        let _ = fs::remove_dir_all(&p);
        fs::create_dir_all(&p).unwrap();
        p
    }

    fn shared(tag: &str, chars: &Path) -> Shared {
        let base = tmp(tag);
        Arc::new(Mutex::new(Inner {
            settings: Settings { dir: chars.display().to_string(), ..Default::default() },
            config: base.join("cfg").join("settings.json"),
            tracks: HashMap::new(),
            dir: None,
            candidates: Vec::new(),
            server: String::new(),
            force: false,
            local: local::start(base.join("state"), 0).unwrap(),
            worlds: Vec::new(),
            world: String::new(),
            world_path: None,
            gen: 0,
        }))
    }

    fn folder(tag: &str, names: &[&str]) -> PathBuf {
        let d = tmp(tag);
        for n in names {
            fs::write(d.join(format!("{n}.fch")), local::support::FchBuilder::new(47).build()).unwrap();
        }
        d
    }

    fn stored(sh: &Shared) -> Vec<String> {
        lock(sh).local.hall.store().names()
    }

    fn change_folder(sh: &Shared, dir: &Path) {
        let (local, gone) = {
            let mut g = lock(sh);
            g.settings.dir = dir.display().to_string();
            (g.local.clone(), clear_tracks(&mut g))
        };
        remove_chars(&local, &gone);
    }

    #[test]
    fn folder_change_during_a_tick_does_not_panic_or_leave_ghosts() {
        let a = folder("a", &["A1", "A2"]);
        let b = folder("b", &["B1"]);
        let sh = shared("mid", &a);
        // the first tick settles 700 ms per character: change the folder while it waits on A1
        let t = {
            let sh = sh.clone();
            thread::spawn(move || sync_once(&sh))
        };
        thread::sleep(Duration::from_millis(300));
        change_folder(&sh, &b);
        t.join().expect("sync_once panicked");
        assert!(!sh.is_poisoned());
        assert!(stored(&sh).iter().all(|n| n.starts_with('B')), "{:?}", stored(&sh));
        sync_once(&sh);
        assert_eq!(stored(&sh), vec!["B1".to_string()]);
        assert_eq!(lock(&sh).tracks.keys().collect::<Vec<_>>(), vec!["B1"]);
    }

    #[test]
    fn ticks_reconcile_the_local_hall_with_the_folder() {
        let a = folder("ra", &["A1", "A2"]);
        let sh = shared("rec", &a);
        sync_once(&sh);
        assert_eq!(stored(&sh), vec!["A1".to_string(), "A2".to_string()]);
        // cleared between ticks, the removal lost (e.g. a save error): the next tick still drops them
        let empty = tmp("rempty");
        {
            let mut g = lock(&sh);
            g.settings.dir = empty.display().to_string();
            clear_tracks(&mut g);
        }
        sync_once(&sh);
        assert!(stored(&sh).is_empty());
        // a file deleted while the app was closed (store has it, no track): gone after one tick
        change_folder(&sh, &a);
        sync_once(&sh);
        fs::remove_file(a.join("A2.fch")).unwrap();
        lock(&sh).tracks.clear();
        sync_once(&sh);
        assert_eq!(stored(&sh), vec!["A1".to_string()]);
        // excluded characters are not kept either
        lock(&sh).settings.excluded = vec!["A1".into()];
        sync_once(&sh);
        assert!(stored(&sh).is_empty());
    }

    #[test]
    fn navigation_is_limited_to_the_app_and_the_local_hall() {
        let u = |s: &str| tauri::Url::parse(s).unwrap();
        let dev = u("http://localhost:1430/");
        for ok in ["tauri://localhost/index.html", "http://tauri.localhost/", "https://tauri.localhost/x", "about:blank", "http://127.0.0.1:4242/#map"] {
            assert!(allowed_nav(&u(ok), Some(4242), None), "{ok}");
        }
        assert!(allowed_nav(&u("http://localhost:1430/index.html"), Some(4242), Some(&dev)));
        for bad in ["https://evil.example/", "http://127.0.0.1:4243/", "http://localhost:1430/", "file:///etc/passwd"] {
            assert!(!allowed_nav(&u(bad), Some(4242), None), "{bad}");
        }
        assert!(!allowed_nav(&u("http://127.0.0.1:4242/"), None, None));
    }

    #[test]
    fn encodes_names() {
        assert_eq!(urlenc("Ragnar Lodbrok"), "Ragnar%20Lodbrok");
        assert_eq!(urlenc("a.b-c_d"), "a.b-c_d");
    }
}
