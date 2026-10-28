#!/bin/sh
# Install build tools inside this repository without changing shell profiles.
set -eu
ROOT=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
export CARGO_HOME="$ROOT/.tools/cargo"
export RUSTUP_HOME="$ROOT/.tools/rustup"
export PATH="$CARGO_HOME/bin:$ROOT/.tools/bin:$PATH"
case "$(uname -m)" in arm64) ARCH=aarch64;; x86_64) ARCH=x86_64;; *) echo 'Unsupported build architecture'; exit 1;; esac
case "$(uname -s)" in Darwin) PLATFORM=apple-darwin;; *) echo 'Use a normal Rust/Trunk installation on this platform'; exit 1;; esac
mkdir -p "$ROOT/.tools/bin"
if [ ! -x "$CARGO_HOME/bin/rustup" ]; then
  curl --proto '=https' --tlsv1.2 -fsSL "https://static.rust-lang.org/rustup/dist/$ARCH-$PLATFORM/rustup-init" -o "$ROOT/.tools/bin/rustup-init"
  chmod +x "$ROOT/.tools/bin/rustup-init"
  "$ROOT/.tools/bin/rustup-init" -y --no-modify-path --profile minimal --default-toolchain 1.99.1
fi
cd "$ROOT"
rustup toolchain install 1.99.1 --profile minimal --component rustfmt --component clippy --target wasm32-unknown-unknown
if [ ! -x "$ROOT/.tools/bin/trunk" ]; then
  curl --proto '=https' --tlsv1.2 -fsSL "https://github.com/trunk-rs/trunk/releases/download/v0.21.14/trunk-$ARCH-$PLATFORM.tar.gz" -o "$ROOT/.tools/trunk.tar.gz"
  tar xzf "$ROOT/.tools/trunk.tar.gz" -C "$ROOT/.tools/bin" trunk
  rm "$ROOT/.tools/trunk.tar.gz"
fi
# Use the CLI matching the wasm-bindgen library locked by Cargo.
VERSION=$(awk '/name = "wasm-bindgen"/{f=1;next} f && /^version = /{gsub(/"/,"",$3);print $3;exit}' "$ROOT/Cargo.lock")
if ! wasm-bindgen --version 2>/dev/null | grep -q "$VERSION"; then
  curl --proto '=https' --tlsv1.2 -fsSL "https://github.com/wasm-bindgen/wasm-bindgen/releases/download/$VERSION/wasm-bindgen-$VERSION-$ARCH-$PLATFORM.tar.gz" -o "$ROOT/.tools/wasm-bindgen.tar.gz"
  tar xzf "$ROOT/.tools/wasm-bindgen.tar.gz" -C "$ROOT/.tools" "wasm-bindgen-$VERSION-$ARCH-$PLATFORM/wasm-bindgen"
  mv "$ROOT/.tools/wasm-bindgen-$VERSION-$ARCH-$PLATFORM/wasm-bindgen" "$ROOT/.tools/bin/wasm-bindgen"
  rmdir "$ROOT/.tools/wasm-bindgen-$VERSION-$ARCH-$PLATFORM"
  rm "$ROOT/.tools/wasm-bindgen.tar.gz"
fi
echo 'Project build tools are ready.'
