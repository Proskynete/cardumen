#!/usr/bin/env bash
# Check Cardumen against its performance budget: time to first frame, resident
# memory and idle CPU, with a fixed 5-tab session outside any git repo.
# Opens a real window for ~16 s (it may take focus). Never touches your session.
set -euo pipefail

MAX_FIRST_FRAME_MS=300
MAX_RSS_MB=60
MAX_IDLE_CPU=0.5      # percent, mean over the samples
SETTLE_S=10
SAMPLES=5

root="$(cd "$(dirname "$0")/.." && pwd)"
cd "$root"
cargo build --release --quiet

tmp="$(mktemp -d)"
pid=""
cleanup() {
  [ -n "$pid" ] && kill "$pid" 2>/dev/null && wait "$pid" 2>/dev/null || true
  rm -rf "$tmp"
}
trap cleanup EXIT

mkdir -p "$tmp"/{a,b,c,d,e,f}
pane() { printf '{"pane":{"cwd":"%s"}}' "$tmp/$1"; }
tab() { printf '{"name":"%s","focused":0,"root":%s}' "$1" "$2"; }
cat > "$tmp/session.json" <<JSON
{"version":1,"active":0,"sidebar":true,"panel":true,"tabs":[
$(tab uno "$(pane a)"),
$(tab dos "{\"split\":{\"dir\":\"right\",\"children\":[$(pane b),$(pane f)]}}"),
$(tab tres "$(pane c)"),
$(tab cuatro "$(pane d)"),
$(tab cinco "$(pane e)")
]}
JSON

CARDUMEN_SESSION="$tmp/session.json" CARDUMEN_STARTUP_LOG=1 target/release/cardumen 2>"$tmp/stderr" &
pid=$!

first=""
for _ in $(seq 100); do
  first="$(sed -n 's/^first frame: //p' "$tmp/stderr")"
  [ -n "$first" ] && break
  sleep 0.1
done
[ -n "$first" ] || { echo "no first frame after 10 s"; cat "$tmp/stderr"; exit 1; }

sleep "$SETTLE_S"
rss_max=0
cpu_sum=0
for _ in $(seq "$SAMPLES"); do
  read -r rss cpu < <(ps -o rss=,%cpu= -p "$pid")
  [ "$rss" -gt "$rss_max" ] && rss_max=$rss
  cpu_sum=$(echo "$cpu_sum + $cpu" | bc -l)
  sleep 1
done
rss_mb=$(echo "scale=1; $rss_max / 1024" | bc -l)
cpu=$(echo "scale=2; $cpu_sum / $SAMPLES" | bc -l)

fail=0
check() { # name value limit unit
  if [ "$(echo "$2 > $3" | bc -l)" = 1 ]; then
    printf '✗ %-12s %8s %s  (límite %s)\n' "$1" "$2" "$4" "$3"; fail=1
  else
    printf '✓ %-12s %8s %s  (límite %s)\n' "$1" "$2" "$4" "$3"
  fi
}
check "primer frame" "$first" "$MAX_FIRST_FRAME_MS" ms
check "memoria" "$rss_mb" "$MAX_RSS_MB" MB
check "CPU reposo" "$cpu" "$MAX_IDLE_CPU" %
exit "$fail"
