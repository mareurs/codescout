//! Embedder leak probe for the memory-leak-x-session-freeze investigation.
//!
//! Calls `build_index` N times in a row on a tiny synthetic project and prints
//! VmSize/VmRSS/VmData after each iteration. If memory grows monotonically
//! across iterations, the indexer's per-call embedder construction (or its
//! ONNX Runtime backing) is the culprit and the fix is to route through
//! `Agent::get_or_create_embedder`.
//!
//! Run:
//!     cargo run --release --example embed_leak_probe -- 50
//!
//! With a custom model (defaults to a small local fastembed model):
//!     CODESCOUT_EMBED_MODEL=local:AllMiniLML6V2Q \
//!         cargo run --release --example embed_leak_probe -- 50

use codescout::embed::index::build_index;
use std::path::Path;
use std::time::Instant;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let iters: usize = std::env::args()
        .nth(1)
        .and_then(|s| s.parse().ok())
        .unwrap_or(20);

    let dir = tempfile::tempdir()?;
    let root = dir.path();
    seed_project(root)?;

    if std::env::var("CODESCOUT_EMBED_MODEL").is_err() {
        // SAFETY: probe is single-threaded at this point, before any tokio
        // tasks that read env vars. Setting before spawning is safe.
        unsafe { std::env::set_var("CODESCOUT_EMBED_MODEL", "local:AllMiniLML6V2Q") };
    }

    println!("iter,elapsed_ms,indexed_files,vm_size_kb,vm_rss_kb,vm_data_kb,vm_peak_kb");
    let baseline = read_self_memory_kb();
    println!(
        "baseline,0,0,{},{},{},{}",
        baseline.size, baseline.rss, baseline.data, baseline.peak
    );

    for i in 1..=iters {
        let t = Instant::now();
        let report = build_index(root, /*force=*/ true, None).await?;
        let elapsed_ms = t.elapsed().as_millis();
        let m = read_self_memory_kb();
        println!(
            "{},{},{},{},{},{},{}",
            i, elapsed_ms, report.indexed, m.size, m.rss, m.data, m.peak
        );
    }

    Ok(())
}

fn seed_project(root: &Path) -> anyhow::Result<()> {
    std::fs::create_dir_all(root.join("src"))?;
    for i in 0..10 {
        let path = root.join(format!("src/mod_{i}.rs"));
        let body = (0..40)
            .map(|j| format!("pub fn fn_{i}_{j}(x: i32) -> i32 {{ x + {j} }}"))
            .collect::<Vec<_>>()
            .join("\n");
        std::fs::write(path, body)?;
    }
    Ok(())
}

#[derive(Default, Copy, Clone)]
struct Mem {
    size: u64,
    rss: u64,
    data: u64,
    peak: u64,
}

fn read_self_memory_kb() -> Mem {
    let mut out = Mem::default();
    let Ok(text) = std::fs::read_to_string("/proc/self/status") else {
        return out;
    };
    for line in text.lines() {
        let Some((key, rest)) = line.split_once(':') else {
            continue;
        };
        let v = rest
            .split_whitespace()
            .next()
            .and_then(|n| n.parse::<u64>().ok())
            .unwrap_or(0);
        match key {
            "VmSize" => out.size = v,
            "VmRSS" => out.rss = v,
            "VmData" => out.data = v,
            "VmPeak" => out.peak = v,
            _ => {}
        }
    }
    out
}
