use crate::e2e::nav_eval::cases;
use crate::e2e::nav_eval::report::Report;
use crate::e2e::nav_eval::runner::{nav_eval_context, run_one};
use chrono::Local;
use std::path::PathBuf;

#[tokio::test(flavor = "multi_thread")]
#[ignore]
async fn run_nav_eval() {
    let ctx = nav_eval_context().await;
    let mut report = Report::new();

    for case in cases::all() {
        let result = run_one(&ctx, case).await;
        report.push(case, result);
    }

    let date = Local::now().format("%Y-%m-%d").to_string();
    let round = next_round_number(&date);
    let out = PathBuf::from(format!(
        "docs/superpowers/specs/{date}-nav-eval-round-{round}.md"
    ));
    std::fs::write(&out, report.render(round, &date)).expect("write report");
    eprintln!("Nav-eval report → {}", out.display());

    report.assert_hard_gates();
}

fn next_round_number(date: &str) -> usize {
    let dir = PathBuf::from("docs/superpowers/specs");
    let prefix = format!("{date}-nav-eval-round-");
    let mut max_seen = 0usize;
    let Ok(entries) = std::fs::read_dir(&dir) else {
        return 1;
    };
    for entry in entries.flatten() {
        let Some(name) = entry.file_name().to_str().map(|s| s.to_string()) else {
            continue;
        };
        let Some(rest) = name.strip_prefix(&prefix) else {
            continue;
        };
        let Some(num_str) = rest.strip_suffix(".md") else {
            continue;
        };
        if let Ok(n) = num_str.parse::<usize>() {
            if n > max_seen {
                max_seen = n;
            }
        }
    }
    max_seen + 1
}
