## Codex CLI work log (high-level)

This PR was iterated on using Codex CLI. I don't have a first-class "chat export" artifact from the toolchain, but this file captures the key prompts/goals and the concrete changes that were made in response to PR review + fuzz regressions.

### Goal

Implement robust graph merge handling for:

- Deterministic DAG traversal / merge ordering
- Delete correctness under complex ancestry + concurrent inserts
- Partial-parent scenarios (per `fugue-simple` ground truth)
- Fuzzer-reported divergences vs `fugue-simple`

### What was fixed / added (summary)

- Deterministic topo walk + order-independent `Graph::add_node` construction.
- Delete-containing ops are OT-transformed against previously applied concurrent ops (non-ancestors) to avoid:
  - concurrent delete+insert losing the insert
  - concurrent deletes deleting "too much"
  - deletes being ignored after complex merge ancestry
- Insert-only ops are OT-transformed where needed to keep delete targets stable under concurrent inserts.
- Added regression tests derived from `fugue-simple` (including Figure 7 `"AXYBC"` ordering).
- Added regression tests for fuzz-found bugs, including:
  - bug1 (`bug1_insert_order.rs`) which should converge to `"FE"`
  - hierarchical insert ordering (`"UROIH"`) from fuzz seed `0xe91cd15e00000020`

### How to validate locally

- Unit tests: `cargo test`
- Fuzz harness / ground truth: see upstream branch `fuzz-testing-bugs` (contains `mako-fuzz` + `fugue-simple` and repro binaries).

