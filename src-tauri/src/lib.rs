use serde::{Deserialize, Serialize};
#[cfg(windows)]
use std::ffi::OsStr;
use std::fs;
use std::io::ErrorKind;
#[cfg(windows)]
use std::os::windows::ffi::OsStrExt;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};
use tauri::{image::Image, Manager, WebviewWindow, WindowEvent};
use walkdir::WalkDir;
#[cfg(windows)]
use windows_sys::Win32::UI::Shell::ShellExecuteW;
#[cfg(windows)]
use windows_sys::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL;

#[derive(Debug, Serialize, Deserialize, Clone)]
struct SymlinkEntry {
    relative: String,
    target: String,
    status: String,
    link_is_dir: Option<bool>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
struct ExportData {
    src_root: String,
    entries: Vec<SymlinkEntry>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
struct MappingRule {
    from: String,
    to: String,
}

#[derive(Debug, Serialize)]
struct ScanResult {
    src_root: String,
    entries: Vec<SymlinkEntry>,
    skipped: usize,
}

#[derive(Debug, Serialize)]
struct PreviewRoot {
    root: String,
    count: usize,
}

#[derive(Debug, Serialize)]
struct PreviewItem {
    link: String,
    target: String,
}

#[derive(Debug, Serialize)]
struct PreviewRootSample {
    root: String,
    link: String,
    target: String,
}

#[derive(Debug, Serialize)]
struct PreviewResult {
    total: usize,
    roots: Vec<PreviewRoot>,
    sample: Vec<PreviewItem>,
    root_samples: Vec<PreviewRootSample>,
}

#[derive(Debug, Serialize)]
struct ConflictReport {
    total: usize,
    non_symlink: usize,
    sample: Vec<String>,
}

#[derive(Debug, Serialize)]
struct RecreateResult {
    created: usize,
    failed: Vec<String>,
    sample_links: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize)]
struct AdminJob {
    data: ExportData,
    dst_root: String,
    mappings: Vec<MappingRule>,
    missing_as_dir: bool,
    job_id: String,
}

#[derive(Debug, Serialize, Deserialize)]
struct AdminResult {
    created: usize,
    failed: usize,
    sample_links: Vec<String>,
    #[serde(default)]
    errors: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone, Copy)]
struct SavedWindowState {
    width: u32,
    height: u32,
    x: i32,
    y: i32,
    maximized: bool,
}

fn window_state_path(window: &WebviewWindow) -> Option<PathBuf> {
    window
        .app_handle()
        .path()
        .app_local_data_dir()
        .ok()
        .map(|dir| dir.join("window-state.json"))
}

fn read_window_state(path: &Path) -> Option<SavedWindowState> {
    serde_json::from_str(&fs::read_to_string(path).ok()?).ok()
}

fn overlaps_work_area(state: SavedWindowState, x: i32, y: i32, width: u32, height: u32) -> bool {
    let left = i64::from(state.x).max(i64::from(x));
    let top = i64::from(state.y).max(i64::from(y));
    let right = (i64::from(state.x) + i64::from(state.width)).min(i64::from(x) + i64::from(width));
    let bottom =
        (i64::from(state.y) + i64::from(state.height)).min(i64::from(y) + i64::from(height));
    right - left >= 80 && bottom - top >= 80
}

fn saved_window_is_visible(window: &WebviewWindow, state: SavedWindowState) -> bool {
    window.available_monitors().is_ok_and(|monitors| {
        monitors.into_iter().any(|monitor| {
            let area = monitor.work_area();
            overlaps_work_area(
                state,
                area.position.x,
                area.position.y,
                area.size.width,
                area.size.height,
            )
        })
    })
}

fn persist_window_state(window: &WebviewWindow, path: &Path) {
    let maximized = window.is_maximized().unwrap_or(false);
    let state = if maximized {
        read_window_state(path).map(|state| SavedWindowState { maximized, ..state })
    } else {
        let (Ok(size), Ok(position)) = (window.outer_size(), window.outer_position()) else {
            return;
        };
        Some(SavedWindowState {
            width: size.width,
            height: size.height,
            x: position.x,
            y: position.y,
            maximized,
        })
    };
    if let (Some(state), Some(parent)) = (state, path.parent()) {
        let _ = fs::create_dir_all(parent);
        let _ = fs::write(path, serde_json::to_vec(&state).unwrap_or_default());
    }
}

