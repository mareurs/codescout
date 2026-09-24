#!/usr/bin/env python3
"""Build the rtk-for-codescout evaluation report (PDF). All numbers are from the 2026-09-24 session.

Rebuild (needs reportlab):
    python3 docs/research/2026-09-24-rtk-evaluation-report.py docs/research/2026-09-24-rtk-evaluation.pdf
"""
import sys
from reportlab.lib import colors
from reportlab.lib.enums import TA_LEFT
from reportlab.lib.pagesizes import A4
from reportlab.lib.styles import ParagraphStyle, getSampleStyleSheet
from reportlab.lib.units import mm
from reportlab.platypus import (KeepTogether, Paragraph, SimpleDocTemplate, Spacer, Table,
                                TableStyle)

OUT = sys.argv[1]
ACCENT = colors.HexColor("#1f4e79")
MUTED = colors.HexColor("#5a5a5a")
RULE = colors.HexColor("#c8d3df")
ZEBRA = colors.HexColor("#f3f6f9")
WARN_BG = colors.HexColor("#fdf1e7")
WARN_EDGE = colors.HexColor("#d9822b")
OK_BG = colors.HexColor("#eaf3ea")

ss = getSampleStyleSheet()
base = ParagraphStyle("base", parent=ss["Normal"], fontName="Helvetica", fontSize=9.5,
                      leading=13.2, alignment=TA_LEFT, spaceAfter=5)
small = ParagraphStyle("small", parent=base, fontSize=8, leading=10.5, textColor=MUTED)
cell = ParagraphStyle("cell", parent=base, fontSize=8.3, leading=10.6, spaceAfter=0)
cellb = ParagraphStyle("cellb", parent=cell, fontName="Helvetica-Bold")
code = ParagraphStyle("code", parent=base, fontName="Courier", fontSize=7.8, leading=10,
                      backColor=ZEBRA, borderPadding=(4, 5, 4, 5), spaceBefore=3, spaceAfter=8)
h1 = ParagraphStyle("h1", parent=base, fontName="Helvetica-Bold", fontSize=13.5, leading=17,
                    textColor=ACCENT, spaceBefore=12, spaceAfter=6)
h2 = ParagraphStyle("h2", parent=base, fontName="Helvetica-Bold", fontSize=10.8, leading=14,
                    textColor=ACCENT, spaceBefore=8, spaceAfter=4)
title = ParagraphStyle("title", parent=base, fontName="Helvetica-Bold", fontSize=20, leading=24,
                       textColor=ACCENT, spaceAfter=4)
subtitle = ParagraphStyle("subtitle", parent=base, fontSize=11, leading=15, textColor=MUTED,
                          spaceAfter=10)
bullet = ParagraphStyle("bullet", parent=base, leftIndent=12, bulletIndent=2, spaceAfter=3)


def P(t, s=base): return Paragraph(t, s)
def B(t): return Paragraph(t, bullet, bulletText="•")
def C(t): return f'<font face="Courier">{t}</font>'


def table(rows, widths, num_cols=(), head=True, zebra=True):
    data = [[Paragraph(str(c), cellb if (head and r == 0) else cell) for c in row]
            for r, row in enumerate(rows)]
    t = Table(data, colWidths=widths, repeatRows=1 if head else 0)
    st = [("VALIGN", (0, 0), (-1, -1), "TOP"),
          ("LINEBELOW", (0, 0), (-1, 0), 0.8, ACCENT),
          ("LINEBELOW", (0, 1), (-1, -1), 0.25, RULE),
          ("TOPPADDING", (0, 0), (-1, -1), 3), ("BOTTOMPADDING", (0, 0), (-1, -1), 3),
          ("LEFTPADDING", (0, 0), (-1, -1), 4), ("RIGHTPADDING", (0, 0), (-1, -1), 4)]
    if zebra:
        for r in range(1, len(rows)):
            if r % 2 == 0:
                st.append(("BACKGROUND", (0, r), (-1, r), ZEBRA))
    for c in num_cols:
        for r in range(len(rows)):
            data[r][c].style = ParagraphStyle("num", parent=data[r][c].style, alignment=2)
    t.setStyle(TableStyle(st))
    return t


