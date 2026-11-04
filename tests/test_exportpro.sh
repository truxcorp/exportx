#!/bin/bash
# End-to-end tests for ExportPro.command (terminal mode, no dialogs).
# Usage: bash tests/test_exportpro.sh   (runs with whatever bash runs it)

here=$(cd "$(dirname "$0")" && pwd)
script="$here/../ExportPro.command"
pass=0 fail=0
export EXPORTPRO_NO_GUI=1 TZ=UTC

tmp=$(mktemp -d "${TMPDIR:-/tmp}/exportpro-test.XXXXXX")
trap 'rm -rf "$tmp"' EXIT

# mk DIR STAMP FILE... : create files with mtime STAMP (CCYYMMDDhhmm)
mk() {
	_d=$1 _t=$2
	shift 2
	mkdir -p "$_d"
	for _f in "$@"; do
		printf '%s' "$_f" >"$_d/$_f"
		touch -t "$_t" "$_d/$_f"
	done
}

# run DIR ANSWERS : run the script, answers separated by newlines
run() { printf '%s' "$2" | bash "$script" "$1" >"$tmp/out" 2>&1; }

check() { # DESCRIPTION TEST...
	_desc=$1
	shift
	if "$@"; then pass=$((pass + 1)); else
		fail=$((fail + 1))
		say_fail "$_desc"
	fi
}
say_fail() { printf 'FAIL: %s\n' "$1"; sed 's/^/    | /' "$tmp/out"; }
has() { [ -f "$1" ] && [ "$(cat "$1")" = "$2" ]; }    # FILE ORIGINAL-NAME
emptydir() { [ -d "$1" ] && [ -z "$(ls -A "$1")" ]; }

# --- 1. photo pairs share a number, order follows time, date tree ------------
s="$tmp/t1"
mk "$s" 201810231430 IMG_0002.CR2 IMG_0002.JPG
mk "$s" 201810231410 IMG_0009.CR2 IMG_0009.JPG # earlier although higher number
mk "$s" 201810231420 IMG_0005.JPG
run "$s" "mariage
dupont
o
"
check "exit 0" [ $? -eq 0 ]
D="$s/2018/10/23/mariage"
check "pair 1 cr2" has "$D/p/o/cr2/dupont_1.cr2" IMG_0009.CR2
check "pair 1 jpg" has "$D/p/o/jpg/dupont_1.jpg" IMG_0009.JPG
check "single 2" has "$D/p/o/jpg/dupont_2.jpg" IMG_0005.JPG
check "pair 3 cr2" has "$D/p/o/cr2/dupont_3.cr2" IMG_0002.CR2
check "pair 3 jpg" has "$D/p/o/jpg/dupont_3.jpg" IMG_0002.JPG
check "p/s/1/jpg empty" emptydir "$D/p/s/1/jpg"
check "no video tree" [ ! -e "$D/v" ]
check "sources gone" [ ! -e "$s/IMG_0002.CR2" ]
check "log written" grep -q "IMG_0009.CR2 -> p/o/cr2/dupont_1.cr2" "$D/exportpro.log"

# --- 2. videos, sidecars follow their video, other suffixes get a folder -----
s="$tmp/t2"
mk "$s" 202403050900 A.CR2 A.JPG
mk "$s" 202403050905 MVI_1.MOV MVI_1.THM
mk "$s" 202403050910 clip.mp4 notes.xmp
run "$s" "concert
txo
o
"
D="$s/2024/03/05/concert"
check "photo" has "$D/p/o/cr2/txo_1.cr2" A.CR2
check "mov" has "$D/v/o/mov/txo_2.mov" MVI_1.MOV
check "thm with its video" has "$D/v/o/thm/txo_2.thm" MVI_1.THM
check "mp4 (name order: clip before notes)" has "$D/v/o/mp4/txo_3.mp4" clip.mp4
check "xmp gets own folder" has "$D/p/o/xmp/txo_4.xmp" notes.xmp
check "v/s/1/mov" emptydir "$D/v/s/1/mov"
check "v/s/1/mp4" emptydir "$D/v/s/1/mp4"
check "no v/s/1/thm" [ ! -e "$D/v/s/1/thm" ]
check "p/s/1/jpg" emptydir "$D/p/s/1/jpg"

# --- 3. default prefix, ignored entries, spaces in names ---------------------
s="$tmp/t3 with space"
mk "$s" 202001011200 "my photo.JPG" .DS_Store README
mkdir -p "$s/sub" && mk "$s/sub" 202001011200 deep.jpg
run "$s" "shoot one

o
"
D="$s/2020/01/01/shoot one"
check "default prefix txo" has "$D/p/o/jpg/txo_1.jpg" "my photo.JPG"
check "hidden untouched" [ -f "$s/.DS_Store" ]
check "no-ext untouched" [ -f "$s/README" ]
check "subdir untouched" [ -f "$s/sub/deep.jpg" ]

# --- 4. second card into the same shoot continues numbering ------------------
s="$tmp/t4"
mk "$s" 202105051000 a.jpg b.jpg
run "$s" "expo
x
o
"
mk "$s" 202105051100 c.jpg
run "$s" "expo
x
o
"
D="$s/2021/05/05/expo"
check "first card kept" has "$D/p/o/jpg/x_2.jpg" b.jpg
check "second card continues" has "$D/p/o/jpg/x_3.jpg" c.jpg

# --- 5. refusing the confirmation moves nothing ------------------------------
s="$tmp/t5"
mk "$s" 202202021000 a.jpg
run "$s" "nope
x
n
"
check "cancel exit 0" [ $? -eq 0 ]
check "cancel: file stays" [ -f "$s/a.jpg" ]
check "cancel: no tree" [ ! -e "$s/2022" ]

# --- 6. invalid shoot names are asked again ----------------------------------
s="$tmp/t6"
mk "$s" 202202021000 a.jpg
run "$s" "a/b
.hidden

good
x
o
"
check "re-asked until valid" has "$s/2022/02/02/good/p/o/jpg/x_1.jpg" a.jpg

# --- 7. empty folder -> error, nothing created -------------------------------
s="$tmp/t7"
mkdir -p "$s"
run "$s" ""
check "empty folder fails" [ $? -ne 0 ]
check "empty folder untouched" [ -z "$(ls -A "$s")" ]

# --- 8. folder asked in terminal when not given (drag & drop style) ----------
s="$tmp/t8 dir"
mk "$s" 202303031000 z.jpg
printf '%s\n' "$(printf '%s' "$s" | sed 's/ /\\ /g') " "s8" "x" "o" |
	bash "$script" >"$tmp/out" 2>&1
check "drag & drop path" has "$s/2023/03/03/s8/p/o/jpg/x_1.jpg" z.jpg

printf '\n%d passed, %d failed (%s)\n' "$pass" "$fail" "$(bash --version | head -1)"
[ "$fail" -eq 0 ]
