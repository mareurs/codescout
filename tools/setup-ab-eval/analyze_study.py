"""The frozen analysis driver (final review I-3): every degree of freedom the analyst had after
seeing the data is fixed here, before the Stage 2 freeze, and this file's hash is frozen with the
rest.

    python3 analyze_study.py --registration <stage2 registration>      (or --selection + --seed)

What it fixes:
  REGISTERED TASKS  {task id: kind} built from each selected task's bundle.json `kind`
                    (stage3.registered_mapping), never typed by hand; with --registration the
                    mapping it records must equal the one rebuilt from the bundles.
  MINIMUM           analyze.REGISTERED_MIN (10 of 12 per model; a stratum the same share,
                    rounded up: 5 of 6).
  INSTRUMENT        instrument_failures(), from the real gates, computed per model and comparison:
    study-wide      `tasks.py verify` on all registered tasks (the leak gate: 0 hits, planted
                    control passed; the controls on the arm image; R12 history; base fails, gold
                    passes), and for each private task its pre-index prescan (control passed,
                    0 hits, no reason);
    per arm         arm integrity at the STUDY level, for each model and arm in the comparison:
                    (a) any run whose integrity reasons carry a CONTAMINATION reason
                    (integrity.CONTAMINATION: a poorer arm showed a richer arm's skills, tools
                    or companion output: the arm was built wrong, not one unlucky run);
                    (b) more registered tasks with an `invalid-arm` run carrying a CONSTRUCTION
                    reason (integrity.CONSTRUCTION: the arm was built wrong) than the registered
                    minimum tolerates (len(registered) - registered_min = 2). Fix round 2 ruling:
                    a run invalid for another reason only lowers n_valid; zero codescout calls is
                    a result, never invalid (analyze.ARM_INTEGRITY_RULE).
  HEADLINE          ruling (I-3): no pooled headline. For each model and comparison the foreign
                    and private strata are decided separately; the headline is their common
                    outcome when the two agree, else "no headline: the strata disagree". The
                    pooled Decision is printed as a secondary line, never as the headline.
  SEED              the bootstrap seed (the registration's `bootstrap_seed`).
  NOTES             the registered rules printed with every report (analyze.STRATUM_MIN_RULE,
                    TOLERANCE_NOTE, ARM_INTEGRITY_RULE, ADMISSION_RULE); an `admission` stop in
                    batch.jsonl adds a DISCLOSE note: the rows are analysed as is.
Secondary metrics (analyze.secondary, analyze.cap_hits) are printed for every model and arm, and
`outcomes_per_arm` (pre-freeze M-7): per model x arm the count of each final outcome (so an
infra-oom or invalid-arm pattern only cs/full show is visible), the oom_kill sum and the max and
median memory peak.

Reads rows.jsonl through run.read_rows (Stage 1's dev rows are read and ignored by the
population). Prints one JSON document. Calls no model; `tasks.py verify` runs `docker image
inspect` only.
"""
import argparse
import json
import statistics
import sys
from pathlib import Path

import analyze
import integrity
import run
import stage3
import tasks

COMPARISONS = (("cs", "bare"), ("full", "bare"))
NO_HEADLINE = "no headline: the strata disagree"


def prescan_reasons(mapping, tasks_root):
    """{task: [reason]} for every private task whose prescan did not pass cleanly."""
    out = {}
    for t, kind in mapping.items():
        if kind != "private":
            continue
        path = Path(tasks_root) / t / "prescan" / "prescan.json"
        try:
            r = json.loads(path.read_text())
        except (OSError, ValueError) as e:
            out[t] = [f"prescan unreadable: {e}"]
            continue
        why = []
        if r.get("id") != t:
            why.append(f"prescan is for {r.get('id')!r}")
        if r.get("control") != "pass":
            why.append(f"prescan control {r.get('control')!r}")
        if r.get("hits") != []:
            why.append(f"prescan hits: {len(r.get('hits') or [])}" if "hits" in r
                       else "prescan has no hit list")
        if r.get("reasons"):
            why += [f"prescan: {x}" for x in r["reasons"]]
        if why:
            out[t] = why
    return out


def study_failures(verify_reasons, prescan):
    """The study-wide instrument failures (gates that hold for every arm and model)."""
    out = [f"tasks.py verify: {t}: {why}" for t, whys in sorted(verify_reasons.items())
           for why in whys]
    out += [f"leak prescan: {t}: {why}" for t, whys in sorted(prescan.items()) for why in whys]
    return out


