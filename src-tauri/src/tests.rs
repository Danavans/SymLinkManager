use super::*;
fn data(relative: &str) -> ExportData {
    ExportData {
        src_root: "/source".into(),
        entries: vec![SymlinkEntry {
            relative: relative.into(),
            target: "/target".into(),
            status: "OK".into(),
            link_is_dir: Some(false),
        }],
    }
}

#[test]
fn saved_window_needs_a_visible_work_area() {
    let state = SavedWindowState {
        width: 1000,
        height: 800,
        x: 1840,
        y: 100,
        maximized: false,
    };
    assert!(overlaps_work_area(state, 0, 0, 1920, 1040));
    assert!(!overlaps_work_area(state, -1920, 0, 1920, 1040));
}

#[test]
fn import_boundaries_and_remapping() {
    for path in [
        "../escape",
        "/absolute",
        "C:\\escape",
        "a/../../b",
        "a\\..\\b",
        "a/./b",
    ] {
        assert!(validate_data(&data(path)).is_err(), "{path}");
    }
    assert!(validate_data(&data("nested/file")).is_ok());
    #[cfg(windows)]
    for path in ["file:stream", "a./b", "CON", "aux.txt", "LPT1"] {
        assert!(validate_data(&data(path)).is_err());
    }
    #[cfg(unix)]
    for path in ["file:stream", "a./b"] {
        assert!(validate_data(&data(path)).is_ok());
    }
    let rules = vec![MappingRule {
        from: "/media".into(),
        to: "/new".into(),
    }];
    assert_eq!(apply_mappings("/media/show/file", &rules), "/new/show/file");
    assert_eq!(
        apply_mappings("/media-other/file", &rules),
        "/media-other/file"
    );
    assert_eq!(replace_root("/a/file", "/a", "/b"), "/b/file");
    assert_eq!(
        normalize_path_display(Path::new(r"\\?\UNC\server\share")),
        r"\\server\share"
    );
}
#[test]
#[ignore = "Creates 3000 real symlinks; run explicitly on a filesystem with symlink permission"]
fn benchmark_scan_3000() {
    let root = std::env::temp_dir().join(format!("symlinkmanager-bench-{}", std::process::id()));
    let external = std::env::var("SYMLINK_BENCH_ROOT").ok();
    let root = external.as_ref().map(PathBuf::from).unwrap_or(root);
    if external.is_none() {
        fs::create_dir(&root).unwrap();
        let targets = root.join("targets");
        fs::create_dir(&targets).unwrap();
        for i in 0..3000 {
            let target = targets.join(format!("target-{i}"));
            if i % 10 != 0 {
                fs::write(&target, b"test").unwrap();
            }
            let link = root.join(format!("link-{i}"));
            #[cfg(windows)]
            std::os::windows::fs::symlink_file(&target, &link)
                .expect("symlink permission required");
            #[cfg(unix)]
            std::os::unix::fs::symlink(&target, &link).unwrap();
        }
    }
    let root_text = root.to_string_lossy().into_owned();
    let mut before = Vec::new();
    let mut after = Vec::new();
    for round in 0..7 {
        // Alternate order to reduce warm-cache bias; discard the warmup.
        let (old, new) = if round % 2 == 0 {
            let t = std::time::Instant::now();
            let old = scan_baseline(root_text.clone()).unwrap();
            let a = t.elapsed();
            let t = std::time::Instant::now();
            let new = scan_inner(root_text.clone()).unwrap();
            let b = t.elapsed();
            if round > 0 {
                before.push(a);
                after.push(b);
            }
            (old, new)
        } else {
            let t = std::time::Instant::now();
            let new = scan_inner(root_text.clone()).unwrap();
            let b = t.elapsed();
            let t = std::time::Instant::now();
            let old = scan_baseline(root_text.clone()).unwrap();
            let a = t.elapsed();
            if round > 0 {
                before.push(a);
                after.push(b);
            }
            (old, new)
        };
        assert_eq!(old.entries.len(), 3000);
        assert_eq!(new.entries.len(), 3000);
        assert_eq!(
            new.entries.iter().filter(|e| e.status == "Broken").count(),
            300
        );
        let signature = |result: ScanResult| {
            let mut items: Vec<_> = result
                .entries
                .into_iter()
                .map(|e| (e.relative, e.target, e.status, e.link_is_dir))
                .collect();
            items.sort();
            items
        };
        assert_eq!(signature(old), signature(new));
    }
    before.sort();
    after.sort();
    let before_median = (before[2] + before[3]) / 2;
    let after_median = (after[2] + after[3]) / 2;
    println!("3000 links / 2700 valid / 300 broken: baseline {:?}, optimized {:?}, speedup {:.2}x (median of 6 warm runs)", before_median, after_median, before_median.as_secs_f64()/after_median.as_secs_f64());
    // Only this newly created, process-specific fixture is removed.
    if external.is_none() {
        assert_eq!(root.parent(), Some(std::env::temp_dir().as_path()));
        fs::remove_dir_all(root).unwrap();
    }
}

#[test]
fn failed_replacement_restores_original() {
    let root = std::env::temp_dir().join(format!(
        "symlinkmanager-rollback-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    fs::create_dir(&root).unwrap();
    let original = root.join("original.txt");
    fs::write(&original, b"must survive").unwrap();
    let mut input = data("original.txt");
    input.entries[0].target = "x".repeat(40000);
    let result =
        recreate_symlinks_inner(input, root.to_string_lossy().into(), vec![], false).unwrap();
    assert_eq!(result.created, 0);
    assert!(!result.failed.is_empty());
    assert_eq!(fs::read(&original).unwrap(), b"must survive");
    assert_eq!(fs::read_dir(&root).unwrap().count(), 1);
    fs::remove_file(&original).unwrap();
    fs::remove_dir(root).unwrap();
}

#[test]
fn root_mapping_handles_root_and_unicode() {
    assert_eq!(replace_root("/media/show", "/", "/new"), "/new/media/show");
    assert_eq!(
        replace_root("//server/share", "/", "/new"),
        "//server/share"
    );
    assert_eq!(replace_root("/İ/file", "/İ", "/new"), "/new/file");
    let rules = vec![
        MappingRule {
            from: "/a".into(),
            to: "/a".into(),
        },
        MappingRule {
            from: "/a".into(),
            to: "/b".into(),
        },
    ];
    assert_eq!(apply_mappings("/a/file", &rules), "/a/file");
}