def callout(paras, bg, edge):
    t = Table([[paras]], colWidths=[170 * mm])
    t.setStyle(TableStyle([("BACKGROUND", (0, 0), (-1, -1), bg),
                           ("LINEBEFORE", (0, 0), (0, -1), 3, edge),
                           ("LEFTPADDING", (0, 0), (-1, -1), 9), ("RIGHTPADDING", (0, 0), (-1, -1), 9),
                           ("TOPPADDING", (0, 0), (-1, -1), 7), ("BOTTOMPADDING", (0, 0), (-1, -1), 7)]))
    return t


def footer(canv, doc):
    canv.saveState()
    canv.setFont("Helvetica", 7.5)
    canv.setFillColor(MUTED)
    canv.drawString(20 * mm, 12 * mm, "rtk for codescout - evaluation report - 2026-09-24")
    canv.drawRightString(190 * mm, 12 * mm, f"page {doc.page}")
    canv.restoreState()


s = []
s += [P("rtk for codescout: evaluation report", title),
      P("Should we adopt rtk-ai/rtk (Rust Token Killer) in front of codescout's "
        + C("run_command") + "? What we measured, how, and what we recommend.", subtitle),
      P("Date: 2026-09-24 &nbsp;&middot;&nbsp; rtk version tested: v0.49.0 (released 2026-09-11) "
        "&nbsp;&middot;&nbsp; codescout: branch " + C("experiments") + " @ " + C("36999188"), small),
      Spacer(1, 8)]

# ---- 1. Summary
s.append(P("1. Summary", h1))
s.append(callout([
    P("<b>Recommendation: do not adopt rtk as a wrapper around codescout's run_command.</b>", base),
    P("The saving it would add is small: an upper bound of <b>0.63% of all codescout tool output</b> "
      "(about 1.2K tokens per session). That is because codescout already buffers every output "
      "larger than ~10 KB behind a short summary. rtk also <b>disables two codescout safety guards</b> "
      "(the empty-test-selection warning and the IL-3 pipe gate) and <b>removes content by default</b> "
      "(commit bodies, diff hunks, grep indentation).", base),
    P("<b>Worth porting instead:</b> summarising small, inline " + C("cargo test") + " output, which "
      "codescout currently returns raw. rtk saves 88% on those calls. Implemented inside codescout, it "
      "keeps the raw output retrievable and keeps the guards working.", base)],
    WARN_BG, WARN_EDGE))
s.append(Spacer(1, 6))
s.append(table([
    ["Question", "Answer (measured)"],
    ["Does rtk's Claude Code hook work with codescout?",
     "No. It rewrites only native <b>Bash</b> tool calls. Native Bash is denied on all our profiles "
     "(since 2026-09-20), and the hook never touches MCP tools such as run_command."],
    ["How much of our shell traffic would rtk rewrite?", "60.9% of run_command calls, 63.3% of bytes returned."],
    ["How much would it save?", "At most 0.97 MB/month, which is 0.63% of the 154 MB of codescout tool output."],
    ["Does it break anything?", "Yes. It silences the empty-test-selection warning, bypasses the IL-3 pipe "
     "gate (a failing suite reported exit 0), and removes content from git log / git diff / grep / clippy output."],
], [58 * mm, 112 * mm]))

# ---- 2. What rtk is
s.append(P("2. What rtk is and how it would plug in", h1))
s.append(P("rtk is a single Rust binary (Apache-2.0, ~82k GitHub stars) that sits between an AI agent and "
           "the shell and filters command output before the model reads it. Its README claims 60-90% "
           "fewer tokens on common dev commands. It does this by filtering noise, grouping, truncating, "
           "and collapsing repeated lines. " + C("rtk init -g") + " installs a Claude Code PreToolUse "
           "hook that rewrites Bash commands, e.g. " + C("git status") + " becomes " + C("rtk git status") + "."))
s.append(P("Three features made offline measurement possible:"))
s += [B(C("rtk rewrite \"&lt;cmd&gt;\"") + " prints the rtk form of a command, or exits 1 if rtk has no "
        "equivalent. It is the same function the hook uses."),
      B(C("rtk pipe -f &lt;filter&gt;") + " applies a filter to stdin, so previously recorded outputs can "
        "be re-filtered without re-running anything."),
      B(C("rtk discover") + " scans Claude Code transcripts for Bash commands rtk could have compressed.")]