def arm_failures(rows, mapping, registered_min):
    """{(model, arm): [failure]}: arm integrity at the study level (see INSTRUMENT). Fix round 2
    ruling: only CONSTRUCTION failures (integrity.CONSTRUCTION reasons: the arm was built wrong)
    count toward the threshold, and any contamination reason is a failure at once; a run that is
    invalid for another reason (a stream defect) only lowers n_valid, and zero codescout calls is
    a result (never invalid)."""
    latest = analyze._latest(rows)
    analyze._check_registered_rows(latest, mapping)
    tolerance = len(mapping) - registered_min
    out = {(m, a): [] for m in run.MODELS for a in analyze.ARMS}
    built_wrong = {(m, a): set() for m in run.MODELS for a in analyze.ARMS}
    for r in latest.values():
        if r["stage"] != "main" or r["taskId"] not in mapping:
            continue
        key = (r["model"], r["arm"])
        reasons = [w for w in (r["integrity"] or {}).get("reasons") or [] if isinstance(w, str)]
        for why in reasons:
            if why.startswith(integrity.CONTAMINATION):
                out[key].append(f"arm-integrity: {r['runId']}: {why}")
        if r["outcome"] == "invalid-arm" and any(w.startswith(integrity.CONSTRUCTION)
                                                 for w in reasons):
            built_wrong[key].add(r["taskId"])
    for key, ts in built_wrong.items():
        if len(ts) > tolerance:
            out[key].append(f"arm-integrity: {key[0]} {key[1]}: {len(ts)} of {len(mapping)} "
                            f"tasks have a construction failure, more than the {tolerance} the "
                            f"registered minimum ({registered_min}) tolerates: {sorted(ts)}")
    return out


def instrument_failures(rows, mapping, registered_min, model, comparison, *, verify_reasons,
                        prescan):
    """The instrument_failures argument of analyze.decide for one model and comparison."""
    per_arm = arm_failures(rows, mapping, registered_min)
    out = study_failures(verify_reasons, prescan)
    for arm in comparison:
        out += per_arm[(model, arm)]
    return tuple(out)


def headline(foreign, private):
    """Ruling I-3: the foreign and private strata must agree; there is no pooled headline."""
    if foreign is None or private is None:
        return "no headline: a stratum has no registered task"
    if foreign.outcome == private.outcome:
        return foreign.outcome
    return f"{NO_HEADLINE} (foreign {foreign.outcome}, private {private.outcome})"


def _decision(d):
    return None if d is None else dict(d._asdict(), excluded=dict(d.excluded))


def outcome_counts(rows, mapping):
    """Pre-freeze M-7, reported, never gated: per model x arm, over the latest FINAL row of
    every main run of a registered task, the count of each outcome (infra-oom, invalid-arm and
    infra-other show arm-dependent missingness), the summed cgroup oom_kill count, and the max
    and median memoryPeakBytes of the runs that recorded one."""
    latest = analyze._latest(rows)
    out = {}
    for m in run.MODELS:
        out[m] = {}
        for a in analyze.ARMS:
            rs = [r for r in latest.values() if r["model"] == m and r["arm"] == a
                  and r["stage"] == "main" and r["taskId"] in mapping
                  and r["outcome"] != run.PENDING]
            peaks = sorted(r["memoryPeakBytes"] for r in rs if r["memoryPeakBytes"] is not None)
            out[m][a] = {"runs": len(rs),
                         **{o: sum(r["outcome"] == o for r in rs) for o in run.OUTCOMES
                            if o != "dev-scored"},
                         "oom_kills": sum(r["oomKills"] or 0 for r in rs),
                         "memory_peak_max_bytes": peaks[-1] if peaks else None,
                         "memory_peak_median_bytes": (statistics.median(peaks) if peaks
                                                      else None)}
    return out

def admission_stops(events):
    """The batch.jsonl `stop` events whose reason is `admission` (the $200 cap held a run)."""
    return [{"stopId": e.get("stopId"), "seq": e.get("seq"), "spent": e.get("spent")}
            for e in events if e.get("event") == "stop" and e.get("reason") == "admission"]


