# Config-manager Rust request-policy recovery — 2026-10-17

Status: **EXECUTED / component VALIDATED / repository BLOCKED; no merge**.

Resumed the existing ready, non-draft [PR #2](https://github.com/LLM-Dev-Ops/config-manager/pull/2), rather than creating another branch/PR. Original recovery head was `0d95469072bf867bed95baa529576958147ca914`; current main remained `02b463c3cbf088183f14b2ec951389f797863ac7`. Implementation commit: `8581b25710c11017386d8ab88c84c6fecec2025a`, tree `93bafba25f461e5d752d623f45cac6dafaaca2c1`. The following evidence-only update changes no production source. The PR was conflict-free; no conflicts were resolved, no force push occurred, and no merge was attempted with red gates.

## Implementation

- Enforce configured origins in the comprehensive Rust middleware, rejecting malformed or repeated Origin values rather than bypassing policy.
- Enforce actual bounded body bytes as well as Content-Length; chunked requests cannot bypass the policy. Restore accepted bytes for the existing handler. Unreadable bodies also fail closed with 413.
- Use Axum's original, once-decoded URI for endpoint policy matching; nested routers previously stripped `/api/v1`, and encoded namespace bytes could otherwise evade the blocklist.
- Validate once-decoded URI components and run security middleware on unmatched API paths too, covering encoded attacks and traversal paths.
- Supply the real server's ConnectInfo in in-process tests and encode invalid URI characters. **No expected status or existing assertion was weakened.** The health-only rate-limit test remains a compatibility test, not proof of API throttling.
- Commit a fixed Cargo.lock, stop ignoring it, and add independent Linux Rust policy, workspace-test, build, strict-Clippy and formatting CI gates. No failing gate is ignored or made optional by this workflow.

## Frozen measurements

The [contract](contract.json) was saved before implementation. The same final live-TCP fixture, repaired in-process tests, Cargo.lock and direct percent-encoding manifest edge were used in both comparison worktrees. Baseline production source remained the original recovery head; these baseline overlays only make requests executable and pin the identical dependency resolution. No baseline source repair was scored as a candidate advantage.

| Gate | Baseline | Candidate |
| --- | --- | --- |
| Original security integration tests | 2 passed / 13 failed | not used as the causal comparator |
| Identically repaired security requests | 10/15 | 15/15 |
| Frozen native HTTP holdout | 0/4 | 4/4 |
| Frozen native HTTP anchors | 6/7 | 7/7 |
| Native TCP cases including five extra regressions | see per-item results | 16/16 |
| Cloud Function tests | 31/31 | 31/31 |
| Production npm audit | 0 vulnerabilities | 0 vulnerabilities |
| Security-library unit tests | 51 passed / 5 failed | same five failures |
| Workspace formatting | exit 1 | exit 1 |

Final fixture SHA-256: `249c170ecb110bc6ad8869e0cbbccf569fe8581312873179c07c9e6858411949`.
Cargo.lock SHA-256: `946e6f891f4dc135312c5f113e0d64b0c882ae8f2e6a5a502a5301a000c03980` (390 packages).

Actual remaining workspace failures: `crypto::tests::test_key_validation`, `crypto::tests::test_password_complexity`, `input::tests::test_command_injection_detection`, `input::tests::test_xss_detection`, and `rate_limit::tests::test_per_ip_limiting`. These reproduced in a fresh baseline security-library run and in candidate hosted CI. Tests were neither skipped nor rewritten to pass.

Candidate commands: `cargo build --locked --workspace` exits 0; `cargo test --locked --workspace` exits 101; strict workspace Clippy exits 101 (first errors: unused `rand::RngCore` import and derivable enum Default in llm-config-crypto); `cargo fmt --all -- --check` exits 1. `cargo audit --file Cargo.lock` exits 0 with zero vulnerabilities and four unmaintained warnings. The custom `cargo run --locked --bin llm-security-scan -- --fail-on-high` exits 0 but still reports 0 critical, 0 high, 4 medium and 2645 low findings: it is **not** a clean whole-workspace lint result.

## Exact published-source hosted evidence

- [Rust Quality run 39034127654](https://github.com/LLM-Dev-Ops/config-manager/actions/runs/39034127654): policy **success** (16 native TCP + 15 integration tests), build **success**, workspace tests **failure**, Clippy **failure**, formatting **failure**. Lock checksum/diff checks passed.
- [Cloud Function Quality run 39034127553](https://github.com/LLM-Dev-Ops/config-manager/actions/runs/39034127553): **success**, Node 20.19.5, 31 tests and production audit.

These are implementation-commit results, not claims about a later evidence commit's CI. Main and PR head are refreshed after publication, and the evidence commit receives its own exact-checkout policy revalidation. Red repository gates forbid merging regardless of branch protection configuration.

## Tools and invocation evidence

Pinned Ruflo **3.56.3**, Transformers **4.3.1**, MetaHarness **0.4.17**, Darwin **0.10.3**, Flywheel **0.1.12**; refreshed upstream MetaHarness revision `e0dfd44da72b7adfb7c57fdd42d124be58ed7086`. Local isolated tool root: `/workspace/scratch/4f25900b71ff/cycle-tools-20261017`. Rust execution used the existing RuOS desktop, Node 20.20.2 / Rust 1.99.0. The idempotent research-lab bootstrap was executed and `pkg-config --modversion openssl` returned **3.0.13**. Hosted Rust pins 1.99.0 and installs pkg-config/libssl-dev explicitly.

Ruflo: `swarm init --topology mesh --max-agents 1`, swarm `swarm-1792240776983-jpv65q`; `hooks pre-task --task-id config-security-oct17b --description ...`. One coordinator, zero delegated workers. Coordination is not counted as implementation; the code and real tests above are implementation evidence.

MetaHarness: `node ../cycle-tools-20261017/node_modules/metaharness/dist/bin.js score . --json`, exit 0, fit **71 → 71**, hard constraints **6/6**. Its static compileConfidence 100 is not a successful Rust build/test result.

Darwin: `node ../cycle-tools-20261017/node_modules/@metaharness/darwin/dist/cli.js evolve . --generations 1 --children 2 --concurrency 1 --seed 2026101712 --sandbox real --selection pareto`. Baseline and reviewer variants `g1_v0`, `g1_v1` all scored **0.985**, with three real `npm test` exits 0 each. Both variants were rejected: **NO_IMPROVEMENT**. The Node substrate cannot discriminate harness surfaces or establish a Rust improvement; root promotion is initialization only.

Flywheel: `node docs/metaharness/2026-10-17/evaluate.mjs /home/ruv/work/config-baseline-oct17b /home/ruv/work/config-candidate-final-oct17b /home/ruv/.local/share/llm-dev-ops-tools /home/ruv/work/config-oct17b-flywheel-isolated.json /home/ruv/work/config-flywheel-targets-oct17b`, exit **0**. The evaluator actually executes cargo/native TCP tests for every score, checks identical test/lock inputs, and fails if any expected test did not execute. Source-hash-isolated target directories prevent baseline build artifacts contaminating the candidate. The first shared-target attempt failed compilation and was **rejected before scoring**; no measurement or promotion from that attempt is used.

The unmodified conjunctive gate promoted one component implementation (holdout **0 → 1**, anchor **6/7 → 1**). Gate fingerprint `c5942e7c9bf8f28fe7af47a6ec3887d11eeb3f8c8e8601cc6a7a54969ea0b981`; Ed25519 replay **8/8**, lineage `gen1(source) → gen0(root)`. Independent local readback also passed. Full [results, executed output, signed lineage and parent-byte inverse](flywheel-results.json) are committed. No LLM, runtime-cost, energy or latency improvement is claimed.

Ruflo post-task learning used the current research-lab `offline-exec.py` and `ruflo-local-model.mjs`, whose blob IDs matched research-lab main `83c4850c04d6243c1f700ec4a6a0e406a7d7198a`. The kernel socket/io_uring restrictions must load before execution. Pinned MiniLM revision `751bff37182d3f1213fa05d7196b954e230abad9` produced a real **384-dimensional** embedding with remote loading disabled. Post-task used the actual task ID and `--success false` because repository validation remains blocked. `learningUpdates.available`, `learningUpdates.outcomePersisted`, `feedback.recorded`, and `trajectory.recorded` were **all true**, with two learning/feedback updates; no active pheromone swarm is claimed. Exact command and outcome are in the ledger. No task data was sent to Hugging Face and no approval restriction was bypassed.

## Coverage, constraints and rollback

All **58** accessible, nonarchived repositories were enumerated (offsets 0/100 returned 58/0), and each main SHA/latest CI was refreshed. Org open PR discovery returned **100 + 17**. Only config-manager source was changed; shield, analytics-hub and incident-manager recovery PRs received read-only inspection. The other **57 named repositories**, their SHAs/CI and deferral reasons are in the [ledger](ledger.json). This distinguishes metadata coverage from deep source/build coverage and preserves bounded rotation after the prior memory-graph and auto-optimizer work.

Core-memory returned no playbook entries or anti-pattern warnings and marked results unscreened. Explicit user constraints governed decisions: Rust core retained, TypeScript/Cloud Function integration preserved, no service/persistence/database/IAM changes, no deployment, package publication or paid provisioning. Temporary test storage is fixture isolation, not a new service-owned database. Learning was recorded through authorized Ruflo/core-memory interfaces.

Rollback: before merge, close the PR or revert this cycle's evidence commit and then `8581b25710c11017386d8ab88c84c6fecec2025a`, retaining the earlier recovery changes. After a future squash merge, revert the merge commit. The signed Flywheel inverse retains exact parent production bytes and hashes; `git apply --check --reverse` passed for the production patch. No production rollback was executed and there is no data migration to undo.
