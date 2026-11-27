# CI runner: "invalid internal status, try resetting the pause process"

## Symptom
`make ci` fails in a few seconds, at the first podman command (`make image`):

```
level=error msg="invalid internal status, try resetting the pause process with
\"/usr/bin/podman system migrate\": could not find any running process: no such process"
```

It fails on one runner only, every time; the other runner stays green. `make image` then
prints `scripts/podman-diagnose.sh`: `pause.pid=<none> alive=no`, and old entries under
`/run/user/1000/containers/overlay-containers` and `/run/user/1000/netns`.

## Cause
A job was cancelled (a new push with `cancel-in-progress`, or a manual cancel) while podman
was building or running a container. The runner kills every process of the job, podman's
pause process included (they all live in the runner service's cgroup,
`system.slice/actions.runner.<org>.<runner>.service`). Podman's database still lists the
container as running, so every later podman command tries to join the dead pause
process's user namespace and fails. First seen 2026-11-26 on gh-runner-1 (PR #11).

## Fix (on the broken runner)
Either one:
- reboot it: `/run/user/1000` is a tmpfs, the stale state goes with it;
- or, as the `runner` user with its runtime dir (as another user or without
  `XDG_RUNTIME_DIR` it changes nothing):
  ```sh
  sudo -u runner XDG_RUNTIME_DIR=/run/user/1000 podman system migrate
  ```
Then re-run the failed checks (`gh run rerun <id> --failed`).

## Prevention (not done yet)
In infra-s1 (`runners/templates/cloud-init.yaml.tftpl`), have the job-completed hook
`/usr/local/bin/runner-job-cleanup.sh` also run `podman system migrate`: a runner runs one
job at a time, so no container is running when the hook fires. That hook also reports
exit code 1 although its script ends with `exit 0`: check the deployed copy.
