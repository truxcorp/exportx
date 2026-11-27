# ExportPro

*[Version française](README.md)*

Copies a camera card's photos and videos into a dated tree, renamed `<prefix>_0001.<ext>`
in capture order. Then, on request, it moves the originals to the Trash, which can be undone.

## Status

| Part | State |
|---|---|
| Desktop app (Tauri v2, Rust core), FR/EN | MVP done, polish under way |
| Linux: AppImage, `.deb` | built on every merge to `main` |
| macOS Apple Silicon: `.dmg` | built only when asked, unsigned, not yet tried on a Mac |
| Windows, Intel/universal Mac, signing | after the MVP |
| Original bash script (`ExportPro.command`) | kept, see the end of this file |

Plan and decisions: `docs/plan/tauri-prototype.md`, `docs/adr/`.

## Using the app

1. **Source**: the card or folder (folder picker or drag and drop).
2. **Destination**: your photo folder (remembered).
3. **Shoot name**, **prefix** (`txo` by default), and an optional date. With no date, each
   file's capture date (EXIF, QuickTime) is used. If the files span several days, you pick
   one day, or one folder per day.
4. **Preview** every `original → destination`, then **Copy**. Each copy is checked (size +
   hash) and written to `exportpro.log` in the shoot folder. Originals are not touched.
5. Then, if you want, **Erase the originals** (to the Trash). **Undo** puts them back from
   the copies.

When the destination is on the same disk as the files, the preview offers **Move** (instant,
no extra space) or **Copy** (the copies take as much room as the originals). A move is undone
with one click or Ctrl+Z; in a terminal, `apply --move` prints the command that undoes it
(`exportpro undo --run <id> <shoot folder>`).

Tree layouts (`--preset`):
- `classique` (default): `YYYY/MM/DD/<shoot>/p|v/o/<ext>/`
- `lisible`: `…/photo|video/originaux/<ext>/`

The same features from a terminal:

```
exportpro plan|apply --source <card> --dest <photos> --shoot <name> [--prefix p]
                     [--date YYYY-MM-DD | --split | --no-date] [--preset classique|lisible]
                     [--move]
exportpro erase|undo <shoot folder>
exportpro undo --run <id> <shoot folder>
```

## Getting a build

Builds are kept on the GitHub run, under *Artifacts*.

- **Linux**: Actions → *build* → the latest run on `main` → `exportpro-linux`.
- **macOS (Apple Silicon)**: never runs on its own. Each build uses about 30 of the
  account's included GitHub minutes.
  - Start it: Actions → *build* → *Run workflow* → tick **macos** → *Run workflow*.
    Or from a terminal: `gh workflow run build.yml -f macos=true`.
  - Download it: the run's `exportpro-macos-arm64` artifact, or
    `gh run download <run-id> -n exportpro-macos-arm64`.
  - Install it: open the `.dmg` and drag ExportPro to *Applications*. The app is unsigned,
    so macOS blocks it the first time. Open it once, go to *System Settings > Privacy &
    Security*, then click *Open Anyway*. Or run
    `xattr -dr com.apple.quarantine /Applications/ExportPro.app`.

## Development

Checks run on the self-hosted Linux runners (`RUNNER_LABELS` variable). Only the Mac job
uses a GitHub runner.

```
make hooks                  # once per clone
make lint typecheck test-unit
make ci                     # everything, security included, in the Containerfile image (podman)
make box T=bundle           # the Linux app: target/box/release/bundle/{appimage,deb}
make bundle-macos           # on a Mac: target/release/bundle/dmg/*.dmg
```

---

## Original script: `ExportPro.command` (bash, macOS)

The starting point, still kept and maintained (`old_for_ref/` has the older versions). It sorts
and renames into:

```
<folder>/YYYY/MM/DD/<shoot name>/p/o/<ext>/     photo originals (cr2, jpg, xmp, ...)
<folder>/YYYY/MM/DD/<shoot name>/p/s/1/jpg/     empty, for corrected photos
<folder>/YYYY/MM/DD/<shoot name>/v/o/<ext>/     video originals (mov, mp4, ...)
<folder>/YYYY/MM/DD/<shoot name>/v/s/1/<ext>/   empty, one per video type found
```

Files become `<prefix>_<n>.<ext>`. Files with the same base name (`IMG_1.CR2` + `IMG_1.JPG`)
get the same number. Numbering follows shooting time and carries on after files already in
the shoot folder. The date is the earliest file's date. Every rename goes to `exportpro.log`.
Unlike the app, the script **moves** the files and does not copy them.

**How to use it on a Mac**

1. Copy `ExportPro.command` to the Mac. If needed, run `chmod +x ExportPro.command` once.
2. Double-click it. Dialogs ask for the folder, the shoot name and the prefix (`txo` by
   default), then ask you to confirm.
3. The first time, macOS may block it. On macOS 14 and earlier, right-click it and choose
   *Open*. On macOS 15 and later, go to *System Settings > Privacy & Security > Open Anyway*.
   Or run `xattr -d com.apple.quarantine ExportPro.command`.
4. For an SD card or Desktop/Documents, allow Terminal's access, or the folder looks empty.

From a terminal: `./ExportPro.command /path/to/folder`. Set `EXPORTPRO_NO_GUI=1` for terminal
questions instead of dialogs. It always runs in `/bin/bash` 3.2, which every Mac ships, Intel or
Apple Silicon, even if it's started from zsh. It needs no Python, Rosetta or signing.

Tests: `bash tests/test_exportpro.sh`, or in bash 3.2:
`podman run --rm -v "$PWD":/w:Z -w /w docker.io/library/bash:3.2 bash tests/test_exportpro.sh`
