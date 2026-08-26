#!/usr/bin/env bash
set -euo pipefail

repo_root="$(git rev-parse --show-toplevel)"
manifest="$repo_root/omniphony-renderer/current-listening-baseline.manifest"
realtime="$repo_root/omniphony-renderer/realtime_ffi/src/lib.rs"

fail=0

while read -r expected path; do
  [[ -z "${expected:-}" || "${expected:0:1}" == "#" ]] && continue
  if [[ ! -f "$repo_root/$path" ]]; then
    echo "::error::protected Current baseline file is missing: $path"
    fail=1
    continue
  fi
  actual="$(git -C "$repo_root" hash-object "$path")"
  if [[ "$actual" != "$expected" ]]; then
    echo "::error file=$path::protected Current listening baseline changed: expected $expected, got $actual"
    fail=1
  fi
done < "$manifest"

require_literal() {
  local literal="$1"
  if ! grep -Fqx "$literal" "$realtime"; then
    echo "::error file=omniphony-renderer/realtime_ffi/src/lib.rs::protected Current output scalar changed or moved: $literal"
    fail=1
  fi
}

# These scalars live in the larger realtime transport module, so pin their
# values explicitly instead of hashing that whole implementation file.
require_literal 'const FIELD_SUPPORT_GAIN: f32 = 1.0;'
require_literal 'const LINEAR_OUTPUT_GAIN: f32 = 0.90;'
require_literal 'const OUTPUT_MAKEUP_GAIN: f32 = 1.380_384_3;'
require_literal 'const OUTPUT_CEILING: f32 = 0.891_250_9;'
require_literal 'const OUTPUT_LOOKAHEAD_MS: usize = 5;'
require_literal 'const OUTPUT_RELEASE_MS: f32 = 160.0;'

if (( fail != 0 )); then
  echo
  echo "Current listening baseline drift detected."
  echo "If physical listening intentionally promoted a new baseline, update"
  echo "current-listening-baseline.manifest and/or the scalar expectations"
  echo "in this script in the same commit as that promotion."
  exit 1
fi

echo "Protected Current listening baseline matches."
