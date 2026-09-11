#!/usr/bin/env bash
#
# Kill leftover Hermes `test-runner` processes.
#
# The Rust test harness spawns `build/test-runner/test-runner` and waits on it.
# A long fixture (e.g. the benchmark) can run for many minutes; if the harness
# is interrupted (Ctrl-C / IDE stop / SIGKILL) the child is reparented and keeps
# grinding at ~100% CPU. Newer test-runner builds self-exit when orphaned (see
# startParentDeathWatchdog in cpp/test-harness/test-runner.cpp), but this is a
# belt-and-suspenders for older builds or a wedged process.

set -euo pipefail

pattern="build/test-runner/test-runner"

pids=$(pgrep -f "$pattern" || true)
if [ -z "$pids" ]; then
  echo "No stray test-runner processes."
  exit 0
fi

echo "Killing stray test-runner processes:"
# shellcheck disable=SC2086
ps -o pid,%cpu,etime,command -p $pids || true
# shellcheck disable=SC2086
kill $pids 2>/dev/null || true
sleep 1
# shellcheck disable=SC2086
kill -9 $(pgrep -f "$pattern" || true) 2>/dev/null || true
echo "Done."
