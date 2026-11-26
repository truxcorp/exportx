# 0004. Configurable tree layout with path templates and presets
Date: 2026-11-26 · Status: Accepted (issue #5)
## Context
v0.2.0 hard-codes the tree: `YYYY/MM/DD/<shoot>/p|v/o/<ext>/<prefix>_<n>.<ext>`, plus
empty `p/s/1/jpg` and `v/s/1/<ext>` folders for edited files. The human wants to rename
parts of it (e.g. `p|v` → `photo|video`) and possibly reshape it.
## Options
- A: a few renamable folder names only (photo, video, originals, edited) — simple,
  but no reshaping (e.g. no date folders, `{shoot}` first, zero-padded numbers).
- B: path templates with tokens, plus built-in presets, editable in a settings screen
  with a live preview — flexible, needs validation.
- C: a free-form script / rules engine — far more than needed.
## Decision
B, accepted with two additions from the human: numbers are zero-padded by
default, and every path and part of a path is a variable, so the layout can later be
edited graphically (dragging "words": date, number, prefix, separators) cheaply. Settings file `layout.toml` in the OS config folder (`directories` crate), shown in
the app under Settings → Tree, with a preset menu, editable fields and a live preview
built from sample files.
```toml
preset = "classique"            # classique | lisible | custom

[kinds]                         # folder name per kind of file, used as {kind}
photo = "p"
video = "v"

[paths]
shoot    = "{yyyy}/{mm}/{dd}/{shoot}"          # path segments left empty are dropped
original = "{kind}/o/{ext}/{prefix}_{n:4}.{ext}" # relative to shoot; txo_0001.cr2
edited.photo = ["{kind}/s/1/jpg"]              # empty folders created after the copy
edited.video = ["{kind}/s/1/{ext}"]            # {ext}: one folder per video type found
```
Tokens: `{yyyy} {mm} {dd}` (empty when "no date"), `{shoot}`, `{prefix}`, `{kind}`,
`{ext}` (lower case), `{n}` or `{n:W}` (zero-padded to W digits; presets use `{n:4}`).
**Model, not strings:** the core parses each template into a list of parts, each a
token or a literal (`/` and separators such as `_` or `-` included), and expands that
list. The TOML strings are only one way to store it; a future drag-and-drop editor
reorders and edits the same list, with no change to the expander. Folder names per kind
(`[kinds]`), literal words (`o`, `s`, `1`) and the padding width are all settings.
Presets: **classique** = today's tree (default, so nothing changes for the current user);
**lisible** = `photo|video`, `originaux`, `retouches/1`.
Validation, before any plan is shown: `original` must end in a file name containing
`{n}` and `{ext}`; no absolute paths or `..`; every expanded segment passes the
cross-OS name rules (plan behaviour 7). Number continuation scans the shoot folder for
names matching the file-name template, padded or not (`txo_7` and `txo_0007` both count
as 7), so shoots made by v0.2.0 continue correctly. The log records the expanded paths and the
template used, so undo never depends on the current settings.
## Consequences
Core: a template parser + expander with its own unit tests (slice 2). UI: one settings
screen (slice 4). Changing the layout later doesn't move existing shoots.
