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

echo "fetching '$LATEST' to '$OUTPUT'"
curl --location \
    --proto '=https' --tlsv1.2 \
    --silent --show-error --fail \
    --output "$OUTPUT" \
    "$LATEST" \
  || exit $?

chmod +x $OUTPUT

# build the site!
SITE_NAME="$1"
echo "building '$SITE_NAME'"
./lx-cli publish "./sites/${SITE_NAME}" || { echo "Build failed with exit code $?"; }
rm ./lx-cli
