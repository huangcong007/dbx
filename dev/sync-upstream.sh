#!/usr/bin/env bash
# 从源项目 (upstream: t8y2/dbx) 同步最新修改到当前 fork。
# 用法:
#   bash dev/sync-upstream.sh                    # 默认: 更新本地 main, 再 merge 到当前分支
#   MODE=rebase bash dev/sync-upstream.sh        # 用 rebase 代替 merge
#   UPSTREAM_BRANCH=main PUSH=1 bash dev/sync-upstream.sh
set -euo pipefail

UPSTREAM_BRANCH="${UPSTREAM_BRANCH:-main}"
MODE="${MODE:-merge}"
PUSH="${PUSH:-0}"

RED='\033[0;31m'; GREEN='\033[0;32m'; YELLOW='\033[0;33m'; CYAN='\033[0;36m'; NC='\033[0m'

# 1. 确保 upstream 存在
if ! git remote | grep -qx upstream; then
  echo -e "${YELLOW}添加 upstream 远程 -> https://github.com/t8y2/dbx.git${NC}"
  git remote add upstream https://github.com/t8y2/dbx.git
fi

# 2. 工作区必须干净
if [ -n "$(git status --porcelain)" ]; then
  echo -e "${RED}工作区有未提交改动, 请先 commit 或 stash 后再同步。${NC}"; exit 1
fi

CURRENT=$(git rev-parse --abbrev-ref HEAD)
echo -e "${CYAN}当前分支: ${CURRENT}  |  同步模式: ${MODE}${NC}"

# 3. 抓取上游
git fetch upstream --prune

# 4. 快进更新本地 main
git checkout main
if ! git merge --ff-only "upstream/${UPSTREAM_BRANCH}"; then
  echo -e "${RED}本地 main 与 upstream/${UPSTREAM_BRANCH} 已分叉, 无法快进。请手动处理 main。${NC}"
  git checkout "$CURRENT"; exit 1
fi
[ "$PUSH" = "1" ] && git push origin main

# 5. 回到开发分支并整合
if [ "$CURRENT" != "main" ]; then
  git checkout "$CURRENT"
  if [ "$MODE" = "rebase" ]; then git rebase main; else git merge main --no-edit; fi
fi

echo -e "${GREEN}同步完成。当前分支 '${CURRENT}' 已包含源项目最新修改。${NC}"