fn restore_window_state(window: &WebviewWindow) {
    let Some(path) = window_state_path(window) else {
        return;
    };
    let saved = read_window_state(&path);
    if let Some(state) = saved.filter(|state| saved_window_is_visible(window, *state)) {
        let _ = window.set_size(tauri::PhysicalSize::new(state.width, state.height));
        let _ = window.set_position(tauri::PhysicalPosition::new(state.x, state.y));
        if state.maximized {
            let _ = window.maximize();
        }
        return;
    }

    if let Ok(Some(monitor)) = window.current_monitor() {
        let area = monitor.work_area();
        let width = area
            .size
            .width
            .saturating_mul(3)
            .saturating_div(4)
            .min(1440);
        let height = area
            .size
            .height
            .saturating_mul(3)
            .saturating_div(4)
            .min(820);
        let _ = window.set_size(tauri::PhysicalSize::new(width, height));
    }
    let _ = window.center();
    if saved.is_some_and(|state| state.maximized) {
        let _ = window.maximize();
    }
}

fn normalize_path_display(path: &Path) -> String {
    let raw = path.to_string_lossy().to_string();
    if let Some(stripped) = raw.strip_prefix(r"\\?\") {
        if let Some(unc) = raw.strip_prefix(r"\\?\UNC\") {
            format!(r"\\{}", unc)
        } else {
            stripped.to_string()
        }
    } else {
        raw
    }
}

fn normalize_for_match(input: &str) -> String {
    let mut value = input.replace('\\', "/");
    #[cfg(windows)]
    {
        value = value.to_lowercase();
    }
    value = collapse_slashes(&value);
    while value.len() > 1 && value.ends_with('/') {
        value.pop();
    }
    value
}

fn normalize_for_match_keep_case(input: &str) -> String {
    let mut value = input.replace('\\', "/");
    value = collapse_slashes(&value);
    while value.len() > 1 && value.ends_with('/') {
        value.pop();
    }
    value
}

fn collapse_slashes(input: &str) -> String {
    fn collapse_single(input: &str) -> String {
        let mut out = String::with_capacity(input.len());
        let mut prev = '\0';
        for ch in input.chars() {
            if ch == '/' && prev == '/' {
                continue;
            }
            out.push(ch);
            prev = ch;
        }
        out
    }

    if let Some(rest) = input.strip_prefix("//") {
        let collapsed = collapse_single(rest);
        format!("//{}", collapsed.trim_start_matches('/'))
    } else {
        collapse_single(input)
    }
}

fn extract_root_bucket(path: &str) -> String {
    let mut normalized = path.replace('\\', "/");
    normalized = collapse_slashes(&normalized);
    if normalized.starts_with("//") {
        let parts: Vec<&str> = normalized
            .trim_start_matches("//")
            .split('/')
            .filter(|part| !part.is_empty())
            .collect();
        return match parts.len() {
            0 => "//".to_string(),
            1 => format!("//{}", parts[0]),
            2 => format!("//{}/{}", parts[0], parts[1]),
            _ => format!("//{}/{}/{}", parts[0], parts[1], parts[2]),
        };
    }

    if normalized.len() >= 2 && normalized.as_bytes()[1] == b':' {
        let drive = normalized[0..2].to_uppercase();
        let rest = normalized[2..].trim_start_matches('/');
        let first = rest.split('/').find(|s| !s.is_empty());
        return match first {
            Some(segment) => format!("{}/{}", drive, segment),
            None => format!("{}/", drive),
        };
    }

    if normalized.starts_with('/') {
        let mut parts = normalized.split('/').filter(|part| !part.is_empty());
        let first = parts.next();
        let second = parts.next();
        return match (first, second) {
            (Some(a), Some(b)) => format!("/{}/{}", a, b),
            (Some(a), None) => format!("/{}", a),
            _ => "/".to_string(),
        };
    }

    normalized
        .split('/')
        .find(|part| !part.is_empty())
        .unwrap_or("")
        .to_string()
}

fn preferred_separator(path: &str) -> char {
    let has_backslash = path.contains('\\');
    let has_slash = path.contains('/');
    if has_backslash && !has_slash {
        return '\\';
    }
    if has_slash && !has_backslash {
        return '/';
    }
    #[cfg(windows)]
    {
        '\\'
    }
    #[cfg(not(windows))]
    {
        '/'
    }
}

