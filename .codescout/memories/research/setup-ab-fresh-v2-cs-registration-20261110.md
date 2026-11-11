# Setup A/B: pool v2 cs/full registration on the onboarded fresh tasks (2026-11-10)

State: registered, `check` ok, leak prescan passed on all four prepared run images, NOT run (no go, no spend). Commit 195317920 (not pushed) adds `freeze_screen --arms` and lets a live bundle keep its template (`run.load_task_from_bundle`; `_run_one` refuses an indexed arm only when the task has no template).

Registration: data root `/home/marius/work/claude/setup-ab-data/fresh-screen-v2-cs` (live-tasks -> fresh-tasks-v2-onb/<id>, tasks -> live-tasks, live/controls-batch -> fresh-build-v2/pool/controls, cli -> fresh-screen-v2/cli); run data `live-screen-cs/` (screen-plan.json 16 runs, screen-registration.json sha256 dbab6178ff097530eacf6a486363a230b54b08861bb9731409b4c14ba22a7c1f, plan sha256 e019ddfc..., GO.template.json with the run command, setup-manifest.json copied from live-screen). Arms cs:1,full:1, profile v2, both models, 4 tasks, seed 20261108, cap 25 / per call 5 / reserve 5, head 195317920. Driver: job tmp `fresh_screen_v2_cs.py` (sets the pool-v2 constants in memory, as fresh_screen_v2.py did for the bare screen). Each task entry records the template volume and its docker CreatedAt (`check` notices a rebuilt volume). GO.json is for the operator to write; the SAB_TOKEN_FILE is /home/marius/.config/s5/token.

Index cost of the onboarding (prepare_index wall time): RA-22992 9988 s, nushell-19052 9465 s, 19283 7999 s, 18863 6958 s; each prepared arm image lists 21 tools.

Leak prescan (2026-11-10, job tmp `prescan_onb.py`): `multilang_tasks.prescan_live` over the prepared RUN image (sab-run-<id>:f9b0fc9f8-v2onb, with the onboarding config commit), not the pre-prepare agent image. All four: control pass, 0 needle hits, 0 sweep hits (gold literals 66 / 75 / 282 / 13 for 18863 / 19283 / 19052 / 22992). Evidence under fresh-tasks-v2-onb/<id>/prescan/prescan.json. The exec-only arm layer was not scanned separately (it only removes read access).

Cost basis: the bare v2 screen cost $5.05 for 16 runs (sonnet mean $0.124, opus $0.507; nushell-19052 is the outlier, opus $1.14). Old study ratios cs 2.36x, full 3.28x give about $14 for the 16 cs/full runs; worst single run about $3.7. An extrapolation, not a measurement.

Incident: the first freeze refused with "frozen inputs changed after the freeze: plugins/codescout-companion". Cause (verified by diff against the freeze source cache/sdd-misc-plugins/codescout-companion/1.20.21): the frozen copy under study/frozen/plugins/codescout-companion had lost `.codescout/usage.db-wal` and `usage.db-shm` (dir mtime 2026-11-10 00:23); SQLite deletes them when the last connection to usage.db closes. Something opened that db inside the frozen tree. I restored both files with `cp -p --no-clobber` from the source and the tree hash matched the registered 60bfbdb8... again. Not found: which process opened it. Any codescout/companion process whose project root is the frozen copy can do this again; run `check` right before the go.

Disk: /home had 150 GB free after the prescans (an external SSD move of docker was ruled out: containerd 975 GB vs 416 GB free on the SSD, which is also mounted nosuid,nodev).

Next: the operator writes GO.json (command in GO.template.json); run `check` first.
