#!/usr/bin/env bash
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "$0")/.." && pwd)"

# Determine target triple
if [ -n "${1:-}" ]; then
  TARGET="$1"
else
  TARGET=$(rustc -vV | awk '/^host:/ { print $2 }')
  echo "No target specified, using host: $TARGET"
fi

# Map Rust target triple to npm platform directory
case "$TARGET" in
  aarch64-apple-darwin)
    NPM_DIR="cli-darwin-arm64"
    ;;
  x86_64-apple-darwin)
    NPM_DIR="cli-darwin-x64"
    ;;
  aarch64-unknown-linux-gnu)
    NPM_DIR="cli-linux-arm64"
    ;;
  x86_64-unknown-linux-gnu)
    NPM_DIR="cli-linux-x64"
    ;;
  x86_64-pc-windows-msvc)
    NPM_DIR="cli-win32-x64"
    ;;
  aarch64-pc-windows-msvc)
    NPM_DIR="cli-win32-arm64"
    ;;
  *)
    echo "Error: unsupported target $TARGET"
    echo "Supported targets:"
    echo "  aarch64-apple-darwin      -> @revet/cli-darwin-arm64"
    echo "  x86_64-apple-darwin       -> @revet/cli-darwin-x64"
    echo "  aarch64-unknown-linux-gnu -> @revet/cli-linux-arm64"
    echo "  x86_64-unknown-linux-gnu  -> @revet/cli-linux-x64"
    echo "  x86_64-pc-windows-msvc    -> @revet/cli-win32-x64"
    echo "  aarch64-pc-windows-msvc   -> @revet/cli-win32-arm64"
    exit 1
    ;;
esac

DEST="$REPO_ROOT/npm/@revet/$NPM_DIR/bin"

echo "Building revet for $TARGET..."
cargo build --release --bin revet --target "$TARGET"

echo "Copying binary to $DEST..."
mkdir -p "$DEST"

# Windows binaries have .exe extension
case "$TARGET" in
  *-windows-*)
    cp "$REPO_ROOT/target/$TARGET/release/revet.exe" "$DEST/revet.exe"
    ;;
  *)
    cp "$REPO_ROOT/target/$TARGET/release/revet" "$DEST/revet"
    chmod +x "$DEST/revet"
    ;;
esac

echo ""
echo "Done! Binary placed in: $DEST/"
echo ""
echo "Next steps:"
echo "  cd $REPO_ROOT/npm/revet && npm pack    # Create tarball"
echo "  npx ./revet-0.1.0.tgz --help           # Test locally"
echo ""
echo "To publish (all packages):"
echo "  cd $REPO_ROOT/npm/@revet/$NPM_DIR && npm publish --access public"
echo "  cd $REPO_ROOT/npm/revet && npm publish"
