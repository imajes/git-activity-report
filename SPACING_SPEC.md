# Spacing Specification (Human-Readable Code Layout)

Status: Draft v0.1 (normative where labeled MUST/SHOULD)

Purpose

- Define precise, machine-applicable rules for inserting blank lines (vertical spacing) in code to maximize human readability without fighting language formatters.
- Rules apply during code creation by agents (preferred), and can be revalidated post‑hoc.
- Scope: Language‑agnostic core with language notes for Rust, Python, and Just.

Outcomes

- Consistent “phase” separation for complex functions (fetch → build → accumulate → enrich → write → return).
- Tight grouping for “mini‑phases” (pairs of closely related statements) to avoid visual fragmentation.
- Predictable I/O boundaries to highlight side effects.
- Minimal diff churn: avoid gratuitous multiple blanks; prefer one.

Notation

- MUST, MUST NOT, SHOULD, SHOULD NOT have RFC‑2119 meaning.
- “Blank” means exactly one empty line. “Multiple blanks” means two or more consecutive empty lines.

Key Concepts

1. Phase and Mini‑Phase

- Phase: a conceptual unit of work that should be visually separated by one blank line from neighboring phases.
- Mini‑phase: a tightly coupled pair (or small cluster) of lines that should remain adjacent (no blank between them) within a phase.

2. Tokens (Line Classifier)

Classify each source line (or cohesive multi‑line construct) as zero or more of:

- import: language import/use/require/include lines (e.g., `use crate::...;`, `from x import y`).
- comment: a standalone comment line not attached to a declaration (not a doc comment).
- blank: an empty line.
- decl: a declaration/binding/assignment (e.g., `let x = ...;`, `x := ...`, `x = ...`).
- literal-start / literal-end: the start/end of a multi‑line literal (object/struct/map/array literal). Single‑line literals do not open/close.
- ctrl-start / ctrl-end: start/end of control constructs (if/else/else if/if let/while/while let/for/match/switch/try/catch), excluding same‑line `} else {` siblings.
- io-call: an I/O or subprocess call (e.g., file read/write, network call, `run_git`, `Command::new("git")`, `std::fs::create_dir_all`, `println!`, logging). Pure formatting or in‑memory operations are not io-call.
- push-call: mutating collection ops (e.g., `.push(...)`, `.insert(...)`).
- derive: pure computation producing derived data (e.g., splitting strings, computing `body_lines`, formatting filenames).
- ret: `return`/`Ok(...)`/`throw`/`raise` finalization.

Language notes for tokenization:

- Rust: attributes (e.g., `#[derive]`, `#[cfg]`) are part of the declaration they annotate and MUST remain adjacent (no blank inserted between attributes and the item).
- Python: first triple‑quoted string after `def`/`class` is a docstring and MUST remain adjacent to the header.
- Justfile: treat recipe headers as decl; recipe bodies are blocks; comments adhere to section‑marker rules.

3. Phase Detection (Heuristics)

Phases are detected by intent (content), not only syntax. Typical function phases include:

- fetch: reading external/project state (git/show/list) or prepare input.
- files-build: assembling per‑file structures from raw outputs.
- accumulate: summarizing numeric totals (add/del counters) or cardinalities.
- author-stats: updating author maps/counters.
- timestamps: computing labels and time objects for later serialization.
- patch-ref: building patch reference metadata structures.
- commit-build: assembling main output object/struct for a unit of work.
- patch-embed: (optional) capturing and possibly clipping patch text.
- save-patch: (optional) I/O emitting `.patch` files; updating references.
- enrich: (optional) best‑effort network/API enrichment; updating metadata.
- path-format: computing stable path/filename strings for output.
- derive-fields: deriving pure fields like `body_lines` from existing data.
- write: performing file writes (JSON, shards, manifests).
- push: appending to in‑memory manifests/collections.
- finalize: returning values / `Ok(...)`.

Section Headers and Grouping

