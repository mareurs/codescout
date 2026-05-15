# edit_eval — Round 4

## Tally

| Verdict | Count |
|---|---:|
| CLEAN_ERROR | 2 |
| CORRECT | 7 |
| PANIC | 1 |
| SILENT_WRONG | 4 |

## Cases

| ID | Verdict | Evidence |
|---|---|---|
| R-01 | CORRECT | triplet matched |
| R-02 | CORRECT | triplet matched |
| R-03 | CORRECT | triplet matched |
| R-04 | CORRECT | triplet matched |
| R-05 | CORRECT | triplet matched |
| R-06 | CORRECT | triplet matched |
| R-07 | CORRECT | triplet matched |
| R-08 | SILENT_WRONG | disk: needle "/// Doc that lives immediately above" appears 0× (want 1×) |
| I-01 | SILENT_WRONG | disk: needle "pub fn method_zero" appears 2× (want 1×) |
| I-02 | SILENT_WRONG | disk: needle "method_zz" appears 7× (want 1×) |
| I-03 | SILENT_WRONG | disk: needle "this is not rust" appears 7× (want 1×) |
| M-01 | CLEAN_ERROR | return: want Ok, got RecoverableError |
| M-02 | CLEAN_ERROR | return: want Ok, got RecoverableError |
| N-01 | PANIC | fatal: LSP error (code -32602): No references found at position |