fn join_root(root: &str, suffix: &str) -> String {
    if suffix.is_empty() {
        return root.to_string();
    }
    let sep = preferred_separator(root);
    let mut out = root.to_string();
    if !out.ends_with('/') && !out.ends_with('\\') {
        out.push(sep);
    }
    let mut normalized_suffix = suffix.to_string();
    if sep != '/' {
        normalized_suffix = normalized_suffix.replace('/', &sep.to_string());
    }
    out.push_str(&normalized_suffix);
    out
}

fn mapped_root(target: &str, from: &str, to: &str) -> Option<String> {
    if from.trim().is_empty() || to.trim().is_empty() {
        return None;
    }
    let target_keep = normalize_for_match_keep_case(target);
    let target_norm = normalize_for_match(target);
    let from_norm = normalize_for_match(from);
    if target_norm == from_norm {
        return Some(to.to_string());
    }
    if from_norm == "/" && target_norm.starts_with('/') && !target_norm.starts_with("//") {
        return Some(join_root(to, target_keep.trim_start_matches('/')));
    }
    if from_norm == "/" {
        return None;
    }
    if target_norm.starts_with(&(from_norm.clone() + "/")) {
        // Count components instead of slicing a lowercased Unicode string by byte length.
        let suffix = target_keep
            .split('/')
            .skip(from_norm.split('/').count())
            .collect::<Vec<_>>()
            .join("/");
        return Some(join_root(to, &suffix));
    }
    None
}

fn replace_root(target: &str, from: &str, to: &str) -> String {
    mapped_root(target, from, to).unwrap_or_else(|| target.to_string())
}

fn apply_mappings(target: &str, mappings: &[MappingRule]) -> String {
    mappings
        .iter()
        .find_map(|rule| mapped_root(target, &rule.from, &rule.to))
        .unwrap_or_else(|| target.to_string())
}

fn remap_target(target: &str, mappings: &[MappingRule], src_root: &str, dst_root: &str) -> String {
    let mapped = apply_mappings(target, mappings);
    replace_root(&mapped, src_root, dst_root)
}

fn normalize_relative_for_os(relative: &str) -> String {
    #[cfg(unix)]
    {
        return relative.replace('\\', "/");
    }
    #[cfg(windows)]
    {
        relative.to_string()
    }
}

fn link_path(dst_root: &str, relative: &str) -> PathBuf {
    let normalized = normalize_relative_for_os(relative);
    PathBuf::from(dst_root).join(normalized)
}

fn validate_data(data: &ExportData) -> Result<(), String> {
    let mut seen = std::collections::HashSet::new();
    for entry in &data.entries {
        let path = entry.relative.replace('\\', "/");
        if path.is_empty()
            || path.starts_with('/')
            || (path.as_bytes().get(1) == Some(&b':') && path.as_bytes()[0].is_ascii_alphabetic())
            || (cfg!(windows) && path.contains(':'))
            || path.contains('\0')
            || path.split('/').any(|part| {
                part.is_empty()
                    || part == ".."
                    || part == "."
                    || (cfg!(windows) && part.ends_with([' ', '.']))
            })
            || entry.target.is_empty()
            || entry.target.contains('\0')
        {
            return Err(format!(
                "Unsafe or unreadable imported entry: {}",
                entry.relative
            ));
        }
        #[cfg(windows)]
        for part in path.split('/') {
            let stem = part.split('.').next().unwrap_or("").to_ascii_uppercase();
            if ["CON", "PRN", "AUX", "NUL", "CONIN$", "CONOUT$"].contains(&stem.as_str())
                || ((stem.starts_with("COM") || stem.starts_with("LPT"))
                    && stem.len() == 4
                    && matches!(stem.as_bytes()[3], b'1'..=b'9'))
            {
                return Err(format!("Reserved Windows name: {}", entry.relative));
            }
        }
        let key = if cfg!(windows) {
            path.to_lowercase()
        } else {
            path
        };
        if !seen.insert(key) {
            return Err(format!("Duplicate imported path: {}", entry.relative));
        }
    }
    Ok(())
}

