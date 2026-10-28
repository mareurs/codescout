// plan-limit.mjs — the detector for a spent Claude subscription, shared by the S5 batch (s5/run.mjs) and the S4 run (s4-run.mjs). Pure: no file,
// no clock, no network, no model. Rulings R-PL1 and R-PL2 (R-PL3..R-PL8 are the consumers' and live in their headers); AMENDMENT A1 of the brief
// widened the pattern list and added the sixth channel, AMENDMENT A2 (rulings on the Opus review) added the typed transcript signal, the second
// typed kind, the note values and the wider note sources.
//
// WHY. `claude -p` on a subscription whose plan usage is spent does not fail in a way a harness notices: the CLI prints a refusal such as
// `You've hit your limit · resets 3pm` and, in at least one reported case (prompt-engineering OP-11, 2026-08-21), returns it as the ordinary
// response. A trial harness then grades the refusal as a model failure. In the S5 study the xhigh arms burn the most plan usage, so a limit hit
// would fabricate a loss for exactly the arms under test. A hit is a STOP, not a result.
//
// R-PL1. Pure, and total over what callers pass: `detectPlanLimit` never throws for any JSON value, primitive or plain object (the callers hand it
// `JSON.parse` output and strings), so a field of the wrong type is simply no hit. It makes NO promise for an object with a throwing getter or a
// Proxy, and it has no try/catch (a throw from such an input is a bug in the caller, and hiding it would hide a missed hit). `assertNoPlanLimit` is
// the throwing form (`PlanLimitError`, with the verdict in `.hit`).
//
// R-PL2. A TEXT channel matches only when its text, after trimming LEADING whitespace, is at most MAX_REFUSAL_CHARS (600) characters AND starts
// (index 0, no multiline flag) with one of the anchored patterns below. Agent prose that merely mentions a limit, a refusal quoted in the
// middle of a paragraph, and a long answer that happens to begin with one all miss. Trailing whitespace is not trimmed, so it counts toward the cap.
//
// THE PATTERNS (case-sensitive; ASCII ' and the typographic ’ where there is an apostrophe). The list MIRRORS the CLI's own classifier of
// usage-limit messages, the function `wms(e)` in the binary: it is true when the text startsWith one of twelve prefixes or matches one regex.
// Both are covered below, together with the message family the CLI builds (`You've hit your ${limitName}${suffix}`; limit names seen: limit,
// usage limit, fast limit, monthly spend limit, org's monthly spend limit, channel's monthly spend limit, team's shared budget, session limit,
// weekly limit; suffixes seen: ` · resets 3pm`, ` · raise it at …`, ` · progress saved`) and the earlier five-pattern set.
//    1. /^You['’]ve hit your\b/                            prefix "You've hit your"; \b keeps "yourself" out (the CLI's own startsWith would not)
//    2. /^You['’]ve reached your\b/                        prefix "You've reached your" (`You've reached your Fable limit`, `… reached your limit`)
//    3. /^You['’]re out of (?:extra usage|usage credits)/  prefixes "You're out of usage credits", "You're out of extra usage"
//    4. /^Your org(?:anization)? is out of usage/          prefixes "Your org is out of usage · add funds to continue" / "· contact your admin",
//                                                          and the long form `Your organization is out of usage credits. Contact your admin …`
//    5. /^Usage limit reached/                             not in the classifier. It is the start of the CLI's usage-limit STATUS templates
//                                                          (`Usage limit reached · wrapping up`, `… again · continuing automatically …`,
//                                                          `… · brief included wrap-up, then usage credits`) and of a workflow-paused text. Those are
//                                                          UI templates: whether `claude -p` prints any of them is NOT verified.
//    6. /^Your seat type doesn['’]t include (?:extra )?usage/   prefixes "… include usage credits", "… include usage", "… include extra usage"
//    7. /^Your usage allocation has been disabled by your admin/
//    8. /^Your group['’]s usage limit is set to \$0/
//    9. /^(?:Fable|Haiku|Mythos|Opus|Sonnet)(?: [^\u00B7\n]{1,40})? requires usage credits\./   the classifier's regex (\u00B7 is the middle dot, U+00B7)
//   10. /^Fable 5 requires usage credits/                  prefix "Fable 5 requires usage credits" (pattern 9 needs the period, the prefix does not)
// The five earlier patterns (hit your + space, reached your + a qualifier + limit, out of, org is out of, Usage limit reached) are all subsumed:
// 1 and 2 are wider than their first versions on purpose, so `You've reached your limit` and a bare `You've hit your` now match.
// The classifier list was re-extracted from both binaries and compared: 2.1.292 and 2.1.296 are IDENTICAL byte for byte (only the minified
// variable names differ: the prefix list is `syr` in 2.1.292 and `qTr` in 2.1.296, the regex list `Cuo` and `Wvo`).
// NEVER matched (each has its own test): `Approaching your 5-hour usage limit — Claude will wrap up the current step.` (a warning: it cuts nothing
// by itself; see the near_limit note below), `Server is temporarily limiting requests (not your usage limit)` (a transient, retryable overload;
// 80 such records exist in the operator's own transcripts), `Context limit reached`, `Fable limit reached · continuing on …` and `Fast limit
// reached and temporarily disabled …` (notices: the CLI continues), `Fast mode requires usage credits` (a mode notice, not a refusal of a model
// run), `Switched to Sonnet · Fable requires usage credits · /model to change` (a notice that starts with `Switched`), and `Failed to authenticate.
// API Error: 401 Invalid bearer token` (an authentication failure: it stays infrastructure `api_error` in S5).
//
// FALSE-POSITIVE TRADE-OFF (A2/F7, F8; ruled and PINNED by tests). Because patterns 1, 2, 5 and 9 are as wide as the CLI's own startsWith, short
// agent prose at index 0 is a hit in every text channel: `You've hit your target of 90% coverage: …`, `You've reached your goal: …`, `Opus
// requires usage credits. I switched the default model …`, and the status templates of pattern 5. A false hit stops a resumable batch (loud, and
// the next resume retries that run first); a false miss would grade a refusal as a model failure and silently corrupt the study. So the wide
// patterns stay. The same prose over 600 characters, or not at index 0, is no hit.
// NOT MATCHED BY DECISION (A2; none of the three is verified to occur): ANSI-coloured text, an `Error: ` / `API Error: ` prefix, and a refusal
// longer than 600 characters.
//
// THE CHANNELS, in this order, first hit wins:
//   (1) envelope.api_error is one of PLAN_LIMIT_API_ERRORS   the typed field of the CLI's SDK schema. Needs neither the anchor nor the cap: the
//                                                     type is authoritative. If the envelope also has a non-blank `result` string, that text
//                                                     (from its first non-blank character, cut to 200 characters) is `matched`; otherwise
//                                                     `matched` is the kind name.
//   (2) envelope.result   when a string               (3) stdout   (4) stderr
//   (5) the LAST main-chain assistant record of the transcript: if its `apiError` is one of PLAN_LIMIT_API_ERRORS it is a hit whatever its text
//       (same `matched` rule as channel 1, source 'transcript'); otherwise its text is checked like channels 2-4.
//   (6) usageLimitNoteIn(transcript)                  the wrap-up note (below); source 'transcript_note'; needs neither the anchor nor the cap
// `envelope` must be a plain object (a string or an array is ignored). No channel depends on `is_error`: the verified API-error envelope has
// `is_error: true` but also `subtype: "success"`, and a refusal can come back as an ordinary `is_error: false` result.
//
// PLAN_LIMIT_API_ERRORS (A2/F5) = ['usage_limit_reached', 'model_requires_usage_credits']: both are refusals of a model run (the SDK enum
// describes the second as "the model runs on usage credits for this account, and credits are off or the included allowance is used up"; the CLI
// builds the 429 overage texts with it). The enum has other credit and consent kinds (seen in the binary: long_context_credits_required,
// consent_unanswered, org_disabled_credential, api_key_auth_disabled, …). They are NOT included: they are not a plan or spend limit.
//
// THE TYPED TRANSCRIPT SIGNAL (A2/F4). The CLI's error builder gives the transcript record of a classified refusal `error: "rate_limit"` plus
// `apiError` (`usage_limit_reached`) and `apiErrorParams: {rate_limit_info}` (camelCase), and sets the last two only when its own classifier
// matches the text. `error: "rate_limit"` ALONE is not a signal: the server-overload record has it too. So channel 5 reads `apiError` and
// `apiErrorParams`. That the transcript file persists those fields is NOT verified (they are read from the builder in the binary).
//
// THE WRAP-UP NOTE (A1, A2/F1). The binary has a flag-gated, default-off wrap-up mode. When it is on and a usage limit is reached, or about to
// be, the CLI injects into the conversation an `isMeta` user message whose record carries the property `usageLimitNote`. The property has exactly
// three values, read in both binaries:
//   wrap_up     content `[Usage limit reached — grace window active. Wrap up: finish or checkpoint; don't start subagents or long work.]` (a
//               variant asks for next steps): the CLI tells the model to wrap up;
//   near_limit  content `[Usage limit approaching. Checkpoint now: finish the current step, then list up to 3 short bullets of the most impactful
//               remaining work. Don't start subagents or long-running work.]`, pushed together with the visible system notice `Approaching your
//               5-hour usage limit — Claude will wrap up the current step.`: the CLI tells the model to checkpoint;
//   release     pushed only after one of the other two, to lift that state.
// RULING A2/F1: EVERY value is a hit. The run is cut short by the plan (the model was told to stop early, or has just been released from that),
// so it is not gradeable. The visible warning text ALONE stays a non-match in the text channels: a warning cuts nothing; only the injected note
// counts. The run then ends with a NORMAL-looking final text, so channel (5) cannot see it.
// `usageLimitNoteIn(transcriptText)` scans the MAIN-chain records (isSidechain !== true; a torn or non-JSON line is skipped) in file order and
// returns, for the first record that qualifies: the record's OWN property `usageLimitNote` when it is a non-empty string (returned as it is), else
// the first of the record's texts that starts, after trimming leading whitespace, with `[Usage limit reached` or `[Usage limit approaching`
// (returned cut to 200 characters; the end of the text is not trimmed). The texts of a record are the locations seen in real records: a top-level
// `content` string; a string message.content and the `text` of its {type:'text'} blocks; attachment.content (a string, or the string elements of
// an array), attachment.text and attachment.prompt; the `content` strings of the top-level arrays `rendered` and `renderedInHumanTurn`. Records of
// every type are read. NOT read, because it is tool or hook output and not the CLI speaking: `toolUseResult`, `tool_result` blocks, hook
// `stdout`/`stderr`. It returns null otherwise.
//
// THE HIT. { source, matched, resetHint, rateLimitType, resetsAt }.
//   source         'envelope_api_error' | 'envelope_result' | 'stdout' | 'stderr' | 'transcript' | 'transcript_note'
//   matched        the candidate text from its first non-blank character, cut to 200 characters (a consumer redacts a token before it stores it)
//   resetHint      the first match of /\bresets?\b[^\n]{0,80}/ in `matched` (case-sensitive; at most 80 characters follow the word; the line ends at
//                  "\n" only, so a CR stays in the hint, and trailing spaces stay), else null
//   rateLimitType  rate_limit_info.rateLimitType when it is a member of RATE_LIMIT_TYPES, else null
//   resetsAt       rate_limit_info.resetsAt when it is a finite number, else null (each field is judged on its own; the unit is not verified)
// Where rate_limit_info comes from (A2/F3, F4): `api_error_params` is declared on the ASSISTANT message schema of the SDK, not on the result
// schema (the result schema has `api_error`, `api_error_status` and `api_error_code`), so expect both fields null from a `--output-format json`
// result envelope. The module still reads `envelope.api_error_params.rate_limit_info` when present. For a channel-5 hit the record's own
// `apiErrorParams.rate_limit_info` comes first, field by field, and the envelope's fills what the record lacks. With neither they are null.
//
// The last assistant record (`lastAssistantRecordText`) is read here on purpose, API-error records INCLUDED. s5/usage.mjs `assistantBlocks` skips
// them (it counts usage), so the final text it shows never contains a refusal; this module must not reuse it.
//
// VERIFIED, from real bytes (the CLI binary, string extraction in s5-work/briefs/limit-strings-*.txt; a real transcript record):
//   - the classifier list and regex above, and the message texts, in 2.1.292 and 2.1.296 (identical, see above);
//   - the typed `api_error` kinds `usage_limit_reached` and `model_requires_usage_credits` in the SDK schema (the result schema carries
//     `api_error`; `api_error_params.rate_limit_info` — `rateLimitType` in five_hour, seven_day, seven_day_opus, seven_day_sonnet,
//     seven_day_overage_included, overage; `resetsAt` — is declared on the assistant message schema);
//   - the CLI's error builder: `error:"rate_limit"` plus `apiError:"usage_limit_reached"` and `apiErrorParams:{rate_limit_info}`, the last two
//     only when the classifier matches the text (both binaries);
//   - the shape of a transcript API-error record (the 401 record in the tests): type assistant, isSidechain false, isApiErrorMessage true,
//     error "authentication_failed", apiErrorStatus 401, message.model "<synthetic>", message.content [{type:'text', text}];
//   - that the binary builds the wrap-up texts and the three `usageLimitNote` values (wrap_up, release, near_limit).
// NOT VERIFIED, and every test that builds one says SYNTHETIC or RECONSTRUCTED:
//   - the envelope and the transcript of a REAL plan-limit hit (none exists on this host);
//   - that the transcript FILE persists `apiError`/`apiErrorParams` on a record (read from the builder, not from a transcript), or `usageLimitNote`
//     (other properties of the same record constructor, such as turnCompanion, are persisted: that raises the chance, it does not verify it);
//   - where the CLI delivers the note text (a user record, an attachment, a system record): hence the wide list of locations;
//   - that `--output-format json` puts `api_error` on the result envelope: the typed channel is an optional extra, used when present and never
//     required; and it will not carry the rate-limit fields (above);
//   - that in plain `-p` text mode the refusal goes to stdout and not stderr (both are read), and what it looks like there (ANSI colour, an
//     `Error: ` prefix); that `claude -p` prints any of the status templates of pattern 5.
// KNOWN GAPS: a refusal that the CLI's own classifier does not list is not detected; a refusal over 600 characters is not detected; a note that
// the CLI delivers in a location the scan does not read (`snippet` of an edited_text_file attachment, the `hookAdditionalContext`/`hookInfos`/
// `hookErrors` arrays of a stop_hook_summary) is missed unless the record carries `usageLimitNote`, which is read on every record.
//
// RE-VERIFY A CLI VERSION before adding it to VERIFIED_CLI_VERSIONS (read-only; never run the binary to do it, and never with a real token).
// Re-extract the classifier's prefix list and compare it with the patterns above and with CLI_CLASSIFIER_PREFIXES in the test:
//   grep -a -o -E 'syr=\[[^]]{0,2500}\]' ~/.local/share/claude/versions/<version>        (the variable is `syr` in 2.1.292 only; a later build renames it)
//   grep -a -o -E '\["You.ve hit your","You.ve reached your"[^]]{0,2500}\]' ~/.local/share/claude/versions/<version>     (works in any version)
// the regex: grep -a -o -E '=\[/\^\(\?:Fable\|Haiku\|Mythos\|Opus\|Sonnet\)\(\?: [^/]{0,80}/\]' <binary>
// the note values: grep -a -o -E 'usageLimitNote:"[a-z_]{1,30}"' <binary>     (expect near_limit, release, wrap_up)
// the typed kinds: grep -a -o -E 'apiError:"[a-z_]{1,40}"' <binary>     (expect usage_limit_reached and model_requires_usage_credits among others)
// Then add the version string to VERIFIED_CLI_VERSIONS below AND to the exact-list test in plan-limit.test.mjs: adding a version is a deliberate
// edit of both.