s.append(P("<b>Integration path for codescout.</b> The stock hook cannot reach us, so adopting rtk would mean "
           "a custom hook that calls " + C("rtk rewrite") + " on the " + C("command") + " argument of "
           + C("mcp__codescout__run_command") + ", or codescout prefixing commands itself. "
           "Either way, codescout would then receive rtk's filtered text instead of the real output. "
           "That matters for the findings in section 5."))

# ---- 3. Method
s.append(P("3. How we tested", h1))
s.append(P("We used four complementary methods. The first two use <b>real recorded traffic</b> rather than a "
           "synthetic benchmark. The last two check <b>fidelity</b>, which byte counts cannot show."))
s.append(table([
    ["#", "Method", "What it measures", "Input"],
    ["1", "Corpus coverage", "Which recorded commands rtk would rewrite at all",
     "All run_command calls in codescout's " + C("usage.db") + ", 2026-08-25 to 2026-09-24: 23,641 calls, "
     "30.5 MB returned, 206 sessions"],
    ["2", "Offline replay", "Bytes before/after rtk on outputs the model actually saw",
     "Recorded outputs codescout returned inline (not buffered), single-segment commands only, piped "
     "through " + C("rtk pipe -f") + " with the matching filter"],
    ["3", "Live A/B pairs", "Raw vs " + C("rtk &lt;cmd&gt;") + " on today's tree, plus content inspection",
     "14 commands (git, grep, ls, wc) on the codescout checkout"],
    ["4", "Guard probes", "Whether rtk output still triggers codescout's own safety checks",
     "A throwaway cargo crate (45 tests: pass, fail, should_panic, ignored; one clippy warning), run "
     "raw and through rtk, both via codescout's run_command"],
], [7 * mm, 27 * mm, 52 * mm, 84 * mm]))
s.append(Spacer(1, 4))
s.append(P("<b>Why bytes, and why only inline outputs.</b> codescout returns an output in full when it is under "
           "~10 KB. Above that it stores the full output in an " + C("@cmd_*") + " buffer and returns a "
           "summary of at most ~3 KB. For buffered calls the model never saw raw output, so rtk's "
           "reduction of raw output is not the right baseline. For inline calls, the recorded output is "
           "exactly what the model read. Tokens are estimated as bytes / 4, the same convention rtk and "
           "codescout both use."))
s.append(P("Hygiene: the rtk binary was the official release tarball, checked against the published sha256 "
           "(" + C("7278231d...0c8f") + "). Telemetry was disabled (" + C("RTK_TELEMETRY_DISABLED=1") + "). "
           "Everything ran from a scratch directory, and no hook was installed.", small))

# ---- 4. Results
s.append(P("4. Results", h1))
s.append(P("4.1 Coverage: what rtk would touch", h2))
s.append(P("Of 23,641 recorded run_command calls, " + C("rtk rewrite") + " would rewrite <b>14,401 (60.9%)</b>, "
           "accounting for <b>19.3 MB (63.3%)</b> of the bytes codescout returned. The largest rewritable families:"))
s.append(table([
    ["rtk subcommand", "calls", "bytes returned", "of which buffered by codescout"],
    ["cargo test", "2,936", "6,097,483", "761"],
    ["grep -n", "1,129", "1,659,020", "38"],
    ["git diff", "797", "1,326,628", "58"],
    ["git log", "789", "981,602", "7"],
    ["git status", "1,074", "790,411", "9"],
    ["git show", "244", "640,443", "27"],
    ["git add", "882", "616,387", "1"],
    ["grep -E", "514", "608,286", "9"],
    ["ls -la", "286", "431,036", "7"],
    ["cargo clippy", "449", "372,955", "98"],
], [45 * mm, 25 * mm, 40 * mm, 60 * mm], num_cols=(1, 2, 3)))
s.append(Spacer(1, 3))
s.append(P("Not rewritable (39.1% of calls): " + C("echo") + ", " + C("python3") + ", " + C("sed") + ", "
           "shell loops, project scripts such as " + C("./scripts/gate.sh") + ", and queries against codescout's "
           "own " + C("@cmd_*") + " buffers.", small))

