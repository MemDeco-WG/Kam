use super::cache::{index_cache_path, module_cache_path, module_url, write_atomic};
use super::{SEARCH_INDEX_PATH, SearchEntry};
use crate::errors::KamError;
use crate::utils::Utils;
use indicatif::{MultiProgress, ProgressBar, ProgressStyle};
use reqwest::blocking::Client;
use reqwest::header::USER_AGENT;
use std::io::{IsTerminal, Read};
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

type FetchTask = (String, String, PathBuf, ProgressBar);

/// # Errors
/// Returns `KamError` when network, I/O, or JSON parsing operations fail.
pub fn repo_sync_with_jobs(
    base_url: &str,
    force: bool,
    jobs: Option<usize>,
    quiet: bool,
) -> Result<(), KamError> {
    let client = Client::builder()
        .timeout(Duration::from_secs(30))
        .build()
        .map_err(|e| KamError::FetchFailed(format!("Failed to build HTTP client: {e}")))?;

    let op_start = std::time::Instant::now();
    let body = fetch_index_body(&client, base_url, quiet)?;
    let path = index_cache_path(base_url)?;
    write_atomic(&path, &body)?;
    report_index_sync(base_url, force, quiet, &path);

    let entries = parse_index(base_url, &body)?;
    if entries.is_empty() {
        return Ok(());
    }
    report_resolution_time(&entries, quiet, op_start);

    let show_progress = !quiet && std::io::stdout().is_terminal();
    let mp = MultiProgress::new();
    let top_pb = top_progress(&mp, entries.len(), show_progress);
    let need_fetch_count = count_needed_fetches(&entries, force);
    if need_fetch_count == 0 {
        finish_up_to_date(quiet, &top_pb);
        return Ok(());
    }

    let workers = worker_count(jobs, entries.len());
    let updated_count = Arc::new(AtomicUsize::new(0));
    let tasks = build_tasks(
        base_url,
        &entries,
        force,
        workers,
        show_progress,
        &mp,
        &top_pb,
    );
    run_fetch_tasks(tasks, workers, force, &client, &top_pb, &updated_count);
    report_updated_count(quiet, updated_count.load(Ordering::SeqCst));
    top_pb.finish();
    Ok(())
}

fn fetch_index_body(client: &Client, base_url: &str, quiet: bool) -> Result<String, KamError> {
    let url = format!("{base_url}{SEARCH_INDEX_PATH}");
    let mut resp = client
        .get(&url)
        .header(USER_AGENT, "kam/repo-sync")
        .send()
        .map_err(|e| KamError::FetchFailed(format!("GET {url} failed: {e}")))?;

    if !resp.status().is_success() {
        return Err(KamError::FetchFailed(format!(
            "{} returned status {}",
            url,
            resp.status()
        )));
    }

    let pb = index_progress(resp.content_length(), quiet);
    let mut buf = Vec::new();
    let mut tmp = [0u8; 8 * 1024];
    loop {
        let n = resp
            .read(&mut tmp)
            .map_err(|e| KamError::FetchFailed(format!("Failed to read {url} body: {e}")))?;
        if n == 0 {
            break;
        }
        buf.extend_from_slice(&tmp[..n]);
        pb.inc(n as u64);
    }
    pb.finish();

    String::from_utf8(buf)
        .map_err(|e| KamError::Json(format!("Failed to parse {url} body as UTF-8: {e}")))
}

fn index_progress(index_len: Option<u64>, quiet: bool) -> ProgressBar {
    let show_progress = !quiet && std::io::stdout().is_terminal();
    if !show_progress {
        return ProgressBar::hidden();
    }
    let pb = index_len.map_or_else(ProgressBar::new_spinner, ProgressBar::new);
    pb.set_style(
        ProgressStyle::with_template("{spinner} Fetching index {bytes}/{total_bytes} ({eta})")
            .unwrap_or_else(|_| ProgressStyle::default_bar()),
    );
    pb.enable_steady_tick(Duration::from_millis(100));
    pb
}

