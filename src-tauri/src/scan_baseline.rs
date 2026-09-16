#[cfg(test)]
fn scan_baseline(root: String) -> Result<ScanResult, String> {
    let root_path = PathBuf::from(root.trim());
    if !root_path.is_dir() {
        return Err("Invalid scan folder.".to_string());
    }

    let mut entries = Vec::new();
    let mut skipped = 0usize;
    for entry in WalkDir::new(&root_path).follow_links(false) {
        let entry = match entry {
            Ok(value) => value,
            Err(_) => {
                skipped += 1;
                continue;
            }
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
            Err(_) => {
                entries.push(SymlinkEntry {
                    relative: normalize_path_display(relative),
                    target: "<unreadable>".to_string(),
                    status: "Unreadable".to_string(),
                    link_is_dir: infer_link_is_dir(Path::new(""), link_path),
                });
                continue;
            }
        };
        let target_abs = if target.is_absolute() {
            target
        } else {
            link_path
                .parent()
                .unwrap_or(&root_path)
                .join(target)
        };
        let status = match fs::metadata(&target_abs) {
            Ok(_) => "OK",
            Err(err) => {
                if err.kind() == ErrorKind::PermissionDenied {
                    "Unreadable"
                } else {
                    "Broken"
                }
            }
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
        skipped,
    })
}