fn validate_destination(link: &Path) -> Result<(), String> {
    let mut parent = link.parent();
    while let Some(path) = parent {
        if let Ok(metadata) = fs::symlink_metadata(path) {
            if metadata.file_type().is_symlink() {
                return Err(format!(
                    "Destination parent is a symlink: {}",
                    path.display()
                ));
            }
        }
        parent = path.parent();
    }
    Ok(())
}

fn recreate_symlinks_inner(
    data: ExportData,
    dst_root: String,
    mappings: Vec<MappingRule>,
    missing_as_dir: bool,
) -> Result<RecreateResult, String> {
    validate_data(&data)?;
    if dst_root.trim().is_empty() {
        return Err("Select a valid target root.".to_string());
    }

    let mut created = 0usize;
    let mut failed = Vec::new();
    let mut sample_links = Vec::new();

    for entry in data.entries {
        if entry.target == "<unreadable>" {
            failed.push(format!(
                "{}: original target could not be read",
                entry.relative
            ));
            continue;
        }
        let link = link_path(&dst_root, &entry.relative);
        let target = remap_target(&entry.target, &mappings, &data.src_root, &dst_root);
        let target_path = PathBuf::from(&target);
        if let Err(err) = validate_destination(&link) {
            failed.push(err);
            continue;
        }

        if let Some(parent) = link.parent() {
            if let Err(err) = fs::create_dir_all(parent) {
                failed.push(format!(
                    "{} -> {} (mkdir failed: {})",
                    normalize_path_display(&link),
                    target,
                    err
                ));
                continue;
            }
        }

        let link_is_dir = entry
            .link_is_dir
            .or_else(|| {
                infer_link_is_dir(
                    &if target_path.is_absolute() {
                        target_path.clone()
                    } else {
                        link.parent()
                            .unwrap_or(Path::new(&dst_root))
                            .join(&target_path)
                    },
                    &link,
                )
            })
            .unwrap_or(missing_as_dir);

        let backup = link.with_file_name(format!(
            ".symlinkmanager-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos()
        ));
        let replacing = fs::symlink_metadata(&link).is_ok();
        if replacing {
            if fs::symlink_metadata(&link)
                .map(|m| m.is_dir() && !m.file_type().is_symlink())
                .unwrap_or(false)
                && fs::read_dir(&link)
                    .map(|mut d| d.next().is_some())
                    .unwrap_or(true)
            {
                failed.push(format!(
                    "Refusing to replace non-empty directory: {}",
                    link.display()
                ));
                continue;
            }
            if let Err(err) = fs::rename(&link, &backup) {
                failed.push(format!("Cannot preserve {}: {err}", link.display()));
                continue;
            }
        }

        let result = {
            #[cfg(unix)]
            {
                std::os::unix::fs::symlink(&target_path, &link).map_err(|err| err.to_string())
            }
            #[cfg(windows)]
            {
                if link_is_dir {
                    std::os::windows::fs::symlink_dir(&target_path, &link)
                        .map_err(|err| err.to_string())
                } else {
                    std::os::windows::fs::symlink_file(&target_path, &link)
                        .map_err(|err| err.to_string())
                }
            }
        };

        if replacing {
            if result.is_err() {
                if let Err(err) = fs::rename(&backup, &link) {
                    failed.push(format!(
                        "Restore failed; original retained at {}: {err}",
                        backup.display()
                    ));
                }
            } else if fs::remove_file(&backup)
                .or_else(|_| fs::remove_dir(&backup))
                .is_err()
            {
                failed.push(format!("Original retained at {}", backup.display()));
            }
        }
        match result {
            Ok(_) => {
                created += 1;
                if sample_links.len() < 5 {
                    sample_links.push(normalize_path_display(&link));
                }
            }
            Err(err) => failed.push(format!(
                "{} -> {} ({})",
                normalize_path_display(&link),
                target,
                err
            )),
        }
    }

    Ok(RecreateResult {
        created,
        failed,
        sample_links,
    })
}

#[cfg(windows)]
fn launch_admin_recreate(job_path: &Path) -> Result<(), String> {
    let exe = std::env::current_exe().map_err(|err| err.to_string())?;
    let args = format!("--admin-recreate \"{}\"", job_path.display());

    let verb: Vec<u16> = OsStr::new("runas")
        .encode_wide()
        .chain(std::iter::once(0))
        .collect();
    let exe_w: Vec<u16> = exe
        .as_os_str()
        .encode_wide()
        .chain(std::iter::once(0))
        .collect();
    let args_w: Vec<u16> = OsStr::new(&args)
        .encode_wide()
        .chain(std::iter::once(0))
        .collect();

    let result = unsafe {
        ShellExecuteW(
            0,
            verb.as_ptr(),
            exe_w.as_ptr(),
            args_w.as_ptr(),
            std::ptr::null(),
            SW_SHOWNORMAL,
        )
    };
    if result <= 32 {
        return Err("Failed to request admin privileges.".to_string());
    }
    Ok(())
}

