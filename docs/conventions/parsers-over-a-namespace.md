---
id: '7adb86d40d878ae7'
kind: convention
status: active
title: Parsers over a namespace — owe an escape and a disambiguator
owners:
- marius
tags:
- parsers
- conventions
- defect-classes
topic: parsers that make an input unrepresentable
---

# Parsers over a namespace — owe an escape and a disambiguator

`CLAUDE.md` § *Parsers Over a Namespace* points here. This page keeps the section's full text, moved here unchanged on 2026-10-04. The class and its members are `issue-clusters:IC-6` ([`IC-6-addressing-without-an-escape-hatch.md`](../trackers/issue-clusters/IC-6-addressing-without-an-escape-hatch.md)).

A parser that interprets every token in its namespace is correct on every input it *accepts*; the
defect is the input it makes **unrepresentable**. That is why ordinary testing does not reach this
class — you cannot write a test for a case you cannot express, so the suite exercises the inputs
the grammar admits and passes. Promoted 2026-08-31 from `issue-clusters:IC-6` at **27 instances
across five subsystems** (file-format navigation, markdown editing, the citation resolver, four
shell gates, symbol navigation) — the largest class in this corpus, and one that sat at n=2 until
the archive was counted.

**Two halves, and a parser owes both.** *No escape*: `---` read as frontmatter wherever it
appears; a nested triple-backtick fence closing an enclosing quadruple one; content whose first
line looks like a heading deleting the heading it was replacing; a documentation example of
citation syntax counted as a real citation. *No disambiguator*: two byte-identical headings, both
permanently unaddressable; two symbols sharing a `name_path`; three ledgers owning one prefix,
kept apart by zero-padding alone; a qualified citation truncated at 31 characters so two file
stems become one. They fail in opposite directions — the first **refuses** work you can describe,
the second silently does it to the **wrong target** — so answering one is not answering the other.

**The heredoc tell.** Four independent shell gates — IL-3's pipe limiter, the dangerous-command
gate, the source-file gate, and `run_command`'s pipe instrumentation — each separately decided a
heredoc body was command text, and each was fixed separately. A construct that exists *precisely*
to mean "this is data, not syntax" will be misread by every scanner in the process, on its own
schedule. Ask what your parser's heredoc is.

**Before shipping one, answer two questions in the code rather than in your head:** how does a
caller write this token literally, and what happens when two collide? *"It cannot happen"* is a
claim about today's corpus and decays with it — three ledgers sharing a prefix was impossible
until the third existed. Where no escape is affordable, say so **at the refusal site**: a
documented limitation and a silent reinterpretation cost a reader very different amounts. The
corpus states the point better than this section can — an entry id cannot be *mentioned* without
citing it, the only escape being a fenced block, so
`docs/issues/2026-08-31-an-entry-id-cannot-be-mentioned-without-citing-it.md` is this class
holding about the very ledger that records it.

**Recording history is where this class bites hardest, so record less of it.** A superseded
fact written in prose — a former filename, an old count, a retired tool name — is
**indistinguishable from a live citation** to any parser over that namespace, and the parsers
here have no escape for *mention*. `audit_doc_refs` checks every backticked path-shaped token
against the filesystem and caps severity only for a fenced block (`code_block`) or a released
changelog section (`released_history`, whose own legend reads *"history, which must not be
rewritten to satisfy a linter"*). **Ordinary backticks are not an escape** — they get
`policy_default`, which is `high`, which reds CI. Measured 2026-09-06: `Audit Doc Refs` failed on
exactly three `high` findings, all naming one retired test filename, and one of the three was a
deliberate note that the file *used to* be called that. The note cost more than it bought — and
this paragraph named that filename too until the gate's own output was re-read, which is the
section above holding about the sentence that describes it.

So the test is **decision**, not interest: keep the history a reader would **act wrongly without**
— a rejected approach they would otherwise retry, a measurement whose method changes how you read
the number, an exception someone already paid for. Everything else — a rename, a tidy-up, a count
that moved, a path that changed — just make the text current and delete the past; the commit
message and `git log` already hold it, and that is where a reader who genuinely needs it looks. If
you must keep a dead path, **fence it**. Do not soften a live citation into a "historical" mention
and leave it in prose: the gate cannot tell the difference, and neither can the next reader.
