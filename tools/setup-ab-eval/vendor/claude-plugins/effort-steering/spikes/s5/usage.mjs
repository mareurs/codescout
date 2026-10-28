// usage.mjs — the S5 harness's transcript reader: token usage, effort levels and a few behaviour
// flags out of Claude Code transcripts. Pure parsing plus file reads; no model call, no spawn.
//
// The quality test's verdict on an effort-steering sentence rests on these numbers.
//
// Usage rule (finding of 2026-10-07). One API message often spans several transcript records that
// share a message.id (a thinking block in one record, text or tool_use in a later one), and the
// early records can carry PARTIAL usage: (output 3, no thinking field) then (output 313, thinking
// 40); (16,-),(16,-),(16,-),(728, thinking 0); (3,-),(4916, thinking 2756). So per message.id this
// module keeps the counted record with the LARGEST usage.output_tokens, the last of them on a tie,
// and takes thinking from that same record. S4's tokensFromTranscript keeps the FIRST record per
// id; S5 must not copy that. Which records count is S4's own filter (countedAssistantRecords).
//
// Subagents. Claude Code writes each subagent's transcript to
// <config>/projects/<slug>/<sessionId>/subagents/agent-<id>.jsonl, next to <sessionId>.jsonl, and a
// Workflow tool run writes its subagents below that, at subagents/workflows/<id>/agent-<id>.jsonl.
// Every agent-*.jsonl at any depth under subagents/ is read (*.meta.json and the like are not).
// Their usage is a run's usage too: `main` and `subagents` are kept apart, `total` is their sum.
//
// A forked subagent (agentType "fork") starts from a copy of its parent's history, so its file
// repeats the parent's message ids and usage. On 2026-10-07, across 131 real sessions and 2,644
// subagent files, 63 (id, subagent file) pairs repeated an id of the main transcript, every one in a
// fork. A message is one billed API call however many files repeat it, so an id already counted in
// the main transcript, or in an earlier subagent file, is not counted again.

import { readdirSync, readFileSync } from 'node:fs';
import path from 'node:path';
import { countedAssistantRecords, effortsFromTranscript, findTranscripts, hasApiError, isApiError } from '../s4-rows.mjs';

const isObject = (v) => v !== null && typeof v === 'object' && !Array.isArray(v);
// A usable count; the same test S4 applies, so a malformed field is 0 and never NaN.
const count = (v) => (typeof v === 'number' && Number.isFinite(v) && v >= 0 ? v : 0);
// thinking_tokens REPORTED: a number >= 0. A reported 0 is a measurement; an absent field is not.
const isReported = (v) => typeof v === 'number' && Number.isFinite(v) && v >= 0;

