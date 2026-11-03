param(
    [ValidateSet('dev', 'build')][string]$Action = 'dev',
    [switch]$DebugBuild,
    [Parameter(ValueFromRemainingArguments = $true)][string[]]$ExtraArgs
)
$ErrorActionPreference = 'Stop'
$cargoBin = Join-Path $env:USERPROFILE '.cargo\bin'
if (Test-Path -LiteralPath $cargoBin) { $env:PATH = "$cargoBin;$env:PATH" }
Push-Location (Join-Path $PSScriptRoot '..\app')
try {
    if ($DebugBuild) { $ExtraArgs = @('--debug') + $ExtraArgs }
    & npm.cmd run tauri -- $Action @ExtraArgs
    exit $LASTEXITCODE
} finally { Pop-Location }
