#!/bin/sh
# Package an offline transcriber and model. Downloads happen at build time only.
set -eu
ROOT=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
PROFILE=${1:-debug}
case "$PROFILE" in debug|release) ;; *) echo 'Usage: scripts/build-transcription.sh [debug|release]'; exit 1;; esac
case "$(uname -s)" in Darwin) ;; *) echo 'Bundled transcription packaging currently requires macOS'; exit 1;; esac
CACHE="$ROOT/.tools/transcription"
DEST="$ROOT/target/$PROFILE/transcription"
mkdir -p "$CACHE" "$DEST"

fetch() {
  url=$1 file=$2 digest=$3
  if [ -f "$file" ] && printf '%s  %s\n' "$digest" "$file" | shasum -a 256 -c - >/dev/null 2>&1; then return; fi
  echo "Downloading $(basename "$file")"
  curl --proto '=https' --tlsv1.2 --fail --location --retry 3 "$url" -o "$file.partial"
  printf '%s  %s\n' "$digest" "$file.partial" | shasum -a 256 -c -
  mv "$file.partial" "$file"
}

# Known release archive and checksum from Kitware's release metadata. Keep the
# build tool local; the installed app does not contain or require CMake.
if command -v cmake >/dev/null 2>&1; then
  CMAKE=$(command -v cmake)
else
  CMAKE="$CACHE/cmake-4.1.2-macos-universal/CMake.app/Contents/bin/cmake"
  if [ ! -x "$CMAKE" ]; then
    fetch 'https://github.com/Kitware/CMake/releases/download/v4.1.2/cmake-4.1.2-macos-universal.tar.gz' \
      "$CACHE/cmake-4.1.2.tar.gz" '3be85f5b999e327b1ac7d804cbc9acd767059e9f603c42ec2765f6ab68fbd367'
    tar xzf "$CACHE/cmake-4.1.2.tar.gz" -C "$CACHE"
  fi
fi

# whisper.cpp v1.9.5, pinned to the signed release's commit and archive hash.
COMMIT=d1be6fde11ac6e0407606b4e42fe72d34add8037
SOURCE="$CACHE/whisper.cpp-$COMMIT"
fetch "https://codeload.github.com/ggml-org/whisper.cpp/tar.gz/$COMMIT" \
  "$CACHE/whisper-$COMMIT.tar.gz" '645445da884df6dbd52b30f82d7be7c01d1767c938714e477e786b88d529e9f4'
if [ ! -f "$SOURCE/CMakeLists.txt" ]; then tar xzf "$CACHE/whisper-$COMMIT.tar.gz" -C "$CACHE"; fi
BUILD="$CACHE/build-$COMMIT-$(uname -m)"
"$CMAKE" -S "$SOURCE" -B "$BUILD" -DCMAKE_BUILD_TYPE=Release \
  -DCMAKE_OSX_DEPLOYMENT_TARGET=12.0 -DBUILD_SHARED_LIBS=OFF \
  -DGGML_NATIVE=OFF -DGGML_METAL=OFF -DGGML_ACCELERATE=ON -DGGML_BLAS=OFF \
  -DGGML_CCACHE=OFF -DWHISPER_BUILD_IS_DEV=OFF \
  -DWHISPER_BUILD_TESTS=OFF -DWHISPER_BUILD_EXAMPLES=ON -DWHISPER_BUILD_SERVER=OFF
"$CMAKE" --build "$BUILD" --config Release --target whisper-cli --parallel 4
cp "$BUILD/bin/whisper-cli" "$DEST/whisper-cli"
chmod 755 "$DEST/whisper-cli"

# Multilingual Base: 147,951,465 bytes, pinned Hugging Face revision and LFS SHA.
fetch 'https://huggingface.co/ggerganov/whisper.cpp/resolve/5359861c739e955e79d9a303bcbc70fb988958b1/ggml-base.bin' \
  "$CACHE/ggml-base.bin" '60ed5bc3dd14eea856493d334349b405782ddcaf0028d4b5df4088345fba2efe'
cp "$CACHE/ggml-base.bin" "$DEST/ggml-base.bin"
cp "$SOURCE/LICENSE" "$DEST/LICENSE-whisper.cpp.txt"
fetch 'https://raw.githubusercontent.com/openai/whisper/v20250625/LICENSE' \
  "$CACHE/LICENSE-whisper-model.txt" 'b5d65a59060e68c4ff940e1eddfa6f94b2d68fdf58ed7f4dd57721c997e35e9d'
cp "$CACHE/LICENSE-whisper-model.txt" "$DEST/LICENSE-whisper-model.txt"
cat > "$DEST/assets.json" <<'JSON'
{"engine":"whisper.cpp","version":"1.9.5","commit":"d1be6fde11ac6e0407606b4e42fe72d34add8037","model":"Whisper Base multilingual","model_sha256":"60ed5bc3dd14eea856493d334349b405782ddcaf0028d4b5df4088345fba2efe","backend":"CPU with Apple Accelerate","model_license":"MIT"}
JSON
echo "Bundled offline transcription is ready in $DEST"