s.append(P("4.2 Offline replay: savings on outputs the model actually read", h2))
s.append(table([
    ["rtk filter", "calls", "bytes before", "bytes after", "saved", "reliability"],
    ["cargo-test", "319", "749,254", "86,579", "88.4%", "<b>Reliable</b>: live rtk behaves the same (2,224 to 437 B on the fixture)"],
    ["git-diff", "333", "671,474", "473,591", "29.5%", "Upper bound (see below)"],
    ["grep", "287", "297,591", "278,934", "6.3%", "Reliable"],
    ["git-log", "111", "106,693", "28,923", "72.9%", "Upper bound: live rtk passes explicit --format through (-1%)"],
    ["git-status", "220", "87,040", "86,812", "0.3%", "Reliable: agents already use --short"],
    ["find", "24", "24,007", "13,506", "43.7%", "Reliable"],
    ["<b>Total</b>", "<b>1,294</b>", "<b>1,936,059</b>", "<b>968,345</b>", "<b>50.0%</b>", "~242K tokens/month, upper bound"],
], [22 * mm, 13 * mm, 23 * mm, 22 * mm, 14 * mm, 76 * mm], num_cols=(1, 2, 3, 4)))
s.append(Spacer(1, 3))
s.append(P("<b>Why two rows are upper bounds.</b> Pipe mode applies rtk's default-format filter no matter which "
           "flags the original command used. We re-ran two outliers live. Recorded "
           + C("git log -2 --format='===%n%B'") + " fell from 3,818 to 274 B in pipe mode, but live "
           + C("rtk git log -2 --format=%B") + " went from 2,882 to 2,854 B: it respects the requested format. "
           "Likewise " + C("git show --stat --format=...") + " went to 0 B in pipe mode (the git-diff filter misread "
           "it) but passed through unchanged live."))

s.append(P("4.3 What the saving amounts to", h2))
s.append(table([
    ["Quantity", "Value"],
    ["All codescout MCP tool output, same 30 days", "154.1 MB (~38.5M tokens)"],
    ["of which run_command", "30.6 MB (20%)"],
    ["Maximum rtk saving (inline outputs, section 4.2)", "0.97 MB = <b>0.63%</b> of all tool output"],
    ["Reliable part (cargo-test only)", "0.66 MB = <b>0.43%</b>"],
    ["Per session (206 sessions)", "~4.7 KB, about <b>1.2K tokens</b>"],
    ["rtk's own estimate from Bash-era transcripts (" + C("rtk discover") + ", 248 sessions)",
     "~1.6M tokens \"saveable\" across 19,445 Bash commands. This path no longer exists now that Bash is denied."],
], [85 * mm, 85 * mm]))

s.append(P("4.4 Live A/B pairs on the current tree", h2))
live = [
    ["Command", "raw B", "rtk B", "saved", "What rtk removed"],
    [C("git status"), "997", "514", "48%", "Git's hint text; rewritten as porcelain-like lines. No information lost."],
    [C("git log -20"), "44,861", "7,376", "83%", "Commit bodies cut to 3 lines + \"[+N lines omitted]\"; subjects cut at ~110 chars"],
    [C("git log --oneline -20"), "1,845", "1,808", "2%", "Long subjects truncated"],
    [C("git log -5 --stat"), "10,080", "10,080", "0%", "Nothing (passthrough)"],
    [C("git diff") + " (dirty tree)", "299,041", "61,273", "79%", "Per-hunk caps (\"702 additions truncated\"); last file's hunks dropped entirely"],
    [C("git diff --stat"), "625", "624", "0%", "Nothing"],
    [C("git show HEAD"), "5,862", "4,565", "22%", "Diff compaction"],
    [C("git show HEAD~3"), "11,842", "10,359", "12%", "Diff compaction"],
    [C("grep -rn RecoverableError src/tools"), "43,431", "17,635", "59%", "Indentation stripped, long lines truncated, 22 files hidden behind rtk recall"],
    [C("grep -rn \"fn call_content\" src"), "1,060", "1,060", "0%", "Nothing"],
    [C("ls -la src/tools"), "2,050", "727", "64%", "Permissions/owner columns"],
    [C("ls docs/issues"), "5,393", "3,916", "27%", "Layout"],
    [C("wc -l") + " (2 files)", "85", "58", "31%", "Padding"],
    [C("cargo test") + " (fixture, 1 failure)", "2,224", "437", "80%", "Passing test lines and compiler warnings; failure panic kept"],
    [C("cargo clippy") + " (fixture)", "944", "179", "81%", "Lint names and the help: fix suggestions"],
]
s.append(table(live, [52 * mm, 17 * mm, 16 * mm, 12 * mm, 73 * mm], num_cols=(1, 2, 3)))
s.append(P("A 15th pair (" + C("find") + ") was excluded because the glob was unquoted and returned nothing in both arms.", small))

