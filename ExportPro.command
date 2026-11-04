#!/bin/bash
#
# ExportPro v0.2.0 - offload camera photos and videos into a dated tree.
#
#   <folder>/YYYY/MM/DD/<name>/p/o/<ext>/      photo originals (cr2, jpg, ...)
#   <folder>/YYYY/MM/DD/<name>/p/s/1/jpg/      empty, for corrected photos
#   <folder>/YYYY/MM/DD/<name>/v/o/<ext>/      video originals (mov, mp4, ...)
#   <folder>/YYYY/MM/DD/<name>/v/s/1/<ext>/    empty, for corrected videos
#
# Files are renamed <prefix>_<n>.<ext> (lower-case extension). Files sharing
# a base name (IMG_0001.CR2 + IMG_0001.JPG) share the same number. Numbers
# follow shooting order (file date, then name) and continue after the highest
# number already present, so a second card can be offloaded into the same shoot.
#
# Portability: written for /bin/bash 3.2 (every macOS since 10.5) and only uses
# tools that ship with macOS: osascript, stat, date, find, sort, awk, mv, xattr.
# No Python, no Homebrew, nothing to compile. Also runs on Linux (terminal
# prompts instead of dialogs), which is how the tests run.
#
# Usage:  double-click ExportPro.command in the Finder, or
#         ./ExportPro.command [folder]
# Env:    EXPORTPRO_NO_GUI=1   ask questions in the terminal, not in dialogs.

# zsh is the default shell on macOS since 10.15 (and on every Apple Silicon
# Mac). Double-clicking or ./ExportPro.command uses the bash line above, but
# "zsh ExportPro.command", "sh ExportPro.command" or "source ExportPro.command"
# would run this file in another shell or mode. Hand it to /bin/bash instead.
# Kept to syntax that zsh, sh and bash all parse.
if [ -z "$BASH_VERSION" ]; then
    _bash=/bin/bash
    [ -x "$_bash" ] || _bash=bash
    # zsh sets $0 to this file, also when sourced. A sourced script must not
    # exec or exit: that would close the user's terminal.
    case "$ZSH_EVAL_CONTEXT" in
    *:file*)
        "$_bash" "$0" "$@"
        return $?
        ;;
    esac
    exec "$_bash" "$0" "$@"
fi
if [ "${BASH_SOURCE[0]}" != "$0" ]; then # sourced from bash
    "$BASH" "${BASH_SOURCE[0]}" "$@"
    return $?
fi
set +o posix # "sh ExportPro.command": macOS /bin/sh is bash in POSIX mode

VERSION="0.2.0"
DEFAULT_PREFIX="txo"
# Lower-case extensions treated as video (goes to v/ instead of p/).
VIDEO_EXTS="mov mp4 m4v avi mts m2ts mpg mpeg 3gp mkv wmv mxf crm braw r3d lrv"

TAB=$(printf '\t')
OS=$(uname -s)

say() { printf '%s\n' "$*"; }

use_gui() {
	[ -z "$EXPORTPRO_NO_GUI" ] && command -v osascript >/dev/null 2>&1
}

# --- dialogs (macOS) with terminal fallback --------------------------------

# ask_text MESSAGE DEFAULT -> prints the answer; status 1 if cancelled.
ask_text() {
	if use_gui; then
		osascript \
			-e 'on run argv' \
			-e 'display dialog (item 1 of argv) default answer (item 2 of argv) with title "ExportPro" buttons {"Annuler", "OK"} default button "OK" cancel button "Annuler"' \
			-e 'return text returned of result' \
			-e 'end run' "$1" "$2" 2>/dev/null
	else
		if [ -n "$2" ]; then printf '%s [%s] : ' "$1" "$2" >&2; else printf '%s : ' "$1" >&2; fi
		IFS= read -r _ans || return 1
		[ -z "$_ans" ] && _ans=$2
		printf '%s\n' "$_ans"
	fi
}

