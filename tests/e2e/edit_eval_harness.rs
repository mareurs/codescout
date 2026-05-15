use crate::e2e::edit_eval::matchers::h1_exempt_for;
use crate::e2e::edit_eval::{cases, runner};
use crate::e2e::eval_common::{git_restore, next_round_number, Report, Verdict};

#[tokio::test]
#[ignore]
async fn edit_eval_harness() {
    let ctx = runner::edit_eval_context().await;
    let round = next_round_number("edit-eval");
    let mut report = Report::new("edit_eval", round);

    for case in cases::all() {
        let _ = git_restore(&ctx.fixture_src);
        let r = runner::run_one(&ctx, case).await;
        report.push(case.id, r.verdict.clone(), r.evidence.clone());
        // Always restore after a case so the next pre-edit check sees a clean tree.
        let _ = git_restore(&ctx.fixture_src);
    }

    let path = format!("docs/superpowers/specs/2026-05-15-edit-eval-round-{round}.md");
    report.write_to(&path).expect("write round file");
    println!("wrote {path}");

    // Hard gates
    let mut failures: Vec<String> = Vec::new();
    for case in cases::all() {
        let silent_wrong = report
            .rows_by_verdict(&Verdict::SilentWrong)
            .contains(&case.id);
        let corrupt = report.rows_by_verdict(&Verdict::Corrupt).contains(&case.id);
        let panicked = report.rows_by_verdict(&Verdict::Panic).contains(&case.id);

        if panicked {
            failures.push(format!("{}: PANIC (H2)", case.id));
            continue;
        }
        if (silent_wrong || corrupt)
            && !h1_exempt_for(case, &Verdict::SilentWrong)
            && !h1_exempt_for(case, &Verdict::Corrupt)
        {
            failures.push(format!("{}: unexpected destructive verdict (H1)", case.id));
        }
    }

    if !failures.is_empty() {
        panic!(
            "edit_eval hard-gate failures:\n  {}\nFull round file: {path}",
            failures.join("\n  ")
        );
    }
}