- Large files SHOULD use section headers (comments such as `// --- Helpers ---`) to delineate major groups: parameters (public API), internal context, helpers, orchestration, tests.
- MUST have exactly one blank line before and after a section header; within the section, apply the usual spacing rules.

Function Group Separation

- MUST have one blank line between top‑level function declarations. If grouped by role (e.g., build*\*/process\*\*/save\_*/enrich*\*/format*\*), the section header defines the boundary.

The exact sequence depends on function design. When in doubt, decide phases by “what a human would want to scan separately”.

Global Invariants

G1. One‑Blank Rule: Insert exactly one blank line between phases. Do NOT insert multiple consecutive blank lines. Collapse 2+ into 1.

G2. No‑Blank at Block Edges: Do NOT insert a blank immediately after an opening brace/indent or immediately before a closing brace/dedent.

G3. Tight Else Chains: Do NOT insert blanks between `} else {`, `else if (...) {`, or analogous chained constructs. They MUST remain adjacent.

G4. Doc/Attr Adjacency: Do NOT insert blanks between doc comments/attribute stacks and their declarations.

G5. Embedded Literals Are Units: A call that embeds a multi‑line literal (e.g., `vec.push(Foo { ... })`) is one visual unit; do NOT insert a blank “inside” or around it unless crossing a phase boundary.

G6. Maximal Value, Minimal Diff: Favor readability improvements that don’t create noisy diffs. Do NOT add spurious blanks within micro‑patterns that already read clearly.

Rules (Normative)

R1. Declaration → Control Flow

- Insert a blank between a declaration (decl) and the next control‑flow (ctrl-start) statement when they start different phases.
- Exception (mini‑phase): If consecutive declarations and the immediate control form a single mini‑phase (e.g., `let fname` + `let shard_path` → both for filename formatting), they MAY stay tight. The blank occurs before the next phase boundary.

R2. Builder Literal → Use

- If a multi‑line literal is bound to a name and the following statement uses it (push/insert/call), insert a blank before the use.
- If a multi‑line literal is embedded directly in a call (e.g., `push(Foo { ... })`), treat it as one visual unit; do NOT insert a blank “between” the call and its arguments.

R3. Accumulator Siblings (Mini‑Phase)

- Keep sibling update guards tight when they update the same counters (e.g., `adds` then `dels`).
- Insert a blank before the next different concern (e.g., set insert, or moving to author stats).

R4. Author Stats (Mini‑Phase)

- Keep computation of author key and the corresponding map update adjacent (tight). Insert a blank before/after when switching phases.

R5. Timestamps (Mini‑Phase)

- Keep timezone label computation and the timestamps literal adjacent (tight). Insert a blank before/after when switching phases.

R6. I/O Boundaries

- Surround significant I/O (mkdir, file write, subprocess, network call, logging) with blanks when adjacent to compute or different side‑effects.
- In compound I/O phases (e.g., save‑patch), separate micro‑steps with blanks: mkdir → compute path → fetch patch → write → update references.

R7. Post‑Block New Phase

- After a multi‑line control block ends (ctrl-end), if the next line begins a new phase (not an `else/else if` chain), insert a blank.

R7.1 Early‑Return Helpers (Specialization)

- After an early‑return subtree (e.g., a small function that returns early based on conditions), if the next line begins a new phase, a blank MUST separate them. This codifies post‑block separation for early exit helpers.

R8. Derived Fields, Write, Push, Return

- Insert a blank before pure derived field computation if it starts a new phase.
- Insert a blank before writes (write) and pushes (push-call) when they follow different computations.
- Insert a blank before finalization (ret) when preceded by non‑trivial blocks (multi‑line literal, I/O sequences, or multi‑line control).

R9. Comments as Section Markers

- Insert a blank before a standalone comment that introduces a section (unless the comment is the first line in a block). Insert a blank after it if the next line is code (not another comment).

R10. Imports (Repo Preference)

- For this repository’s Rust code, keep `use` lists compact as a single group; insert one blank after the imports block before the next top‑level item. Do NOT insert blanks to separate std/third‑party/local here (unless a file explicitly opts out).