s.append(KeepTogether([
    P("4.5 Interaction with codescout's buffering", h2),
    P("codescout already caps what the model sees for large outputs, so rtk's large-output savings mostly "
      "don't reach the model. Sometimes they make things worse:"),
    table([
        ["Command", "Under codescout today", "Under codescout + rtk"],
        [C("git log -20") + " (44.9 KB raw)",
         "Buffered; the model sees a head/tail summary of at most ~3 KB, and the full log stays in "
         + C("@cmd_*") + " (lossless)",
         "rtk output is 7.4 KB, under the 10 KB threshold, so the model sees <b>all 7.4 KB</b>, "
         "and it is missing the commit bodies"],
        [C("git diff") + " (299 KB raw)", "Buffered; ~3 KB summary; full diff queryable",
         "Still buffered (61 KB); same ~3 KB summary, but the buffer now holds the <b>truncated</b> diff"],
        [C("cargo test --workspace") + " (~830 KB raw)",
         "Buffered; the summary is about 120 B on a green run (passed/ignored counts)", "Similar size; no gain"],
    ], [42 * mm, 64 * mm, 64 * mm]),
]))

# ---- 5. Fidelity & guards
s.append(P("5. Fidelity and safety findings", h1))
s.append(P("Each finding below was observed live via codescout's run_command, not inferred from the source."))
s.append(table([
    ["Finding", "Evidence", "Impact"],
    ["<b>Empty-test-selection warning silenced</b>",
     "Raw " + C("cargo test no_such_test") + " prints " + C("test result: ok. 0 passed; ... 45 filtered out")
     + " and codescout attaches its " + C("empty_test_selection") + " warning. Through rtk the output is "
     + C("cargo test: 0 passed, 45 filtered out") + " and <b>no warning is attached</b>.",
     "A test filter that matches nothing reads as a pass. This is the incident the guard was built for."],
    ["<b>IL-3 pipe gate bypassed; exit code masked</b>",
     C("cargo test | tail") + " is blocked by IL-3. " + C("rtk cargo test 2&gt;&amp;1 | tail -2")
     + " is <b>not</b> blocked (the gate reads the left-hand side as " + C("rtk") + "), and run_command reported "
     + C("exit_code: 0") + " for a suite with a failing test.",
     "Reopens the masked-failure hazard IL-3 exists to prevent."],
    ["Lossless buffer contract broken",
     "rtk stores removed content in its own store (" + C("rtk recall &lt;hash&gt;") + "). codescout's "
     + C("@cmd_*") + " buffer would hold the filtered text.",
     "\"Query the buffer instead of re-running\" stops being safe; agents would have to learn a second recall mechanism."],
    ["Commit bodies cut", C("git log") + " keeps 3 body lines per commit.",
     "In this repo, commit bodies carry measurements, rationale and Session-Id trailers."],
    ["Diffs cut", "Per-hunk caps, and trailing files are dropped with \"(more changes truncated)\".",
     "Reviews and patch-id work on incomplete diffs."],
    ["grep lines altered", "Leading whitespace stripped; long lines end in \"...\".",
     "A matched line can no longer be pasted into an edit's " + C("old_string") + "."],
    ["clippy detail dropped", "Lint names and " + C("help:") + " suggestions removed; only grouped messages and locations remain.",
     "The model loses the suggested fix."],
    ["<i>Works correctly</i>", "Exit codes preserved (101 on failure); failure panics kept, with left/right values.", "-"],
], [36 * mm, 80 * mm, 54 * mm]))

