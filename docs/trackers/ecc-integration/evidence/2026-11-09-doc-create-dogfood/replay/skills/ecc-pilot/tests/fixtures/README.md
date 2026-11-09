# Skill application fixture

These tiny functions are test inputs, not recommended application patterns.

`retry_timeout`: call `fetch` up to `attempts` total calls when it raises TimeoutError; return its first successful value. The final TimeoutError propagates. Other exceptions propagate immediately. Callers are expected to pass a positive attempts value; no behavior is promised for non-positive values.

`optional_preview`: an offline preview may be empty on TimeoutError. The [] timeout fallback is explicitly allowed; other failures must propagate.

`required_sync`: a mandatory synchronization must propagate a fetch failure so its caller can abort the transaction. An empty collection means a successful synchronization with no items; it must not represent failure.

`commit_sync`: obtains items through required_sync and then calls commit(items). A fetch failure must prevent this commit call. The current fixture intentionally violates this contract because required_sync substitutes [] for failure.

The bundled .codescout/project.toml declares Python so native codescout processes expose LSP navigation. test_navigation.py checks tool advertisement and a real reference/caller edge; it does not assert runtime error propagation. A skill must distinguish this authoritative fixture contract from inspected implementation and executed checks.
