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
    $Tag = "v0.1.0"
}

$Archive = "harnesscope-$Tag-$Target.zip"
$Url = "https://github.com/$Repo/releases/download/$Tag/$Archive"

$TempZip = [System.IO.Path]::Combine([System.IO.Path]::GetTempPath(), $Archive)
$TempExtract = [System.IO.Path]::Combine([System.IO.Path]::GetTempPath(), "harnesscope_extract_$([System.Guid]::NewGuid().ToString('N'))")

Write-Host "Downloading $Url..." -ForegroundColor Green
Invoke-WebRequest -Uri $Url -OutFile $TempZip

Write-Host "Extracting..." -ForegroundColor Green
Expand-Archive -Path $TempZip -DestinationPath $TempExtract -Force

if (-not (Test-Path $InstallDir)) {
    New-Item -ItemType Directory -Path $InstallDir -Force | Out-Null
}

Copy-Item -Path "$TempExtract\harnesscope.exe" -Destination "$InstallDir\harnesscope.exe" -Force

Remove-Item -Path $TempZip -Force -ErrorAction SilentlyContinue
Remove-Item -Path $TempExtract -Recurse -Force -ErrorAction SilentlyContinue

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
