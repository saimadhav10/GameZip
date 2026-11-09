<#
Download an actual published GameZip release, verify its SHA-256 and run the
per-user installer. Does not bypass Windows publisher/reputation prompts.
Use -DownloadOnly to inspect the installer first. No public release is assumed.
#>
param(
    [ValidatePattern('^v?[0-9]+\.[0-9]+\.[0-9]+(?:-[0-9A-Za-z.-]+)?$')][string]$Version,
    [switch]$DownloadOnly
)
$ErrorActionPreference = 'Stop'
if (-not [Environment]::Is64BitOperatingSystem) { throw 'GameZip requires Windows x64.' }
if ($env:PROCESSOR_ARCHITECTURE -eq 'ARM64' -or $env:PROCESSOR_ARCHITEW6432 -eq 'ARM64') { throw 'This release targets x64, not ARM64.' }
[Net.ServicePointManager]::SecurityProtocol = [Net.SecurityProtocolType]::Tls12
$repository = 'saimadhav10/GameZip'
$releasePath = if ($Version) { 'tags/v' + $Version.TrimStart('v') } else { 'latest' }
$client = New-Object Net.WebClient
$client.Headers['User-Agent'] = 'GameZip-installer'
try {
    try { $release = $client.DownloadString("https://api.github.com/repos/$repository/releases/$releasePath") | ConvertFrom-Json }
    catch { throw 'No downloadable release was found, or GitHub is unavailable. Check the GameZip Releases page and try again.' }
    if ($release.draft) { throw 'Draft releases cannot be installed.' }
    $installers = @($release.assets | Where-Object { $_.name -match '^GameZip_[0-9A-Za-z.+-]+_x64-setup\.exe$' })
    $checksums = @($release.assets | Where-Object { $_.name -eq 'SHA256SUMS.txt' })
    if ($installers.Count -ne 1 -or $checksums.Count -ne 1) { throw 'This release has no complete Windows x64 installer and checksum pair.' }
    $installer = $installers[0]
    foreach ($asset in @($installer, $checksums[0])) {
        $uri = [Uri]$asset.browser_download_url
        if ($uri.Scheme -ne 'https' -or $uri.Host -ne 'github.com' -or -not $uri.AbsolutePath.StartsWith('/saimadhav10/GameZip/releases/download/') -or $uri.Query -or $uri.Fragment) { throw 'Unexpected release download address.' }
    }
    $hashText = $client.DownloadString($checksums[0].browser_download_url)
    $pattern = '^([a-fA-F0-9]{64})\s+\*?' + [Regex]::Escape($installer.name) + '$'
    $entries = @($hashText -split '\r?\n' | Where-Object { $_.Trim() -match $pattern })
    if ($entries.Count -ne 1) { throw 'The installer checksum is missing or ambiguous.' }
    $expected = ($entries[0].Trim() -split '\s+')[0].ToLowerInvariant()
    $downloadDirectory = Join-Path ([IO.Path]::GetTempPath()) ('GameZip-' + [Guid]::NewGuid().ToString('N'))
    $null = New-Item -ItemType Directory -Path $downloadDirectory
    $installerPath = Join-Path $downloadDirectory $installer.name
    Write-Output "Downloading GameZip $($release.tag_name)..."
    $client.DownloadFile($installer.browser_download_url, $installerPath)
    $stream = [IO.File]::OpenRead($installerPath)
    $sha = [Security.Cryptography.SHA256]::Create()
    try { $actual = [BitConverter]::ToString($sha.ComputeHash($stream)).Replace('-', '').ToLowerInvariant() }
    finally { $stream.Dispose(); $sha.Dispose() }
    if ($actual -ne $expected) { throw 'Checksum mismatch. The installer was not started.' }
    if ($DownloadOnly) { Write-Output "Verified download: $installerPath"; return }
    # The installer is interactive so the user can see Windows publisher prompts
    # and choose whether to proceed. No elevated PowerShell process is required.
    $installerProcess = Start-Process -FilePath $installerPath -PassThru -Wait
    if ($installerProcess.ExitCode -ne 0) { throw "Installer exited with code $($installerProcess.ExitCode)." }
} finally { $client.Dispose() }
