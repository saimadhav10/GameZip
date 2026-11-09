param([string]$MediaToolsDirectory)
$ErrorActionPreference = 'Stop'
if (Get-Process gamezip -ErrorAction SilentlyContinue) { throw 'Close the existing GameZip instance before running isolated tests.' }
$projectRoot = (Resolve-Path (Join-Path $PSScriptRoot '..')).Path
$binaryPath = Join-Path $projectRoot 'app\src-tauri\target\debug\gamezip.exe'
if (-not (Test-Path -LiteralPath $binaryPath)) { throw 'Build the debug desktop app before running this test.' }
$smokeRoot = Join-Path $projectRoot ('.local\smoke-' + [guid]::NewGuid().ToString('N'))
New-Item -ItemType Directory -Path $smokeRoot -Force | Out-Null
$env:GAMEZIP_TEST_DATA_DIR = Join-Path $smokeRoot 'profile'
$env:GAMEZIP_TEST_ACTIVE = '1'
if ($MediaToolsDirectory) { $env:GAMEZIP_TEST_FFMPEG = Join-Path $MediaToolsDirectory 'ffmpeg.exe' }
$env:WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS = '--remote-debugging-port=9227'
$process = Start-Process -FilePath $binaryPath -WindowStyle Hidden -PassThru
try {
    Push-Location (Join-Path $projectRoot 'app')
    & node tests/native-smoke.mjs $smokeRoot
    if ($LASTEXITCODE -ne 0) { throw 'Native smoke test failed.' }
} finally {
    Pop-Location
    # Only stop the process this script launched; never touch another GameZip instance.
    if (-not $process.HasExited) { Stop-Process -Id $process.Id }
    Remove-Item Env:\GAMEZIP_TEST_DATA_DIR -ErrorAction SilentlyContinue
    Remove-Item Env:\GAMEZIP_TEST_ACTIVE -ErrorAction SilentlyContinue
    if ($MediaToolsDirectory) { Remove-Item Env:\GAMEZIP_TEST_FFMPEG -ErrorAction SilentlyContinue }
    Remove-Item Env:\WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS -ErrorAction SilentlyContinue
}
