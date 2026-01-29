#!/bin/bash
# Compare STT configurations using pairwise testing
# Total: 11 tests per file (9 Whisper + 2 TDT)
#
# Usage: ./scripts/compare_stt.sh [file1.wav] [file2.wav]
# If no files specified, uses default recordings from ~/.local/share/voice-dictation/recordings/

set -o pipefail

RECORDINGS_DIR="${RECORDINGS_DIR:-$HOME/.local/share/voice-dictation/recordings}"
OUTPUT_DIR="${OUTPUT_DIR:-./stt_comparison_results}"
BIN="${BIN:-./target/release/voice-dictation}"

# Default test files
DEFAULT_FILES=(
    "conference_2026-01-29_15-53-31.wav"
    "conference_2026-01-29_15-59-29.wav"
)

# Use provided files or defaults
if [ $# -gt 0 ]; then
    FILES=("$@")
else
    FILES=("${DEFAULT_FILES[@]}")
fi

# Pairwise covering array for Whisper: model, diarization, denoise
# Covers all 2-way combinations with minimum tests (9 instead of 18)
#
# | # | Model           | Diarization | Denoise |
# |---|-----------------|-------------|---------|
# | 1 | base            | none        | off     |
# | 2 | base            | channel     | on      |
# | 3 | base            | sortformer  | on      |
# | 4 | small-q8_0      | none        | on      |
# | 5 | small-q8_0      | channel     | off     |
# | 6 | small-q8_0      | sortformer  | off     |
# | 7 | medium          | none        | on      |
# | 8 | medium          | channel     | off     |
# | 9 | medium          | sortformer  | on      |
#
# Coverage verification:
# - Model x Diarization: all 9 pairs covered
# - Model x Denoise: all 6 pairs covered
# - Diarization x Denoise: all 6 pairs covered

WHISPER_TESTS=(
    "ggml-base.bin:none:off"
    "ggml-base.bin:channel:on"
    "ggml-base.bin:sortformer:on"
    "ggml-small-q8_0.bin:none:on"
    "ggml-small-q8_0.bin:channel:off"
    "ggml-small-q8_0.bin:sortformer:off"
    "ggml-medium.bin:none:on"
    "ggml-medium.bin:channel:off"
    "ggml-medium.bin:sortformer:on"
)

# TDT tests (no diarization support - TDT is pure STT)
# denoise=on is MANDATORY for TDT
TDT_TESTS=(
    "on"   # MANDATORY
    "off"
)

# Check binary exists
if [ ! -f "$BIN" ]; then
    echo "Error: Binary not found at $BIN"
    echo "Run: cargo build --release --features sortformer"
    exit 1
fi

mkdir -p "$OUTPUT_DIR"
echo "Starting STT comparison at $(date)" | tee "$OUTPUT_DIR/log.txt"
echo "Binary: $BIN" | tee -a "$OUTPUT_DIR/log.txt"
echo "Output dir: $OUTPUT_DIR" | tee -a "$OUTPUT_DIR/log.txt"
echo "" | tee -a "$OUTPUT_DIR/log.txt"

run_test() {
    local file="$1"
    local backend="$2"
    local model="$3"
    local diar="$4"
    local denoise="$5"

    local base
    base=$(basename "$file" .wav)
    local model_short
    model_short=$(echo "$model" | sed 's/ggml-//;s/.bin//')
    local denoise_suffix=""
    [ "$denoise" == "on" ] && denoise_suffix="_denoise"

    local output="${OUTPUT_DIR}/${base}_${backend}_${model_short}_${diar}${denoise_suffix}.json"

    echo "[$(date +%H:%M:%S)] $backend | $model_short | $diar | denoise=$denoise" | tee -a "$OUTPUT_DIR/log.txt"

    # Build command
    local cmd=("$BIN" "transcribe" "$file" "--backend=$backend" "-f" "json" "-o" "$output")

    # Add model for whisper
    if [ "$backend" == "whisper" ]; then
        cmd+=("-m" "$model")
    fi

    # Add diarization (only for whisper, TDT doesn't support it)
    if [ "$backend" == "whisper" ]; then
        cmd+=("--diarization=$diar")
        # Channel diarization needs --channel=both
        if [ "$diar" == "channel" ]; then
            cmd+=("--channel=both")
        fi
    fi

    # Add denoise
    if [ "$denoise" == "on" ]; then
        cmd+=("--denoise")
    fi

    # Run command
    if ! "${cmd[@]}" 2>&1 | tee -a "$OUTPUT_DIR/log.txt"; then
        echo "  [FAILED]" | tee -a "$OUTPUT_DIR/log.txt"
        return 1
    fi

    # Extract metrics from JSON
    if [ -f "$output" ]; then
        local rtf words
        rtf=$(jq -r '.metrics.rtf // "N/A"' "$output" 2>/dev/null)
        words=$(jq -r '.metrics.word_count // "N/A"' "$output" 2>/dev/null)
        echo "  RTF: $rtf | Words: $words" | tee -a "$OUTPUT_DIR/log.txt"
    fi

    return 0
}

total_tests=0
failed_tests=0

for file in "${FILES[@]}"; do
    # Resolve file path
    if [ ! -f "$file" ]; then
        full_path="$RECORDINGS_DIR/$file"
        if [ ! -f "$full_path" ]; then
            echo "Warning: File not found: $file (tried $full_path)" | tee -a "$OUTPUT_DIR/log.txt"
            continue
        fi
        file="$full_path"
    fi

    echo "" | tee -a "$OUTPUT_DIR/log.txt"
    echo "=== Processing: $(basename "$file") ===" | tee -a "$OUTPUT_DIR/log.txt"
    echo "Size: $(du -h "$file" | cut -f1)" | tee -a "$OUTPUT_DIR/log.txt"
    echo "" | tee -a "$OUTPUT_DIR/log.txt"

    # Whisper pairwise tests
    echo "--- Whisper tests (9) ---" | tee -a "$OUTPUT_DIR/log.txt"
    for test in "${WHISPER_TESTS[@]}"; do
        IFS=':' read -r model diar denoise <<< "$test"
        total_tests=$((total_tests + 1))
        if ! run_test "$file" "whisper" "$model" "$diar" "$denoise"; then
            failed_tests=$((failed_tests + 1))
        fi
    done

    # TDT tests (only if TDT feature is available)
    if "$BIN" transcribe --help 2>&1 | grep -q "tdt"; then
        echo "" | tee -a "$OUTPUT_DIR/log.txt"
        echo "--- TDT tests (2) ---" | tee -a "$OUTPUT_DIR/log.txt"
        for denoise in "${TDT_TESTS[@]}"; do
            total_tests=$((total_tests + 1))
            if ! run_test "$file" "tdt" "tdt" "none" "$denoise"; then
                failed_tests=$((failed_tests + 1))
            fi
        done
    else
        echo "" | tee -a "$OUTPUT_DIR/log.txt"
        echo "--- TDT tests skipped (feature not enabled) ---" | tee -a "$OUTPUT_DIR/log.txt"
    fi
done

echo "" | tee -a "$OUTPUT_DIR/log.txt"
echo "=== DONE ===" | tee -a "$OUTPUT_DIR/log.txt"
echo "Total tests: $total_tests" | tee -a "$OUTPUT_DIR/log.txt"
echo "Failed tests: $failed_tests" | tee -a "$OUTPUT_DIR/log.txt"
echo "Results in: $OUTPUT_DIR" | tee -a "$OUTPUT_DIR/log.txt"

json_count=$(find "$OUTPUT_DIR" -name "*.json" -type f 2>/dev/null | wc -l)
echo "Total JSON files: $json_count" | tee -a "$OUTPUT_DIR/log.txt"

# Generate summary table if we have results
if [ "$json_count" -gt 0 ]; then
    echo "" | tee -a "$OUTPUT_DIR/log.txt"
    echo "=== METRICS SUMMARY ===" | tee "$OUTPUT_DIR/summary.md"
    echo "" >> "$OUTPUT_DIR/summary.md"
    echo "| File | Backend | Model | Diarization | Denoise | RTF | Words | Segments |" >> "$OUTPUT_DIR/summary.md"
    echo "|------|---------|-------|-------------|---------|-----|-------|----------|" >> "$OUTPUT_DIR/summary.md"

    for json in "$OUTPUT_DIR"/*.json; do
        [ -f "$json" ] || continue
        jq -r '
            [
                (.input_file | split("/") | last | split(".") | first),
                .backend,
                .model,
                .diarization,
                .denoise,
                (.metrics.rtf | tostring | .[0:5]),
                .metrics.word_count,
                .metrics.segment_count
            ] | "| " + join(" | ") + " |"
        ' "$json" 2>/dev/null >> "$OUTPUT_DIR/summary.md"
    done

    echo "" >> "$OUTPUT_DIR/summary.md"
    echo "Summary saved to: $OUTPUT_DIR/summary.md"
    cat "$OUTPUT_DIR/summary.md"
fi

exit $failed_tests
