import assert from 'node:assert/strict';
import { createHash } from 'node:crypto';
import { readFileSync, writeFileSync } from 'node:fs';
import { resolve, join } from 'node:path';
import { pathToFileURL } from 'node:url';
import { spawnSync } from 'node:child_process';

// Runs real, identical Rust/TCP tests; never substitutes recorded or mock scores.
// node evaluate.mjs BASELINE CANDIDATE PINNED_TOOLS OUTPUT [ISOLATED_TARGET_ROOT]
const [baseline, candidate, runtime, output, target] = process.argv.slice(2).map(value => resolve(value));
const fw = await import(pathToFileURL(join(runtime, 'node_modules/@metaharness/flywheel/dist/index.js')));
assert.equal(JSON.parse(readFileSync(join(runtime, 'node_modules/@metaharness/flywheel/package.json'))).version, '0.1.12');
const contract = JSON.parse(readFileSync(new URL('./contract.json', import.meta.url)));
const hash = bytes => createHash('sha256').update(bytes).digest('hex');
const fixture = 'crates/llm-config-api/tests/policy_boundary.rs';
for (const file of [fixture, 'Cargo.lock', 'crates/llm-config-api/Cargo.toml']) {
  assert.equal(hash(readFileSync(join(baseline, file))), hash(readFileSync(join(candidate, file))), `identical comparison input: ${file}`);
}
const parentFiles = {};
for (const file of ['crates/llm-config-api/src/middleware.rs', 'crates/llm-config-api/src/server.rs', 'crates/llm-config-security/src/policy.rs']) {
  parentFiles[file] = readFileSync(join(baseline, file), 'utf8');
}
const parentBytes = JSON.stringify(parentFiles);
const sourceHash = dir => hash(readFileSync(join(dir, 'crates/llm-config-api/src/middleware.rs')));
const rootHash = sourceHash(baseline), fixedHash = sourceHash(candidate);
assert.notEqual(rootHash, fixedHash);
const supplementary = ['repeated_origins_fail_closed', 'encoded_endpoint_cannot_bypass_blocklist', 'encoded_script_rejected', 'invalid_utf8_path_rejected', 'small_chunked_body_preserved'];
const measurements = [];
const evaluator = async (policy, suite) => {
  const dir = policy.source === rootHash ? baseline : candidate;
  assert.equal(sourceHash(dir), policy.source, 'evaluated source must match policy');
  const command = ['test', '--locked', '-p', 'llm-config-api', '--test', 'policy_boundary', '--', '--test-threads=1'];
  const run = spawnSync('cargo', command, {
    cwd: dir, encoding: 'utf8', timeout: 300000, maxBuffer: 4 * 1024 * 1024,
    env: { ...process.env, CARGO_INCREMENTAL: '0', CARGO_PROFILE_TEST_DEBUG: '0', CARGO_PROFILE_DEV_DEBUG: '0', CARGO_TARGET_DIR: target ? join(target, policy.source.slice(0, 16)) : join(dir, 'target-flywheel') },
  });
  assert.equal(run.signal, null, 'signal/timeout is not a test result');
  assert.ok(run.status === 0 || run.status === 101, 'expected real Rust test completion');
  const cases = Object.fromEntries([...run.stdout.matchAll(/^test (\w+) \.\.\. (ok|FAILED)$/gm)].map(m => [m[1], m[2] === 'ok']));
  for (const name of [...contract.holdout, ...contract.anchor, ...supplementary]) assert.ok(Object.hasOwn(cases, name), `test actually executed: ${name}\n${run.stdout}\n${run.stderr}`);
  const itemWins = suite.items.map(name => cases[name]);
  const primary = itemWins.filter(Boolean).length / itemWins.length;
  const score = { primary, noopRate: 1 - primary, costPerWin: 0, regressed: dir === candidate && supplementary.some(name => !cases[name]), itemWins };
  measurements.push({ source: policy.source, suite: suite.id, command: ['cargo', ...command], exit: run.status, cases, score, stdout: run.stdout, stderr: run.stderr });
  return score;
};
const fingerprint = fw.gateFingerprint(fw.meetsPromotionRule);
const result = await fw.runFlywheelGenerations({
  rootPolicy: { source: rootHash },
  proposer: async () => ({ value: fixedHash, summary: 'Enforce real origin, actual body size and original endpoint policies', inverse: { path: 'production-parent-files.json', parentBytes, hash: hash(parentBytes) } }),
  evaluator,
  holdout: { id: 'holdout', items: contract.holdout },
  anchor: { id: 'anchor', items: contract.anchor },
  maxGenerations: 1, signer: fw.makeSigner(), promotionRule: fw.meetsPromotionRule,
  dataSource: 'LIVE_RUST_TCP_IDENTICAL_TESTS_AND_LOCK',
});
const replay = fw.verifyReplayBundle(result.replayBundle, { pinnedGateFingerprint: fingerprint });
assert.equal(replay.pass, true);
assert.equal(result.promotions.length, 1);
const anchors = measurements.filter(m => m.suite === 'anchor');
for (const name of contract.anchor) assert.ok(!anchors[0].cases[name] || anchors[1].cases[name], `retained anchor ${name}`);
writeFileSync(output, JSON.stringify({ contract, fixtureSha256: hash(readFileSync(join(candidate, fixture))), lockSha256: hash(readFileSync(join(candidate, 'Cargo.lock'))), measurements, fingerprint, replay, promotions: result.promotions.length, replayBundle: result.replayBundle, boundary: 'component correctness only; no workspace validation, deployment, LLM efficacy, paid cost, latency or energy claim' }, null, 2) + '\n');
console.log(JSON.stringify({ promotions: result.promotions.length, replay, measurements: measurements.map(({ suite, exit, score }) => ({ suite, exit, score })) }, null, 2));
