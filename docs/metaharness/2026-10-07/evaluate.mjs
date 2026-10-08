import assert from 'node:assert/strict';
import { createHash } from 'node:crypto';
import { createRequire } from 'node:module';
import { readFileSync, writeFileSync, mkdtempSync, rmSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { resolve, join } from 'node:path';
import { spawnSync } from 'node:child_process';
import { pathToFileURL } from 'node:url';

// Usage: node evaluate.mjs <baseline-checkout> <candidate-checkout> <pinned-runtime>
const [baseline, candidate, runtime] = process.argv.slice(2).map(value => resolve(value));
const here = createRequire(import.meta.url);
const express = createRequire(join(candidate, 'package.json'))('express');
const flywheel = await import(pathToFileURL(join(runtime, 'node_modules/@metaharness/flywheel/dist/index.js')));
const contract = JSON.parse(readFileSync(new URL('./contract.json', import.meta.url), 'utf8'));
const rootBytes = readFileSync(join(baseline, 'index.js'), 'utf8');
const lookup = 'const schema = SCHEMAS[schemaId];';
const fixed = 'const schema = Object.hasOwn(SCHEMAS, schemaId) ? SCHEMAS[schemaId] : undefined;';

assert.equal(rootBytes.split(lookup).length, 2, 'expected one schema lookup');
assert.equal(readFileSync(join(candidate, 'index.js'), 'utf8'), rootBytes.replace(lookup, fixed));

const measurements = [];
const tests = {};
for (const [name, dir] of [['baseline', baseline], ['candidate', candidate]]) {
  const run = spawnSync('node', ['test/handler.test.js'], { cwd: dir, encoding: 'utf8', timeout: 30000 });
  tests[name] = { exit: run.status, stdout: run.stdout, stderr: run.stderr };
  assert.equal(run.status, 0, `${name} existing tests must pass`);
}

const hash = bytes => createHash('sha256').update(bytes).digest('hex');
const evaluator = async (policy, suite) => {
  const bytes = rootBytes.replace(lookup, policy.schemaLookup);
  const dir = mkdtempSync(join(tmpdir(), 'schema-eval-'));
  const modulePath = join(dir, 'index.cjs');
  writeFileSync(modulePath, bytes);
  const handler = here(modulePath).handler;
  const app = express();
  app.use(express.json());
  app.use(handler);
  app.use((error, _req, res, _next) => res.status(500).json({ error: error.name }));
  const server = app.listen(0, '127.0.0.1');
  await new Promise(r => server.once('listening', r));
  const url = `http://127.0.0.1:${server.address().port}`;
  const itemWins = [];
  const requests = [];
  try {
    for (const item of suite.items) {
      const start = performance.now();
      let response;
      if (item === 'health') {
        response = await fetch(url + '/health');
      } else {
        response = await fetch(url + '/v1/config-manager/validate', {
          method: 'POST',
          headers: { 'content-type': 'application/json' },
          body: JSON.stringify({
            schema: item,
            config: item === 'provider-config-v1'
              ? { provider_type: 'gcp', endpoint: 'https://example.com' }
              : { namespace: 'app/db', key: 'timeout', value: 30 },
          }),
        });
      }
      const body = await response.json();
      const expected = ['llm-config-v1', 'provider-config-v1', 'health'].includes(item) ? 200 : 404;
      const win = response.status === expected
        && Boolean(body.execution_metadata?.trace_id)
        && (expected === 200 || body.error?.code === 'NOT_FOUND');
      itemWins.push(win);
      requests.push({
        item,
        status: response.status,
        passed: win,
        milliseconds: Number((performance.now() - start).toFixed(3)),
      });
    }
  } finally {
    server.closeAllConnections();
    await new Promise((r, reject) => server.close(e => e ? reject(e) : r()));
    delete here.cache[modulePath];
    rmSync(dir, { recursive: true, force: true });
  }
  const passed = itemWins.filter(Boolean).length;
  const score = {
    primary: passed / itemWins.length,
    noopRate: 1 - passed / itemWins.length,
    costPerWin: 0,
    regressed: suite.id === 'anchor' && passed !== itemWins.length,
    itemWins,
  };
  measurements.push({ sourceSha256: hash(bytes), suite: suite.id, score, requests });
  return score;
};

const pinnedGateFingerprint = flywheel.gateFingerprint(flywheel.meetsPromotionRule);
const result = await flywheel.runFlywheelGenerations({
  rootPolicy: { schemaLookup: lookup },
  proposer: async () => ({
    value: fixed,
    summary: 'Reject inherited schema names after reproduced toString crash',
    inverse: { path: 'index.js', parentBytes: rootBytes, hash: hash(rootBytes) },
  }),
  evaluator,
  holdout: { id: 'holdout', items: contract.holdout },
  anchor: { id: 'anchor', items: contract.anchor.filter(x => x !== 'existing 15-test suite') },
  maxGenerations: 2,
  signer: flywheel.makeSigner(),
  promotionRule: flywheel.meetsPromotionRule,
  dataSource: 'LIVE_HTTP_LOCAL_HANDLER',
  now: generation => `2026-10-07-generation-${generation}`,
});
const replay = flywheel.verifyReplayBundle(result.replayBundle, { pinnedGateFingerprint });

assert.equal(replay.pass, true, 'signed replay must pass');
assert.equal(result.promotions.length, 1, 'only the measured fix may promote');
assert.equal(result.finalPolicy.schemaLookup, fixed);

const report = {
  contract,
  sourceSha256: hash(readFileSync(join(candidate, 'index.js'))),
  tests,
  measurements,
  pinnedGateFingerprint,
  replay,
  finalPolicy: result.finalPolicy,
  promotions: result.promotions.length,
  replayBundle: result.replayBundle,
  billingUsd: 0,
  runtimeCostAndEnergy: 'unmeasured',
  boundary: 'local real HTTP correctness evaluation; no production/load/LLM efficacy claim',
};
writeFileSync(new URL('./flywheel-results.json', import.meta.url), JSON.stringify(report, null, 2) + '\n');
console.log(JSON.stringify({
  promotions: report.promotions,
  replay: replay.pass,
  measurements: measurements.map(x => ({
    sourceSha256: x.sourceSha256,
    suite: x.suite,
    primary: x.score.primary,
    noopRate: x.score.noopRate,
    requests: x.requests,
  })),
  tests: Object.fromEntries(Object.entries(tests).map(([k, v]) => [k, v.exit])),
}, null, 2));
