# Capture-date fixtures

| File | Origin | Stored date |
|---|---|---|
| `camera.cr3` | Canon EOS R6, `testdata/canon-r6.cr3` of [nom-exif](https://github.com/mindeng/nom-exif) (MIT) | EXIF DateTimeOriginal 2025-09-06 20:24:47 (camera-local, -08:00); QuickTime 2025-09-07 04:24:47 UTC |
| `clip.mov` | `testdata/auth.mov` of nom-exif (MIT) | QuickTime creation 2025-02-24 10:11:57 UTC |
| `clip.mp4` | Sony A7 XAVC, `testdata/sony-a7-xavc.MP4` of nom-exif (MIT) | QuickTime creation 2026-04-26 09:25:15 UTC |
| `photo.jpg` | generated: ImageMagick 8×8 gray, exiftool | DateTimeOriginal 2021-06-15 23:50:00 |
| `future.jpg` | `photo.jpg`, exiftool | DateTimeOriginal 2099-06-01 10:00:00 (a camera clock set in the future) |
| `photo.heic` | generated: heif-enc (x265) 64×64 gray, exiftool | DateTimeOriginal 2022-01-02 03:04:05 |
| `photo.cr2` | generated: uncompressed TIFF, exiftool | DateTimeOriginal 2019-11-30 08:00:00 |
| `photo.nef` | generated: uncompressed TIFF, exiftool | DateTimeOriginal 2019-12-01 09:00:00 |
| `photo.arw` | generated: uncompressed TIFF, exiftool | DateTimeOriginal 2019-12-02 10:00:00 |
| `photo.dng` | generated: uncompressed TIFF, exiftool | DateTimeOriginal 2019-12-03 11:00:00 |

CR2, NEF, ARW and DNG are TIFF containers; the generated ones carry only the TIFF/EXIF
structure, not a camera's maker notes or image data. Dates were read back with
`exiftool -api QuickTimeUTC=0` (exiftool 13, Ubuntu 26.04). Real camera samples of these
formats are welcome.
