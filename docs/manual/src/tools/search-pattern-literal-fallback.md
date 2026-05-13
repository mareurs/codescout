# grep: Literal Fallback

When `grep` receives a pattern that fails regex compilation,
it now checks whether the input looks like intended regex syntax before
returning an error.

## How It Works

If the pattern fails to compile **and** does not look like intentional
regex (no alternation `|`, wildcards `.*`/`.+`, anchors `^`/`$`,
escape sequences `\w`/`\d`/`\b`, or balanced grouping `(...)`), the
tool falls back to a **literal text search** by escaping all
metacharacters automatically.

```
// User types this — unescaped `(` makes it invalid regex,
// but there is no closing `)` so it looks like plain text:
pattern: "if (x > 0"

// codescout escapes it to: \Qif \(x > 0\E  (effectively)
// and searches for the literal string
```

The response includes two extra fields to signal the fallback:

```json
{
  "matches": [...],
  "total": 2,
  "mode": "literal_fallback",
  "reason": "pattern was not valid regex — searched as literal text"
}
```

The compact format output is prefixed with `[literal fallback]` so the
mode is visible at a glance.

## When the Error Is Preserved

If the broken pattern contains regex-like syntax — alternation (`foo|bar`),
quantified wildcards (`fn.*call`), escape sequences (`\w+`), etc. — the
original `RecoverableError` is returned. This avoids silently
misinterpreting a malformed regex as a literal string.

```
// "(foo|bar" — has alternation, so is_regex_like = true
// → RecoverableError: "invalid regex: unclosed group"
pattern: "(foo|bar"
```

## Compact Output

```
[literal fallback] 3 matches
src/lib.rs:42  if (x > 0) {
src/lib.rs:87  if (x > 0) {
src/lib.rs:103 if (x > 0) {
```