fn write_admin_job(job: &AdminJob) -> Result<PathBuf, String> {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|err| err.to_string())?
        .as_nanos();
    let filename = format!("symlinkmanager_admin_{}_{}.json", std::process::id(), now);
    let path = std::env::temp_dir().join(filename);
    let payload = serde_json::to_string_pretty(job).map_err(|err| err.to_string())?;
    use std::io::Write;
    let mut file = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&path)
        .map_err(|err| err.to_string())?;
    file.write_all(payload.as_bytes())
        .map_err(|err| err.to_string())?;
    Ok(path)
}

fn admin_result_path(job_id: &str) -> PathBuf {
    std::env::temp_dir().join(format!("symlinkmanager_admin_result_{}.json", job_id))
}

pub fn handle_admin_recreate() -> bool {
    let mut args = std::env::args().skip(1);
    let flag = match args.next() {
        Some(value) => value,
        None => return false,
    };
    if flag != "--admin-recreate" {
        return false;
    }
    let job_path = match args.next() {
        Some(value) => value,
        None => return true,
    };

    let job_raw = match fs::read_to_string(&job_path) {
        Ok(value) => value,
        Err(err) => {
            eprintln!("Failed to read admin job: {}", err);
            return true;
        }
    };
    let job: AdminJob = match serde_json::from_str(&job_raw) {
        Ok(value) => value,
        Err(err) => {
            eprintln!("Failed to parse admin job: {}", err);
            return true;
        }
    };
    let result = recreate_symlinks_inner(job.data, job.dst_root, job.mappings, job.missing_as_dir);
    {
        let payload = match result {
            Ok(result) => AdminResult {
                created: result.created,
                failed: result.failed.len(),
                sample_links: result.sample_links,
                errors: result.failed,
            },
            Err(err) => AdminResult {
                created: 0,
                failed: 1,
                sample_links: Vec::new(),
                errors: vec![err],
            },
        };
        let result_path = admin_result_path(&job.job_id);
        if let Ok(json) = serde_json::to_string_pretty(&payload) {
            let pending = result_path.with_extension("pending");
            if fs::write(&pending, json).is_ok() {
                let _ = fs::rename(pending, result_path);
            }
        }
    }
    let _ = fs::remove_file(&job_path);
    true
}

fn infer_link_is_dir(target: &Path, link_path: &Path) -> Option<bool> {
    if let Ok(metadata) = fs::metadata(target) {
        return Some(metadata.is_dir());
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        if let Ok(metadata) = fs::symlink_metadata(link_path) {
            const FILE_ATTRIBUTE_DIRECTORY: u32 = 0x10;
            return Some(metadata.file_attributes() & FILE_ATTRIBUTE_DIRECTORY != 0);
        }
    }
    None
}

#[tauri::command]
async fn scan_symlinks(root: String) -> Result<ScanResult, String> {
    tauri::async_runtime::spawn_blocking(move || scan_inner(root))
        .await
        .map_err(|err| err.to_string())?
}

fn link_directory_hint(link: &Path) -> Option<bool> {
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        fs::symlink_metadata(link)
            .ok()
            .map(|m| m.file_attributes() & 0x10 != 0)
    }
    #[cfg(not(windows))]
    {
        let _ = link;
        None
    }
}

