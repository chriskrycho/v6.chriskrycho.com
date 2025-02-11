#!/usr/bin/env bash

# Make bash fail on first error, unset variables, and pipe failures
set -o errexit -o nounset -o pipefail

# Set field separators to newlines and tabs only, for safer word splitting in loops
IFS=$'\n\t'

if [[ $# -eq 0 ]]; then
  echo 'usage: build.sh <site name>'
  exit 1
fi

if [[ $(uname) == "Linux" ]]; then
  cat /proc/version;
else
  uname -v;
fi

RELEASES="https://github.com/chriskrycho/v6.chriskrycho.com/releases"
LATEST="${RELEASES}/latest/download/lx-linux"
OUTPUT="lx-cli"
rm -f $OUTPUT

download() {
  local url="$1"
  local output="$2"

  echo "fetching '$url' to '$output'"

  curl --location \
    --proto '=https' --tlsv1.2 \
    --silent --show-error --fail \
    --output "$output" \
    "$url"
}

# Note for jj usage (read: local testing): make sure the repo is colocated!
download_for_pr() {
  local sha
  sha=$(git rev-parse --short HEAD)

  local pr="${RELEASES}/download/lx-${sha}/lx-linux"

  local pr_result
  pr_result=$(download "$pr" "$OUTPUT")

  local pr_exit=$?;
  echo "PR: $pr_exit $pr_result"

  if [[ "$pr_exit" -ne 0 ]]; then
    echo "falling back to latest: $LATEST"
    download $LATEST $OUTPUT
  fi
}

# This works regardless of whether Render understands that a given deploy hook
# was triggered by a pull request or not.
download_for_pr || exit $?

chmod +x $OUTPUT

# build the site!
SITE_NAME="$1"
echo "building '$SITE_NAME'"
./lx-cli publish "./sites/${SITE_NAME}" || { echo "Build failed with exit code $?"; }
rm ./lx-cli
