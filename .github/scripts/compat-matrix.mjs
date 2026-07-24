/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/
 */
// Derives the date-driven RN compat matrix. Pure functions below are unit
// tested; main() (added later) wires them to npm and the schedule table.

import { execSync } from 'node:child_process';
import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import process from 'node:process';

const STABLE = /^\d+\.\d+\.\d+$/;
const WINDOW_MS = 365 * 24 * 60 * 60 * 1000;

export function compareSemver(a, b) {
  const pa = a.split('.').map(Number);
  const pb = b.split('.').map(Number);
  for (let i = 0; i < 3; i++) {
    if (pa[i] !== pb[i]) return pa[i] - pb[i];
  }
  return 0;
}

export function selectRnVersions(timeJson, nowMs) {
  const cutoff = nowMs - WINDOW_MS;
  const inWindow = Object.entries(timeJson)
    .filter(([v]) => STABLE.test(v))
    .map(([version, iso]) => ({ version, dateMs: Date.parse(iso) }))
    .filter(({ dateMs }) => dateMs >= cutoff);

  const byMinor = new Map();
  for (const entry of inWindow) {
    const [maj, min] = entry.version.split('.');
    const key = `${maj}.${min}`;
    const current = byMinor.get(key);
    if (!current || compareSemver(entry.version, current.version) > 0) {
      byMinor.set(key, entry);
    }
  }
  return [...byMinor.values()].sort((a, b) => compareSemver(a.version, b.version));
}

export function selectBob(rnDateMs, bobTimeJson) {
  const stable = Object.entries(bobTimeJson)
    .filter(([v]) => STABLE.test(v))
    .map(([version, iso]) => ({ version, dateMs: Date.parse(iso) }))
    .sort((a, b) => a.dateMs - b.dateMs || compareSemver(a.version, b.version));

  const eligible = stable.filter(({ dateMs }) => dateMs <= rnDateMs);
  if (eligible.length > 0) return eligible[eligible.length - 1].version;
  return stable[0].version; // fallback: earliest published bob
}

export function selectRunner(rnDateMs, schedule, platform) {
  const rows = [...schedule].sort((a, b) => Date.parse(a.since) - Date.parse(b.since));
  let chosen = rows[0];
  for (const row of rows) {
    if (Date.parse(row.since) <= rnDateMs) chosen = row;
    else break;
  }
  return chosen[platform];
}

export function buildTriples({ rnTimeJson, bobTimeJson, schedule, platform, nowMs }) {
  return selectRnVersions(rnTimeJson, nowMs).map(({ version, dateMs }) => ({
    rn: version,
    bob: selectBob(dateMs, bobTimeJson),
    runner: selectRunner(dateMs, schedule, platform),
  }));
}

export function prTriples(platform) {
  const runner = platform === 'ios' ? 'macos-latest' : 'ubuntu-latest';
  return [{ rn: 'latest', bob: 'latest', runner }];
}

function npmTime(pkg) {
  return JSON.parse(execSync(`npm view ${pkg} time --json`, { encoding: 'utf8' }));
}

async function main() {
  const [platform, mode] = process.argv.slice(2);
  if (!['ios', 'android'].includes(platform)) throw new Error(`bad platform: ${platform}`);
  if (!['pr', 'full'].includes(mode)) throw new Error(`bad mode: ${mode}`);

  if (mode === 'pr') {
    console.log(JSON.stringify(prTriples(platform)));
    return;
  }

  const schedule = JSON.parse(
    readFileSync(new URL('../compat-runner-schedule.json', import.meta.url), 'utf8'),
  );
  const triples = buildTriples({
    rnTimeJson: npmTime('react-native'),
    bobTimeJson: npmTime('create-react-native-library'),
    schedule,
    platform,
    nowMs: Date.now(),
  });
  if (triples.length === 0) {
    throw new Error('no RN versions in window — refusing to emit an empty matrix');
  }
  console.log(JSON.stringify(triples));
}

if (process.argv[1] === fileURLToPath(import.meta.url)) {
  main().catch((err) => {
    console.error(err);
    process.exit(1);
  });
}
