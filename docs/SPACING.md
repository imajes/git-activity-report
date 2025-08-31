# SPACING — Human‑Friendly Structural Spacing Guide

Make code easier to scan by adding intentional blank lines between conceptual phases. These rules are language‑agnostic and work for brace‑ and indent‑based languages. When in doubt, prefer a single blank line to separate ideas and keep related lines together.

Principles

- One blank line between different concerns; avoid multiple consecutive blanks (collapse 2+ to 1).
- Never separate syntax that must be adjacent (e.g., `} else {`, chained `else if`, doc comments from their targets).
- Don’t insert blanks inside strings, docstrings, or macro invocations.
- Keep formatting symmetric across mirrored branches.

---

## 1) Imports & Modules

Group imports by origin (standard, third‑party, local). Separate groups with one blank line. Add one blank line after the last import group.

BAD (mixed, no grouping):

```rust
use serde::Serialize; use crate::util; use std::fs;
fn main() {}
```

GOOD (grouped, spaced):

```rust
use std::fs;

use serde::Serialize;

use crate::util;

fn main() {}
```

---

## 2) Top‑Level Declarations

Use one blank line between types, functions, and constants. Keep doc comments directly attached with no blank line in between.

BAD:

```go
// User holds basic info

type User struct { Name string }
func NewUser() User { return User{"x"} }
```

GOOD:

```go
// User holds basic info
type User struct { Name string }

func NewUser() User { return User{"x"} }
```

---

## 3) Inside Functions: Phase Separation

Add a blank line between declaration/setup, fetch/compute, transform/build, and output/return stages.

BAD:

```python
def generate():
    items = []
    data = read()
    for d in data:
        m = build(d)
        items.append(m)
    return items
```

GOOD:

```python
def generate():
    items = []

    data = read()

    for d in data:
        m = build(d)
        items.append(m)

    return items
```

### 3a) Declaration → Control Flow

Always insert a blank line between variable declarations/setup and the next control‑flow statement (`if/else`, `while`, `for`, `match`, `switch`). This separates “define/setup” from “decide/iterate”.

BAD:

```rust
let u = url.trim();
if let Some(c) = re.captures(u) {
    return Some((c.get(1).unwrap().as_str().to_string(),
                 c.get(2).unwrap().as_str().to_string()));
}
```

GOOD:

```rust
let u = url.trim();

if let Some(c) = re.captures(u) {
    return Some((c.get(1).unwrap().as_str().to_string(),
                 c.get(2).unwrap().as_str().to_string()));
}
```

---

## 4) Control‑Flow Boundaries

Before a multi‑line `if/for/while/match` block, insert a blank line when it follows non‑control code. After closing a multi‑line block, insert a blank if the next line shifts concern. Never split `} else {` or `else if`.

BAD:

```javascript
const xs = load();
if (xs.length) {
  process(xs);
}
log(xs.length);
```

GOOD:

```javascript
const xs = load();

if (xs.length) {
  process(xs);
}

log(xs.length);
```

Preserve adjacency:

```rust
if ok {
    do_a();
} else if maybe {
    do_b();
} else {
    do_c();
}
```

---

## 5) Long Literals / Builders

Surround multi‑line literals (structs, maps, objects) with a blank line when followed or preceded by a different concern (e.g., building → pushing/using). Threshold: 3+ lines inside braces.

BAD:

```rust
let item = Item {
    id: 1,
    name: "n".into(),
    tags: vec!["a".into(), "b".into()],
};
list.push(item);
```

GOOD:

```rust
let item = Item {
    id: 1,
    name: "n".into(),
    tags: vec!["a".into(), "b".into()],
};

list.push(item);
```

---

## 6) Accumulators & Push/Insert

When constructing a value across multiple lines and then pushing/inserting it, add a blank line before the push/insert. Keep accumulation loops distinct from subsequent concerns.

BAD:

```python
for row in rows:
    rec = {"id": row.id, "name": row.name}
    out.append(rec)
total = len(out)
```

GOOD:

```python
for row in rows:
    rec = {"id": row.id, "name": row.name}

    out.append(rec)

total = len(out)
```

---

## 7) I/O and External Effects

Separate pure computation from I/O (file/network/subprocess) with blank lines on both sides when adjacent to other concerns.

BAD:

```rust
let json = serde_json::to_string(&v)?; std::fs::write(path, json)?; let n = v.len();
```

GOOD:

```rust
let json = serde_json::to_string(&v)?;

std::fs::write(path, json)?;

let n = v.len();
```

---

## 8) Returns / Finalization

Add a blank line before `return`/`Ok(..)`/`throw` when preceded by a non‑trivial block or long literal.

BAD:

```typescript
const res = build();
return res;
```

