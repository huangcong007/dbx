# 定制开发说明 (dev/)

本目录是本 fork 的**定制开发辅助工具**,不属于上游 `t8y2/dbx` 源项目。
放在独立的 `dev/` 目录可避免与上游文件冲突,便于随时同步上游修改。

## 仓库远程配置

| 远程       | 地址                                      | 用途                     |
| ---------- | ----------------------------------------- | ------------------------ |
| `origin`   | `https://github.com/huangcong007/dbx.git` | 你的 fork,推送定制改动  |
| `upstream` | `https://github.com/t8y2/dbx.git`         | 源项目,只读同步更新     |

## 分支约定

- `main`：**保持与上游一致**,只用于接收 `upstream/main` 的更新,不要在此直接开发。
- `custom-dev`：**定制开发分支**(当前分支),所有自定义改动都提交到这里。

## 1. 环境检测

要求(源自 `package.json` 与 `README`):

- Node.js `>= 22.13.0`
- pnpm `10.27.0`（`corepack enable` 后由 `packageManager` 字段锁定）
- Rust `>= 1.77`(含 cargo)
- 可选:`cargo-watch`(仅 `pnpm dev:backend` 需要)

运行检测:

```powershell
# Windows PowerShell 5.1
powershell -ExecutionPolicy Bypass -File dev/check-env.ps1
# 或 PowerShell 7+
pwsh dev/check-env.ps1
```

```bash
# macOS / Linux
bash dev/check-env.sh
```

启用 pnpm(如未安装):

```bash
corepack enable
corepack prepare pnpm@10.27.0 --activate
```

## 2. 启动开发

```bash
pnpm install          # 安装前端依赖
pnpm dev:tauri        # 启动桌面端 (Tauri)  —— 或 Linux/macOS 上直接 make

# Web 版
pnpm dev:web          # 前端
pnpm dev:backend      # 后端 (需要 cargo-watch)
```

> DuckDB 编译较慢。若不涉及 DuckDB 功能,可用 `make dev-fast` / `make cargo-check-fast` 跳过。

## 3. 随时同步源项目 (upstream) 的修改

一条命令完成:抓取上游 → 快进更新本地 `main` → 合并回当前定制分支。

```powershell
# Windows PowerShell 5.1 (前缀 powershell -File; 或 PowerShell 7+ 用 pwsh)
powershell -ExecutionPolicy Bypass -File dev/sync-upstream.ps1               # 默认 merge
powershell -ExecutionPolicy Bypass -File dev/sync-upstream.ps1 -Mode rebase  # 或 rebase
powershell -ExecutionPolicy Bypass -File dev/sync-upstream.ps1 -Push         # 顺便把更新后的 main 推到 fork
```

```bash
# macOS / Linux
bash dev/sync-upstream.sh
MODE=rebase bash dev/sync-upstream.sh
PUSH=1 bash dev/sync-upstream.sh
```

脚本安全约束:

- 工作区不干净(有未提交改动)时会**中止**,先 `commit` 或 `git stash`。
- 只对 `main` 做**快进(ff-only)**更新;若 `main` 被改动导致分叉会中止并提示。
- 不会自动强制推送。

### 等价的手动命令

```bash
git fetch upstream --prune
git checkout main
git merge --ff-only upstream/main
git checkout custom-dev
git merge main            # 或 git rebase main
```
