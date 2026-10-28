// Single-writer evaluation in an explicitly supplied, isolated detached worktree.
// Baseline is main production source with the identical new test module overlaid.
import { execFileSync, spawnSync } from 'node:child_process';
import { createHash } from 'node:crypto';
import { writeFileSync } from 'node:fs';
import path from 'node:path';
const [root, parent, head, toolsRoot, target, output] = process.argv.slice(2);
if (![root, parent, head, toolsRoot, target, output].every(Boolean)) throw Error('six arguments required');
const fw = await import(path.join(toolsRoot, 'node_modules/@metaharness/flywheel/dist/index.js'));
const git = (...args) => execFileSync('git', args, { cwd: root, encoding: 'utf8' }).trim();
const sourcePath = 'crates/llm-config-core/src/providers/env.rs';
const tests = rev => git('show', `${rev}:${sourcePath}`).split('#[cfg(test)]')[1];
if (!tests(parent) || tests(parent) !== tests(head)) throw Error('regression fixtures differ');
const env = { ...process.env, CARGO_TARGET_DIR: target, CARGO_INCREMENTAL: '0',
  CARGO_PROFILE_DEV_DEBUG: '0', CARGO_PROFILE_TEST_DEBUG: '0', CARGO_PROFILE_TEST_OPT_LEVEL: '0' };
// Fixed before evaluation; applies equally to baseline and candidate. Cold
// compilation and all doctests must fit, rather than silently skipping either.
const budgets = Object.freeze({ core: 180, workspace: 300, node: 30 });
const runBounded = (command, args, seconds) => spawnSync('timeout',
  ['--kill-after=5', String(seconds), command, ...args],
  { cwd: root, env, encoding: 'utf8', timeout: (seconds + 10) * 1000, maxBuffer: 4000000 });
const holdoutNames = ['test_case_sensitive_names_round_trip', 'test_case_sensitive_dotenv_list_get_consistency'];
const anchorNames = ['test_naming_config_default', 'test_naming_config_with_prefix', 'test_naming_config_parse',
  'test_naming_handles_dots_and_slashes', 'test_env_provider_not_found', 'test_env_provider_reads_env',
  'test_dotenv_parsing', 'test_default_dotenv_prefix_remains_case_insensitive'];
const measurements = {};
const evaluator = async (policy, suite) => {
  const score = m => {
    const items = suite.id === 'env-existing-and-default-v1' ? m.anchors : m.holdout;
    const wins = items.filter(Boolean).length;
    return { primary: wins / items.length, noopRate: 1 - wins / items.length,
      costPerWin: m.elapsedSeconds / Math.max(wins, 1), itemWins: items,
      regressed: !m.anchors.every(Boolean) || m.coreExit !== 0 || m.nodeExit !== 0 || m.workspaceExit !== 0 };
  };
  // Both suites are slices of the same executed Rust test run. Reuse its
  // immutable measurement for this exact revision, not a simulated score.
  if (measurements[policy.revision]) return score(measurements[policy.revision]);
  const start = performance.now();
  if (![parent, head].includes(policy.revision)) throw Error('unexpected revision');
  git('switch', '--detach', policy.revision);
  const run = runBounded('cargo', ['test', '--locked', '--offline', '-p', 'llm-config-core', '--lib', '-j', '2'], budgets.core);
  const text = (run.stdout || '') + (run.stderr || '');
  const passed = name => text.includes(`providers::env::tests::${name} ... ok`);
  const seen = name => text.includes(`providers::env::tests::${name} ... `);
  if (![...holdoutNames, ...anchorNames].every(seen)) throw Error('expected Rust tests did not execute');
  const wins = holdoutNames.map(passed), anchors = anchorNames.map(passed);
  const node = runBounded('npm', ['test'], budgets.node);
  const workspace = runBounded('cargo', ['test', '--locked', '--offline', '--workspace', '-j', '2'], budgets.workspace);
  measurements[policy.revision] = { holdout: wins, anchors, coreExit: run.status,
    nodeExit: node.status, workspaceExit: workspace.status, workspaceError: workspace.error?.message,
    coreOutput: text, workspaceOutput: (workspace.stdout || '') + (workspace.stderr || ''),
    elapsedSeconds: (performance.now() - start) / 1000 };
  return score(measurements[policy.revision]);
};
try {
  const result = await fw.runFlywheelGenerations({ rootPolicy: { revision: parent },
    proposer: async () => ({ value: head, summary: 'Case-sensitive env provider consistency', inverse: {
      path: 'git-revision', parentBytes: parent, hash: createHash('sha256').update(parent).digest('hex') } }),
    evaluator, promotionRule: fw.meetsPromotionRule,
    holdout: { id: 'case-sensitive-env-v1', items: holdoutNames }, anchor: { id: 'env-existing-and-default-v1', items: anchorNames },
    mutationTargets: ['revision'], maxGenerations: 1, signer: fw.makeSigner(), dataSource: 'LIVE' });
  const replay = fw.verifyReplayBundle(result.replayBundle, { pinnedGateFingerprint: fw.gateFingerprint(fw.meetsPromotionRule) });
  const report = { parent, head, budgets, measurements, gateFingerprint: fw.gateFingerprint(fw.meetsPromotionRule),
    replay: { pass: replay.pass, chainSummary: replay.chainSummary }, finalPolicy: result.finalPolicy,
    commits: result.replayBundle.all_commits.map(c => ({ verdict: c.verdict, failureReasons: c.failureReasons, primaryDelta: c.primaryDelta })),
    replayBundle: result.replayBundle };
  writeFileSync(output, JSON.stringify(report, null, 2) + '\n');
  console.log(JSON.stringify({ ...report, measurements: undefined, replayBundle: undefined }));
} finally { git('switch', '--detach', head); }
