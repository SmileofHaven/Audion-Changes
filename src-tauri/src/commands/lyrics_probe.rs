// diagnostic only probe for lyrics sidecar read/delete on android
// logs every entry before any filtering

use std::fs;
use std::path::{Path, PathBuf};
use tauri::{AppHandle, State};

use crate::db::{queries, Database};

const KNOWN_FORMATS: &[&str] = &["lrc", "ttml", "xml", "srt", "json"];

fn describe_meta(p: &Path) -> String {
    match fs::symlink_metadata(p) {
        Ok(m) => format!(
            "ok(is_file={} is_dir={} is_symlink={} len={})",
            m.is_file(),
            m.is_dir(),
            m.file_type().is_symlink(),
            m.len()
        ),
        Err(e) => format!("err(kind={:?} msg={})", e.kind(), e),
    }
}

// raw bytes of the name catch invisible/unicode differences
// that a plain string compare would hide
fn name_bytes(name: &str) -> String {
    name.bytes().map(|b| format!("{:02x}", b)).collect::<Vec<_>>().join(" ")
}

#[tauri::command]
pub async fn lyrics_fs_probe(
    _app: AppHandle,
    db: State<'_, Database>,
    dir: Option<String>,
    token: String,
    delete: bool,
    limit: Option<usize>,
) -> Result<String, String> {
    let limit = limit.unwrap_or(2000);
    let mut out: Vec<String> = Vec::new();
    macro_rules! log {
        ($($a:tt)*) => {{
            let line = format!($($a)*);
            tracing::info!("[PROBE] {}", line);
            out.push(line);
        }};
    }

    let music_paths: Vec<String> = {
        let conn = db.conn.lock().map_err(|e| e.to_string())?;
        queries::get_all_track_paths(&conn).map_err(|e| e.to_string())?
    };
    log!("tracks_in_db={} token={} delete={}", music_paths.len(), token, delete);

    // pick the probe dir: explicit arg, else the parent of the first db track
    let probe_dir: PathBuf = match dir {
        Some(d) => PathBuf::from(d),
        None => match music_paths.first().and_then(|p| Path::new(p).parent().map(|x| x.to_path_buf())) {
            Some(p) => p,
            None => return Err("no dir given and no tracks in db".into()),
        },
    };
    log!("probe_dir={}", probe_dir.display());
    log!("probe_dir meta: {}", describe_meta(&probe_dir));
    match probe_dir.canonicalize() {
        Ok(c) => log!("probe_dir canonical={}", c.display()),
        Err(e) => log!("probe_dir canonicalize failed: {}", e),
    }

    // strategy 1: read_dir, logging EVERY entry before any filter
    log!("--- strategy 1: read_dir unfiltered ---");
    let mut total = 0usize;
    let mut by_ext: std::collections::BTreeMap<String, usize> = Default::default();
    let mut candidates: Vec<PathBuf> = Vec::new();
    match fs::read_dir(&probe_dir) {
        Ok(entries) => {
            for entry in entries {
                match entry {
                    Ok(e) => {
                        total += 1;
                        let p = e.path();
                        let name = e.file_name().to_string_lossy().to_string();
                        let ext = p
                            .extension()
                            .map(|x| x.to_string_lossy().to_lowercase())
                            .unwrap_or_else(|| "<none>".into());
                        *by_ext.entry(ext.clone()).or_insert(0) += 1;

                        // file_type from the dirent vs a real stat, these can disagree
                        let ft = match e.file_type() {
                            Ok(t) => format!("dirent(is_file={} is_dir={} is_symlink={})", t.is_file(), t.is_dir(), t.is_symlink()),
                            Err(err) => format!("dirent_err({})", err),
                        };

                        if ext == "json" || name.contains(&token) {
                            log!("HIT name={} ext={} {} stat={} bytes={}", name, ext, ft, describe_meta(&p), name_bytes(&name));
                            candidates.push(p);
                        } else if total <= limit {
                            log!("entry name={} ext={} {} is_file_guard={}", name, ext, ft, p.is_file());
                        }
                    }
                    Err(e) => log!("read_dir entry error: {}", e),
                }
            }
            log!("read_dir total={} ext_counts={:?}", total, by_ext);
        }
        Err(e) => log!("read_dir FAILED: kind={:?} msg={}", e.kind(), e),
    }

    // strategy 2: walkdir, different traversal impl than read_dir
    log!("--- strategy 2: walkdir depth=1 ---");
    let mut wd_json = 0usize;
    let mut wd_total = 0usize;
    for entry in walkdir::WalkDir::new(&probe_dir).max_depth(1).into_iter() {
        match entry {
            Ok(e) => {
                wd_total += 1;
                let name = e.file_name().to_string_lossy().to_string();
                if name.to_lowercase().ends_with(".json") {
                    wd_json += 1;
                    if wd_json <= limit {
                        log!("walkdir json: {} stat={}", e.path().display(), describe_meta(e.path()));
                    }
                }
            }
            Err(e) => log!("walkdir error: {}", e),
        }
    }
    log!("walkdir total={} json_count={}", wd_total, wd_json);

    // strategy 3: direct construction, no listing at all
    // proves whether the file is reachable when we already know its name
    log!("--- strategy 3: direct path probe ---");
    let mut direct_hits: Vec<PathBuf> = Vec::new();
    for music_path in music_paths.iter() {
        let mp = Path::new(music_path);
        if mp.parent() != Some(probe_dir.as_path()) {
            continue;
        }
        let Some(stem) = mp.file_stem().and_then(|s| s.to_str()) else { continue };
        for fmt in KNOWN_FORMATS {
            let cand = probe_dir.join(format!("{}.{}.{}", stem, token, fmt));
            let exists = cand.exists();
            let meta = describe_meta(&cand);
            // File::open is a different syscall path than stat, try it too
            let openable = match fs::File::open(&cand) {
                Ok(_) => "open_ok".to_string(),
                Err(e) => format!("open_err(kind={:?})", e.kind()),
            };
            if exists || !openable.starts_with("open_err(kind=NotFound") {
                log!("direct HIT {} exists={} stat={} {}", cand.display(), exists, meta, openable);
                direct_hits.push(cand);
            }
        }
    }
    log!("direct_hits={}", direct_hits.len());

    // strategy 4: read a hit to confirm content is actually reachable
    log!("--- strategy 4: read content ---");
    let mut readable: Vec<PathBuf> = Vec::new();
    for p in candidates.iter().chain(direct_hits.iter()).take(limit) {
        match fs::read(p) {
            Ok(b) => {
                log!("read ok {} bytes={}", p.display(), b.len());
                readable.push(p.clone());
            }
            Err(e) => log!("read FAILED {} kind={:?} msg={}", p.display(), e.kind(), e),
        }
    }

    // strategy 5: delete, only when explicitly asked
    if delete {
        log!("--- strategy 5: delete ---");
        for p in readable.iter() {
            match fs::remove_file(p) {
                Ok(()) => log!("remove_file ok {}", p.display()),
                Err(e) => {
                    log!("remove_file FAILED {} kind={:?} msg={}", p.display(), e.kind(), e);
                    // a rename into the same dir tells us if it is a permission
                    // problem or a delete specific restriction
                    let tmp = p.with_extension("probetmp");
                    match fs::rename(p, &tmp) {
                        Ok(()) => {
                            log!("rename fallback ok => {}", tmp.display());
                            let _ = fs::rename(&tmp, p);
                        }
                        Err(e2) => log!("rename fallback FAILED kind={:?} msg={}", e2.kind(), e2),
                    }
                    // truncating to zero proves write access even if unlink is blocked
                    match fs::OpenOptions::new().write(true).truncate(true).open(p) {
                        Ok(_) => log!("truncate probe ok (write allowed, unlink blocked)"),
                        Err(e3) => log!("truncate probe FAILED kind={:?}", e3.kind()),
                    }
                }
            }
        }
    } else {
        log!("--- strategy 5: delete skipped (delete=false) ---");
    }

    log!("probe done");
    Ok(out.join("\n"))
}