fn scan_inner(root: String) -> Result<ScanResult, String> {
    let root_path =
        fs::canonicalize(root.trim()).map_err(|err| format!("Invalid scan folder: {err}"))?;
    if !root_path.is_dir() {
        return Err("Invalid scan folder.".into());
    }
    let mut skipped = 0;
    let mut links = Vec::new();
    for entry in WalkDir::new(&root_path).follow_links(false).min_depth(1) {
        match entry {
            Ok(entry) if entry.file_type().is_symlink() => links.push(entry),
            Err(_) => skipped += 1,
            _ => {}
        }
    }
    // ponytail: eight bounded workers; tune only after measurements on network storage.
    let chunk_size = links.len().div_ceil(8).max(1);
    let entries = std::thread::scope(|scope| {
        let workers: Vec<_> = links
            .chunks(chunk_size)
            .map(|chunk| {
                let root_path = &root_path;
                scope.spawn(move || {
                    chunk
                        .iter()
                        .filter_map(|entry| scan_entry(entry, root_path))
                        .collect::<Vec<_>>()
                })
            })
            .collect();
        workers
            .into_iter()
            .flat_map(|worker| worker.join().expect("scan worker panicked"))
            .collect()
    });
    Ok(ScanResult {
        src_root: normalize_path_display(&root_path),
        entries,
        skipped,
    })
}

fn scan_entry(entry: &walkdir::DirEntry, root_path: &Path) -> Option<SymlinkEntry> {
    let link_path = entry.path();
    let relative = match link_path.strip_prefix(root_path) {
        Ok(rel) => rel,
        Err(_) => return None,
    };
    let target = match fs::read_link(link_path) {
        Ok(value) => value,
        Err(_) => {
            return Some(SymlinkEntry {
                relative: normalize_path_display(relative),
                target: "<unreadable>".to_string(),
                status: "Unreadable".to_string(),
                link_is_dir: link_directory_hint(link_path),
            });
        }
    };
    let target_abs = if target.is_absolute() {
        target
    } else {
        link_path.parent().unwrap_or(root_path).join(target)
    };
    let metadata = fs::metadata(&target_abs);
    let status = match &metadata {
        Ok(_) => "OK",
        Err(err) => {
            if err.kind() != ErrorKind::NotFound {
                "Unreadable"
            } else {
                "Broken"
            }
        }
    };
    let link_is_dir = metadata
        .as_ref()
        .ok()
        .map(|m| m.is_dir())
        .or_else(|| link_directory_hint(link_path));
    Some(SymlinkEntry {
        relative: normalize_path_display(relative),
        target: normalize_path_display(&target_abs),
        status: status.to_string(),
        link_is_dir,
    })
}

#[tauri::command]
fn export_symlinks(path: String, data: ExportData) -> Result<(), String> {
    let payload = serde_json::to_string_pretty(&data).map_err(|err| err.to_string())?;
    fs::write(path, payload).map_err(|err| err.to_string())
}

#[tauri::command]
fn load_export(path: String) -> Result<ExportData, String> {
    let raw = fs::read_to_string(path).map_err(|err| err.to_string())?;
    let data: ExportData = serde_json::from_str(&raw).map_err(|err| err.to_string())?;
    validate_data(&data)?;
    Ok(data)
}

#[tauri::command]
fn preview_recreate(
    data: ExportData,
    dst_root: String,
    mappings: Vec<MappingRule>,
    max_preview: usize,
) -> Result<PreviewResult, String> {
    validate_data(&data)?;
    if dst_root.trim().is_empty() {
        return Err("Select a valid target root.".to_string());
    }

    let mut sample = Vec::new();
    for entry in data.entries.iter().take(max_preview.min(100)) {
        let link = link_path(&dst_root, &entry.relative);
        let target = remap_target(&entry.target, &mappings, &data.src_root, &dst_root);
        sample.push(PreviewItem {
            link: normalize_path_display(&link),
            target,
        });
    }
    let mut counts: std::collections::HashMap<String, usize> = std::collections::HashMap::new();
    let mut root_samples: std::collections::HashMap<String, PreviewRootSample> =
        std::collections::HashMap::new();
    for entry in data.entries.iter() {
        let root = extract_root_bucket(&entry.target);
        *counts.entry(root.clone()).or_insert(0) += 1;
        if !root_samples.contains_key(&root) {
            let link = link_path(&dst_root, &entry.relative);
            let target = remap_target(&entry.target, &mappings, &data.src_root, &dst_root);
            root_samples.insert(
                root.clone(),
                PreviewRootSample {
                    root,
                    link: normalize_path_display(&link),
                    target,
                },
            );
        }
    }
    let mut roots: Vec<PreviewRoot> = counts
        .into_iter()
        .map(|(root, count)| PreviewRoot { root, count })
        .collect();
    roots.sort_by(|a, b| a.root.cmp(&b.root));
    let mut root_samples: Vec<PreviewRootSample> = root_samples.into_values().collect();
    root_samples.sort_by(|a, b| a.root.cmp(&b.root));

    Ok(PreviewResult {
        total: data.entries.len(),
        roots,
        sample,
        root_samples,
    })
}