def report(rows, mapping, *, registered_min, seed, verify_reasons, prescan, gold_files=None,
           events=()):
    analyze.check_registered(mapping)
    stops = admission_stops(events)
    notes = [analyze.STRATUM_MIN_RULE, analyze.TOLERANCE_NOTE, analyze.ARM_INTEGRITY_RULE,
             analyze.ADMISSION_RULE]
    if stops:
        notes.append(f"DISCLOSE: {len(stops)} admission stop(s) "
                     f"({', '.join(str(s['stopId']) for s in stops)}): the rows are analysed as "
                     f"is (registered rule); runs after the stop were never made")
    out = {"registered_tasks": mapping, "registered_min": registered_min, "seed": seed,
           "stratum_min": {k: analyze.stratum_min(registered_min,
                                                  sum(v == k for v in mapping.values()),
                                                  len(mapping))
                           for k in analyze.KINDS if k in mapping.values()},
           "notes": notes, "admission_stops": stops,
           "instrument": {"study": study_failures(verify_reasons, prescan),
                          "arms": {f"{m} {a}": f for (m, a), f in
                                   arm_failures(rows, mapping, registered_min).items() if f}},
           "models": {}}
    for model in run.MODELS:
        out["models"][model] = {}
        for comp in COMPARISONS:
            failures = instrument_failures(rows, mapping, registered_min, model, comp,
                                           verify_reasons=verify_reasons, prescan=prescan)
            s = analyze.strata(rows, model, mapping, registered_min=registered_min, seed=seed,
                               instrument_failures=failures, comparison=comp)
            effect = analyze.EFFECTS[comp]
            out["models"][model][f"{comp[0]} - {comp[1]}"] = {
                "effect": effect,
                "headline": f"{effect}: {headline(s['foreign'], s['private'])}",
                "foreign": _decision(s["foreign"]), "private": _decision(s["private"]),
                "pooled_not_a_headline": _decision(s["pooled"])}
    out["secondary"] = analyze.secondary(rows, gold_files)
    out["cap_hits"] = analyze.cap_hits(rows)
    out["outcomes_per_arm"] = outcome_counts(rows, mapping)
    return out


def main(argv=None, *, verify=tasks.verify):
    ap = argparse.ArgumentParser(description=__doc__.split("\n")[0])
    ap.add_argument("--data", type=Path, default=tasks.DATA / "study")
    ap.add_argument("--tasks-root", type=Path, default=tasks.DATA / "tasks")
    ap.add_argument("--registration", type=Path, default=None,
                    help="the Stage 2 registration: registered_tasks, registered_min, "
                         "bootstrap_seed")
    ap.add_argument("--selection", type=Path,
                    default=tasks.DATA / "selection" / "selection.json")
    ap.add_argument("--seed", type=int, default=None, help="bootstrap seed (no registration)")
    a = ap.parse_args(argv)
    if a.registration is not None:
        reg = json.loads(a.registration.read_text())
        body = {k: v for k, v in reg.items() if k != "registration_sha256"}
        if reg.get("registration_sha256") != run.registration_digest(body):
            raise SystemExit(f"registration {a.registration}: registration_sha256 does not match "
                             f"its content (edited after the freeze?)")  # fix round 2, N-3
        ids, seed = list(reg["registered_tasks"]), reg["bootstrap_seed"]
        if reg.get("registered_min") != analyze.REGISTERED_MIN:
            raise SystemExit(f"registration registered_min {reg.get('registered_min')} != "
                             f"analyze.REGISTERED_MIN {analyze.REGISTERED_MIN}")
    else:
        if a.seed is None:
            ap.error("--seed is required without --registration")
        ids, seed = json.loads(a.selection.read_text())["selected"], a.seed
    mapping = stage3.registered_mapping(ids, a.tasks_root)
    if a.registration is not None and mapping != reg["registered_tasks"]:
        raise SystemExit("the registration's registered_tasks differ from the bundles' kinds")
    verify_reasons = {k: v for k, v in verify(list(mapping), a.tasks_root,
                                              selected=list(mapping)).items() if v}
    out = report(run.read_rows(a.data), mapping, registered_min=analyze.REGISTERED_MIN,
                 seed=seed, verify_reasons=verify_reasons,
                 prescan=prescan_reasons(mapping, a.tasks_root), events=run.read_events(a.data))
    json.dump(out, sys.stdout, indent=2, default=list)
    print()
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
