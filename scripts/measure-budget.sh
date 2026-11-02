#!/usr/bin/env bash
# Reproducible resource budget measurement harness (PERF-01, PERF-02, PERF-06).
# Samples VmRSS from /proc/<pid>/status and utime+stime from /proc/<pid>/stat.

set -euo pipefail

SOAK_MODE=false
SOAK_DURATION=0

if [ "${1:-}" = "--soak" ]; then
    SOAK_MODE=true
    SOAK_DURATION="${2:-7200}"
    shift 2
fi

STAGE="${1:-dashboard-open}"
DURATION="${2:-60}"
shift 2

if [ "$#" -lt 1 ]; then
    echo "Usage: $0 [--soak <seconds>] <stage> <duration-seconds> <command...>" >&2
    exit 1
fi

if [ "$SOAK_MODE" = true ]; then
    DURATION="$SOAK_DURATION"
fi

CLK_TCK=$(getconf CLK_TCK 2>/dev/null || echo 100)

echo "Starting command in background: $*" >&2
"$@" &
PID=$!

cleanup() {
    if kill -0 "$PID" 2>/dev/null; then
        kill "$PID" 2>/dev/null || true
        wait "$PID" 2>/dev/null || true
    fi
}
trap cleanup EXIT INT TERM

# Warm-up delay
sleep 3

if ! kill -0 "$PID" 2>/dev/null; then
    echo "Error: Process $PID exited prematurely during warm-up" >&2
    exit 1
fi

get_rss_kb() {
    grep -i '^VmRSS:' "/proc/$1/status" 2>/dev/null | awk '{print $2}' || echo 0
}

get_cpu_jiffies() {
    local stat
    stat=$(cat "/proc/$1/stat" 2>/dev/null || true)
    if [ -z "$stat" ]; then
        echo 0
        return
    fi
    # Strip up to last ')'
    local post_comm="${stat##*)}"
    # utime is field 12, stime is field 13 in 1-based post_comm tokens
    awk '{print $12 + $13}' <<< "$post_comm"
}

RSS_PEAK_KB=0
RSS_FINAL_KB=0
RSS_WARMUP_5M_KB=0
PREV_JIFFIES=$(get_cpu_jiffies "$PID")
TOTAL_CPU_SECONDS=0
VALID_SAMPLES=0

START_TIME=$(date +%s)

for ((i=1; i<=DURATION; i++)); do
    sleep 1

    if ! kill -0 "$PID" 2>/dev/null; then
        echo "Error: Process $PID died after $i seconds (target duration was $DURATION)" >&2
        exit 1
    fi

    CURR_RSS_KB=$(get_rss_kb "$PID")
    if [ "$CURR_RSS_KB" -gt "$RSS_PEAK_KB" ]; then
        RSS_PEAK_KB="$CURR_RSS_KB"
    fi
    RSS_FINAL_KB="$CURR_RSS_KB"

    if [ "$i" -eq 300 ]; then
        RSS_WARMUP_5M_KB="$CURR_RSS_KB"
    fi

    CURR_JIFFIES=$(get_cpu_jiffies "$PID")
    DELTA_JIFFIES=$(( CURR_JIFFIES - PREV_JIFFIES ))
    if [ "$DELTA_JIFFIES" -lt 0 ]; then
        DELTA_JIFFIES=0
    fi
    PREV_JIFFIES="$CURR_JIFFIES"

    CPU_SEC=$(awk -v d="$DELTA_JIFFIES" -v tck="$CLK_TCK" 'BEGIN { printf "%.6f", d / tck }')
    TOTAL_CPU_SECONDS=$(awk -v tot="$TOTAL_CPU_SECONDS" -v s="$CPU_SEC" 'BEGIN { printf "%.6f", tot + s }')
    VALID_SAMPLES=$(( VALID_SAMPLES + 1 ))
done

END_TIME=$(date +%s)
ELAPSED=$(( END_TIME - START_TIME ))
if [ "$ELAPSED" -le 0 ]; then
    ELAPSED=1
fi

RSS_PEAK_MB=$(awk -v k="$RSS_PEAK_KB" 'BEGIN { printf "%.2f", k / 1024 }')
RSS_FINAL_MB=$(awk -v k="$RSS_FINAL_KB" 'BEGIN { printf "%.2f", k / 1024 }')
CPU_AVG_PERCENT=$(awk -v sec="$TOTAL_CPU_SECONDS" -v el="$ELAPSED" 'BEGIN { printf "%.4f", (sec / el) * 100 }')

mkdir -p dist
CSV_OUT="dist/budget-${STAGE}.csv"
SUMMARY_LINE="${STAGE},${PID},${RSS_PEAK_MB},${RSS_FINAL_MB},${CPU_AVG_PERCENT},${VALID_SAMPLES}"

if [ "$SOAK_MODE" = true ]; then
    RSS_WARMUP_MB=$(awk -v k="$RSS_WARMUP_5M_KB" 'BEGIN { printf "%.2f", k / 1024 }')
    GROWTH_MB=$(awk -v f="$RSS_FINAL_MB" -v w="$RSS_WARMUP_MB" 'BEGIN { printf "%.2f", f - w }')
    SUMMARY_LINE="${STAGE},${PID},${RSS_PEAK_MB},${RSS_FINAL_MB},${CPU_AVG_PERCENT},${VALID_SAMPLES},warmup_5m=${RSS_WARMUP_MB},growth=${GROWTH_MB}"
fi

echo "$SUMMARY_LINE"
echo "$SUMMARY_LINE" >> "$CSV_OUT"
