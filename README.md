# ExportPro

Sorts and renames camera photos and videos into:

```
<folder>/YYYY/MM/DD/<shoot name>/p/o/<ext>/     photo originals (cr2, jpg, xmp, ...)
<folder>/YYYY/MM/DD/<shoot name>/p/s/1/jpg/     empty, for corrected photos
<folder>/YYYY/MM/DD/<shoot name>/v/o/<ext>/     video originals (mov, mp4, ...)
<folder>/YYYY/MM/DD/<shoot name>/v/s/1/<ext>/   empty, one per video type found
```

Files become `<prefix>_<n>.<ext>`. Files with the same base name (`IMG_1.CR2` + `IMG_1.JPG`,
`MVI_1.MOV` + `MVI_1.THM`) get the same number. Numbering follows shooting time. If the shoot
folder already has files, numbering carries on after them. The date is the earliest file's date.
Every rename is written to `exportpro.log` in the shoot folder.

## Running it on a Mac

1. Copy `ExportPro.command` to the Mac. If the file has lost its execute bit, run
   `chmod +x ExportPro.command` once.
2. Double-click it. A Terminal window opens, followed by dialogs that ask for the folder, the
   shoot name and the file prefix (`txo` by default). A last dialog asks you to confirm.
3. The first time, macOS may block the file because it was downloaded. Right-click it, choose
   *Open*, then click *Open* again.

From a terminal you can also run `./ExportPro.command /path/to/folder`. Set
`EXPORTPRO_NO_GUI=1` to get questions in the terminal instead of dialogs.

## Why plain bash

`/bin/bash` 3.2 and `osascript` come with every macOS from 10.5 (2007) to today, on both Intel
and Apple Silicon. Python was removed in macOS 12.3. A compiled C or Rust binary would need a
universal build, plus signing or a Gatekeeper workaround. This script uses only the stock tools
(`stat`, `date`, `find`, `sort`, `awk`, `mv`, `xattr`).

## Tests

```
bash tests/test_exportpro.sh
podman run --rm -v "$PWD":/w:Z -w /w docker.io/library/bash:3.2 bash tests/test_exportpro.sh
```
