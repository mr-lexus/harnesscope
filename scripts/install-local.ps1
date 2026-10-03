<# Install a locally built Windows collector. No downloads or administrator rights. #>
[CmdletBinding()]
param(
    [string]$Binary = (Join-Path $PSScriptRoot '..\target\release\harnesscope.exe'),
    [string]$CodexPath = '',
    [ValidateRange(1024,65535)][int]$Port = 4242,
    [string]$InstallDir = "$env:LOCALAPPDATA\Programs\harnesscope\bin",
    [string]$DataDir = "$env:LOCALAPPDATA\harnesscope\harnesscope\data",
    [switch]$AutoStart
)
$ErrorActionPreference = 'Stop'
$Binary = (Resolve-Path -LiteralPath $Binary).Path
$InstallDir = [IO.Path]::GetFullPath($InstallDir)
$DataDir = [IO.Path]::GetFullPath($DataDir)
$Database = Join-Path $DataDir 'harnesscope.db'
$Url = "http://127.0.0.1:$Port"
if (-not $CodexPath) {
    $CodexRoot = if ($env:CODEX_HOME) { $env:CODEX_HOME } else { Join-Path $env:USERPROFILE '.codex' }
    $CodexPath = Join-Path $CodexRoot 'sessions'
}
$CodexPath = (Resolve-Path -LiteralPath $CodexPath).Path
if (-not (Test-Path -LiteralPath $CodexPath -PathType Container)) { throw 'Choose an existing Codex sessions directory.' }

# Refuse to stop an unrelated listener or another database's collector.
$Socket = New-Object Net.Sockets.TcpClient
try {
    $Connection = $Socket.ConnectAsync('127.0.0.1',$Port)
    $Listening = $Connection.Wait(500) -and $Socket.Connected
} catch { $Listening = $false } finally { $Socket.Dispose() }
if ($Listening) {
    $Status = Invoke-RestMethod "$Url/api/v1/collection" -TimeoutSec 3
    $ObservedPath = ([string]$Status.database_path) -replace '^\\\\\?\\',''
    if (-not $ObservedPath -or [IO.Path]::GetFullPath($ObservedPath) -ne $Database) {
        throw "Port $Port belongs to a different database or application. Choose another port."
    }
    Invoke-RestMethod "$Url/api/v1/shutdown" -Method Post -TimeoutSec 3 | Out-Null
    $Stopped = $false
    for ($Attempt=0;$Attempt -lt 100;$Attempt++) {
        try { Invoke-RestMethod "$Url/api/v1/health" -TimeoutSec 1 | Out-Null }
        catch { $Stopped = $true; break }
        Start-Sleep -Milliseconds 200
    }
    if (-not $Stopped) { throw 'The previous collector did not stop; installation was not continued.' }
    Start-Sleep -Milliseconds 200
}

New-Item -ItemType Directory -Path $InstallDir,$DataDir -Force | Out-Null
$env:HARNESSCOPE_DB_PATH = $Database
$env:HARNESSCOPE_DATA_DIR = $DataDir
$env:HARNESSCOPE_SERVER_URL = $Url
$env:HARNESSCOPE_SERVER_PORT = [string]$Port
$env:HARNESSCOPE_SERVER_HOST = '127.0.0.1'
if (Test-Path -LiteralPath $Database) {
    $BackupDir = Join-Path $DataDir 'backups'
    New-Item -ItemType Directory -Path $BackupDir -Force | Out-Null
    $BackupPath = Join-Path $BackupDir ("before-install-{0}-{1}.db" -f (Get-Date -Format 'yyyyMMdd-HHmmss'),[guid]::NewGuid().ToString('N').Substring(0,8))
    & $Binary backup create --output $BackupPath
    if ($LASTEXITCODE -ne 0) { throw 'Backup failed; installation stopped before changing the database.' }
}
$InstalledExe = Join-Path $InstallDir 'harnesscope.exe'
if ($Binary -ne $InstalledExe) {
    # HTTP stops accepting requests before the collector finishes its current
    # batch. Windows keeps the executable locked until that process exits.
    $CopyDeadline = [DateTime]::UtcNow.AddSeconds(30)
    while ($true) {
        try { Copy-Item -LiteralPath $Binary -Destination $InstalledExe -Force; break }
        catch [IO.IOException], [UnauthorizedAccessException] {
            $NativeError = $_.Exception.HResult -band 0xFFFF
            if ($NativeError -notin 5,32,33,1224 -or [DateTime]::UtcNow -ge $CopyDeadline) { throw }
            Start-Sleep -Milliseconds 200
        }
    }
}
$Registration = & $InstalledExe sources add-codex --path $CodexPath
if ($LASTEXITCODE -ne 0) { throw 'Could not connect the Codex source.' }
$Source = ($Registration -join "`n") | ConvertFrom-Json
& $InstalledExe sources resume $Source.id
if ($LASTEXITCODE -ne 0) { throw 'Could not enable collection.' }