R11. Maximum Single Blank

- Never exceed one blank line between any two non‑blank lines.

Priority of Rules (Conflict Resolution)

P1. Syntax Adjacency > All: G2/G3/G4 take precedence. Never violate language block boundaries, else‑chains, or doc/attr adjacency.
P2. Phase Separation > Heuristics: When in doubt between R1–R9, if a line begins a new phase, insert the blank.
P3. Mini‑Phases > Generic Separation: Tight pairs (R3–R5) remain adjacent even if a generic rule suggests inserting a blank between them.
P4. Embedded Units > Generic Separation: R5 prevails — do not split embedded literal calls.

Language‑Specific Notes

Rust

- Attributes MUST stay glued to the item they annotate (G4).
- `} else {` and `else if` chains MUST remain compact (G3).
- Match arms: keep each arm’s body compact. If an arm body is multi‑line and followed by another multi‑line arm, a blank MAY be inserted between arms for readability; however, prefer consistency with rustfmt (default: no extra blank between arms).
- Macros: treat as opaque unless they clearly perform I/O — then apply R6 around them.

Orchestrator Step Separation

- In orchestrators (e.g., run_simple, run_full), each pipeline step MUST be separated by one blank: compute inputs, mkdir, process ranges, optional unmerged processing, build manifest, write to disk, return.

Python

- Docstrings MUST be adjacent to the `def`/`class` header (G4).
- Blocks defined by indentation: G2 applies to indents/dedents.
- `try/except/else/finally` siblings: KEEP adjacent (G3); insert a blank only when leaving the `try` family (R7) to a new phase.

Justfile

- Treat each recipe definition as a top‑level item; insert a blank between recipes.
- Within a recipe, treat groups of related commands as phases and separate with blanks per R6/R8; do not split logical command pipelines.

Examples (Rust)

E1. Builder literal → use (bound literal)

BAD:

```rust
let fe = FileEntry { /* … */ };
files.push(fe);
```

GOOD:

```rust
let fe = FileEntry { /* … */ };

files.push(fe);
```

E2. Embedded literal in call (single visual unit)

GOOD:

```rust
files.push(FileEntry { /* … */ });
```

E3. Accumulators + set (mini‑phase + phase change)

GOOD:

```rust
if let Some(a) = f.additions { adds += a; }
if let Some(d) = f.deletions { dels += d; }

files_touched.insert(f.file.clone());
```

E4. Post‑block new phase (after PR enrichment)

GOOD:

```rust
if p.github_prs {
  // … mutate patch_ref/commit …
}

let fname = format_shard_name(...);
```

E5. Save‑patches cadence (I/O boundaries)

GOOD:

```rust
std::fs::create_dir_all(dir)?;

let path = format!("{}/{}.patch", dir, short_sha);

let txt = run_git(repo, show_args)?;

std::fs::write(&path, txt)?;

patch_ref.local_patch_file = Some(path);
```

Online Emission Algorithm (for Agents)

Given a stream of lines being written, keep a sliding window of the last non‑blank and current prospective line. For each prospective line:

1. Classify the line into token kinds (import/comment/decl/literal‑start/ctrl‑start/io‑call/push/derive/ret).
2. Determine if the line begins a new phase relative to the previous non‑blank line.
   - If phase changes (per Phase Detection), and G2/G3/G4 are not violated, EMIT one blank line before the current line unless the previous line is already blank.
3. Apply Mini‑Phase exceptions:
   - If the previous and current line form a mini‑phase (author key + increment, adds+dels, tz label + timestamps, filename + shard path), DO NOT insert a blank.
4. Apply Embedded Literal rule:
   - If the current line is a call embedding a literal (or is the closing line of such call), DO NOT insert extra blanks around the call itself. Treat the call as one token for spacing purposes.
5. Apply I/O boundaries:
   - If current is io‑call and previous is compute (or vice versa), and not already separated by a blank, EMIT one blank.
