# Sync latest changes from the source project (upstream: t8y2/dbx) into this fork.
# Usage:
#   powershell -ExecutionPolicy Bypass -File dev/sync-upstream.ps1                 # update main, then merge into current branch
#   powershell -ExecutionPolicy Bypass -File dev/sync-upstream.ps1 -Mode rebase    # rebase instead of merge
#   powershell -ExecutionPolicy Bypass -File dev/sync-upstream.ps1 -Push           # also push updated main to origin
#
# Safety: never force-pushes; aborts if the working tree is dirty; only fast-forwards main.

param(
    [string]$UpstreamBranch = 'main',
    [ValidateSet('merge','rebase')][string]$Mode = 'merge',
    [switch]$Push
)

$ErrorActionPreference = 'Stop'
function Run($argline) {
    Write-Host "> git $argline" -ForegroundColor DarkGray
    & git $argline.Split(' ')
    if ($LASTEXITCODE -ne 0) { throw "git $argline failed" }
}

# 1. Ensure upstream remote exists
if ((git remote) -notcontains 'upstream') {
    Write-Host "Adding upstream remote -> https://github.com/t8y2/dbx.git" -ForegroundColor Yellow
    git remote add upstream https://github.com/t8y2/dbx.git
}

# 2. Working tree must be clean
if (git status --porcelain) {
    Write-Host "Working tree has uncommitted changes. Commit or stash first." -ForegroundColor Red
    exit 1
}

$current = (git rev-parse --abbrev-ref HEAD).Trim()
Write-Host "Current branch: $current  |  mode: $Mode" -ForegroundColor Cyan

# 3. Fetch upstream
Run "fetch upstream --prune"

# 4. Fast-forward local main from upstream
Run "checkout main"
& git merge --ff-only "upstream/$UpstreamBranch"
if ($LASTEXITCODE -ne 0) {
    Write-Host "Local main has diverged from upstream/$UpstreamBranch; cannot fast-forward. Resolve main manually." -ForegroundColor Red
    Run "checkout $current"
    exit 1
}
if ($Push) { Run "push origin main" }

# 5. Bring updates into the dev branch
if ($current -ne 'main') {
    Run "checkout $current"
    if ($Mode -eq 'rebase') { Run "rebase main" } else { Run "merge main --no-edit" }
}

Write-Host "Done. Branch '$current' now contains the latest upstream changes." -ForegroundColor Green
