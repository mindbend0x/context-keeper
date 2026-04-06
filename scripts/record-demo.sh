#!/usr/bin/env bash
set -euo pipefail

# Record the Context Keeper temporal reasoning demo with asciinema.
#
# Usage:
#   ./scripts/record-demo.sh              # interactive recording
#   ./scripts/record-demo.sh --auto       # headless (runs example directly)
#
# Prerequisites: asciinema (`brew install asciinema` / `pip install asciinema`)
#
# Output: docs/demos/cli-temporal.cast

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
OUTPUT="$ROOT/docs/demos/cli-temporal.cast"

mkdir -p "$(dirname "$OUTPUT")"

echo "Building temporal_demo example..."
cargo build --example temporal_demo -p context-keeper-cli --release

BINARY="$ROOT/target/release/examples/temporal_demo"

if [[ "${1:-}" == "--auto" ]]; then
    echo "Recording (headless)..."
    asciinema rec "$OUTPUT" \
        --cols 90 \
        --rows 30 \
        --command "$BINARY" \
        --overwrite \
        --title "Context Keeper — Temporal Reasoning Demo"
else
    echo ""
    echo "Starting interactive recording."
    echo "Run:  $BINARY"
    echo "Then type 'exit' to stop recording."
    echo ""
    asciinema rec "$OUTPUT" \
        --cols 90 \
        --rows 30 \
        --overwrite \
        --title "Context Keeper — Temporal Reasoning Demo"
fi

echo ""
echo "Recording saved to: $OUTPUT"
echo ""
echo "To upload:  asciinema upload $OUTPUT"
echo "To play:    asciinema play $OUTPUT"
