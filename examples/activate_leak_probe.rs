//! Activate-project leak probe for the memory-leak-x-session-freeze
//! investigation.
//!
//! Creates N synthetic project directories, then loops `Agent::activate` across
//! them M times each (so M*N total activations). Logs VmSize/VmRSS/VmData per
//! iteration. If memory grows linearly with activations, per-project state
//! retained in `Agent::inner` after a switch is the leak source.
//!
//! Run:
//!     cargo run --release --example activate_leak_probe -- 8 50
//! (8 distinct projects, 50 round-robin passes = 400 activations)

use codescout::agent::Agent;
use std::path::Path;
use std::time::Instant;
use tempfile::TempDir;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let mut args = std::env::args().skip(1);
    let n_projects: usize = args.next().and_then(|s| s.parse().ok()).unwrap_or(8);
    let n_passes: usize = args.next().and_then(|s| s.parse().ok()).unwrap_or(50);

    let dirs: Vec<TempDir> = (0..n_projects)
        .map(|i| {
            let dir = tempfile::tempdir()?;
            seed_project(dir.path(), i)?;
            anyhow::Ok(dir)
        })
        .collect::<anyhow::Result<_>>()?;

    let agent = Agent::new(None).await?;

    println!("iter,project,elapsed_ms,vm_size_kb,vm_rss_kb,vm_data_kb,vm_peak_kb");
    let baseline = read_self_memory_kb();
    println!(
        "baseline,-,0,{},{},{},{}",
        baseline.size, baseline.rss, baseline.data, baseline.peak
    );

    let mut iter = 0usize;
    for pass in 0..n_passes {
        for (idx, dir) in dirs.iter().enumerate() {
            iter += 1;
            let t = Instant::now();
            agent
                .activate(dir.path().to_path_buf(), Some(false))
                .await?;
            let elapsed_ms = t.elapsed().as_millis();
            // Sample memory only once per pass to keep output small.
            if idx == 0 || pass == n_passes - 1 {
                let m = read_self_memory_kb();
                println!(
                    "{},{},{},{},{},{},{}",
                    iter, idx, elapsed_ms, m.size, m.rss, m.data, m.peak
                );
            }
        }
    }

    Ok(())
}

fn seed_project(root: &Path, idx: usize) -> anyhow::Result<()> {
    std::fs::create_dir_all(root.join("src"))?;
    for i in 0..3 {
        let body = format!("pub fn f_{idx}_{i}(x: i32) -> i32 {{ x + {i} }}\n");
        std::fs::write(root.join(format!("src/m{i}.rs")), body)?;
    }
    let cargo =
        format!("[package]\nname = \"probe_p{idx}\"\nversion = \"0.0.1\"\nedition = \"2021\"\n");
    std::fs::write(root.join("Cargo.toml"), cargo)?;
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
