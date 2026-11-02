// s4-rows.mjs — joins one S4 session's decision-log arm with its transcript tokens.
//
// Pure parsing plus two small file lookups; no model call, no spawn. Ruling R3: a row's
// input_total is input_tokens + cache_creation_input_tokens + cache_read_input_tokens,
// thinking_tokens is usage.output_tokens_details.thinking_tokens, cache_creation is
// usage.cache_creation_input_tokens; all summed over assistant messages deduped by
// message.id. s2-usage-report.mjs exports a UsageRecord with no input_tokens field, so
// it cannot carry input_total; this module has its own small dedupe by message.id
// (S2 measured that usage fields agree inside one message.id group, so the first
// record of a group is kept).
//
// Ruling R22: Claude Code writes an API failure as an assistant record with
// model "<synthetic>", all-zero usage and isApiErrorMessage:true. Such a record is never
// a usage row, and a session holding one is excluded (reason api_error): zeros from a
// failed call must not look like a thinking-token measurement. For the same reason a
// transcript whose usage never REPORTS output_tokens_details.thinking_tokens is excluded
// (no_thinking_field): a missing field would otherwise be summed as 0.
//
// Ruling R24 and final review I-1: the RUNNER assigns each run's arm, so the arm in the
// decision log must equal the planned arm (else arm_mismatch), a treated run must have
// been steered (else not_steered) and a holdout run must not have been (else
// steered_holdout). The log is checked before the transcript.
//
// buildRow returns { row } or { reason }; a reason means the run is EXCLUDED and counted.
// It is never treated as holdout.
//
// Effort guard: a run must have been made at the REGISTERED effort. A saved settings or
// /model default never reaches a `--setting-sources local` child, so a run that silently fell
// back to `medium` would measure nothing (Sonnet 5.5 thinks 0 tokens on most prompts there,
// ES-18). Claude Code records the effort of each request as a top-level `effort` on the
// assistant record. Every counted assistant message must record exactly the registered effort;
// a different or absent value excludes the run (effort_mismatch).

import { existsSync, readFileSync, readdirSync } from 'node:fs';
import path from 'node:path';
import os from 'node:os';

const ARMS = new Set(['treated', 'holdout']);
const count = (v) => (typeof v === 'number' && Number.isFinite(v) && v >= 0 ? v : 0);

function parseLines(text) {
  const out = [];
  for (const line of String(text).split('\n')) {
    if (line.trim() === '') continue;
    try {
      out.push(JSON.parse(line));
    } catch {
      // a torn or non-JSON line carries no usable record
    }
  }
  return out;
}

export const isApiError = (obj) =>
  obj !== null && typeof obj === 'object' && (obj.isApiErrorMessage === true || (obj.message && obj.message.model === '<synthetic>'));

// { arm, steered } of a session's decision log; null when there is no usable line. One
// prompt per S4 session, so the first line with a valid arm decides.
export function decisionFromLog(text) {
  for (const rec of parseLines(text)) {
    if (rec !== null && typeof rec === 'object' && ARMS.has(rec.arm)) return { arm: rec.arm, steered: rec.steered };
  }
  return null;
}

// The arm alone; null when there is no usable line.
export function armFromLog(text) {
  const d = decisionFromLog(text);
  return d === null ? null : d.arm;
}

// True when the transcript holds an API-error record (R22).
export function hasApiError(text) {
  return parseLines(text).some(isApiError);
}

// The assistant records that count as usage: not an API error, a string message.id, an
// assistant role (or none), a usage object. One place, so tokens and effort read the same set.
export function countedAssistantRecords(text) {
  const out = [];
  for (const obj of parseLines(text)) {
    if (isApiError(obj)) continue;
    const msg = obj && obj.message;
    if (msg === null || typeof msg !== 'object' || typeof msg.id !== 'string' || msg.id === '') continue;
    if (msg.role !== undefined && msg.role !== 'assistant') continue;
    const usage = msg.usage;
    if (usage === null || typeof usage !== 'object' || Array.isArray(usage)) continue;
    out.push({ obj, msg, usage });
  }
  return out;
}

// Assistant usage objects deduped by message.id, API-error records left out. A record
// without a message.id cannot be deduped and is not counted.
function assistantUsages(text) {
  const seen = new Map();
  for (const { msg, usage } of countedAssistantRecords(text)) {
    if (!seen.has(msg.id)) seen.set(msg.id, usage);
  }
  return [...seen.values()];
}