# ---- 6. Caveats
s.append(P("6. Caveats and limits of this evaluation", h1))
s += [B("One machine, one month, one project (codescout itself). A team with a different command mix, "
        "such as heavy JS/pytest use, could see different coverage."),
      B("The offline replay covers inline, single-segment commands only (1,294 calls). Chained or piped commands "
        "were excluded because their recorded output is not what rtk would see."),
      B("Pipe mode overstates git-log/git-diff savings (section 4.2), so the 0.63% total is an upper bound."),
      B("Byte counts are a token proxy (bytes / 4). They say nothing about cache effects or model accuracy."),
      B("The guard findings were shown on a small fixture crate. The mechanisms (missing libtest summary line; "
        "IL-3 reading the first word) do not depend on crate size."),
      B("The live " + C("git diff") + " ran on a working tree dirty with other sessions' edits, including an "
        "800-line audit log. Absolute sizes are illustrative; what rtk removes is not."),
      B(C("rtk discover") + "'s 1.6M-token figure is rtk's own estimate and was not independently checked.")]

# ---- 7. Recommendation
s.append(P("7. Recommendation", h1))
s.append(callout([
    P("<b>1. Don't wrap run_command with rtk.</b> The gain is at most 0.6% and would cost two safety guards and "
      "the lossless-buffer guarantee.", base),
    P("<b>2. Port the one idea that pays:</b> summarise " + C("cargo test") + " output even below the 10 KB inline "
      "threshold (the median is 1.3 KB and it is currently returned raw). Constraints: keep the raw output in "
      + C("@cmd_*") + "; keep compiler warnings; keep libtest's " + C("test result:") + " line so the "
      "empty-test-selection and partial-selection checks still fire. Expected effect: about -88% on ~320 "
      "calls/month, around 0.4% of total output. It is small but cheap, and it improves every codescout user's "
      "shell calls, not just ours.", base),
    P("<b>3. Optional, low value:</b> rtk-style deduplication of repeated log lines in codescout's generic "
      "summariser. We did not measure it separately.", base),
    P("<b>4. If Bash is ever re-enabled:</b> re-run " + C("rtk discover") + ". The stock rtk hook would then apply "
      "to that path, which codescout's buffering does not cover.", base)],
    OK_BG, colors.HexColor("#4a8a4a")))

# ---- 8. Reproduce
s.append(P("8. How to reproduce", h1))
s.append(P("Install rtk into a scratch directory and turn telemetry off:"))
s.append(P("curl -sL -o rtk.tgz https://github.com/rtk-ai/rtk/releases/download/v0.49.0/rtk-x86_64-unknown-linux-musl.tar.gz<br/>"
           "tar xzf rtk.tgz &amp;&amp; export RTK_TELEMETRY_DISABLED=1 &amp;&amp; ./rtk --version", code))
s.append(P("Coverage and replay. Read " + C("input_json") + " and " + C("output_json") + " for "
           + C("tool_name='run_command'") + " from " + C(".codescout/usage.db") + ". Note that "
           + C("output_json") + " is an MCP content array, so parse " + C("[0].text") + " as JSON. Then:"))
s.append(P("rtk rewrite \"$cmd\"             # exit 0 = rewritable (coverage)<br/>"
           "printf '%s' \"$stdout\" | rtk pipe -f git-diff   # only when the call was inline (no output_id)", code))
s.append(P("Guard probes, on any cargo crate:"))
s.append(P("run_command(\"cargo test no_such_test\")        # empty_test_selection warning attached<br/>"
           "run_command(\"rtk cargo test no_such_test\")    # no warning<br/>"
           "run_command(\"rtk cargo test 2&gt;&amp;1 | tail -2\")  # not blocked by IL-3; exit_code 0 on a red suite", code))
s.append(P("The full findings are also stored in codescout memory " + C("research/rtk-evaluation") + ".", small))

doc = SimpleDocTemplate(OUT, pagesize=A4, leftMargin=20 * mm, rightMargin=20 * mm,
                        topMargin=18 * mm, bottomMargin=20 * mm,
                        title="rtk for codescout - evaluation report",
                        author="codescout team", subject="rtk (Rust Token Killer) evaluation")
doc.build(s, onFirstPage=footer, onLaterPages=footer)
print("wrote", OUT)