# ask_yes MESSAGE -> status 0 if the user confirms.
ask_yes() {
	if use_gui; then
		osascript \
			-e 'on run argv' \
			-e 'display dialog (item 1 of argv) with title "ExportPro" buttons {"Annuler", "Continuer"} default button "Continuer" cancel button "Annuler"' \
			-e 'end run' "$1" >/dev/null 2>&1
	else
		printf '%s\nContinuer ? (o/n) : ' "$1" >&2
		IFS= read -r _ans || return 1
		case "$_ans" in o|O|oui|Oui|OUI|y|Y|yes) return 0 ;; *) return 1 ;; esac
	fi
}

# tell MESSAGE [stop] -> informational dialog (terminal: nothing extra).
tell() {
	use_gui || return 0
	_icon=note
	[ "$2" = stop ] && _icon=stop
	osascript \
		-e 'on run argv' \
		-e "display dialog (item 1 of argv) with title \"ExportPro\" buttons {\"OK\"} default button \"OK\" with icon $_icon" \
		-e 'end run' "$1" >/dev/null 2>&1
}

# ask_folder -> prints an existing directory; status 1 if cancelled.
ask_folder() {
	if use_gui; then
		osascript -e 'on run argv' \
			-e 'return POSIX path of (choose folder with prompt (item 1 of argv))' \
			-e 'end run' "Choisissez le dossier contenant les photos et vidéos à ranger :" 2>/dev/null
		return
	fi
	say "Glissez-déposez le dossier à trier ici puis appuyez sur Entrée (\"exit\" pour quitter) :" >&2
	while :; do
		# No -r on purpose: drag & drop escapes spaces ("My\ Photos").
		# shellcheck disable=SC2162
		IFS= read _p || return 1
		_p=$(printf '%s' "$_p" | sed "s/^[[:space:]'\"]*//; s/[[:space:]'\"]*\$//")
		case "$_p" in exit|quit) return 1 ;; esac
		[ -f "$_p" ] && _p=$(dirname "$_p")
		if [ -d "$_p" ]; then printf '%s\n' "$_p"; return 0; fi
		if [ -z "$_p" ]; then say "Aucun dossier déposé, réessayez." >&2
		else say "\"$_p\" n'est pas un dossier, réessayez." >&2; fi
	done
}

die() {
	say "Erreur : $*" >&2
	tell "$*" stop
	exit 1
}

cancelled() {
	say "Annulé. Aucun fichier n'a été déplacé."
	exit 0
}

# --- helpers -----------------------------------------------------------------

lower() { printf '%s' "$1" | tr '[:upper:]' '[:lower:]'; }

trim() { printf '%s' "$1" | sed 's/^[[:space:]]*//; s/[[:space:]]*$//'; }

# A folder / file name part: not empty, no "/" or ":", not hidden.
valid_part() {
	case "$1" in
	'' | .* | */* | *:* | *"$TAB"*) return 1 ;;
	esac
	return 0
}

# file_time FILE -> epoch seconds of the earliest of creation and modification
# time. Cameras set the modification time; copies sometimes reset one of them.
file_time() {
	# shellcheck disable=SC2046 # two numbers, splitting wanted
	if [ "$OS" = Darwin ]; then
		set -- $(stat -f '%B %m' "$1" 2>/dev/null)
	else
		set -- $(stat -c '%W %Y' "$1" 2>/dev/null)
	fi
	_b=$1 _m=$2
	case "$_m" in '' | *[!0-9]*) _m=$(date +%s) ;; esac
	case "$_b" in '' | 0 | *[!0-9]*) _b=$_m ;; esac
	if [ "$_b" -lt "$_m" ]; then echo "$_b"; else echo "$_m"; fi
}

# fmt_date EPOCH FORMAT
fmt_date() {
	if [ "$OS" = Darwin ]; then date -r "$1" "+$2"; else date -d "@$1" "+$2"; fi
}

# highest_number DIR PREFIX -> highest N among DIR/**/PREFIX_N.ext (0 if none)
highest_number() {
	_max=0
	[ -d "$1" ] || { echo 0; return; }
	find "$1" -type f >"$work/existing"
	while IFS= read -r _f; do
		_n=${_f##*/}
		case "$_n" in
		"$2"_*.*)
			_n=${_n#"$2"_}
			_n=${_n%%.*}
			case "$_n" in '' | *[!0-9]*) ;; *) [ "$_n" -gt "$_max" ] && _max=$_n ;; esac
			;;
		esac
	done <"$work/existing"
	echo "$_max"
}

