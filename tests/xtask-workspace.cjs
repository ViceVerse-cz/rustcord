// A cached xtask must operate on its invocation workspace, not its compilation checkout.
const assert = require('node:assert/strict');
const fs = require('node:fs');
const os = require('node:os');
const path = require('node:path');
const { spawnSync } = require('node:child_process');

const repo = path.resolve(__dirname, '..');
const binary = path.resolve(process.env.CARGO_TARGET_DIR || path.join(repo, 'target'),
  'debug', process.platform === 'win32' ? 'xtask.exe' : 'xtask');
assert.ok(fs.existsSync(binary), 'Build xtask first (cargo xtask check)');
const parent = fs.realpathSync(os.tmpdir());
const fixture = fs.mkdtempSync(path.join(parent, 'serein-xtask-workspace-'));
function run(program, args, cwd) {
  const result = spawnSync(program, args, {
    cwd, encoding: 'utf8', windowsHide: true, timeout: 30000, maxBuffer: 1024 * 1024,
    env: { ...process.env, CARGO_NET_OFFLINE: 'true' },
  });
  assert.ifError(result.error);
  assert.equal(result.signal, null);
  return { status: result.status, output: `${result.stdout}\n${result.stderr}` };
}
try {
  fs.mkdirSync(path.join(fixture, 'src'));
  fs.mkdirSync(path.join(fixture, 'apps/desktop/src'), { recursive: true });
  fs.writeFileSync(path.join(fixture, 'Cargo.toml'),
    '[workspace]\n[package]\nname="serein-xtask-fixture"\nversion="0.0.0"\nedition="2021"\nlicense="MIT"\n');
  fs.writeFileSync(path.join(fixture, 'src/lib.rs'), '');
  fs.writeFileSync(path.join(fixture, 'apps/desktop/src/main.rs'), '// Missing persistence controls\n');
  const lock = run('cargo', ['generate-lockfile', '--offline'], fixture);
  assert.equal(lock.status, 0, lock.output);
  // Invoke the SAME binary from a nested directory in a different workspace.
  const checked = run(binary, ['policy'], path.join(fixture, 'src'));
  assert.equal(checked.status, 1, checked.output);
  assert.match(checked.output, /Native persistence controls changed/);
  console.log('Cached xtask selects the invocation workspace: passed');
} finally {
  assert.equal(path.dirname(fs.realpathSync(fixture)), parent);
  fs.rmSync(fixture, { recursive: true });
}
