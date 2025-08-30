# Code Layout Specification (Language-Agnostic)

Status: Draft v0.1 (normative where labeled MUST/SHOULD)

Intent

- Define structural layout rules (beyond whitespace) to maximize human readability, scanability, and diff quality.
- Complement SPACING_SPEC.md (vertical spacing) with guidance on how to structure statements, builders, and transformations during code creation.

Principles

P‑L1 Readability first: prefer simple, explicit steps over compact, dense expressions.
P‑L2 Phase before action: build values in a “create” phase, then use them in a separate “act” phase.
P‑L3 Stable grouping: cluster related fields/steps; keep a stable order across files.
P‑L4 Diffability: prefer layouts that produce focused, minimal diffs (small, isolated changes).

Reference Pattern (Case Study)

From a Rust rewrite extracting PR fields (generalized for any language):

- BAD (dense, inline): build a complex object inside a push/return with many nested lookups and map/and_then chains.
- GOOD (clear, staged): compute nested values (`user`, `head`, `base`) as locals with clear names; then assemble a `pr` object; then push/use it. Group related fields and separate groups with blanks per SPACING_SPEC.

Normative Rules

R‑L1 Extract‑Before‑Build (Nested Access)

- MUST NOT inline long chains of nested lookups (e.g., JSON path extractions) directly inside object/struct literals or call arguments when the chain has 3+ operations or spans multiple lines.
- MUST extract such chains into well‑named local variables before constructing the larger literal or making the call.
- SHOULD limit method/property chain lines to one deref/hop per line when multi‑line (e.g., `.get("user")` newline `.and_then(...)` newline …).

R‑L2 Name for Semantics, Not Mechanics

- MUST name extracted locals to reflect domain semantics (e.g., `pr_user`, `pr_head`, `pr_base`), not mechanics (e.g., `tmp1`, `value`).
- SHOULD keep names short but meaningful; prefer prefixes that match the enclosing concept when multiple related locals exist (e.g., `pr_*`).

R‑L2.1 No Single‑Letter Idents (Scoped Exception)

- MUST NOT use single‑letter variable names (e.g., `p`, `x`) for values that persist beyond a tiny scope.
- Exception: MAY use single‑letter indices/counters in tight loops (e.g., `i`, `j`) or for values whose entire lifecycle spans ≤ 3 consecutive lines within a small block.
- SHOULD otherwise use semantic names (`params`, `branch`, `context`, `path`, `author_key`).

R‑L3 Builder Objects: Build Then Act

- MUST construct complex objects/structs into a local variable when:
  - They span more than ~6 lines, OR
  - They include nested lookups/extractions, OR
  - Multiple fields are optional/derived.
- MUST perform the action (push/insert/return/call) in a subsequent statement (build → use), separated by a blank per SPACING_SPEC.
- MAY embed a small single‑line literal in a call when trivially readable; otherwise prefer the two‑step pattern.

R‑L4 Field Grouping and Order (Inside Literals)

- SHOULD group fields inside a literal by concern and keep those groups in a stable order across the codebase. Suggested grouping order:
  1) Identity and relationships (e.g., nested `user`, `head`, `base` objects)
  2) Core scalar fields (e.g., `number`, `title`, `state`)
  3) Temporal fields (e.g., `created_at`, `merged_at`)
  4) Location/links/paths (e.g., `html_url`, `diff_url`, `patch_url`)
  5) Optional/derived fields (e.g., `body_lines`)
- SHOULD separate groups with a blank line where the language/formatter allows (see SPACING_SPEC R1–R11 for blank line rules). If the language disallows intra‑literal blanks, preserve grouping by ordering alone.

R‑L5 One Concern per Statement

- SHOULD avoid mixing multiple concerns in a single statement. Examples:
  - Do not compute a path, fetch data, and write a file in one expression; split into one step per concern.
  - Do not mutate multiple distinct objects in the same dense block without visual separation.

R‑L6 Chain Length and Rebinding

- SHOULD keep method/property chains to ≤3 hops when inline. If longer or if any hop has non‑trivial logic, rebind to a local.
- SHOULD prefer reusing extracted locals across multiple fields rather than re‑computing the same chain.

R‑L7 Act After Validation/Optionality

- SHOULD compute and validate optional pieces (e.g., `user`, `head`, `base`) before building the main object; keep the build phase free of control flow where possible.
- MUST NOT hide fallible/optional logic inside a large literal when it impairs readability; extract and name it.

R‑L8 Return/Push at the End of a Phase

