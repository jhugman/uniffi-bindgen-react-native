/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/
 */
import { test } from 'node:test';
import assert from 'node:assert/strict';
import { compareSemver, selectRnVersions, selectBob, selectRunner } from './compat-matrix.mjs';

test('compareSemver orders numerically, not as strings', () => {
  assert.ok(compareSemver('0.81.10', '0.81.2') > 0); // 10 > 2
  assert.ok(compareSemver('0.80.0', '0.81.0') < 0);
  assert.equal(compareSemver('0.80.3', '0.80.3'), 0);
});

const RN_TIME = {
  created: '2015-01-01T00:00:00.000Z',
  modified: '2026-07-20T00:00:00.000Z',
  '0.76.5': '2025-01-10T00:00:00.000Z', // out of window
  '0.80.0': '2025-08-01T00:00:00.000Z',
  '0.80.3': '2025-09-15T00:00:00.000Z', // higher patch of 0.80 wins
  '0.81.0': '2025-11-01T00:00:00.000Z',
  '0.81.2': '2026-02-01T00:00:00.000Z',
  '0.81.10': '2026-05-01T00:00:00.000Z', // 0.81.10 > 0.81.2
  '0.82.0-rc.1': '2026-06-01T00:00:00.000Z', // prerelease dropped
  '0.82.0': '2026-06-20T00:00:00.000Z',
};
const NOW = Date.parse('2026-07-23T00:00:00.000Z');

test('selectRnVersions: one per minor, highest patch, in-window, semver-sorted', () => {
  const result = selectRnVersions(RN_TIME, NOW);
  assert.deepEqual(
    result.map((r) => r.version),
    ['0.80.3', '0.81.10', '0.82.0'],
  );
});

test('selectRnVersions: carries the publish date of the chosen patch', () => {
  const result = selectRnVersions(RN_TIME, NOW);
  const rn81 = result.find((r) => r.version === '0.81.10');
  assert.equal(rn81.dateMs, Date.parse('2026-05-01T00:00:00.000Z'));
});

const BOB_TIME = {
  created: '2018-01-01T00:00:00.000Z',
  modified: '2026-07-01T00:00:00.000Z',
  '0.48.0': '2025-05-01T00:00:00.000Z',
  '0.49.10': '2025-08-10T00:00:00.000Z',
  '0.50.0': '2026-03-01T00:00:00.000Z',
  '0.51.0-rc.0': '2026-06-15T00:00:00.000Z', // prerelease ignored
};

test('selectBob: latest published on or before the RN date', () => {
  assert.equal(selectBob(Date.parse('2025-09-15T00:00:00.000Z'), BOB_TIME), '0.49.10');
  assert.equal(selectBob(Date.parse('2026-06-20T00:00:00.000Z'), BOB_TIME), '0.50.0');
});

test('selectBob: falls back to the earliest bob when none is old enough', () => {
  assert.equal(selectBob(Date.parse('2025-01-01T00:00:00.000Z'), BOB_TIME), '0.48.0');
});

test('selectBob: inclusive on the boundary (RN date === a bob publish date)', () => {
  // Exactly on 0.49.10's publish date → 0.49.10 is eligible (<=, not <).
  assert.equal(selectBob(Date.parse('2025-08-10T00:00:00.000Z'), BOB_TIME), '0.49.10');
});

const SCHEDULE = [
  { since: '2026-06-30', ios: 'macos-26', android: 'ubuntu-24.04' }, // deliberately unsorted
  { since: '2024-08-01', ios: 'macos-14', android: 'ubuntu-22.04' },
  { since: '2025-06-01', ios: 'macos-15', android: 'ubuntu-24.04' },
];

test('selectRunner: latest row with since <= the RN date, per platform', () => {
  assert.equal(selectRunner(Date.parse('2025-09-15T00:00:00.000Z'), SCHEDULE, 'ios'), 'macos-15');
  assert.equal(selectRunner(Date.parse('2026-07-01T00:00:00.000Z'), SCHEDULE, 'ios'), 'macos-26');
  assert.equal(selectRunner(Date.parse('2026-07-01T00:00:00.000Z'), SCHEDULE, 'android'), 'ubuntu-24.04');
});

test('selectRunner: falls back to the earliest row when the RN predates the table', () => {
  assert.equal(selectRunner(Date.parse('2023-01-01T00:00:00.000Z'), SCHEDULE, 'ios'), 'macos-14');
});

test('selectRunner: inclusive on the boundary (RN date === a row since)', () => {
  // Exactly on the macos-15 row's since → that row wins (<=, not <).
  assert.equal(selectRunner(Date.parse('2025-06-01T00:00:00.000Z'), SCHEDULE, 'ios'), 'macos-15');
});

import { execFileSync } from 'node:child_process';
import { fileURLToPath } from 'node:url';
import { buildTriples, prTriples } from './compat-matrix.mjs';

test('buildTriples composes rn + contemporaneous bob + runner (iOS)', () => {
  const triples = buildTriples({
    rnTimeJson: RN_TIME,
    bobTimeJson: BOB_TIME,
    schedule: SCHEDULE,
    platform: 'ios',
    nowMs: NOW,
  });
  assert.deepEqual(triples, [
    { rn: '0.80.3', bob: '0.49.10', runner: 'macos-15' },
    { rn: '0.81.10', bob: '0.50.0', runner: 'macos-15' },
    { rn: '0.82.0', bob: '0.50.0', runner: 'macos-15' },
  ]);
});

test('prTriples is the single latest/latest today-row per platform', () => {
  assert.deepEqual(prTriples('ios'), [{ rn: 'latest', bob: 'latest', runner: 'macos-latest' }]);
  assert.deepEqual(prTriples('android'), [{ rn: 'latest', bob: 'latest', runner: 'ubuntu-latest' }]);
});

test('CLI pr mode prints one-line JSON without touching the network', () => {
  const script = fileURLToPath(new URL('./compat-matrix.mjs', import.meta.url));
  const out = execFileSync('node', [script, 'ios', 'pr'], { encoding: 'utf8' }).trim();
  assert.equal(out, '[{"rn":"latest","bob":"latest","runner":"macos-latest"}]');
});
