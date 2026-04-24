# Language Patterns

Per-language anti-patterns and correct patterns for this project's languages.
Each section lists the top 5 mistakes LLMs make and the top 5 idiomatic patterns.

### Rust

**Anti-patterns (Don't → Do):**
1. Gratuitous `.clone()` to silence borrow checker → borrow: `&str` over `&String`, `&[T]` over `&Vec<T>`
2. `.unwrap()` everywhere → `?` with `.context()` from anyhow, `.expect("invariant: ...")` only for proven invariants
3. `Rc<RefCell<T>>` / interior mutability overuse → restructure data flow and ownership
4. `String` params where `&str` suffices → `fn greet(name: &str)`, use `Cow<'_, str>` when ownership is conditional
5. Catch-all `_ => {}` in match → handle all variants explicitly, let compiler check exhaustiveness

**Correct patterns:**
1. `thiserror` for library errors, `anyhow` for application errors — propagate with `?`
2. Iterator chains over explicit loops — `.iter().map(f).collect()`, avoid unnecessary `.collect()`
3. `Vec::with_capacity()` when size is known
4. Derive common traits: `#[derive(Debug, Clone, PartialEq)]`, `#[derive(Default)]` when sensible
5. `if let`/`while let` for single-pattern matching instead of full match
