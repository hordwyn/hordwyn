#!/bin/sh
# Writes THIRD-PARTY-NOTICES.txt: the licence notices of everything third-party the image
# ships (issue 9, DEC-855): the Rust crates (cargo-about, src/rust/about.toml), the vendored
# icons, the brand fonts and the Tailwind base styles. Fails when any part cannot be made,
# so the image build and the CI job `notices` fail with it.
# usage: notices.sh <out file>   needs cargo and cargo-about (ci-tools.sh about)
set -eu
[ $# -eq 1 ] || { echo "usage: notices.sh <out file>" >&2; exit 2; }
root=$(cd "$(dirname "$0")/../.." && pwd)
out=$1
tmp="$out.part"
trap 'rm -f "$tmp"' EXIT
rm -f "$tmp"
section() {
  printf '\n==============================================================================\n%s\n==============================================================================\n\n' "$1"
}
{
  echo "Third-party notices"
  echo
  echo "This program contains software and other works of third parties. Their licences ask that"
  echo "the copyright and licence notices below travel with every copy. The program itself is"
  echo "under the licence in LICENSE of its source repository. The packages of the base image"
  echo "keep their own notices under /usr/share/doc."
  section "Rust crates"
  cargo about -L error generate \
    --manifest-path "$root/src/rust/Cargo.toml" \
    --config "$root/src/rust/about.toml" \
    --workspace --all-features --fail \
    "$root/src/rust/notices/about.hbs"
  for dir in "$root"/src/rust/crates/ui/icons/vendor/*/; do
    set_name=$(basename "$dir")
    [ -s "$dir/LICENSE" ] || { echo "notices: $dir has no LICENSE" >&2; exit 1; }
    section "Icons: $set_name (src/rust/crates/ui/icons/vendor/VERSIONS)"
    cat "$dir/LICENSE"
  done
  found=0
  for f in "$root"/config/brand/*/fonts/OFL.txt "$root"/config/brand/*/fonts/LICENSE*; do
    [ -s "$f" ] || continue
    found=1
    section "Fonts: ${f#"$root"/}"
    cat "$f"
  done
  [ "$found" = 1 ] || { echo "notices: no font licence under config/brand" >&2; exit 1; }
  section "Style sheet"
  cat "$root/src/rust/notices/tailwindcss.txt"
} > "$tmp"
# a sanity floor: hundreds of crates make this far longer
[ "$(wc -l < "$tmp")" -gt 1000 ] || { echo "notices: $tmp is too short" >&2; exit 1; }
mv "$tmp" "$out"
echo "notices: wrote $out ($(wc -l < "$out") lines)"
