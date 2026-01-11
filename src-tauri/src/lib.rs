use serde::{Deserialize, Serialize};
use std::fs;
use std::time::{SystemTime, UNIX_EPOCH};
use std::path::{Path, PathBuf};
use walkdir::WalkDir;
use tauri::{image::Image, Manager};
#[cfg(windows)]
use std::ffi::OsStr;
#[cfg(windows)]
use std::os::windows::ffi::OsStrExt;
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
struct PreviewResult {
    total: usize,
    roots: Vec<PreviewRoot>,
    sample: Vec<PreviewItem>,
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
}

fn normalize_path_display(path: &Path) -> String {
    let raw = path.to_string_lossy().to_string();
    if raw.starts_with(r"\\?\") {
        raw.trim_start_matches(r"\\?\").to_string()
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

    if input.starts_with("//") {
        let rest = &input[2..];
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
        let first = rest.split('/').filter(|s| !s.is_empty()).next();
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
        .filter(|part| !part.is_empty())
        .next()
        .unwrap_or("")
        .to_string()
}

fn join_root(root: &str, suffix: &str) -> String {
    if suffix.is_empty() {
        return root.to_string();
    }
    let mut out = root.to_string();
    if !out.ends_with('/') && !out.ends_with('\\') {
        out.push('/');
    }
    out.push_str(suffix);
    out
}

fn replace_root(target: &str, from: &str, to: &str) -> String {
    if from.trim().is_empty() || to.trim().is_empty() {
        return target.to_string();
    }
    let target_norm = normalize_for_match(target);
    let from_norm = normalize_for_match(from);
    if target_norm == from_norm || target_norm.starts_with(&(from_norm.clone() + "/")) {
        let mut suffix = &target_norm[from_norm.len()..];
        if let Some(stripped) = suffix.strip_prefix('/') {
            suffix = stripped;
        }
        return join_root(to, suffix);
    }
    target.to_string()
}

fn apply_mappings(target: &str, mappings: &[MappingRule]) -> String {
    let target_norm = normalize_for_match(target);
    for mapping in mappings {
        if mapping.from.trim().is_empty() || mapping.to.trim().is_empty() {
            continue;
        }
        let from_norm = normalize_for_match(&mapping.from);
        if target_norm == from_norm || target_norm.starts_with(&(from_norm.clone() + "/")) {
            let mut suffix = &target_norm[from_norm.len()..];
            if let Some(stripped) = suffix.strip_prefix('/') {
                suffix = stripped;
            }
            return join_root(&mapping.to, suffix);
        }
    }
    target.to_string()
}

fn remap_target(target: &str, mappings: &[MappingRule], src_root: &str, dst_root: &str) -> String {
    let mapped = apply_mappings(target, mappings);
    replace_root(&mapped, src_root, dst_root)
}

fn link_path(dst_root: &str, relative: &str) -> PathBuf {
    PathBuf::from(dst_root).join(relative)
}

fn recreate_symlinks_inner(
    data: ExportData,
    dst_root: String,
    mappings: Vec<MappingRule>,
    missing_as_dir: bool,
) -> Result<RecreateResult, String> {
    if dst_root.trim().is_empty() {
        return Err("Select a valid target root.".to_string());
    }

    let mut created = 0usize;
    let mut failed = Vec::new();
    let mut sample_links = Vec::new();

    for entry in data.entries {
        let link = link_path(&dst_root, &entry.relative);
        let target = remap_target(&entry.target, &mappings, &data.src_root, &dst_root);
        let target_path = PathBuf::from(&target);

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

        if fs::symlink_metadata(&link).is_ok() {
            if fs::remove_file(&link).is_err() {
                let _ = fs::remove_dir(&link);
            }
        }

        let link_is_dir = entry
            .link_is_dir
            .or_else(|| infer_link_is_dir(&target_path, &link))
            .unwrap_or(missing_as_dir);

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
        .as_secs();
    let filename = format!("symlinkmanager_admin_{}_{}.json", std::process::id(), now);
    let path = std::env::temp_dir().join(filename);
    let payload = serde_json::to_string_pretty(job).map_err(|err| err.to_string())?;
    fs::write(&path, payload).map_err(|err| err.to_string())?;
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
    let result = recreate_symlinks_inner(
        job.data,
        job.dst_root,
        job.mappings,
        job.missing_as_dir,
    );
    if let Ok(result) = result {
        let payload = AdminResult {
            created: result.created,
            failed: result.failed.len(),
            sample_links: result.sample_links,
        };
        let result_path = admin_result_path(&job.job_id);
        if let Ok(json) = serde_json::to_string_pretty(&payload) {
            let _ = fs::write(result_path, json);
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
fn scan_symlinks(root: String) -> Result<ScanResult, String> {
    let root_path = PathBuf::from(root.trim());
    if !root_path.is_dir() {
        return Err("Invalid scan folder.".to_string());
    }

    let mut entries = Vec::new();
    for entry in WalkDir::new(&root_path).follow_links(false) {
        let entry = match entry {
            Ok(value) => value,
            Err(_) => continue,
        };
        if !entry.file_type().is_symlink() {
            continue;
        }
        let link_path = entry.path();
        let relative = match link_path.strip_prefix(&root_path) {
            Ok(rel) => rel,
            Err(_) => continue,
        };
        let target = match fs::read_link(link_path) {
            Ok(value) => value,
            Err(_) => continue,
        };
        let target_abs = if target.is_absolute() {
            target
        } else {
            link_path
                .parent()
                .unwrap_or(&root_path)
                .join(target)
        };
        let status = if fs::metadata(&target_abs).is_ok() {
            "OK"
        } else {
            "Broken"
        };
        let link_is_dir = infer_link_is_dir(&target_abs, link_path);
        entries.push(SymlinkEntry {
            relative: normalize_path_display(relative),
            target: normalize_path_display(&target_abs),
            status: status.to_string(),
            link_is_dir,
        });
    }

    Ok(ScanResult {
        src_root: normalize_path_display(&root_path),
        entries,
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
    serde_json::from_str(&raw).map_err(|err| err.to_string())
}

#[tauri::command]
fn preview_recreate(
    data: ExportData,
    dst_root: String,
    mappings: Vec<MappingRule>,
    max_preview: usize,
) -> Result<PreviewResult, String> {
    if dst_root.trim().is_empty() {
        return Err("Select a valid target root.".to_string());
    }

    let mut sample = Vec::new();
    for entry in data.entries.iter().take(max_preview) {
        let link = link_path(&dst_root, &entry.relative);
        let target = remap_target(&entry.target, &mappings, &data.src_root, &dst_root);
        sample.push(PreviewItem {
            link: normalize_path_display(&link),
            target,
        });
    }
    let mut counts: std::collections::HashMap<String, usize> = std::collections::HashMap::new();
    for entry in data.entries.iter() {
        let root = extract_root_bucket(&entry.target);
        *counts.entry(root).or_insert(0) += 1;
    }
    let mut roots: Vec<PreviewRoot> = counts
        .into_iter()
        .map(|(root, count)| PreviewRoot { root, count })
        .collect();
    roots.sort_by(|a, b| a.root.cmp(&b.root));

    Ok(PreviewResult {
        total: data.entries.len(),
        roots,
        sample,
    })
}

#[tauri::command]
fn check_recreate_conflicts(
    data: ExportData,
    dst_root: String,
    mappings: Vec<MappingRule>,
) -> Result<ConflictReport, String> {
    let _ = mappings;
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
}

#[tauri::command]
fn recreate_symlinks(
    data: ExportData,
    dst_root: String,
    mappings: Vec<MappingRule>,
    missing_as_dir: bool,
) -> Result<RecreateResult, String> {
    recreate_symlinks_inner(data, dst_root, mappings, missing_as_dir)
}

#[tauri::command]
fn recreate_symlinks_admin(
    data: ExportData,
    dst_root: String,
    mappings: Vec<MappingRule>,
    missing_as_dir: bool,
) -> Result<(), String> {
    #[cfg(windows)]
    {
        let job_id = format!(
            "{}_{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map_err(|err| err.to_string())?
                .as_secs()
        );
        let job = AdminJob {
            data,
            dst_root,
            mappings,
            missing_as_dir,
            job_id,
        };
        let job_path = write_admin_job(&job)?;
        launch_admin_recreate(&job_path)?;
        return Ok(());
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
fn start_admin_recreate(
    data: ExportData,
    dst_root: String,
    mappings: Vec<MappingRule>,
    missing_as_dir: bool,
) -> Result<String, String> {
    #[cfg(windows)]
    {
        let job_id = format!("{}_{}", std::process::id(), SystemTime::now().duration_since(UNIX_EPOCH).map_err(|err| err.to_string())?.as_secs());
        let job = AdminJob {
            data,
            dst_root,
            mappings,
            missing_as_dir,
            job_id: job_id.clone(),
        };
        let job_path = write_admin_job(&job)?;
        launch_admin_recreate(&job_path)?;
        return Ok(job_id);
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
    #[cfg(windows)]
    {
        let path = admin_result_path(&job_id);
        if !path.exists() {
            return Ok(None);
        }
        let raw = fs::read_to_string(&path).map_err(|err| err.to_string())?;
        let result: AdminResult = serde_json::from_str(&raw).map_err(|err| err.to_string())?;
        let _ = fs::remove_file(&path);
        return Ok(Some(result));
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
        .plugin(tauri_plugin_opener::init())
        .setup(|app| {
            let window = app
                .get_webview_window("main")
                .ok_or("main window not found")?;
            let icon = Image::from_bytes(include_bytes!("../icons/128x128.png"))?;
            let _ = window.set_icon(icon);
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            scan_symlinks,
            export_symlinks,
            load_export,
            preview_recreate,
            check_recreate_conflicts,
            recreate_symlinks,
            recreate_symlinks_admin,
            start_admin_recreate,
            poll_admin_result
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
