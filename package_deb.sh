#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "$0")" && pwd)"
cd "$ROOT_DIR"

VERSION="$(grep '^version' Cargo.toml | head -1 | sed -E 's/version = "([^"]+)"/\1/')"
RAW_ARCH="$(uname -m)"

case "$RAW_ARCH" in
  x86_64)
    ARCH="amd64"
    ;;
  aarch64)
    ARCH="arm64"
    ;;
  armv7l)
    ARCH="armhf"
    ;;
  *)
    echo "Unsupported architecture: $RAW_ARCH"
    exit 1
    ;;
esac

echo "Building release binary with full language support..."
cargo build --release --features full-syntax

PKG_ROOT="target/deb/notepadx_${VERSION}_${ARCH}"
DEBIAN_DIR="$PKG_ROOT/DEBIAN"
BIN_DIR="$PKG_ROOT/usr/bin"
APP_DIR="$PKG_ROOT/usr/share/applications"
PIXMAP_DIR="$PKG_ROOT/usr/share/pixmaps"

rm -rf "$PKG_ROOT"
mkdir -p "$DEBIAN_DIR" "$BIN_DIR" "$APP_DIR" "$PIXMAP_DIR"

cp target/release/notepadx "$BIN_DIR/notepadx"
cp assets/logo.png "$PIXMAP_DIR/notepadx.png"

cat > "$DEBIAN_DIR/control" <<EOF
Package: notepadx
Version: $VERSION
Section: editors
Priority: optional
Architecture: $ARCH
Maintainer: NotepadX Team <maintainers@notepadx.dev>
Depends: libgtk-3-0, libglib2.0-0, libxdo3
Description: Fast native text editor with GPU rendering
 NotepadX is a Rust-built native text editor focused on performance,
 modern UX, and large-file editing workflows.
EOF

cat > "$APP_DIR/notepadx.desktop" <<EOF
[Desktop Entry]
Type=Application
Name=NotepadX
Comment=Fast native text editor
Exec=notepadx
Icon=notepadx
Terminal=false
Categories=Utility;TextEditor;Development;
StartupNotify=true
EOF

chmod 755 "$BIN_DIR/notepadx"
chmod 644 "$DEBIAN_DIR/control" "$APP_DIR/notepadx.desktop" "$PIXMAP_DIR/notepadx.png"

OUT_FILE="target/deb/notepadx_${VERSION}_${ARCH}.deb"
dpkg-deb --build --root-owner-group "$PKG_ROOT" "$OUT_FILE"

echo "Debian package created: $OUT_FILE"