# WScript hides the console without changing PowerShell policy. Configuration is
# explicit so login/start-menu launches use the same database as installation.
function VbString([string]$Value) { '"' + $Value.Replace('"','""') + '"' }
$Common = @"
Option Explicit
Dim shell, environment, result
Set shell = CreateObject("WScript.Shell")
Set environment = shell.Environment("Process")
environment("HARNESSCOPE_DB_PATH") = $(VbString $Database)
environment("HARNESSCOPE_DATA_DIR") = $(VbString $DataDir)
environment("HARNESSCOPE_SERVER_HOST") = "127.0.0.1"
environment("HARNESSCOPE_SERVER_PORT") = "$Port"
environment("HARNESSCOPE_SERVER_URL") = $(VbString $Url)
"@
$StartScript = Join-Path $InstallDir 'start-collector.vbs'
$PanelScript = Join-Path $InstallDir 'open-panel.vbs'
$StopScript = Join-Path $InstallDir 'stop-collector.vbs'
foreach ($Launcher in @(@($StartScript,"server start --port $Port"),@($PanelScript,"ui --port $Port"),@($StopScript,"server stop --port $Port"))) {
    $Command = '"' + $InstalledExe + '" ' + $Launcher[1]
    $Content = $Common + "`r`nresult = shell.Run(" + (VbString $Command) + ", 0, True)`r`nWScript.Quit result`r`n"
    [IO.File]::WriteAllText($Launcher[0],$Content,[Text.Encoding]::Unicode)
}
$Shell = New-Object -ComObject WScript.Shell
function Write-Shortcut([string]$Path,[string]$Script,[string]$Description) {
    $Shortcut = $Shell.CreateShortcut($Path)
    $Shortcut.TargetPath = "$env:WINDIR\System32\wscript.exe"
    $Shortcut.Arguments = '"' + $Script + '"'
    $Shortcut.WorkingDirectory = $InstallDir
    $Shortcut.Description = $Description
    $Shortcut.WindowStyle = 7
    $Shortcut.Save()
}
$Programs = [Environment]::GetFolderPath('Programs')
Write-Shortcut (Join-Path $Programs 'Harnesscope.lnk') $PanelScript 'Open Harnesscope and start collection if needed'
Write-Shortcut (Join-Path $Programs 'Harnesscope - stop collection.lnk') $StopScript 'Stop the local Harnesscope server'
$StartupShortcut = Join-Path ([Environment]::GetFolderPath('Startup')) 'Harnesscope Collector.lnk'
if ($AutoStart) {
    Write-Shortcut $StartupShortcut $StartScript 'Start local Codex metadata collection at sign-in'
}
$UserPath = [Environment]::GetEnvironmentVariable('Path','User')
if (@($UserPath -split ';' | ForEach-Object { $_.TrimEnd('\') }) -notcontains $InstallDir.TrimEnd('\')) {
    [Environment]::SetEnvironmentVariable('Path',(([string]$UserPath).TrimEnd(';')+';'+$InstallDir),'User')
}
& $InstalledExe server start --port $Port
if ($LASTEXITCODE -ne 0) { throw 'Server start failed.' }
$Ready = $null
for ($Attempt=0;$Attempt -lt 30;$Attempt++) {
    try { $Ready = Invoke-RestMethod "$Url/api/v1/collection" -TimeoutSec 2; break } catch { Start-Sleep -Milliseconds 200 }
}
if (-not $Ready -or $Ready.enabled_sources -lt 1) { throw 'Collector did not become ready. Run the installed executable with serve to see diagnostics.' }
[pscustomobject]@{Url=$Url;Executable=$InstalledExe;Database=$Database;CodexSource=$CodexPath;StartAtLogin=(Test-Path -LiteralPath $StartupShortcut);Collection=$Ready.status} | ConvertTo-Json