# --- main ----------------------------------------------------------------------

main() {
	say ""
	say "ExportPro v$VERSION"
	say "Range et renomme photos et vidéos dans annee/mois/jour/nom/."
	say ""

	# 1. Source folder
	src=$1
	if [ -z "$src" ]; then
		src=$(ask_folder) || cancelled
	fi
	[ -f "$src" ] && src=$(dirname "$src")
	[ -d "$src" ] || die "\"$src\" n'est pas un dossier."
	src=$(cd "$src" && pwd) || die "Impossible d'ouvrir \"$src\"."
	say "Dossier : $src"

	work=$(mktemp -d "${TMPDIR:-/tmp}/exportpro.XXXXXX") || die "Impossible de créer un dossier temporaire."
	trap 'rm -rf "$work"' EXIT

	# 2. Scan top-level files: key(lower base) time ext(lower) name
	: >"$work/files"
	for f in "$src"/*; do
		[ -f "$f" ] || continue
		[ "$f" -ef "$0" ] && continue
		name=${f##*/}
		case "$name" in .* | *"$TAB"*) continue ;; *?.?*) ;; *) continue ;; esac
		base=${name%.*}
		ext=$(lower "${name##*.}")
		printf '%s\t%s\t%s\t%s\n' "$(lower "$base")" "$(file_time "$f")" "$ext" "$name" >>"$work/files"
	done
	total=$(wc -l <"$work/files" | tr -d ' ')
	[ "$total" -gt 0 ] || die "Aucun fichier à ranger dans \"$src\"."

	# 3. Group by base name; a group is video if any member is a video file.
	#    Sort groups by their earliest time, then by name.
	awk -F'\t' -v vids=" $VIDEO_EXTS " '
		{
			key[NR] = $1; ext[NR] = $3; nm[NR] = $4
			if (!($1 in gmin) || $2 + 0 < gmin[$1] + 0) gmin[$1] = $2
			if (index(vids, " " $3 " ")) vid[$1] = 1
		}
		END {
			for (i = 1; i <= NR; i++)
				printf "%s\t%s\t%d\t%s\t%s\n", gmin[key[i]], key[i], (key[i] in vid), ext[i], nm[i]
		}' "$work/files" | sort -t "$TAB" -k1,1n -k2,2 -k5,5 >"$work/sorted"

	first=$(head -1 "$work/sorted" | cut -f1)
	y=$(fmt_date "$first" %Y) m=$(fmt_date "$first" %m) d=$(fmt_date "$first" %d)
	exts=$(cut -f4 "$work/sorted" | sort -u | tr '\n' ' ')
	say "$total fichier(s) trouvé(s), types : $exts"
	say "Date de la séance : $y/$m/$d"

	# 4. Shoot name and file prefix
	while :; do
		shoot=$(ask_text "Nom de la séance (dossier $y/$m/$d/<nom>) :" "") || cancelled
		shoot=$(trim "$shoot")
		valid_part "$shoot" && break
		say "Nom invalide : il ne doit pas être vide, commencer par \".\" ni contenir \"/\" ou \":\"." >&2
		tell "Nom invalide : il ne doit pas être vide, commencer par \".\" ni contenir \"/\" ou \":\"." stop
	done
	while :; do
		prefix=$(ask_text "Racine des noms de fichiers (ex. ${DEFAULT_PREFIX}_1.jpg) :" "$DEFAULT_PREFIX") || cancelled
		prefix=$(trim "$prefix")
		[ -z "$prefix" ] && prefix=$DEFAULT_PREFIX
		valid_part "$prefix" && break
		say "Racine invalide : elle ne doit pas commencer par \".\" ni contenir \"/\" ou \":\"." >&2
		tell "Racine invalide : elle ne doit pas commencer par \".\" ni contenir \"/\" ou \":\"." stop
	done

	dest="$src/$y/$m/$d/$shoot"
	offset=$(highest_number "$dest" "$prefix")

	# 5. Plan: name -> relative destination
	: >"$work/plan"
	n=$offset prev="" photos=0 videos=0
	while IFS="$TAB" read -r _ key isv ext name; do
		if [ "$key" != "$prev" ]; then
			n=$((n + 1)) prev=$key
			if [ "$isv" = 1 ]; then videos=$((videos + 1)); else photos=$((photos + 1)); fi
		fi
		if [ "$isv" = 1 ]; then cat=v; else cat=p; fi
		rel="$cat/o/$ext/${prefix}_$n.$ext"
		[ -e "$dest/$rel" ] && die "\"$dest/$rel\" existe déjà, rien n'a été déplacé."
		printf '%s\t%s\n' "$name" "$rel" >>"$work/plan"
	done <"$work/sorted"

	summary="$total fichier(s) ($photos photo(s), $videos vidéo(s)) vont être renommés ${prefix}_$((offset + 1)) à ${prefix}_$n et déplacés dans :