- SHOULD position the final action (push/insert/return) at the end of a phase, not in the middle of computations.
- MUST separate final actions from preceding non‑trivial builds with a blank per SPACING_SPEC (see R8 and S4).

R‑L9 Consistency with Formatter

- MUST respect the project’s formatter (e.g., rustfmt, black). Layout rules SHOULD be applied in ways that do not fight the formatter; use trailing commas and multi‑line builders to allow clean wrapping.

R‑L10 Cross‑File Consistency

R‑L11 Layered Function Roles and Boundaries

- MUST separate functions by role and keep responsibilities tight:
  - build_*: pure constructors of domain objects; no I/O; return the value.
  - process_*: orchestrate phases (build → enrich/derive → I/O/push → finalize).
  - save_*/write_*: perform I/O in narrow helpers; return Result; mutate only their target.
  - enrich_*: integrate with external systems; narrow, testable effects.
  - format_*: pure formatting helpers.
- SHOULD name functions according to these roles; avoid multi‑purpose functions.

R‑L12 Context Object for Shared Environment

- SHOULD bundle environment/settings (repo path, tz label, flags) into a light context object passed by borrow to helpers.
- MUST keep context read‑only within helpers; mutability belongs to orchestrators and I/O helpers.

R‑L13 Orchestrator Pipelines (Phases)

- MUST follow the phased pipeline in orchestrators:
  1) Build pure values
  2) Optional enrich and derive fields
  3) I/O (save/write) and push/manifest updates
  4) Final assembly and return
- MUST NOT interleave unrelated concerns; do not write while still building core values.

R‑L14 Ranges Return Bundles

- For range processing helpers, SHOULD return cohesive bundles (e.g., `(items, summary, authors)`), not scattered side effects.

R‑L15 Path Handling

- SHOULD prefer `Path`/`PathBuf` and `join` for building file paths; convert to strings only at boundaries where necessary (serialization, string fields).

- SHOULD apply the same grouping and order conventions for analogous structures across files (e.g., always group `user/head/base` before scalars in PR objects).

Layout Patterns (Good/Bad)

L‑E1 Extract‑Before‑Build (JSON‑ish lookups)

BAD (dense):
```
out.push(Pr {
  user: pr.get("user").and_then(|u| u.get("login")).and_then(|l| l.as_str()).map(|s| s.to_string()),
  // … many other fields …
});
```

GOOD (staged):
```
let pr_user = pr
  .get("user")
  .and_then(|u| u.get("login"))
  .and_then(|l| l.as_str())
  .map(|s| s.to_string());

let pr_obj = Pr {
  user: pr_user,
  // … other groups …
};

out.push(pr_obj);
```

L‑E2 Field Grouping (order + spacing)

GOOD:
```
let item = Pr {
  // Identity / relations
  user: pr_user,
  head: pr_head,
  base: pr_base,

  // Scalars
  number,
  title,
  state,

  // Temporal
  created_at,
  merged_at,

  // Links
  html_url,
  diff_url,
  patch_url,
};
```

L‑E3 Save‑Patch Cadence (create → use)

GOOD:
```
std::fs::create_dir_all(dir)?;

let path = format!("{}/{}.patch", dir, short_sha);

let txt = run_git(repo, show_args)?;

std::fs::write(&path, txt)?;

patch_ref.local_patch_file = Some(path);
```

Application During Creation (Agent Guidance)

1) Sketch phases before writing code; map steps to phases and mini‑phases (see SPACING_SPEC).
2) For each nested lookup, ask “will this hurt readability if inline?” If yes, extract and name it.
3) Build complex literals into locals, then act; do not inline large literals in pushes/returns.
4) Group fields logically and keep the grouping order consistent across files. Use blanks between groups where allowed.
5) Keep one concern per statement. Split compute vs I/O vs mutation into distinct steps.
6) Use trailing commas to let formatters wrap multi‑line builders predictably.
7) Place final actions (push/return) at the end of phases with a blank before them.

Audit & CI (Optional)

- Add checks that flag very long inline chains inside literals/calls; suggest extraction (threshold: ≥3 hops or ≥80 chars).
- Flag object literals longer than N lines embedded directly in pushes/returns; suggest binding to a local first.
- Flag mixed concerns (compute + I/O in one line) in known hotspots.

Interplay with SPACING_SPEC.md

- SPACING_SPEC controls vertical separation; LAYOUT_SPEC controls structural decomposition and ordering. Apply both: stage steps into logical phases (layout) and separate phases (spacing).