fn parse_index(base_url: &str, body: &str) -> Result<Vec<SearchEntry>, KamError> {
    let url = format!("{base_url}{SEARCH_INDEX_PATH}");
    serde_json::from_str(body)
        .map_err(|e| KamError::Json(format!("Failed to parse {url} JSON: {e}")))
}

fn report_index_sync(base_url: &str, force: bool, quiet: bool, path: &std::path::Path) {
    if quiet {
        return;
    }
    let url = format!("{base_url}{SEARCH_INDEX_PATH}");
    if force {
        Utils::success(&trf!(
            "repo.index_force_synced",
            url,
            path.display().to_string()
        ));
    } else {
        Utils::success(trf!("repo.index_synced", path.display().to_string()));
    }
}

fn report_resolution_time(entries: &[SearchEntry], quiet: bool, op_start: std::time::Instant) {
    if !quiet {
        Utils::info(format!(
            "Resolved {count} modules in {ms}ms",
            count = entries.len(),
            ms = op_start.elapsed().as_millis()
        ));
    }
}

fn top_progress(mp: &MultiProgress, len: usize, show_progress: bool) -> ProgressBar {
    if show_progress {
        let pb = mp.add(ProgressBar::new_spinner());
        pb.set_style(
            ProgressStyle::with_template("{spinner} Preparing modules... ({pos}/{len})")
                .unwrap_or_else(|_| ProgressStyle::default_bar()),
        );
        pb.set_length(len as u64);
        pb.enable_steady_tick(Duration::from_millis(80));
        pb
    } else {
        ProgressBar::hidden()
    }
}

fn count_needed_fetches(entries: &[SearchEntry], force: bool) -> usize {
    entries
        .iter()
        .filter(|entry| {
            let module_cache = module_cache_path(&entry.name);
            !matches!((&module_cache, force), (Ok(p), false) if p.exists())
        })
        .count()
}

fn finish_up_to_date(quiet: bool, top_pb: &ProgressBar) {
    if quiet {
        top_pb.finish();
        return;
    }
    let msg = crate::i18n::tr("repo.everything_up_to_date");
    Utils::success(&msg);
    top_pb.finish_with_message(msg);
}

fn worker_count(jobs: Option<usize>, entries_len: usize) -> usize {
    let default_workers =
        std::thread::available_parallelism().map_or(1, std::num::NonZeroUsize::get);
    let workers = match jobs {
        Some(j) if j > 0 => j,
        _ => std::env::var("KAM_REPO_CONCURRENCY")
            .ok()
            .and_then(|s| s.parse::<usize>().ok())
            .filter(|&n| n > 0)
            .unwrap_or(default_workers),
    };
    std::cmp::min(workers, entries_len)
}

fn build_tasks(
    base_url: &str,
    entries: &[SearchEntry],
    force: bool,
    display_limit: usize,
    show_progress: bool,
    mp: &MultiProgress,
    top_pb: &ProgressBar,
) -> Vec<FetchTask> {
    let mut tasks = Vec::with_capacity(entries.len());
    let mut visible_cnt = 0usize;

    for entry in entries {
        let module_id = entry.name.clone();
        let module_cache = module_cache_path(&module_id);
        let cached = matches!((&module_cache, force), (Ok(p), false) if p.exists());
        let visible = visible_cnt < display_limit;
        let pb = module_progress(mp, &module_id, visible && show_progress);
        if visible {
            visible_cnt += 1;
        }

        if cached {
            finish_cached(&pb, &module_id);
            top_pb.inc(1);
        } else if let Ok(path) = module_cache {
            tasks.push((
                module_id.clone(),
                module_url(base_url, &module_id),
                path,
                pb,
            ));
        } else {
            pb.finish_with_message(format!("{module_id:20}"));
            top_pb.inc(1);
        }
    }
    tasks
}

