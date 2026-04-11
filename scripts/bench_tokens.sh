#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
CTK_BIN="${CTK_BIN:-$ROOT_DIR/target/release/ctk}"
MAX_LINES="${MAX_LINES:-200}"
ENCODING="${ENCODING:-cl100k_base}"
CMDS_FILE="${1:-}"
CTK_PROXY_ARGS="${CTK_PROXY_ARGS:-}"

if [[ ! -x "$CTK_BIN" ]]; then
  echo "[bench] building ctk release binary..."
  (cd "$ROOT_DIR" && cargo build --release >/dev/null)
fi

count_tokens() {
  local tmp
  tmp="$(mktemp)"
  cat >"$tmp"
  python3 - "$ENCODING" "$tmp" <<'PY'
import sys
enc_name = sys.argv[1]
path = sys.argv[2]
with open(path, "r", encoding="utf-8", errors="ignore") as f:
    text = f.read()
try:
    import tiktoken
    enc = tiktoken.get_encoding(enc_name)
    print(len(enc.encode(text)))
except Exception:
    print((len(text) + 3) // 4)
PY
  rm -f "$tmp"
}

run_cmd_capture() {
  local cmd="$1"
  set +e
  local out
  out=$(sh -lc "$cmd" 2>&1)
  local code=$?
  set -e
  printf "%s" "$out"
  return $code
}

run_ctk_capture() {
  local cmd="$1"
  set +e
  local out
  # shellcheck disable=SC2086
  out=$("$CTK_BIN" proxy --max-lines "$MAX_LINES" $CTK_PROXY_ARGS -- sh -lc "$cmd" 2>/dev/null)
  local code=$?
  set -e
  printf "%s" "$out"
  return $code
}

load_commands() {
  if [[ -n "$CMDS_FILE" ]]; then
    grep -vE '^\s*($|#)' "$CMDS_FILE"
    return
  fi

  cat <<'CMDS'
ls -la
rg --files | head -n 200
git status --short
git diff --stat
CMDS
}

printf "%-3s | %-45s | %10s | %10s | %8s\n" "#" "command" "raw_tok" "ctk_tok" "saved"
printf "%s\n" "---------------------------------------------------------------------------------------------------------"

total_raw=0
total_ctk=0
idx=1

while IFS= read -r cmd; do
  [[ -z "$cmd" ]] && continue

  raw_out="$(run_cmd_capture "$cmd" || true)"
  ctk_out="$(run_ctk_capture "$cmd" || true)"

  raw_tok="$(printf "%s" "$raw_out" | count_tokens)"
  ctk_tok="$(printf "%s" "$ctk_out" | count_tokens)"

  total_raw=$((total_raw + raw_tok))
  total_ctk=$((total_ctk + ctk_tok))

  if [[ "$raw_tok" -gt 0 ]]; then
    saved=$(((100 * (raw_tok - ctk_tok)) / raw_tok))
  else
    saved=0
  fi

  short_cmd="$cmd"
  if [[ ${#short_cmd} -gt 45 ]]; then
    short_cmd="${short_cmd:0:42}..."
  fi

  printf "%-3s | %-45s | %10d | %10d | %7d%%\n" "$idx" "$short_cmd" "$raw_tok" "$ctk_tok" "$saved"
  idx=$((idx + 1))
done < <(load_commands)

printf "%s\n" "---------------------------------------------------------------------------------------------------------"
if [[ "$total_raw" -gt 0 ]]; then
  total_saved=$(((100 * (total_raw - total_ctk)) / total_raw))
else
  total_saved=0
fi
printf "TOTAL raw=%d ctk=%d saved=%d%%\n" "$total_raw" "$total_ctk" "$total_saved"

echo
echo "Note: If python package 'tiktoken' is unavailable, token counts are estimated by chars/4."
echo "Tip: Tune ctk via env, e.g. CTK_PROXY_ARGS='--level aggressive --max-chars-per-line 220' MAX_LINES=80 ./scripts/bench_tokens.sh"