#[tauri::command]
async fn check_recreate_conflicts(
    data: ExportData,
    dst_root: String,
    mappings: Vec<MappingRule>,
) -> Result<ConflictReport, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let _ = mappings;
        validate_data(&data)?;
        if dst_root.trim().is_empty() {
            return Err("Select a valid target root.".to_string());
        }

        let mut total = 0usize;
        let mut non_symlink = 0usize;
        let mut sample = Vec::new();

        for entry in data.entries.iter() {
            let link = link_path(&dst_root, &entry.relative);
            if let Ok(metadata) = fs::symlink_metadata(&link) {
                total += 1;
                if !metadata.file_type().is_symlink() {
                    non_symlink += 1;
                }
                if sample.len() < 10 {
                    sample.push(normalize_path_display(&link));
                }
            }
        }

        Ok(ConflictReport {
            total,
            non_symlink,
            sample,
        })
    })
    .await
    .map_err(|err| err.to_string())?
}

#[tauri::command]
async fn recreate_symlinks(
    data: ExportData,
    dst_root: String,
    mappings: Vec<MappingRule>,
    missing_as_dir: bool,
) -> Result<RecreateResult, String> {
    tauri::async_runtime::spawn_blocking(move || {
        recreate_symlinks_inner(data, dst_root, mappings, missing_as_dir)
    })
    .await
    .map_err(|err| err.to_string())?
}

#[tauri::command]
fn start_admin_recreate(
    data: ExportData,
    dst_root: String,
    mappings: Vec<MappingRule>,
    missing_as_dir: bool,
) -> Result<String, String> {
    #[cfg(windows)]
    {
        let job_id = format!(
            "{}_{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map_err(|err| err.to_string())?
                .as_nanos()
        );
        let job = AdminJob {
            data,
            dst_root,
            mappings,
            missing_as_dir,
            job_id: job_id.clone(),
        };
        let job_path = write_admin_job(&job)?;
        if let Err(err) = launch_admin_recreate(&job_path) {
            let _ = fs::remove_file(job_path);
            return Err(err);
        }
        Ok(job_id)
    }
    #[cfg(not(windows))]
    {
        let _ = data;
        let _ = dst_root;
        let _ = mappings;
        let _ = missing_as_dir;
        Err("Admin elevation is only available on Windows.".to_string())
    }
}

#[tauri::command]
fn poll_admin_result(job_id: String) -> Result<Option<AdminResult>, String> {
    if job_id.is_empty() || !job_id.chars().all(|c| c.is_ascii_digit() || c == '_') {
        return Err("Invalid admin job ID".into());
    }
    #[cfg(windows)]
    {
        let path = admin_result_path(&job_id);
        if !path.exists() {
            return Ok(None);
        }
        let raw = fs::read_to_string(&path).map_err(|err| err.to_string())?;
        let result: AdminResult = serde_json::from_str(&raw).map_err(|err| err.to_string())?;
        let _ = fs::remove_file(&path);
        Ok(Some(result))
    }
    #[cfg(not(windows))]
    {
        let _ = job_id;
        Err("Admin elevation is only available on Windows.".to_string())
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            let window = app
                .get_webview_window("main")
                .ok_or("main window not found")?;
            let icon = Image::from_bytes(include_bytes!("../icons/128x128.png"))?;
            let _ = window.set_icon(icon);
            restore_window_state(&window);
            if let Some(path) = window_state_path(&window) {
                persist_window_state(&window, &path);
                let window_for_events = window.clone();
                window.on_window_event(move |event| {
                    if matches!(
                        event,
                        WindowEvent::Moved(_)
                            | WindowEvent::Resized(_)
                            | WindowEvent::CloseRequested { .. }
                    ) {
                        persist_window_state(&window_for_events, &path);
                    }
                });
            }
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            scan_symlinks,
            export_symlinks,
            load_export,
            preview_recreate,
            check_recreate_conflicts,
            recreate_symlinks,
            start_admin_recreate,
            poll_admin_result
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

#[cfg(test)]
include!("scan_baseline.rs");

#[cfg(test)]
mod tests;
