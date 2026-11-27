# ExportX brand kit

Offload and order your raw rushes.

## Colours
| Name | Hex | Use |
|---|---|---|
| Ink | `#111418` | App icon background, text on light backgrounds |
| Paper | `#F4F2EE` | Text and lines on dark backgrounds |
| Mustard | `#FFB020` | Accent on dark backgrounds |
| Deep mustard | `#B86E00` | Accent on light backgrounds (bright mustard is too faint on white) |

Typeface: Space Grotesk (Medium for "Export", Bold for "X"). In every logo file the text is converted to shapes, so no font needs to be installed.

## 01-app-icons
- `windows/ExportX.ico`: 16–256 px, for the .exe and installer. `png/` has 44, 150 and 310 px tiles for the Microsoft Store and Start menu.
- `macos/ExportX.icns`: set it as `CFBundleIconFile`. `ExportX.iconset/` has every PNG size; on a Mac, `iconutil -c icns ExportX.iconset` rebuilds a complete .icns.
- `linux/hicolor/`: copy into `/usr/share/icons/hicolor/` (or `~/.local/share/icons/hicolor/`), then use `Icon=exportx` in the `.desktop` file. Includes `scalable/` (SVG) and `symbolic/` (one-colour version, for GNOME).
- `web/`: favicon (.ico + .svg), apple-touch-icon and 192/512 px PNGs for a website.

At 32 px and below, the icon switches to a simpler two-card version so it stays readable.

## 02-logos (transparent background)
Each logo comes as the full logo (icon + name) and as the icon alone, in SVG and in PNG @2x/@4x:
- `light-theme`: for light backgrounds
- `dark-theme`: for dark backgrounds
- `mono-black` / `mono-white`: one colour, for watermarks, print, engraving
- `adaptive` (SVG only): switches by itself with the system theme where the SVG is shown by a browser engine (Electron, Tauri, web views, websites)

Leave clear space around the logo at least as wide as the front card.

## 03-tray-icons
- `macos/ExportXTemplate-*.png`: menu bar "template" images (black + transparency). Keep "Template" at the end of the file name, or call `setTemplate(true)`, and macOS will recolour them for light/dark mode. 18 pt is the usual size; 16 and 22 pt are also included.
- `windows/`: white version for the dark taskbar (the Windows 11 default), black for a light taskbar, plus a colour version. Pick based on the `SystemUsesLightTheme` setting.
- `linux/`: white and black PNGs, plus a `-symbolic.svg` that GNOME/KDE recolour to fit the panel theme.

## 04-source
Master SVGs, plus `generate.py`, the script that produced every file in this kit.

## In this repository
- The app icons from `01-app-icons` are in `crates/app/icons/` (Linux/Windows PNGs and `.ico`, macOS `.icns`), listed in `tauri.conf.json`.
- The header logos (`exportx-logo-light-theme.svg`, `exportx-logo-dark-theme.svg`) are served from `ui/brand/`.
- Kept here: the logos (`02-logos`), the master SVGs (`04-source`, without `generate.py`) and `preview.png`. The tray icons and web favicons aren't used by the app yet.