// S4's parseLines is private and R9 allows exporting only two names from s4-rows.mjs, so the
// readers that need records beyond the counted assistant ones parse for themselves.
function records(text) {
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

// ---- usage -------------------------------------------------------------------------------------

// The final-record rule for one id: the larger output_tokens wins, a tie goes to the later record.
function keepFinal(map, id, usage) {
  const prev = map.get(id);
  if (prev === undefined || count(usage.output_tokens) >= count(prev.output_tokens)) map.set(id, usage);
}

// Map<message.id, usage> of the chosen record per id, over the records S4 counts (an API-error
// record, a record without a string id, a non-assistant role, a record without a usage object are
// not).
export function messageUsages(text) {
  const out = new Map();
  for (const { msg, usage } of countedAssistantRecords(text)) keepFinal(out, msg.id, usage);
  return out;
}

const ZERO = Object.freeze({ input_total: 0, cache_creation: 0, cache_read: 0, output: 0, thinking: 0, messages: 0, thinkingFieldMissing: 0 });

// Sums over the chosen usage objects (a Map from messageUsages, or any iterable of usage objects).
// input_total is input + cache_creation + cache_read, as in S4 ruling R3. thinkingFieldMissing is
// the number of chosen records that do not REPORT thinking_tokens; their thinking counts as 0.
export function totals(usages) {
  const sum = { ...ZERO };
  for (const u of usages instanceof Map ? usages.values() : usages) {
    sum.input_total += count(u.input_tokens) + count(u.cache_creation_input_tokens) + count(u.cache_read_input_tokens);
    sum.cache_creation += count(u.cache_creation_input_tokens);
    sum.cache_read += count(u.cache_read_input_tokens);
    sum.output += count(u.output_tokens);
    sum.thinking += count(u.output_tokens_details?.thinking_tokens);
    sum.messages += 1;
    if (!isReported(u.output_tokens_details?.thinking_tokens)) sum.thinkingFieldMissing += 1;
  }
  return sum;
}

const addTotals = (a, b) => Object.fromEntries(Object.keys(ZERO).map((k) => [k, a[k] + b[k]]));

// Every agent-*.jsonl at any depth under `dir`, sorted by its path relative to `dir`. A Workflow
// tool run writes its subagents at subagents/workflows/<id>/agent-*.jsonl, below the flat ones.
// A missing `dir` is no subagents; any other read failure, at any depth, is not silently read as
// zero usage.
function subagentTranscripts(dir) {
  const found = [];
  const walk = (current) => {
    let entries;
    try {
      entries = readdirSync(current, { withFileTypes: true });
    } catch (err) {
      if (current === dir && err && (err.code === 'ENOENT' || err.code === 'ENOTDIR')) return;
      throw err;
    }
    for (const e of entries) {
      const full = path.join(current, e.name);
      if (e.isDirectory()) walk(full);
      else if (e.isFile() && e.name.startsWith('agent-') && e.name.endsWith('.jsonl')) found.push(full);
    }
  };
  walk(dir);
  return found.sort((a, b) => {
    const ra = path.relative(dir, a);
    const rb = path.relative(dir, b);
    return ra < rb ? -1 : ra > rb ? 1 : 0;
  });
}

function noRun(reason) {
  return {
    main: { ...ZERO },
    subagents: { ...ZERO },
    total: { ...ZERO },
    subagentFiles: [],
    efforts: [],
    subagentEfforts: [],
    apiError: false,
    subagentApiErrors: 0,
    transcriptPath: null,
    reason,
  };
}

// One run's usage from its session id. { main, subagents, total, subagentFiles, efforts,
// subagentEfforts, apiError, subagentApiErrors, transcriptPath, reason }:
//  - no or several transcripts for the id: reason 'no_transcript' / 'ambiguous_transcript', zeroed
//    totals, nothing read;
//  - efforts come from the MAIN transcript (the effort guard reads only those); subagentEfforts are
//    the union over the subagent files, recorded and not guarded;
//  - apiError is the main transcript's; subagentApiErrors is the number of subagent FILES with one.
export function readRunUsage(configDir, sessionId) {
  const found = findTranscripts(sessionId, configDir);
  if (found.length === 0) return noRun('no_transcript');
  if (found.length > 1) return noRun('ambiguous_transcript');
  const transcriptPath = found[0];
  const mainText = readFileSync(transcriptPath, 'utf8');
  const mainUsages = messageUsages(mainText);

  const subagentFiles = subagentTranscripts(path.join(path.dirname(transcriptPath), sessionId, 'subagents'));
  const subUsages = new Map();
  const subEfforts = new Set();
  let subagentApiErrors = 0;
  for (const file of subagentFiles) {
    const text = readFileSync(file, 'utf8');
    for (const [id, usage] of messageUsages(text)) {
      if (!mainUsages.has(id)) keepFinal(subUsages, id, usage);
    }
    for (const effort of effortsFromTranscript(text)) subEfforts.add(effort);
    if (hasApiError(text)) subagentApiErrors += 1;
  }

  const main = totals(mainUsages);
  const subagents = totals(subUsages);
  return {
    main,
    subagents,
    total: addTotals(main, subagents),
    subagentFiles,
    efforts: effortsFromTranscript(mainText),
    subagentEfforts: [...subEfforts],
    apiError: hasApiError(mainText),
    subagentApiErrors,
    transcriptPath,
    reason: null,
  };
}

// ---- behaviour flags and text ------------------------------------------------------------------

const isTextBlock = (b) => b.type === 'text' && typeof b.text === 'string';

// The content blocks of an assistant record that holds model output. Two kinds of record are not
// that: an API-error record (an assistant record whose text is Claude Code's error message), and a
// user record, which can also hold text blocks (headless runs write isMeta user records such as a
// Skill body or "Continue from where you left off.").
function assistantBlocks(obj) {
  if (!isObject(obj) || obj.type !== 'assistant' || isApiError(obj) || !isObject(obj.message)) return [];
  const content = obj.message.content;
  return Array.isArray(content) ? content.filter(isObject) : [];
}

// The first user prompt, byte for byte: the first type:'user' record whose content is a string, or
// an array holding a text block and no tool_result block (the text blocks joined with ''). A tool
// result is also a user record, and is not the prompt. null when there is none.
export function firstUserText(text) {
  for (const obj of records(text)) {
    if (!isObject(obj) || obj.type !== 'user' || !isObject(obj.message)) continue;
    const content = obj.message.content;
    if (typeof content === 'string') return content;
    if (!Array.isArray(content)) continue;
    const blocks = content.filter(isObject);
    if (blocks.some((b) => b.type === 'tool_result')) continue;
    const texts = blocks.filter(isTextBlock);
    if (texts.length > 0) return texts.map((b) => b.text).join('');
  }
  return null;
}

// True iff a Bash tool_use whose input.command contains one of `patterns` (a plain, case-sensitive
// substring; an empty pattern matches nothing) sits in a record BEFORE the last record that holds
// an assistant text block. With no assistant text at all: whether any such Bash ran. (Claude Code
// writes one content block per record: 1 of 75,753 sampled assistant records held several, so
// "before the record" and "before the block" agree.)
export function testsRunBeforeDone(text, patterns) {
  const wanted = patterns.filter((p) => typeof p === 'string' && p !== '');
  let lastText = -1;
  let firstRun = -1;
  records(text).forEach((obj, i) => {
    for (const b of assistantBlocks(obj)) {
      if (isTextBlock(b)) lastText = i;
      else if (firstRun === -1 && b.type === 'tool_use' && b.name === 'Bash' && isObject(b.input) && typeof b.input.command === 'string' && wanted.some((p) => b.input.command.includes(p))) firstRun = i;
    }
  });
  if (firstRun === -1) return false;
  return lastText === -1 || firstRun < lastText;
}

// The last n characters (UTF-16 units) of the last assistant text block; '' if there is none.
export function finalTextTail(text, n = 300) {
  let last = '';
  for (const obj of records(text)) {
    for (const b of assistantBlocks(obj)) if (isTextBlock(b)) last = b.text;
  }
  return n <= 0 ? '' : last.slice(-n);
}

// The content of one UserPromptSubmit hook_additional_context attachment record, or null when the record is not one (or its content is
// neither a string nor a list). The one filter hookContexts and steerReachedFirstTurn share.
function hookContextOf(obj) {
  if (!isObject(obj) || obj.type !== 'attachment' || !isObject(obj.attachment)) return null;
  const a = obj.attachment;
  if (a.type !== 'hook_additional_context' || a.hookEvent !== 'UserPromptSubmit') return null;
  if (typeof a.content === 'string') return a.content;
  if (Array.isArray(a.content)) return a.content.filter((s) => typeof s === 'string').join('\n');
  return null;
}

// The content of each UserPromptSubmit hook_additional_context attachment, in order. The hook's
// text is NOT in the user message; Claude Code writes it as its own `attachment` record, with
// `content` a string or an array of strings (joined with '\n'). Other attachments are ignored.
export function hookContexts(text) {
  const out = [];
  for (const obj of records(text)) {
    const context = hookContextOf(obj);
    if (context !== null) out.push(context);
  }
  return out;
}

// R-ST2: did the steer reach the model on its FIRST turn? True iff the transcript holds a hook context (hookContexts' filter) and the first
// one comes BEFORE the first `assistant` record, in file order; a steer and no assistant record at all is true, no steer is false. Claude
// Code also fires the hook on a queued task-notification (cs-p07.B.0, 2026-10-12): that context comes after the first assistant record. The
// hook's 500 ms kill can miss ANY turn, the repeat included, but a miss on the first turn is the one that leaves the model unsteered from the
// start and the one the rule must detect; a later context is therefore no evidence for it.
export function steerReachedFirstTurn(text) {
  for (const obj of records(text)) {
    if (hookContextOf(obj) !== null) return true;
    if (isObject(obj) && obj.type === 'assistant') return false;
  }
  return false;
}
