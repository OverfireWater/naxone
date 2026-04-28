#!/usr/bin/env bash
set -euo pipefail

ROOT="D:/phpstudy_pro/WWW/utils/ruststudy"
LOG_DIR="$ROOT/target/stability-reports"
TS="$(date +%Y%m%d-%H%M%S)"
REPORT="$LOG_DIR/stability-$TS.md"
RAW_LOG="$LOG_DIR/stability-$TS.log"

mkdir -p "$LOG_DIR"

run_step() {
  local name="$1"
  local cmd="$2"
  echo "==== $name ====" | tee -a "$RAW_LOG"
  echo "$cmd" | tee -a "$RAW_LOG"
  if bash -lc "$cmd" >>"$RAW_LOG" 2>&1; then
    echo "PASS: $name" | tee -a "$RAW_LOG"
    return 0
  else
    echo "FAIL: $name" | tee -a "$RAW_LOG"
    return 1
  fi
}

pass=0
fail=0
results=""

add_result() {
  local status="$1"
  local name="$2"
  if [[ "$status" == "PASS" ]]; then
    pass=$((pass+1))
    results+="- [x] $name\n"
  else
    fail=$((fail+1))
    results+="- [ ] $name\n"
  fi
}

cd "$ROOT"

echo "RustStudy Stability Suite @ $TS" > "$RAW_LOG"

eval_cmd_1="cargo check -p ruststudy-tauri"
eval_cmd_2="cargo test -p ruststudy-core"
eval_cmd_3="cargo test -p ruststudy-adapters"
eval_cmd_4="cd crates/ruststudy-tauri/frontend && npm run test"

actions=(
  "Rust 编译检查|$eval_cmd_1"
  "Core 单元与集成测试|$eval_cmd_2"
  "Adapters 单元与集成测试|$eval_cmd_3"
  "Frontend Vitest|$eval_cmd_4"
)

for item in "${actions[@]}"; do
  name="${item%%|*}"
  cmd="${item#*|}"
  if run_step "$name" "$cmd"; then
    add_result "PASS" "$name"
  else
    add_result "FAIL" "$name"
  fi
done

cat > "$REPORT" <<EOF
# RustStudy Stability Report

- Time: $TS
- Repo: $ROOT
- Pass: $pass
- Fail: $fail
- Raw Log: \
  \
  \
  \
  \
  \
  \
  \
  \
  \
  $RAW_LOG

## Automated Checks
$results

## Manual Checks Required
- [ ] Web 互斥 + PHP 联动（Apache/Nginx 二选一行为）
- [ ] 批量启动失败可见性（端口冲突时前端报错）
- [ ] 商店断网强刷 stale 回退
- [ ] 安装中断后目录无半残留
- [ ] 同一包重复卸载幂等

## How to Run
\`bash scripts/run-stability-suite.sh\`
EOF

echo ""
echo "Report generated: $REPORT"
echo "Raw log: $RAW_LOG"

if [[ $fail -gt 0 ]]; then
  exit 1
fi