const isObject = (v) => v !== null && typeof v === 'object' && !Array.isArray(v);

// The CLI versions whose refusal texts were read (see RE-VERIFY above). S5's batchPreflight refuses a registration for any other version.
export const VERIFIED_CLI_VERSIONS = ['2.1.292', '2.1.296'];

// R-PL2: the longest text (after the leading whitespace) that can be a refusal.
export const MAX_REFUSAL_CHARS = 600;

// The `rateLimitType` values of the CLI's SDK schema (api_error_params.rate_limit_info).
export const RATE_LIMIT_TYPES = ['five_hour', 'seven_day', 'seven_day_opus', 'seven_day_sonnet', 'seven_day_overage_included', 'overage'];

// A2/F5: the typed `api_error` / `apiError` kinds that are a refusal of a model run by a plan or spend limit.
export const PLAN_LIMIT_API_ERRORS = ['usage_limit_reached', 'model_requires_usage_credits'];

// How much of the matched text a Hit keeps, and the reset phrase in it.
const MATCHED_CHARS = 200;
const RESET_HINT = /\bresets?\b[^\n]{0,80}/;

// The wrap-up markers (A1, A2/F1): the starts of the texts of the notes the CLI injects.
const NOTE_MARKERS = ['[Usage limit reached', '[Usage limit approaching'];