$dest"
	[ "$offset" -gt 0 ] && summary="$summary

Ce dossier contient déjà ${prefix}_1 à ${prefix}_$offset : la numérotation continue."
	say ""
	ask_yes "$summary" || cancelled

	# 6. Move
	mkdir -p "$dest" || die "Impossible de créer \"$dest\"."
	log="$dest/exportpro.log"
	printf '# ExportPro %s - %s - depuis %s\n' "$VERSION" "$(date '+%Y-%m-%d %H:%M:%S')" "$src" >>"$log"
	ok=0 failed=0
	while IFS="$TAB" read -r name rel; do
		if mkdir -p "$dest/${rel%/*}" && [ ! -e "$dest/$rel" ] &&
			mv -n "$src/$name" "$dest/$rel" && [ -e "$dest/$rel" ] && [ ! -e "$src/$name" ]; then
			ok=$((ok + 1))
			say "$name -> $rel"
			printf '%s -> %s\n' "$name" "$rel" >>"$log"
		else
			failed=$((failed + 1))
			say "ECHEC : $name n'a pas été déplacé." >&2
			printf 'ECHEC %s -> %s\n' "$name" "$rel" >>"$log"
		fi
	done <"$work/plan"

	# 7. Empty trees waiting for the corrected files
	[ "$photos" -gt 0 ] && mkdir -p "$dest/p/s/1/jpg"
	# shellcheck disable=SC2013 # extensions have no spaces
	for e in $(awk -F'\t' '$3 == 1 { print $4 }' "$work/sorted" | sort -u); do
		case " $VIDEO_EXTS " in *" $e "*) mkdir -p "$dest/v/s/1/$e" ;; esac
	done

	# 8. Strip extended attributes (v0.1.2 fix: files made unusable, "xattr 164")
	if [ "$OS" = Darwin ] && command -v xattr >/dev/null 2>&1; then
		xattr -c -r "$dest" 2>/dev/null
	fi

	say ""
	if [ "$failed" -gt 0 ]; then
		die "$ok fichier(s) rangé(s), $failed en échec (voir $log)."
	fi
	say "Terminé : $ok fichier(s) rangé(s) dans $dest"
	if use_gui; then
		osascript \
			-e 'on run argv' \
			-e 'display dialog (item 1 of argv) with title "ExportPro" buttons {"OK", "Ouvrir"} default button "Ouvrir"' \
			-e 'return button returned of result' \
			-e 'end run' "Terminé : $ok fichier(s) rangé(s) dans
$dest" 2>/dev/null | grep -q Ouvrir && open "$dest"
	fi
	exit 0
}

main "$@"
