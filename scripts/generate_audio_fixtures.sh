#!/usr/bin/env bash

set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
OUT="$ROOT/tests/fixtures/audio"

mkdir -p "$OUT"

if ! command -v ffmpeg >/dev/null 2>&1; then
    echo "error: ffmpeg is required to generate fixtures"
    exit 1
fi

echo "Generating VDISC audio fixtures..."

COMMON_INPUT=(
    -f lavfi
    -i "sine=frequency=440:sample_rate=48000:duration=1"
    -ac 2
)

COMMON_METADATA=(
    -metadata title="VDISC Matrix Fixture"
    -metadata artist="VDISC Test Artist"
    -metadata album="VDISC Compatibility Matrix"
)

ffmpeg \
    -hide_banner \
    -loglevel error \
    -y \
    "${COMMON_INPUT[@]}" \
    "${COMMON_METADATA[@]}" \
    -c:a pcm_s16le \
    "$OUT/valid.wav"

ffmpeg \
    -hide_banner \
    -loglevel error \
    -y \
    "${COMMON_INPUT[@]}" \
    "${COMMON_METADATA[@]}" \
    -c:a flac \
    "$OUT/valid.flac"

ffmpeg \
    -hide_banner \
    -loglevel error \
    -y \
    "${COMMON_INPUT[@]}" \
    "${COMMON_METADATA[@]}" \
    -c:a libmp3lame \
    -q:a 2 \
    "$OUT/valid.mp3"

ffmpeg \
    -hide_banner \
    -loglevel error \
    -y \
    "${COMMON_INPUT[@]}" \
    "${COMMON_METADATA[@]}" \
    -c:a libopus \
    -b:a 128k \
    "$OUT/valid.opus"

echo
echo "Generated:"
ls -lh \
    "$OUT/valid.wav" \
    "$OUT/valid.flac" \
    "$OUT/valid.mp3" \
    "$OUT/valid.opus"

echo
echo "SHA-256:"
sha256sum \
    "$OUT/valid.wav" \
    "$OUT/valid.flac" \
    "$OUT/valid.mp3" \
    "$OUT/valid.opus"
