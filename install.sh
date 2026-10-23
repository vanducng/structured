#!/bin/sh
# Installs or updates structured from GitHub releases:
#   curl -fsSL https://raw.githubusercontent.com/vanducng/structured/main/install.sh | sh
# STRUCTURED_INSTALL_DIR picks the directory (default ~/.local/bin). STRUCTURED_VERSION pins a tag like v0.3.0.
set -eu

repo=vanducng/structured
dir=${STRUCTURED_INSTALL_DIR:-$HOME/.local/bin}

fail() {
  echo "structured install: $*" >&2
  exit 1
}

case "$(uname -s)/$(uname -m)" in
  Darwin/arm64) target=aarch64-apple-darwin ;;
  Linux/x86_64) target=x86_64-unknown-linux-gnu ;;
  *) fail "no release build for $(uname -s) $(uname -m); build it with: cargo install --git https://github.com/$repo" ;;
esac

tag=${STRUCTURED_VERSION:-}
if [ -z "$tag" ]; then
  latest=$(curl -fsSLI -o /dev/null -w '%{url_effective}' "https://github.com/$repo/releases/latest") ||
    fail "could not reach github.com"
  tag=${latest##*/}
fi
case "$tag" in v[0-9]*) ;; *) fail "could not find the latest release" ;; esac

if [ -x "$dir/structured" ] && [ "$("$dir/structured" --version 2>/dev/null)" = "structured ${tag#v}" ]; then
  echo "structured ${tag#v} is already installed at $dir/structured"
  exit 0
fi

archive="structured-$tag-$target.tar.gz"
tmp=$(mktemp -d)
trap 'rm -rf "$tmp"' EXIT
base="https://github.com/$repo/releases/download/$tag"
echo "downloading structured ${tag#v} for $target"
curl -fsSL -o "$tmp/$archive" "$base/$archive" || fail "could not download $archive"
curl -fsSL -o "$tmp/SHA256SUMS" "$base/SHA256SUMS" || fail "could not download SHA256SUMS"

if command -v sha256sum >/dev/null; then
  got=$(sha256sum "$tmp/$archive" | cut -d' ' -f1)
else
  got=$(shasum -a 256 "$tmp/$archive" | cut -d' ' -f1)
fi
want=$(grep " $archive\$" "$tmp/SHA256SUMS" | cut -d' ' -f1)
[ -n "$want" ] && [ "$got" = "$want" ] || fail "checksum mismatch for $archive"

tar -xzf "$tmp/$archive" -C "$tmp"
mkdir -p "$dir"
cp "$tmp/structured-$tag-$target/structured" "$dir/.structured.new"
chmod 755 "$dir/.structured.new"
mv -f "$dir/.structured.new" "$dir/structured"
echo "installed structured ${tag#v} at $dir/structured"

case ":$PATH:" in
  *":$dir:"*) ;;
  *) echo "add $dir to PATH, for example: export PATH=\"$dir:\$PATH\"" ;;
esac