6. Apply Return finalization:
   - If current is ret and the previous phase is non‑trivial, EMIT one blank.
7. Enforce Global Invariants:
   - Never create more than one blank between lines (G1). Never place blanks at block edges (G2). Never split else‑chains (G3). Never separate doc/attrs from items (G4).

Strict Mode (Opt‑In, Normative When Enabled)

When strict mode is requested (by author intent, CI configuration, or file‑level policy), the following rules become mandatory in addition to R1–R11 and the priorities P1–P4. Strict mode purposefully trades compactness for maximum scanability.

S1. Decl → Ctrl Always

- MUST insert a blank between ANY declaration and the immediately following control‑flow statement (if/if let/for/while/while let/match), even when they could be considered a mini‑phase. Only P1 (syntax adjacency) can override this.

S2. Ctrl → Ctrl Separation

- MUST insert a blank between successive control‑flow blocks that are not `else` siblings, including after a `}` that closes a multi‑line block when the next line begins a new control block.

S3. Write/Push Separation

- MUST insert a blank before any write/push/logging call that follows computation or a literal build on the immediately preceding line (not only after literal closing braces). This includes `std::fs::write`, `println!`, and collection `.push`/`.insert` calls when they are not part of an embedded literal call.

S4. Finalization Separation

- MUST insert a blank before `Ok(...)`/`return`/`throw`/`raise` unless the previous non‑blank line is itself a single‑line return. This highlights finalization in all cases.

S5. Post‑Block New Phase (Amplified)

- MUST insert a blank line after closing any multi‑line block if the next line starts a different phase, even if the block ended with a single statement and the next phase is compact.

S6. I/O Granularity

- SHOULD separate each I/O micro‑step in an I/O phase with blanks, even when multiple calls are short and adjacent (e.g., mkdir → compute path → fetch → write → updates). This is already recommended by R6; strict mode treats it as normative.

Detection Aids (Strict)

The following patterns are suitable for CI audit or pre‑commit checks (ripgrep, multiline with `-U`):

- S1 (decl→ctrl): `^\s*let\s+[^;]+;\n[\t ]*(if|if\s+let|for|while|while\s+let|match)\b`
- S2 (post‑block ctrl): `^\s*}\s*\n[\t ]*(if|if\s+let|for|while|while\s+let|match)\b`
- S3 (write/push directly after code): `^[^\n{}].+\n[\t ]*(push|insert|write|println!)\b`
- S4 (finalization adjacency): `^[^\n{}].+\n[\t ]*(Ok\(|return\b)`

Note: these are heuristics; the authoritative behavior is the Rules + Priorities above.

Audit Patterns (Ripgrep‑Friendly)

- decl → ctrl without blank (Rust): `^\s*let\s+[^;]+;\R\s*(if|if\s+let|for|while|while\s+let|match)\b`
- run_git(...) followed by non‑blank (Rust): `run_git\([^\)]*\);\R\s*[^\s]`
- literal close brace directly followed by push/write (Rust): `}\s*;\R\s*(push|insert|write|println!)\b`

Compliance Checklist

- [ ] Imports compact; blank after import block.
- [ ] Phase boundaries separated with one blank.
- [ ] Mini‑phases (author/tz/adds+dels/fname+path) kept tight.
- [ ] Builder literal bound → blank before use; embedded literal → single unit.
- [ ] I/O boundaries blank‑separated from compute/sibling I/O steps.
- [ ] Post‑block new phase gets a blank.
- [ ] Derived fields, writes, pushes, and returns have blanks as specified.
- [ ] No block‑edge blanks; no split else‑chains; attrs/doc glued.
- [ ] No multiple blanks; no gratuitous blanks.

Rationale

- Humans read in conceptual chunks, not tokens; the spec enforces consistent chunking.
- Tight pairs reduce vertical bloat; blanks highlight shifts in intent.
- Separating I/O clarifies side‑effects and makes diffs easier to review.
- The rules sit above language formatters (rustfmt/black); they do not fight them but complement them by adding human‑centric spacing.
