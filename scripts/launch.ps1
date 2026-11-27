# Book Keeper launcher (started by launch.cmd).
#   1. Pulls updates from GitHub (fast-forward only; skipped when offline or not possible).
#   2. Rebuilds target\release\book-keeper.exe only when the checked-out commit differs from
#      the one the exe was built from (stamp file target\release\.built-commit).
#   3. Starts the exe and closes this console.
# If a build fails, the console stays open with the error and the previous exe can still be opened.

$root = Split-Path -Parent $PSScriptRoot
Set-Location $root
$release = Join-Path $root 'target\release'
$exe = Join-Path $release 'book-keeper.exe'
$commitStamp = Join-Path $release '.built-commit'
$lockStamp = Join-Path $release '.npm-lock-hash'

# The shortcut starts this console minimized; bring it up when there is something to show.
function Show-Console {
    try {
        Add-Type -Namespace BK -Name Win -MemberDefinition @'
[DllImport("kernel32.dll")] public static extern System.IntPtr GetConsoleWindow();
[DllImport("user32.dll")] public static extern bool ShowWindow(System.IntPtr hWnd, int nCmdShow);
'@ -ErrorAction SilentlyContinue
        [void][BK.Win]::ShowWindow([BK.Win]::GetConsoleWindow(), 9) # SW_RESTORE
    } catch { }
}

function Start-App {
    Start-Process -FilePath $exe -WorkingDirectory $root
    exit 0
}

function Stop-WithBuildError([string]$msg) {
    Show-Console
    Write-Host ''
    Write-Host $msg -ForegroundColor Red
    if (Test-Path $exe) {
        Read-Host 'Press Enter to open the previous version of Book Keeper'
        Start-App
    }
    Read-Host 'Press Enter to close'
    exit 1
}

# One window at a time: two copies would share the same shop database.
if (Get-Process book-keeper -ErrorAction SilentlyContinue) {
    Show-Console
    Write-Host 'Book Keeper is already open. Switch to it from the taskbar.' -ForegroundColor Yellow
    Start-Sleep -Seconds 4
    exit 0
}

# 1. Updates. GIT_TERMINAL_PROMPT=0 makes git fail instead of waiting for a login prompt.
$env:GIT_TERMINAL_PROMPT = '0'
Write-Host 'Checking for updates...'
& git pull --ff-only --quiet
if ($LASTEXITCODE -ne 0) {
    Write-Host 'Could not get updates (offline, or local changes). Opening the current version.' -ForegroundColor Yellow
}

# 2. Rebuild only when the commit changed (or the exe is missing).
$head = (& git rev-parse HEAD).Trim()
$built = if (Test-Path $commitStamp) { (Get-Content $commitStamp -Raw).Trim() } else { '' }
if ((Test-Path $exe) -and $built -eq $head) { Start-App }

Show-Console
Write-Host ''
Write-Host 'Book Keeper was updated. Building the new version; this takes a few minutes...' -ForegroundColor Cyan

# npm ci (not npm install) so package-lock.json is never rewritten, which would block later pulls.
$lockHash = (Get-FileHash (Join-Path $root 'package-lock.json') -Algorithm SHA256).Hash
$oldLockHash = if (Test-Path $lockStamp) { (Get-Content $lockStamp -Raw).Trim() } else { '' }
if (-not (Test-Path (Join-Path $root 'node_modules')) -or $lockHash -ne $oldLockHash) {
    Write-Host 'Installing npm dependencies...'
    & cmd.exe /c 'npm ci'
    if ($LASTEXITCODE -ne 0) { Stop-WithBuildError 'npm ci failed. See the output above.' }
}

# --no-bundle: build the exe (UI embedded) and skip the MSI/NSIS installers.
& cmd.exe /c 'npx tauri build --no-bundle'
if ($LASTEXITCODE -ne 0 -or -not (Test-Path $exe)) {
    Stop-WithBuildError 'The build failed. See the output above.'
}

Set-Content -Path $commitStamp -Value $head -Encoding ascii
Set-Content -Path $lockStamp -Value $lockHash -Encoding ascii
Start-App