GOOD:

```typescript
const res = build();

return res;
```

---

## 9) Comments as Section Markers

Keep a blank line before a standalone comment that introduces a section (unless it starts a block) and a blank after if the next line is code.

BAD:

```python
# accumulate totals
for x in xs:
    total += x
```

GOOD:

```python

# accumulate totals
for x in xs:
    total += x

```

---

## 10) Blocks & Braces / Indentation

No blank line immediately after an opening brace/indent or immediately before a closing brace/dedent. Preserve adjacency for `else` chains.

BAD:

```rust
fn f() {

    do_it();

}
```

GOOD:

```rust
fn f() {
    do_it();
}
```

---

## 11) Thresholds & Symmetry

Apply spacing when moving between “long” chunks (≥3 lines) or different concerns. Keep branch formatting symmetric so both sides of a conditional look balanced.

BAD:

```javascript
if (a) {
  one();
  two();
  three();
} else {
  four();

  five();
}
```

GOOD:

```javascript
if (a) {
  one();
  two();
  three();
} else {
  four();
  five();
}
```

---

## 12) Language Awareness

- Brace languages (C/JS/TS/Java/Rust): detect blocks via `{}`; don’t insert blanks between `} else {}` or `case` labels and their blocks.
- Indent languages (Python): use indentation changes to infer block starts/ends; never insert blanks between a docstring and the function/class header.

BAD (Python docstring split):

```python
def f():

    """Doc."""
    pass
```

GOOD:

```python
def f():
    """Doc."""
    pass
```

---

## 13) Safety Rails

- Never insert inside strings/docstrings or format/macro bodies.
- Don’t create more than one consecutive blank line; collapse to one.
- Don’t touch lines that tooling depends on being adjacent (license headers, region pragmas, `// prettier-ignore`, etc.).

---

## 14) Applying These Heuristics (Automation)

Minimal approach to an auto‑spacer:

1. Tokenize lines and classify each as: import, decl, control‑start, control‑end, literal‑start, literal‑end, io, return, comment, blank, other.
2. Track nesting via braces or indentation to avoid inserting at block edges.
3. When the current line’s class differs from the previous “phase” (or either side is a long chunk), ensure exactly one blank line separates them—except where adjacency is required (e.g., `} else {`).
4. Collapse 2+ consecutive blanks to exactly one.
5. Maintain a deny‑list of contexts where insertion is forbidden (strings/docstrings, macro invocations, `/*…*/` blocks, etc.).

Dry‑run mode should print suggested insertions with line numbers before applying changes.

---

FAQ

- Q: How many blank lines are okay?
  A: At most one between concepts; zero inside tight logical pairs (e.g., `} else {`).

- Q: Do these conflict with language formatters?
  A: These rules complement most formatters. If a formatter reflows aggressively (e.g., gofmt), run spacing after formatter or restrict to comment‑based sectioning.

---

Future Tool Integration (Codex Function Call) — Not For Current Use

Note for tool authors only. Agents should ignore this for now and apply spacing manually per the rules above. This sketches a Rust‑focused helper that could be invoked as part of a Codex workflow.

- Tool name: `spacing.apply`
- Purpose: Suggest or apply spacing changes to Rust code (and optionally other text) using the heuristics in this document.
- Inputs:
  - `paths: string[]` — files or directories to process (globs allowed).
  - `mode: "suggest" | "apply"` — dry‑run suggestions vs. rewrite files.
  - `language_hint?: "rust" | "python" | "text"` — bias classifier; default auto.
  - `deny?: string[]` — substrings/regex to skip (e.g., `vendor/`, `target/`).
  - `max_changes_per_file?: number` — guardrail against large diffs.
  - `context_lines?: number` — for suggestions, lines of context.
- Behavior:
  - Classify lines (decl, control‑start, literal‑start, io, return, comment, blank, other).
  - Track nesting via braces/indentation; never split `} else {}` or doc comments and their targets.
  - Insert exactly one blank line at phase boundaries (declaration→control flow; build→use/push; compute↔I/O; before finalization), respecting “no blank after open / before close”.
  - Collapse multi‑blank runs to one; skip strings/docstrings/macro bodies.
- Output (suggest mode):
  - Unified diff per file or machine‑readable JSON of suggestions: `{ file, line, action: "insert_blank" | "collapse_blanks", reason }`.
- Agent usage (instruction example):
  - “If `spacing.apply` is available, run in `suggest` mode on `src/**/*.rs` with `language_hint=rust`. Show the diff. If acceptable, run in `apply` mode. Otherwise, perform manual edits guided by suggestions.”

Reminder: This is a future integration sketch only. Do not invoke any auto‑spacer by default; agents should apply spacing judgment while editing.
