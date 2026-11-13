#!/bin/sh
# Assembles what GitLab Pages publishes: the tested build of the website and, below /docs/,
# the Antora documentation. Used by the CI job `pages`; nothing is compiled here.
#
#   pages-assemble.sh <website build> <antora output> <blockers file> <out>
#
# Fails (and publishes nothing) when
# - the blockers file of `website check --report` is missing or not empty,
# - a link of the website into /docs/ has no file in the Antora output,
# - a published page loads a picture, script, frame or stylesheet from another server.
set -eu

site=$1
docs=$2
blockers=$3
out=$4

if [ ! -f "$blockers" ]; then
  echo "pages: no gate report at $blockers" >&2
  exit 1
fi
if [ -s "$blockers" ]; then
  echo "pages: the website may not be published yet:" >&2
  cat "$blockers" >&2
  exit 1
fi
if [ ! -f "$site/index.html" ] || [ ! -d "$docs" ]; then
  echo "pages: $site or $docs is missing" >&2
  exit 1
fi

rm -rf "$out"
mkdir -p "$out/docs"
cp -R "$site/." "$out/"
cp -R "$docs/." "$out/docs/"

fail=0

# every link of the website into the documentation leads to a file
links=$(grep -rhoE 'href="/docs/[^"#?]*' "$site" --include='*.html' | sed 's/^href="//' | sort -u)
for link in $links; do
  target="$out$link"
  case "$link" in */) target="${target}index.html" ;; esac
  if [ ! -f "$target" ]; then
    echo "pages: the website links to $link, which the documentation does not have" >&2
    fail=1
  fi
done

# nothing is loaded from another server (links to other sites are plain <a href>)
foreign=$(grep -rnoE '<(img|script|iframe|source|video|audio|embed)[^>]* src="(https?:)?//[^"]*|<link[^>]* rel="stylesheet"[^>]* href="(https?:)?//[^"]*|<link[^>]* href="(https?:)?//[^"]*"[^>]* rel="stylesheet"' "$out" --include='*.html' || true)
if [ -n "$foreign" ]; then
  echo "pages: published pages load from another server:" >&2
  echo "$foreign" | head -n 20 >&2
  fail=1
fi

if [ "$fail" -ne 0 ]; then
  rm -rf "$out"
  exit 1
fi
echo "pages: $(find "$out" -type f | wc -l) files in $out"
