#!/usr/bin/env bash
# Render the IP range counts table for the release notes.
#   release-notes.sh <current-stats.json> [<previous-stats.json>]
# Without previous stats (first release, or the asset is missing) it prints
# the counts with no deltas.
set -euo pipefail

cur="${1:?usage: release-notes.sh <current-stats.json> [<previous-stats.json>]}"
prev_file="${2:-}"
prev=null
if [ -n "$prev_file" ] && [ -s "$prev_file" ]; then
  prev=$(cat "$prev_file")
fi

jq -r --argjson prev "$prev" '
  def commas:
    (if . < 0 then "-" else "" end) as $sign
    | (if . < 0 then -. else . end | tostring) as $s
    | ($s | length) as $n
    | $sign + ([range(0; $n)]
        | map($s[.:.+1] + (if . < $n - 1 and ($n - . - 1) % 3 == 0 then "," else "" end))
        | join(""));
  def delta($k):
    if $prev == null or $prev[$k] == null then ""
    else (.[$k] - $prev[$k]) as $d
      | if $d > 0 then "+\($d | commas)" elif $d < 0 then ($d | commas) else "0" end
    end;
  def row($label; $k): "| \($label) | \(.[$k] | commas) | \(delta($k)) |";
  "| | Count | Change |",
  "|---|---:|---:|",
  row("IP ranges"; "entries"),
  row("IPv4 ranges"; "ipv4"),
  row("IPv6 ranges"; "ipv6"),
  row("Providers"; "providers"),
  if $prev == null then "\n_No previous release stats to compare against._"
  else "\nChange is relative to release \($prev.version // "previous")." end
' "$cur"
