#!/usr/bin/env sh
set -eu

if [ "$#" -lt 2 ]; then
    echo "Usage: $0 <output.pot> <source>..." >&2
    exit 1
fi

output="$1"
shift

refs_tmp="$(mktemp)"
sorted_tmp="$(mktemp)"
pot_tmp="$(mktemp)"
trap 'rm -f "$refs_tmp" "$sorted_tmp" "$pot_tmp"' EXIT INT TERM HUP

for path in "$@"; do
    LC_ALL=C grep -obPzo 'trf?\(\s*"\K(?:\\.|[^"\\])*' "$path" \
        | tr '\000' '\n' \
        | while IFS=: read -r _offset msgid; do
            [ -n "$msgid" ] || continue
            printf '%s\t%s\n' "$msgid" "$path" >> "$refs_tmp"
        done
done

LC_ALL=C sort -u "$refs_tmp" > "$sorted_tmp"

escape_po() {
    printf '%s' "$1" | sed 's/\\/\\\\/g; s/"/\\"/g'
}

{
    printf '%s\n' 'msgid ""'
    printf '%s\n' 'msgstr ""'
    printf '%s\n' '"Project-Id-Version: streamrs\n"'
    printf '%s\n' "\"POT-Creation-Date: $(date -u +%Y-%m-%d\ %H:%M%z)\\n\""
    printf '%s\n' '"MIME-Version: 1.0\n"'
    printf '%s\n' '"Content-Type: text/plain; charset=UTF-8\n"'
    printf '%s\n' '"Content-Transfer-Encoding: 8bit\n"'
    printf '\n'
} > "$pot_tmp"

if [ -s "$sorted_tmp" ]; then
    current_msgid=''
    while IFS="$(printf '\t')" read -r msgid ref; do
        [ -n "$msgid" ] || continue
        if [ "$msgid" != "$current_msgid" ]; then
            if [ -n "$current_msgid" ]; then
                escaped_msgid="$(escape_po "$current_msgid")"
                printf 'msgid "%s"\n' "$escaped_msgid" >> "$pot_tmp"
                printf 'msgstr ""\n\n' >> "$pot_tmp"
            fi
            current_msgid="$msgid"
        fi
        printf '#: %s\n' "$ref" >> "$pot_tmp"
    done < "$sorted_tmp"
    if [ -n "$current_msgid" ]; then
        escaped_msgid="$(escape_po "$current_msgid")"
        printf 'msgid "%s"\n' "$escaped_msgid" >> "$pot_tmp"
        printf 'msgstr ""\n\n' >> "$pot_tmp"
    fi
fi

# Keep the existing timestamp and references until a message is added or removed.
if [ -f "$output" ]; then
    msgcomm --unique --omit-header "$output" "$pot_tmp" > "$refs_tmp"
    if [ ! -s "$refs_tmp" ]; then
        exit 0
    fi
fi

mkdir -p "$(dirname "$output")"
mv "$pot_tmp" "$output"
