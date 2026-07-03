# DBX dev environment check (Windows PowerShell 5.1+ / PowerShell 7+)
# Usage:
#   powershell -ExecutionPolicy Bypass -File dev/check-env.ps1
#   pwsh dev/check-env.ps1
# Validates the minimum versions from package.json (engines/packageManager) and README.

$ErrorActionPreference = 'Continue'

$REQ_NODE = [Version]'22.13.0'
$REQ_PNPM = '10.27.0'            # locked by package.json "packageManager"
$REQ_RUST = [Version]'1.77.0'

$fail = $false

function Get-CmdVersion($cmd, $arg) {
    if (-not (Get-Command $cmd -ErrorAction SilentlyContinue)) { return $null }
    try { return (& $cmd $arg 2>&1 | Out-String).Trim() } catch { return $null }
}

function Report($name, $found, $ok, $want) {
    if ($ok) { $tag = '[ OK ]'; $col = 'Green' } else { $tag = '[FAIL]'; $col = 'Red' }
    if (-not $found) { $found = '(not installed)' }
    Write-Host ("{0} {1,-11} require {2,-14} found {3}" -f $tag, $name, $want, $found) -ForegroundColor $col
}

Write-Host "=== DBX dev environment check ===" -ForegroundColor Cyan

# Node.js
$nodeRaw = Get-CmdVersion 'node' '-v'
$nodeOk = $false
if ($nodeRaw) {
    try { $nodeOk = ([Version]($nodeRaw.TrimStart('v'))) -ge $REQ_NODE } catch { $nodeOk = $false }
}
Report 'Node.js' $nodeRaw $nodeOk ">=$REQ_NODE"
if (-not $nodeOk) { $fail = $true }

# pnpm (parse only the version line; -v may print a registry warning first)
$pnpmRaw = Get-CmdVersion 'pnpm' '-v'
$pnpmVer = $null
if ($pnpmRaw) {
    $m = [regex]::Match($pnpmRaw, '(\d+\.\d+\.\d+)')
    if ($m.Success) { $pnpmVer = $m.Value }
}
$pnpmOk = ($pnpmVer -eq $REQ_PNPM)
Report 'pnpm' $pnpmVer $pnpmOk "=$REQ_PNPM"
if (-not $pnpmOk) {
    $fail = $true
    if (-not $pnpmVer) {
        Write-Host "       hint: run 'corepack enable; corepack prepare pnpm@$REQ_PNPM --activate'" -ForegroundColor Yellow
    }
}

# Rust
$rustRaw = Get-CmdVersion 'rustc' '--version'
$rustOk = $false
if ($rustRaw) {
    $m = [regex]::Match($rustRaw, '(\d+\.\d+\.\d+)')
    if ($m.Success) { $rustOk = ([Version]$m.Value) -ge $REQ_RUST }
}
Report 'Rust' $rustRaw $rustOk ">=$REQ_RUST"
if (-not $rustOk) { $fail = $true }

# Cargo
$cargoRaw = Get-CmdVersion 'cargo' '--version'
Report 'Cargo' $cargoRaw ([bool]$cargoRaw) '(with Rust)'
if (-not $cargoRaw) { $fail = $true }

Write-Host "--- optional ---" -ForegroundColor Cyan
$cw = Get-CmdVersion 'cargo-watch' '--version'
Report 'cargo-watch' $cw ([bool]$cw) '(dev:backend)'
if (-not $cw) { Write-Host "       install: cargo install cargo-watch" -ForegroundColor Yellow }

$gitRaw = Get-CmdVersion 'git' '--version'
Report 'git' $gitRaw ([bool]$gitRaw) '(required)'

Write-Host ""
if ($fail) {
    Write-Host "Some required tools are missing/outdated. Fix the [FAIL] items above." -ForegroundColor Red
    exit 1
} else {
    Write-Host "Core dev environment is ready. Next: pnpm install; then pnpm dev:tauri" -ForegroundColor Green
    exit 0
}
