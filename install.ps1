$ErrorActionPreference = "Stop"

$Repo = "mr-lexus/harnesscope"
$Target = "x86_64-pc-windows-msvc"
$InstallDir = "$env:LOCALAPPDATA\Programs\harnesscope\bin"

Write-Host "============================================================" -ForegroundColor Cyan
Write-Host "  Harnesscope Installer (Windows)" -ForegroundColor Cyan
Write-Host "============================================================" -ForegroundColor Cyan

try {
    $ReleaseInfo = Invoke-RestMethod -Uri "https://api.github.com/repos/$Repo/releases/latest"
    $Tag = $ReleaseInfo.tag_name
} catch {
    throw "Could not retrieve the latest release. Please retry later. $($_.Exception.Message)"
}
if ($Tag -notmatch '^v[0-9][0-9A-Za-z.-]*$') { throw "Invalid release version." }

$Archive = "harnesscope-$Tag-$Target.zip"
$Url = "https://github.com/$Repo/releases/download/$Tag/$Archive"

$InstallerTempRoot = [System.IO.Path]::GetFullPath([System.IO.Path]::GetTempPath())
$InstallerTempDir = Join-Path $InstallerTempRoot "harnesscope_install_$([System.Guid]::NewGuid().ToString('N'))"
New-Item -ItemType Directory -Path $InstallerTempDir | Out-Null
$TempZip = Join-Path $InstallerTempDir $Archive
$TempExtract = Join-Path $InstallerTempDir "extracted"

try {
Write-Host "Downloading $Url..." -ForegroundColor Green
Invoke-WebRequest -Uri $Url -OutFile $TempZip
$Checksums = (Invoke-WebRequest -Uri "https://github.com/$Repo/releases/download/$Tag/checksums.txt").Content
$Pattern = '^([a-fA-F0-9]{64})\s+\*?' + [regex]::Escape($Archive) + '$'
$MatchesFound = @($Checksums -split "`r?`n" | Where-Object { $_ -match $Pattern })
if ($MatchesFound.Count -ne 1) { throw "Release checksum missing or ambiguous." }
$ExpectedHash = [regex]::Match($MatchesFound[0], $Pattern).Groups[1].Value
if ((Get-FileHash -LiteralPath $TempZip -Algorithm SHA256).Hash -ne $ExpectedHash) {
    throw "Checksum mismatch; installation aborted."
}

Write-Host "Extracting..." -ForegroundColor Green
Expand-Archive -Path $TempZip -DestinationPath $TempExtract -Force

if (-not (Test-Path $InstallDir)) {
    New-Item -ItemType Directory -Path $InstallDir -Force | Out-Null
}

Copy-Item -Path "$TempExtract\harnesscope.exe" -Destination "$InstallDir\harnesscope.exe" -Force

} finally {
    $ResolvedInstallerDir = [System.IO.Path]::GetFullPath($InstallerTempDir)
    $TempPrefix = $InstallerTempRoot.TrimEnd([IO.Path]::DirectorySeparatorChar) + [IO.Path]::DirectorySeparatorChar
    if (-not $ResolvedInstallerDir.StartsWith($TempPrefix, [StringComparison]::OrdinalIgnoreCase) -or
        [IO.Path]::GetFileName($ResolvedInstallerDir) -notmatch '^harnesscope_install_[a-f0-9]{32}$') {
        throw "Refusing to clean an unexpected installer path."
    }
    Remove-Item -LiteralPath $ResolvedInstallerDir -Recurse -Force
}

# Ensure directory is on User PATH
$UserPath = [System.Environment]::GetEnvironmentVariable("Path", [System.EnvironmentVariableTarget]::User)
if ($UserPath -notlike "*$InstallDir*") {
    Write-Host "Adding $InstallDir to User PATH..." -ForegroundColor Yellow
    [System.Environment]::SetEnvironmentVariable("Path", "$UserPath;$InstallDir", [System.EnvironmentVariableTarget]::User)
    $env:Path += ";$InstallDir"
}

Write-Host "✓ Successfully installed harnesscope to $InstallDir\harnesscope.exe" -ForegroundColor Green
Write-Host ""
Write-Host "Verify installation:" -ForegroundColor Cyan
Write-Host "  harnesscope --version"
Write-Host "  harnesscope doctor"
Write-Host "  harnesscope ui"
