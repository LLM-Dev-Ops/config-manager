# MetaHarness cycle — 2026-10-15

## Scope and frozen contract

Baseline: `main` at `02b463c3cbf088183f14b2ec951389f797863ac7`.

The fixed HTTP-boundary holdout checks JSON media type enforcement, a 64 KiB request limit, object-shaped configuration input, correct `405` behavior, an `Allow` response header, and the existing health response. The frozen anchor runs the full pre-existing test suite plus a valid configuration and health request.

No model was used for the deterministic correctness evaluator. Project-source evaluation and Darwin harness evolution were kept separate.

## Pinned tools

| Component | Version/revision |
| --- | --- |
| Ruflo | `3.56.3` |
| MetaHarness | `0.4.17` |
| Darwin | `0.10.3` |
| Flywheel | `0.1.12` |
| Upstream MetaHarness | `e0dfd44da72b7adfb7c57fdd42d124be58ed7086` |
| Node.js | `24.19.0` locally; CI pins `20.19.5` |

## Results

| Gate | Baseline | Candidate |
| --- | ---: | ---: |
| Existing Cloud Function tests | 20/20 | 31/31 |
| Frozen HTTP holdout | 1/6 | 6/6 |
| Composite Flywheel holdout | 1/8 | 8/8 |
| Flywheel anchor | 4/4 | 4/4 |
| Production npm audit | 0 findings | 0 findings |
| RustSec vulnerabilities | 2 | 0 |
| Custom security scan | 24 critical, 2 high | 0 critical, 0 high |
| Node syntax check | pass | pass |
| MetaHarness fit | 71 | 71 |
| MetaHarness hard constraints | 6/6 | 6/6 |
| Rust workspace check | pass | pass |
| Rust fmt / default Clippy | fail / fail | fail / fail (unchanged baseline debt) |
| Rust security integration suite | 2/15 | 2/15 (unchanged baseline debt) |

MetaHarness scoring exited `0`. Darwin ran the documented real-sandbox interface with one generation, two children, concurrency one, seed `2026101520`, deterministic mutation, and Pareto selection. The baseline, score-policy variant, and reviewer variant each scored `0.985`; both variants were rejected with delta `0.000`.

Flywheel used `runFlywheelGenerations`, the unmodified conjunctive promotion rule, Ed25519 receipts, lineage, and independent replay verification. Its fixed composite holdout covered the six HTTP checks plus the live custom-security and dependency-scan commands. It promoted the implementation from `0.125` to `1.0`, while the four-item anchor remained `1.0`. Replay passed all eight integrity checks. Gate fingerprint: `c5942e7c9bf8f28fe7af47a6ec3887d11eeb3f8c8e8601cc6a7a54969ea0b981`.

## Implementation and compatibility

- Reject non-JSON validation requests with `415`.
- Reject bodies over 64 KiB using the maximum of raw bytes, content length, and serialized body size.
- Reject scalar/array configuration objects and malformed schema/options types with `400`.
- Return `405` plus `Allow` for unsupported methods on known routes.
- Support exact configured CORS allowlists while preserving explicit/default wildcard compatibility.
- Run deterministic Node tests and a production audit in least-privilege Ubuntu 24.04 CI with commit-pinned actions.

The candidate removes the unused `validator` dependency that retained vulnerable `idna 0.4`, updates Prometheus to the protobuf-free 0.14 line, and refreshes git2 to remove three unsoundness advisories. `cargo audit 0.22.2` exits successfully with zero vulnerabilities and four unmaintained-crate warnings. The security scanner now ignores environment placeholders, explicit deployment placeholders, `#[cfg(test)]` fixtures, and its own pattern definitions while retaining live-source secret and SQL checks. The security workflow pins its Linux image, Rust toolchain, audit binary, and every action to an immutable revision; it also replaces retired v3 cache/artifact actions.

The pre-existing Rust security integration suite remains at 2/15 on both baseline and candidate, with the same 13 failures. `cargo fmt --check` and strict `cargo clippy -- -D warnings` also remain pre-existing debt; neither was hidden or suppressed. These gates therefore do not establish a regression, but the failing suite remains a blocker for any claim that the whole workspace is green.

Rollback is one atomic revert of the eventual merge commit. No deployment, package publication, persistence change, or data migration is included.
