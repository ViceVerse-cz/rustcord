import assert from 'node:assert/strict';
import { appendFileSync, mkdirSync, readFileSync, readdirSync, writeFileSync } from 'node:fs';
import { createRequire } from 'node:module';
import { execFileSync } from 'node:child_process';
import semanticRelease from 'semantic-release';

const require = createRequire(import.meta.url);
const mode = process.argv[2];
assert(['plan', 'publish'].includes(mode), 'Use release.mjs plan|publish');
assert.equal(process.env.GITHUB_REF_NAME, 'main', 'Release from main only');
const channel = process.env.RELEASE_CHANNEL;
assert(['nightly', 'production'].includes(channel), 'Choose nightly or production');
const planPath = 'target/release-plan.json';
const plan = mode === 'publish' ? JSON.parse(readFileSync(planPath, 'utf8')) : null;
if (plan) assert.equal(channel, plan.channel, 'Release channel changed');
const conventional = { preset: 'conventionalcommits' };
const plugins = [
  [require.resolve('@semantic-release/commit-analyzer'), conventional],
  [require.resolve('@semantic-release/release-notes-generator'), conventional],
];
if (plan && channel === 'production') {
  plugins.push(
    [require.resolve('@semantic-release/git'), {
      assets: ['Cargo.toml', 'Cargo.lock', 'fuzz/Cargo.lock', 'packaging/macos/Info.plist'],
      message: 'chore(release): ${nextRelease.version} [skip ci]',
    }],
    [require.resolve('@semantic-release/github'), {
      assets: ['release-assets/*'],
      successComment: false,
      failComment: false,
      releasedLabels: false,
      releaseNameTemplate: 'Serein <%= nextRelease.version %>',
    }],
  );
}
const result = await semanticRelease({
  branches: ['main'],
  tagFormat: 'v${version}',
  plugins,
  dryRun: mode === 'plan' || channel === 'nightly',
  verifyRelease: async (_config, { nextRelease }) => {
    if (plan) {
      assert.equal(nextRelease.version, plan.stableVersion, 'Release changed while packages were building');
      assert.equal(nextRelease.gitHead, plan.gitHead, 'Release commit changed while packages were building');
    }
  },
});
if (mode === 'plan') {
  appendFileSync(process.env.GITHUB_OUTPUT, `release=${Boolean(result)}\n`);
  if (result) {
    const { version: stableVersion, gitHead, notes } = result.nextRelease;
    const version = channel === 'nightly'
      ? `${stableVersion}-nightly.${process.env.GITHUB_RUN_NUMBER}.${process.env.GITHUB_RUN_ATTEMPT}`
      : stableVersion;
    const gitTag = `v${version}`;
    mkdirSync('target', { recursive: true });
    writeFileSync(planPath, JSON.stringify({ version, stableVersion, gitTag, gitHead, notes, channel }));
    appendFileSync(process.env.GITHUB_OUTPUT, `version=${version}\ntag=${gitTag}\n`);
  }
} else {
  assert(result, 'No release published; branch or tags changed after planning');
  if (channel === 'nightly') {
    // Nightlies do not create stable tags or version commits that consume production changes.
    writeFileSync('target/release-notes.md', plan.notes);
    const assets = readdirSync('release-assets')
      .map(name => `release-assets/${name}`);
    execFileSync('gh', ['release', 'create', plan.gitTag, ...assets,
      '--target', plan.gitHead, '--title', `Serein ${plan.version}`,
      '--notes-file', 'target/release-notes.md', '--prerelease', '--latest=false', '--draft'],
    { stdio: 'inherit' });
    execFileSync('gh', ['release', 'edit', plan.gitTag, '--draft=false', '--prerelease', '--latest=false'],
    { stdio: 'inherit' });
  }
}
