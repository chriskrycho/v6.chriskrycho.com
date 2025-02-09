#!/usr/bin/env bash

# Make bash fail on first error, unset variables, and pipe failures
set -o errexit -o nounset -o pipefail

# Set field separators to newlines and tabs only, for safer word splitting in loops
IFS=$'\n\t'

RELEASES="https://github.com/chriskrycho/v6.chriskrycho.com/releases"
LATEST="${RELEASES}/latest/download/lx"
OUTPUT="lx.tgz"

download() {
  local url="$1"
  local output="$2"
  echo "fetching ${url}"

  curl --location \
    --proto '=https' --tlsv1.2 \
    --silent --show-error --fail \
    --output "$output" \
    "$url"
}

download_for_pr() {
  local sha
  sha=$(git rev-parse --short HEAD)

  local pr="${RELEASES}/lx-${sha}/download/lx-${sha}.tgz"

  local pr_result
  pr_result=$(download "$pr" $OUTPUT)

  if [[ "$pr_result" -ne 0 ]]; then
    download "$LATEST" "$OUTPUT"
  fi
}

# This works regardless of whether Render understands that a given deploy hook
# was triggered by a pull request or not.
download_for_pr || exit $?

tar --extract --gzip --file "$OUTPUT"

# build the site!
SITE_NAME="$1"
./lx build "./sites/${SITE_NAME}"
