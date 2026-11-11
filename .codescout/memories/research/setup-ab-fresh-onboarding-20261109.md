# Setup A/B fresh live tasks: not onboarded, codescout arms would lose the LSP tools (measured 2026-11-09)

Trigger: another project found that in a temp directory codescout reports `languages: []`, which hides the tools that need LSP (`references`, `call_graph`, `symbol_at`); after activating a repo with detected languages they appear. Same mechanism as arms.py round-4 ruling I1 (`LSP_GATED_TOOLS`; only `onboarding()` writes the detected languages; `codescout index` does not).

Measured with the study's own mcp_probe in a throwaway container (--network none, removed after) from `sab-arm-nushell__nushell-19283:f9b0fc9f8-v2`, server started as the arm starts it (`/opt/sab/bin/codescout start --debug`, cwd /testbed):
- before onboarding(): 18 tools, `call_graph`/`references`/`symbol_at` missing; /testbed/.codescout does not exist in the fresh arm images (also RA-22992).
- after onboarding() (3 s, no error): 21 tools, the three present; detected languages bash, javascript, markdown, python, rust. onboarding() wrote `[embeddings] model = local:JinaEmbeddingsV2BaseCode` (hardware recommendation), which `arms.prepare_index` replaces with the daily CodeRankEmbed.
Probe outputs: job tmp onboard-probe/ (not git).

Consequence: the fresh live tasks (`kind: live`, bare only, no Qdrant template) have had NO onboarding or index. The pool v2 screens are bare-arm, so they are unaffected. Running the cs/full arms on them needs `arms.prepare_index` per task image first (onboarding, set daily embeddings, memories cleanup, `codescout index`, commit config only, index state untracked), then re-verify 21 tools in the committed image, then a new bundle/registration. Do one task first (RA-22992) and measure time and cost; one heavy docker job at a time.
