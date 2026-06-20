/**
 * codescout-mode — curate-and-substitute integration for codescout.
 *
 * - session_start: drop Pi's native `edit` and activate codescout's hot-set,
 *   but ONLY if codescout's tools are actually registered (cache warm / code
 *   project). Otherwise no-op — never leave the session with no edit tool.
 * - tool_result: append a one-time, non-blocking hint when bash was used to
 *   grep/find source, steering future calls to codescout. bash still runs.
 *
 * Source of truth: codescout repo contrib/pi/codescout-mode.ts, symlinked to
 * ~/.pi/agent/extensions/codescout-mode.ts.
 */
import type { ExtensionAPI } from "@earendil-works/pi-coding-agent";
import { isBashToolResult } from "@earendil-works/pi-coding-agent";

// Must match the directTools list in contrib/pi/mcp.json.
// `grep` intentionally excluded — collides with Pi's built-in grep (F-1);
// reach codescout's grep via the mcp proxy instead.
const CODESCOUT_HOT_SET = [
	"symbols",
	"symbol_at",
	"tree",
	"semantic_search",
	"references",
	"read_file",
	"read_markdown",
	"edit_code",
	"edit_file",
	"edit_markdown",
];

// Pi built-ins codescout supersedes and we drop from the active set.
const DROP_BUILTINS = ["edit"];

// bash invocations that should have used codescout (source search):
// ripgrep, ag, recursive grep, or `find <path> -name`.
const SOURCE_SEARCH = /(^|\s|\|)(rg|ag)\b|(^|\s|\|)grep\s+[^|]*-[a-zA-Z]*r|(^|\s|\|)find\s+\S+\s+-name\b/;

export default function (pi: ExtensionAPI) {
	let nudged = false;

	pi.on("session_start", async (_event, ctx) => {
		const all = pi.getAllTools();
		const has = (name: string) => all.some((t) => t.name === name);

		// Safety guard: only curate when codescout's core tools are present.
		// Cold directTools cache (first session) or a non-code dir => no-op.
		if (!has("edit_code") || !has("symbols")) return;

		const active = new Set(pi.getActiveTools());
		for (const name of DROP_BUILTINS) active.delete(name);
		for (const name of CODESCOUT_HOT_SET) if (has(name)) active.add(name);

		// setActiveTools rejects on unknown/duplicate names (F-1). Inputs are
		// deduped via Set and guarded by has(), but degrade safely if it throws
		// (keep native tools rather than crash the session).
		try {
			await pi.setActiveTools([...active]);
		} catch (e) {
			if (ctx.hasUI) ctx.ui.notify(`codescout-mode: setActiveTools failed (${String(e)})`, "info");
			return;
		}

		if (ctx.hasUI) {
			ctx.ui.notify("codescout-mode: codescout tools active; native `edit` dropped", "info");
		}
	});

	pi.on("tool_result", async (event) => {
		if (nudged) return undefined;
		if (!isBashToolResult(event)) return undefined;
		const command = (event.input as { command?: string }).command ?? "";
		if (!SOURCE_SEARCH.test(command)) return undefined;
		nudged = true;
		return {
			content: [
				...event.content,
				{
					type: "text" as const,
					text:
						"\n[codescout-mode] For source search prefer `semantic_search` / `references` " +
						"(or codescout `grep` via the mcp proxy); for reading code prefer `symbols` / " +
						"`read_file`. (Shown once per session.)",
				},
			],
		};
	});
}
