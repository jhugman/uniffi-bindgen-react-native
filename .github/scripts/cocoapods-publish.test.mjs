/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/
 */
// Runs cocoapods-publish.sh against stand-ins for pod, curl and sleep:
//   node --test .github/scripts/cocoapods-publish.test.mjs
import { test } from 'node:test';
import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';
import { chmodSync, existsSync, mkdirSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

const SCRIPT = join(dirname(fileURLToPath(import.meta.url)), 'cocoapods-publish.sh');

const STUBS = {
  // Trunk's pod API serves whatever trunk.json holds.
  curl: '[ -e "$STATE/curl-fails" ] && exit 22\ncat "$STATE/trunk.json"',
  // `ok` lands and succeeds, `fail-but-lands` lands and fails, `fail` only fails.
  pod: [
    'echo "$*" >> "$STATE/pod-calls"',
    'mode=$(cat "$STATE/pod-mode")',
    '[ "$mode" != fail ] && cp "$STATE/after.json" "$STATE/trunk.json"',
    '[ "$mode" = ok ]',
  ].join('\n'),
  sleep: 'exit 0',
};

const BEFORE = { versions: [{ name: '0.31.0-6' }] };
const AFTER = { versions: [{ name: '0.31.0-6' }, { name: '9.9.9-1' }] };

function publish({ podMode = 'ok', alreadyOnTrunk = false, trunkDown = false } = {}) {
  const dir = mkdtempSync(join(tmpdir(), 'cocoapods-publish-'));
  try {
    const bin = join(dir, 'bin');
    mkdirSync(bin);
    for (const [name, body] of Object.entries(STUBS)) {
      writeFileSync(join(bin, name), `#!/bin/bash\n${body}\n`);
      chmodSync(join(bin, name), 0o755);
    }
    writeFileSync(join(dir, 'trunk.json'), JSON.stringify(alreadyOnTrunk ? AFTER : BEFORE));
    writeFileSync(join(dir, 'after.json'), JSON.stringify(AFTER));
    writeFileSync(join(dir, 'pod-mode'), podMode);
    if (trunkDown) writeFileSync(join(dir, 'curl-fails'), '');
    writeFileSync(join(dir, 'package.json'), JSON.stringify({ version: '9.9.9-1' }));

    const run = spawnSync('bash', [SCRIPT], {
      cwd: dir,
      encoding: 'utf8',
      env: { ...process.env, STATE: dir, PATH: `${bin}:${process.env.PATH}` },
    });
    const calls = join(dir, 'pod-calls');
    const pushes = existsSync(calls) ? readFileSync(calls, 'utf8').trim().split('\n').length : 0;
    return { status: run.status, pushes, output: run.stdout + run.stderr };
  } finally {
    rmSync(dir, { recursive: true, force: true });
  }
}

test('a version already on trunk is not pushed again', () => {
  const r = publish({ alreadyOnTrunk: true });
  assert.equal(r.status, 0, r.output);
  assert.equal(r.pushes, 0);
});

test('a clean push passes', () => {
  const r = publish();
  assert.equal(r.status, 0, r.output);
  assert.equal(r.pushes, 1);
});

test('a push that reports failure passes once trunk lists the version', () => {
  const r = publish({ podMode: 'fail-but-lands' });
  assert.equal(r.status, 0, r.output);
  assert.equal(r.pushes, 1);
  assert.match(r.output, /::warning::/);
});

test('a push that fails without landing fails', () => {
  const r = publish({ podMode: 'fail' });
  assert.equal(r.status, 1, r.output);
  assert.equal(r.pushes, 1);
  assert.match(r.output, /::error::.*is not on trunk/);
});

test('an unreachable trunk before the push does not stop the push', () => {
  const r = publish({ trunkDown: true });
  assert.equal(r.status, 0, r.output);
  assert.equal(r.pushes, 1);
});
