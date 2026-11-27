#!/usr/bin/env bash
# Printed by `make image` when podman fails, to tell a broken runner from a broken build:
# rootless podman's runtime state for the current user (read-only, changes nothing).
set -u
c=${1:-podman}
echo "== podman diagnostics ($(hostname), $(id -un) uid $(id -u))"
echo "XDG_RUNTIME_DIR=${XDG_RUNTIME_DIR:-<unset>}"
"$c" --version 2>&1
rt=${XDG_RUNTIME_DIR:-/run/user/$(id -u)}
ls -la "$rt" "$rt/libpod/tmp" "$rt/containers" 2>&1
pause=$(cat "$rt/libpod/tmp/pause.pid" 2>/dev/null || true)
echo "pause.pid=${pause:-<none>} alive=$([ -n "$pause" ] && kill -0 "$pause" 2>/dev/null && echo yes || echo no)"
echo "-- processes of $(id -un):"
ps -u "$(id -u)" -o pid,ppid,lstart,cmd 2>&1 | grep -vE ' ps -u| grep ' | head -40
echo "-- cgroup: $(cat /proc/self/cgroup 2>&1)"
echo "-- systemd --user: $(systemctl --user is-system-running 2>&1)"
echo "-- known runner problem and its fix: docs/ci-runner.md"
echo "-- podman --log-level=debug info (tail):"
"$c" --log-level=debug info 2>&1 | tail -25
