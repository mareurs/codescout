/**
 * codescout-mode — hard-replacement integration for codescout.
 *
 * - session_start: drop Pi's native `edit`/`write`, but ONLY if codescout's
 *   replacement tools are actually registered (cache warm / code project).
 *   Otherwise no-op — never leave the session without an edit/write tool.
 * - tool_call: hard-block native `read` (unless the target is an image) and
 *   `bash` (unless the command falls outside codescout's redundant
 *   read/search set, or carries an explicit `# codescout-override` marker).
 *   Also re-blocks `edit`/`write` per call as defense-in-depth, in case the
 *   session_start drop silently no-ops.
 *
 * F-3: pi-mcp-adapter defaults to `toolPrefix: "server"`, so codescout's
 * direct tools register as `codescout_<name>`, not `<name>`. An earlier
 * revision of this file checked unprefixed names (`edit_code`, `symbols`)
 * against `pi.getAllTools()` and always failed, so the whole extension
 * silently no-op'd every session — native edit/write/read/bash were never
 * actually touched despite AGENTS.md claiming otherwise.
 *
 * Source of truth: codescout repo contrib/pi/codescout-mode.ts, symlinked to
 * ~/.pi/agent/extensions/codescout-mode.ts.
 */
import type { ExtensionAPI } from "@earendil-works/pi-coding-agent";

// Must match the directTools list in contrib/pi/mcp.json. pi-mcp-adapter's
// default toolPrefix ("server") means these are the real registered names.
const EDIT_TOOLS = ["codescout_edit_code", "codescout_edit_file", "codescout_edit_markdown"];
const WRITE_TOOL = "codescout_create_file";
const READ_TOOL = "codescout_read_file";

// Native `read` supports images; codescout_read_file does not. Keep native
// `read` usable for exactly that case.
const IMAGE_EXT = /\.(jpe?g|png|gif|webp|bmp)$/i;

// bash invocations that duplicate codescout's read/search path: ripgrep, ag,
// `find <path>... -name`, or cat/head/tail/sed/awk on a source file. Mirrors
// codescout's own run_command "source file access blocking".
const RG_AG = /(^|\s|\|)(rg|ag)\b/;
const FIND_NAME = /(^|\s|\|)find\s+\S+(?:\s+\S+)*\s+-name\b/;
const SOURCE_DUMP = /(^|\s|\|)(cat|head|tail|sed|awk)\s+/;
const SOURCE_EXT = /\.(rs|py|ts|tsx|js|jsx|go|java|kt|kts|c|cc|cpp|h|hpp|rb|php|swift|scala|cs)\b/;
const OVERRIDE_MARKER = /#\s*codescout-override\b/;

// Recursive/regex grep: "grep" as the command word of a pipeline segment,
// with a -r/-R/--recursive flag among its arguments. Tokenized rather than
// matched with a single regex — a regex like `grep\s+[^|]*-[a-zA-Z]*r` false
// positives on any argument containing a run of letters ending in "r" (e.g. a
// path like ".../pi-integration-session-log.md" matches "-integ" + "r" inside
// "integration"). Caught live in testing.
function isRecursiveGrep(command: string): boolean {
	for (const segment of command.split("|")) {
		const tokens = segment.trim().split(/\s+/).filter(Boolean);
		if (tokens[0] !== "grep") continue;
		for (const token of tokens.slice(1)) {
			if (token === "--recursive" || token === "-r" || token === "-R") return true;
			if (/^-[a-zA-Z]+$/.test(token) && /[rR]/.test(token.slice(1))) return true;
		}
	}
	return false;
}

function isRedundantBashCommand(command: string): boolean {
	if (OVERRIDE_MARKER.test(command)) return false;
	if (RG_AG.test(command)) return true;
	if (isRecursiveGrep(command)) return true;
	if (FIND_NAME.test(command)) return true;
	return SOURCE_DUMP.test(command) && SOURCE_EXT.test(command);
}

export default function (pi: ExtensionAPI) {
	const has = (name: string) => pi.getAllTools().some((t) => t.name === name);
	const hasAll = (names: string[]) => names.every(has);

	pi.on("session_start", async (_event, ctx) => {
		// Safety guard: only curate when codescout's replacements are present.
		// Cold directTools cache (first session) or a non-code dir => no-op.
		const dropEdit = hasAll(EDIT_TOOLS);
		const dropWrite = has(WRITE_TOOL);
		if (!dropEdit && !dropWrite) return;

		const active = new Set(pi.getActiveTools());
		if (dropEdit) active.delete("edit");
		if (dropWrite) active.delete("write");

		// setActiveTools rejects on unknown/duplicate names. Degrade safely if it
		// throws (keep native tools rather than crash the session).
		try {
			await pi.setActiveTools([...active]);
		} catch (e) {
			if (ctx.hasUI) ctx.ui.notify(`codescout-mode: setActiveTools failed (${String(e)})`, "info");
			return;
		}

		if (ctx.hasUI) {
			const dropped = [dropEdit && "edit", dropWrite && "write"].filter(Boolean).join("/");
			ctx.ui.notify(`codescout-mode: native \`${dropped}\` dropped; codescout tools active`, "info");
		}
	});

	pi.on("tool_call", async (event) => {
		// Defense-in-depth against F-3-style guard failures: re-block edit/write
		// per call too, not just once at session_start.
		if (event.toolName === "edit" && hasAll(EDIT_TOOLS)) {
			return {
				block: true,
				reason: "Use codescout's edit_code/edit_file/edit_markdown instead of native `edit`.",
			};
		}
		if (event.toolName === "write" && has(WRITE_TOOL)) {
			return {
				block: true,
				reason: "Use codescout's create_file instead of native `write` (pass overwrite:true to replace an existing file).",
			};
		}

		if (event.toolName === "read" && has(READ_TOOL)) {
			const path = (event.input as { path?: string }).path ?? "";
			if (IMAGE_EXT.test(path)) return undefined;
			return {
				block: true,
				reason: "Use codescout's read_file/read_markdown/symbols instead of native `read` (reserved for images).",
			};
		}

		if (event.toolName === "bash" && has(READ_TOOL)) {
			const command = (event.input as { command?: string }).command ?? "";
			if (isRedundantBashCommand(command)) {
				return {
					block: true,
					reason:
						"This duplicates codescout's read/search path — use read_file/grep/symbols/references instead. " +
						"Append '# codescout-override' to the command if raw shell access is genuinely required.",
				};
			}
		}

		return undefined;
	});
}