fn module_progress(mp: &MultiProgress, module_id: &str, visible: bool) -> ProgressBar {
    let pb = if visible {
        let p = mp.add(ProgressBar::new_spinner());
        p.set_style(
            ProgressStyle::with_template("{msg:20} {bar:40.cyan/blue} {bytes}/{total_bytes}")
                .unwrap_or_else(|_| ProgressStyle::default_bar()),
        );
        p
    } else {
        ProgressBar::hidden()
    };
    pb.set_message(format!("{module_id:20}"));
    pb
}

fn finish_cached(pb: &ProgressBar, module_id: &str) {
    pb.set_length(1);
    pb.set_position(1);
    pb.finish_with_message(format!("{module_id:20} (cached)"));
}

fn run_fetch_tasks(
    tasks: Vec<FetchTask>,
    workers: usize,
    force: bool,
    client: &Client,
    top_pb: &ProgressBar,
    updated_count: &Arc<AtomicUsize>,
) {
    if tasks.is_empty() {
        return;
    }
    let worker_count = std::cmp::max(1, workers);
    let (tx, rx) = crossbeam_channel::unbounded::<FetchTask>();
    for task in tasks {
        let _ = tx.send(task);
    }
    drop(tx);

    let mut handles = Vec::new();
    for _ in 0..worker_count {
        let rx_clone = rx.clone();
        let client_clone = client.clone();
        let top = top_pb.clone();
        let updated = Arc::clone(updated_count);
        handles.push(std::thread::spawn(move || {
            while let Ok(task) = rx_clone.recv() {
                fetch_one_task(task, force, &client_clone, &top, &updated);
            }
        }));
    }
    for handle in handles {
        let _ = handle.join();
    }
}

fn fetch_one_task(
    task: FetchTask,
    force: bool,
    client: &Client,
    top: &ProgressBar,
    updated: &AtomicUsize,
) {
    let (module_id, url, path, pb) = task;
    if path.exists() && !force {
        finish_cached(&pb, &module_id);
        top.inc(1);
        return;
    }

    match fetch_module_body(client, &url, &pb, &module_id) {
        Ok(body) => {
            if let Err(e) = write_atomic(&path, &body) {
                Utils::warn(format!("Failed to write cache {module_id}: {e}"));
                pb.finish_with_message(format!("{module_id:20} (failed)"));
            } else {
                updated.fetch_add(1, Ordering::SeqCst);
                pb.finish_with_message(format!("{module_id:20}"));
            }
        }
        Err(message) => {
            Utils::warn(message);
            pb.finish_with_message(format!("{module_id:20} (failed)"));
        }
    }
    top.inc(1);
}

fn fetch_module_body(
    client: &Client,
    url: &str,
    pb: &ProgressBar,
    module_id: &str,
) -> Result<String, String> {
    let mut response = client
        .get(url)
        .header(USER_AGENT, "kam/repo-sync-module")
        .send()
        .map_err(|e| format!("GET {url} failed: {e}"))?;
    if !response.status().is_success() {
        return Err(format!(
            "{url} returned status {status}",
            status = response.status()
        ));
    }
    if let Some(len) = response.content_length() {
        pb.set_length(len);
    }

    let mut buf = Vec::new();
    let mut tmp = [0u8; 8 * 1024];
    loop {
        match response.read(&mut tmp) {
            Ok(n) if n > 0 => {
                buf.extend_from_slice(&tmp[..n]);
                pb.inc(n as u64);
            }
            Ok(_) => break,
            Err(e) => return Err(format!("Failed to read {url} body: {e}")),
        }
    }
    String::from_utf8(buf)
        .map_err(|_| format!("Failed to parse {url} body as UTF-8 for {module_id}"))
}

fn report_updated_count(quiet: bool, updated_total: usize) {
    if quiet {
        return;
    }
    if updated_total == 0 {
        Utils::success(crate::i18n::tr("repo.everything_up_to_date"));
    } else {
        Utils::success(trf!("repo.updated_modules", updated_total.to_string()));
    }
}