// The distinct `effort` values recorded on the counted assistant records, as strings; a record
// that records none (or a non-string) contributes null. A child started with `--effort xhigh`
// records only 'xhigh'. Empty when no assistant record counts.
export function effortsFromTranscript(text) {
  const seen = new Set();
  for (const { obj } of countedAssistantRecords(text)) seen.add(typeof obj.effort === 'string' ? obj.effort : null);
  return [...seen];
}

// Token sums over assistant messages deduped by message.id; null when none were found.
export function tokensFromTranscript(text) {
  const usages = assistantUsages(text);
  if (usages.length === 0) return null;
  let thinking = 0;
  let creation = 0;
  let inputTotal = 0;
  for (const u of usages) {
    thinking += count(u.output_tokens_details && u.output_tokens_details.thinking_tokens);
    creation += count(u.cache_creation_input_tokens);
    inputTotal += count(u.input_tokens) + count(u.cache_creation_input_tokens) + count(u.cache_read_input_tokens);
  }
  return { thinking_tokens: thinking, cache_creation: creation, input_total: inputTotal };
}

// True when at least one deduped assistant usage reports thinking_tokens as a number >= 0.
// A reported 0 is a measurement; an absent field is not.
export function reportsThinkingTokens(text) {
  return assistantUsages(text).some((u) => {
    const v = u.output_tokens_details && u.output_tokens_details.thinking_tokens;
    return typeof v === 'number' && Number.isFinite(v) && v >= 0;
  });
}

// Ruling R6: glob <config dir>/projects/*/<sessionId>.jsonl; never compute the slug.
// Returns every match. An empty CLAUDE_CONFIG_DIR means the default (|| not ??).
export function findTranscripts(sessionId, configDir) {
  const base = configDir || process.env.CLAUDE_CONFIG_DIR || path.join(os.homedir(), '.claude');
  const root = path.join(base, 'projects');
  let dirs;
  try {
    dirs = readdirSync(root, { withFileTypes: true });
  } catch {
    return [];
  }
  const out = [];
  for (const d of dirs) {
    if (!d.isDirectory()) continue;
    const file = path.join(root, d.name, `${sessionId}.jsonl`);
    if (existsSync(file)) out.push(file);
  }
  return out;
}

// { row } for analyzeArms, or { reason } when the run must be excluded. plannedArm and
// promptIndex come from the runner's plan; a caller that omits them is a bug, not a run.
export function buildRow(sessionId, { stateDir, configDir, plannedArm, promptIndex, effort }) {
  if (!ARMS.has(plannedArm)) throw new TypeError(`buildRow: plannedArm must be 'treated' or 'holdout', got ${String(plannedArm)}`);
  if (!Number.isSafeInteger(promptIndex) || promptIndex < 0) throw new TypeError(`buildRow: promptIndex must be a safe integer >= 0, got ${String(promptIndex)}`);
  if (typeof effort !== 'string' || effort.trim() === '') throw new TypeError(`buildRow: effort must be the registered effort level, got ${String(effort)}`);
  const logFile = path.join(stateDir, `${sessionId}.jsonl`);
  if (!existsSync(logFile)) return { reason: 'no_log' };
  const decision = decisionFromLog(readFileSync(logFile, 'utf8'));
  if (decision === null) return { reason: 'no_log' };
  if (decision.arm !== plannedArm) return { reason: 'arm_mismatch' };
  if (plannedArm === 'treated' && decision.steered !== true) return { reason: 'not_steered' };
  if (plannedArm === 'holdout' && decision.steered === true) return { reason: 'steered_holdout' };
  const found = findTranscripts(sessionId, configDir);
  if (found.length === 0) return { reason: 'no_transcript' };
  if (found.length > 1) return { reason: 'ambiguous_transcript' };
  let text;
  try {
    text = readFileSync(found[0], 'utf8');
  } catch {
    return { reason: 'no_transcript' };
  }
  if (hasApiError(text)) return { reason: 'api_error' };
  const tokens = tokensFromTranscript(text);
  if (tokens === null) return { reason: 'no_transcript' };
  if (!reportsThinkingTokens(text)) return { reason: 'no_thinking_field' };
  const efforts = effortsFromTranscript(text);
  if (efforts.length !== 1 || efforts[0] !== effort) return { reason: 'effort_mismatch' };
  return { row: { promptIndex, arm: decision.arm, ...tokens } };
}
