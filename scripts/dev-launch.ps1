# Book Keeper dev launcher (started by dev-launch.cmd).
# Runs the app from source with `tauri dev` and closes this console when the app closes, so
# closing the app window (or Restore from backup, which closes the app) leaves nothing behind.
# If the app never starts (a build error), the console stays open with the error.

$root = Split-Path -Parent $PSScriptRoot
Set-Location $root
$exe = Join-Path $root 'target\debug\book-keeper.exe'

function Stop-WithError([string]$msg) {
    Write-Host ''
    Write-Host $msg -ForegroundColor Red
    Read-Host 'Press Enter to close'
    exit 1
}

if (-not (Test-Path (Join-Path $root 'node_modules'))) {
    Write-Host 'Installing npm dependencies...'
    & cmd.exe /c 'npm install'
    if ($LASTEXITCODE -ne 0) { Stop-WithError 'npm install failed. See the output above.' }
}

$dev = Start-Process -FilePath 'cmd.exe' -ArgumentList '/c', 'npx tauri dev' -NoNewWindow -PassThru
$null = $dev.Handle # keep the handle so ExitCode is readable later

# Process ids of the dev process tree (tauri CLI, vite, cargo, the app).
function Get-TreeIds([int]$rootId) {
    $all = Get-CimInstance Win32_Process -Property ProcessId, ParentProcessId
    $ids = @($rootId)
    $i = 0
    while ($i -lt $ids.Count) {
        $ids += @($all | Where-Object { $_.ParentProcessId -eq $ids[$i] } | ForEach-Object { [int]$_.ProcessId })
        $i++
    }
    $ids
}

function Test-AppRunning {
    [bool](Get-Process book-keeper -ErrorAction SilentlyContinue | Where-Object { $_.Path -eq $exe })
}

# A Rust change makes tauri dev rebuild and relaunch the app; that is not a close.
function Test-Building([int]$rootId) {
    $tree = Get-TreeIds $rootId
    [bool](Get-Process cargo, rustc -ErrorAction SilentlyContinue | Where-Object { $tree -contains $_.Id })
}

$started = $false
while (-not $dev.HasExited) {
    if (Test-AppRunning) {
        $started = $true
    } elseif ($started) {
        # The app closed. Wait up to 10 quiet seconds for a rebuild/relaunch before stopping.
        $quiet = 0
        while (-not $dev.HasExited -and -not (Test-AppRunning) -and $quiet -lt 10) {
            if (Test-Building $dev.Id) { $quiet = 0 } else { $quiet++ }
            Start-Sleep -Seconds 1
        }
        if (-not $dev.HasExited -and -not (Test-AppRunning)) {
            & taskkill.exe /T /F /PID $dev.Id | Out-Null
            exit 0
        }
    }
    Start-Sleep -Seconds 1
}

if (-not $started) { Stop-WithError 'Book Keeper did not start. See the output above.' }
exit 0
