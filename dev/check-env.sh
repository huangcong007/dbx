#!/usr/bin/env bash
# DBX 开发环境检测脚本 (macOS / Linux)
# 用法: bash dev/check-env.sh
set -u

REQ_NODE_MAJOR=22
REQ_NODE_MINOR=13
REQ_PNPM="10.27.0"
REQ_RUST_MAJOR=1
REQ_RUST_MINOR=77

fail=0
GREEN='\033[0;32m'; RED='\033[0;31m'; YELLOW='\033[0;33m'; CYAN='\033[0;36m'; NC='\033[0m'

report() { # name ok want found
  if [ "$2" = "1" ]; then printf "${GREEN}[ OK ]${NC} %-10s 需要 %-14s 检测到 %s\n" "$1" "$3" "$4";
  else printf "${RED}[FAIL]${NC} %-10s 需要 %-14s 检测到 %s\n" "$1" "$3" "${4:-未安装}"; fi
}

echo -e "${CYAN}=== DBX 开发环境检测 ===${NC}"

# Node.js
if command -v node >/dev/null 2>&1; then
  NODE_RAW=$(node -v); NV=${NODE_RAW#v}
  NMAJ=${NV%%.*}; REST=${NV#*.}; NMIN=${REST%%.*}
  if [ "$NMAJ" -gt "$REQ_NODE_MAJOR" ] || { [ "$NMAJ" -eq "$REQ_NODE_MAJOR" ] && [ "$NMIN" -ge "$REQ_NODE_MINOR" ]; }; then NODE_OK=1; else NODE_OK=0; fi
else NODE_RAW=""; NODE_OK=0; fi
report "Node.js" "$NODE_OK" ">=${REQ_NODE_MAJOR}.${REQ_NODE_MINOR}.0" "$NODE_RAW"; [ "$NODE_OK" = 1 ] || fail=1

# pnpm
if command -v pnpm >/dev/null 2>&1; then PNPM_RAW=$(pnpm -v); [ "$PNPM_RAW" = "$REQ_PNPM" ] && PNPM_OK=1 || PNPM_OK=0; else PNPM_RAW=""; PNPM_OK=0; fi
report "pnpm" "$PNPM_OK" "=${REQ_PNPM}" "$PNPM_RAW"
if [ "$PNPM_OK" != 1 ]; then fail=1; [ -z "$PNPM_RAW" ] && echo -e "${YELLOW}       提示: corepack enable && corepack prepare pnpm@${REQ_PNPM} --activate${NC}"; fi

# Rust
if command -v rustc >/dev/null 2>&1; then
  RUST_RAW=$(rustc --version)
  RV=$(echo "$RUST_RAW" | grep -oE '[0-9]+\.[0-9]+\.[0-9]+' | head -1)
  RMAJ=${RV%%.*}; RREST=${RV#*.}; RMIN=${RREST%%.*}
  if [ "$RMAJ" -gt "$REQ_RUST_MAJOR" ] || { [ "$RMAJ" -eq "$REQ_RUST_MAJOR" ] && [ "$RMIN" -ge "$REQ_RUST_MINOR" ]; }; then RUST_OK=1; else RUST_OK=0; fi
else RUST_RAW=""; RUST_OK=0; fi
report "Rust" "$RUST_OK" ">=${REQ_RUST_MAJOR}.${REQ_RUST_MINOR}.0" "$RUST_RAW"; [ "$RUST_OK" = 1 ] || fail=1

# Cargo
if command -v cargo >/dev/null 2>&1; then CARGO_RAW=$(cargo --version); CARGO_OK=1; else CARGO_RAW=""; CARGO_OK=0; fi
report "Cargo" "$CARGO_OK" "(随 Rust 安装)" "$CARGO_RAW"; [ "$CARGO_OK" = 1 ] || fail=1

echo -e "${CYAN}--- 可选工具 ---${NC}"
if command -v cargo-watch >/dev/null 2>&1; then report "cargo-watch" 1 "(dev:backend 可选)" "$(cargo-watch --version)"; else report "cargo-watch" 0 "(dev:backend 可选)" ""; echo -e "${YELLOW}       安装: cargo install cargo-watch${NC}"; fi
command -v git >/dev/null 2>&1 && report "git" 1 "(必需)" "$(git --version)" || report "git" 0 "(必需)" ""

echo
if [ "$fail" = 1 ]; then echo -e "${RED}环境检测未通过,请安装/升级标记 [FAIL] 的工具。${NC}"; exit 1;
else echo -e "${GREEN}核心开发环境就绪。可运行:  pnpm install  然后  make (或 pnpm dev:tauri)${NC}"; exit 0; fi
