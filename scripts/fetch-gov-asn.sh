#!/usr/bin/env bash
# Download gov-domains ASN prefix CSVs into the given directory.
# Shared by the Dockerfile and the release workflow so both build the same DB.
set -euo pipefail

out="${1:?usage: fetch-gov-asn.sh <output-dir>}"
mkdir -p "$out"
for f in fed-gov-prefixes state-gov-prefixes city-gov-prefixes \
          health-it-prefixes hospitals-prefixes insurance-prefixes \
          pbm-prefixes academic-prefixes; do
  curl -fsSL \
    "https://raw.githubusercontent.com/fartbagxp/gov-domains/main/data/asn/${f}.csv" \
    -o "${out}/${f}.csv"
done