// R-PL2 + A1: no `g`, `y` or `m` flag on any of these (a stateful or line-anchored regex would change the verdict). Numbered as in the header.
const REFUSAL_PATTERNS = [
  /^You['’]ve hit your\b/,
  /^You['’]ve reached your\b/,
  /^You['’]re out of (?:extra usage|usage credits)/,
  /^Your org(?:anization)? is out of usage/,
  /^Usage limit reached/,
  /^Your seat type doesn['’]t include (?:extra )?usage/,
  /^Your usage allocation has been disabled by your admin/,
  /^Your group['’]s usage limit is set to \$0/,
  /^(?:Fable|Haiku|Mythos|Opus|Sonnet)(?: [^\u00B7\n]{1,40})? requires usage credits\./,
  /^Fable 5 requires usage credits/,
];

export class PlanLimitError extends Error {
  constructor(hit) {
    super(`plan limit: ${hit.source}: ${hit.matched}`);
    this.name = 'PlanLimitError';
    this.hit = hit;
  }
}

// The text of a content value: a string as it is, or the `text` of every {type:'text'} block joined with a newline; null when there is none.
function contentText(content) {
  if (typeof content === 'string') return content === '' ? null : content;
  if (!Array.isArray(content)) return null;
  const joined = content.filter((b) => isObject(b) && b.type === 'text' && typeof b.text === 'string').map((b) => b.text).join('\n');
  return joined === '' ? null : joined;
}

// The `message.content` of a record, or undefined.
const messageContent = (record) => (isObject(record.message) ? record.message.content : undefined);

// The LAST main-chain assistant record of a transcript (JSONL), API-error records included; null when there is none. A blank, torn or non-JSON
// line (JSON.parse of a blank line throws like any other), and a JSON value that is no record, is skipped. Read from the end, so only the tail of
// a long transcript is parsed.
function lastAssistantRecord(transcriptText) {
  if (typeof transcriptText !== 'string') return null;
  const lines = transcriptText.split('\n');
  for (let i = lines.length - 1; i >= 0; i -= 1) {
    let record;
    try {
      record = JSON.parse(lines[i]);
    } catch {
      continue;
    }
    if (!isObject(record) || record.type !== 'assistant' || record.isSidechain === true) continue;
    return record;
  }
  return null;
}

// The text of that record; null when there is no such record or it has no text. The last assistant record decides: an earlier one is not
// consulted when the last has no text.
export function lastAssistantRecordText(transcriptText) {
  const record = lastAssistantRecord(transcriptText);
  return record === null ? null : contentText(messageContent(record));
}

// The strings of one record that can hold the wrap-up marker (A2, section 7): a top-level `content` string; a string message.content or the `text`
// of its {type:'text'} blocks; attachment.content (a string, or the string elements of an array), attachment.text and attachment.prompt; the
// `content` strings of the arrays `rendered` and `renderedInHumanTurn`. NOT toolUseResult, tool_result blocks, or hook stdout/stderr. Each is a
// candidate on its own.
function noteCandidates(record) {
  const out = [];
  const add = (value) => {
    if (typeof value === 'string') out.push(value);
  };
  add(record.content);
  const content = messageContent(record);
  if (typeof content === 'string') out.push(content);
  else if (Array.isArray(content)) {
    for (const block of content) if (isObject(block) && block.type === 'text') add(block.text);
  }
  if (isObject(record.attachment)) {
    add(record.attachment.text);
    add(record.attachment.prompt);
    if (typeof record.attachment.content === 'string') out.push(record.attachment.content);
    else if (Array.isArray(record.attachment.content)) record.attachment.content.forEach(add);
  }
  for (const rendered of [record.rendered, record.renderedInHumanTurn]) {
    if (Array.isArray(rendered)) for (const item of rendered) if (isObject(item)) add(item.content);
  }
  return out;
}

// A1, A2: the wrap-up note of a transcript (JSONL), or null. Over the main-chain records in file order, the first record that has an OWN property
// `usageLimitNote` which is a non-empty string (that string, as it is), or one of whose texts starts, after leading whitespace, with
// `[Usage limit reached` or `[Usage limit approaching` (that text, cut to 200 characters). A torn or non-JSON line, a JSON value that is no
// record, and a sidechain record are skipped.
export function usageLimitNoteIn(transcriptText) {
  if (typeof transcriptText !== 'string') return null;
  for (const line of transcriptText.split('\n')) {
    let record;
    try {
      record = JSON.parse(line);
    } catch {
      continue;
    }
    if (!isObject(record) || record.isSidechain === true) continue;
    if (Object.hasOwn(record, 'usageLimitNote') && typeof record.usageLimitNote === 'string' && record.usageLimitNote !== '') return record.usageLimitNote;
    for (const text of noteCandidates(record)) {
      const body = text.trimStart();
      if (NOTE_MARKERS.some((marker) => body.startsWith(marker))) return body.slice(0, MATCHED_CHARS);
    }
  }
  return null;
}

// R-PL2: the candidate after its leading whitespace when it is a refusal (a string, at most MAX_REFUSAL_CHARS, anchored); else null.
function refusalText(candidate) {
  if (typeof candidate !== 'string') return null;
  const text = candidate.trimStart();
  if (text.length > MAX_REFUSAL_CHARS) return null;
  return REFUSAL_PATTERNS.some((pattern) => pattern.test(text)) ? text : null;
}

// { rateLimitType, resetsAt } of an `api_error_params` / `apiErrorParams` object's rate_limit_info, each field null unless well formed.
function rateInfoOf(params) {
  const info = isObject(params) ? params.rate_limit_info : null;
  if (!isObject(info)) return { rateLimitType: null, resetsAt: null };
  return {
    rateLimitType: RATE_LIMIT_TYPES.includes(info.rateLimitType) ? info.rateLimitType : null,
    resetsAt: typeof info.resetsAt === 'number' && Number.isFinite(info.resetsAt) ? info.resetsAt : null,
  };
}

// Field by field: the primary's value when it has one, else the fallback's.
const mergeRateInfo = (primary, fallback) => ({
  rateLimitType: primary.rateLimitType ?? fallback.rateLimitType,
  resetsAt: primary.resetsAt ?? fallback.resetsAt,
});

function hitOf(source, text, info) {
  const matched = text.slice(0, MATCHED_CHARS);
  const reset = RESET_HINT.exec(matched);
  return { source, matched, resetHint: reset === null ? null : reset[0], ...info };
}

// A hit decided by a typed kind: `matched` is the text from its first non-blank character, or the kind name when there is none.
function typedHit(source, kind, text, info) {
  const body = typeof text === 'string' ? text.trimStart() : '';
  return hitOf(source, body === '' ? kind : body, info);
}

// null, or the Hit of the first channel that carries a refusal. Never throws for a JSON value, a primitive or a plain object (R-PL1).
export function detectPlanLimit(input) {
  const { envelope: rawEnvelope, stdout, stderr, transcript } = isObject(input) ? input : {};
  const envelope = isObject(rawEnvelope) ? rawEnvelope : null;
  const envelopeInfo = rateInfoOf(envelope === null ? null : envelope.api_error_params);

  // (1) the typed kind on the envelope
  if (envelope !== null && PLAN_LIMIT_API_ERRORS.includes(envelope.api_error)) {
    return typedHit('envelope_api_error', envelope.api_error, envelope.result, envelopeInfo);
  }

  // (2) to (4) the text channels
  const textChannels = [
    ['envelope_result', envelope === null ? undefined : envelope.result],
    ['stdout', stdout],
    ['stderr', stderr],
  ];
  for (const [source, candidate] of textChannels) {
    const text = refusalText(candidate);
    if (text !== null) return hitOf(source, text, envelopeInfo);
  }

  // (5) the last main-chain assistant record: the typed kind on the record (A2/F4), else its text
  const record = lastAssistantRecord(transcript);
  if (record !== null) {
    const content = contentText(messageContent(record));
    const info = mergeRateInfo(rateInfoOf(record.apiErrorParams), envelopeInfo);
    if (PLAN_LIMIT_API_ERRORS.includes(record.apiError)) return typedHit('transcript', record.apiError, content, info);
    const text = refusalText(content);
    if (text !== null) return hitOf('transcript', text, info);
  }

  // (6) A1: the wrap-up note is a typed property or a bracketed marker, not free text: no anchor, no cap, only the 200-character cut of `matched`.
  const note = usageLimitNoteIn(transcript);
  return note === null ? null : hitOf('transcript_note', note, envelopeInfo);
}

// Returns undefined when no channel carries a refusal; throws PlanLimitError (verdict in `.hit`) when one does.
export function assertNoPlanLimit(input) {
  const hit = detectPlanLimit(input);
  if (hit !== null) throw new PlanLimitError(hit);
}
