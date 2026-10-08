# Install a verified, packaged build into the game's mods folder.
# -PreviousVersion: the build currently installed; its files are checked first (omit only for a first install).
# -GameRoot: the game folder (default: $env:TFM2_GAME_DIR, else Steam's default path).
param([Parameter(Mandatory=$true)][string]$Version, [string]$PreviousVersion, [string]$GameRoot, [switch]$AllowRunningWithModUnloaded)
$ErrorActionPreference = 'Stop'
$taskGameProcesses = @(Get-Process -Name TeamfightManager2 -ErrorAction SilentlyContinue)
if ($taskGameProcesses.Count -ne 0) {
    if (-not $AllowRunningWithModUnloaded) { throw 'Close the game before installing.' }
    foreach ($taskGameProcess in $taskGameProcesses) {
        $taskModules = @($taskGameProcess.Modules)
        if ($taskModules.Count -eq 0) { throw 'Cannot verify that the running game has unloaded the mod.' }
        if (@($taskModules | Where-Object { $_.ModuleName -ieq 'lt_direct_control_probe.dll' }).Count -ne 0) {
            throw 'The mod DLL is loaded; close the game before installing.'
        }
    }
    Write-Output 'Running game checked: the mod DLL is not loaded. Restart the game after installation.'
}
$taskRoot = Split-Path -Parent $PSScriptRoot
$packageRoot = Join-Path $taskRoot 'dist\lt_direct_control_probe'
$gameRoot = if ($GameRoot) { $GameRoot } elseif ($env:TFM2_GAME_DIR) { $env:TFM2_GAME_DIR } else { 'C:\Program Files (x86)\Steam\steamapps\common\Teamfight Manager2' }
$installRoot = Join-Path $gameRoot 'mods\lt_direct_control_probe'
$newRecord = Get-Content -LiteralPath (Join-Path $taskRoot "dist\build-$Version.json") -Raw | ConvertFrom-Json
$oldRecord = if ($PreviousVersion) { Get-Content -LiteralPath (Join-Path $taskRoot "dist\build-$PreviousVersion.json") -Raw | ConvertFrom-Json } else { $null }
if (-not $oldRecord -and (Test-Path -LiteralPath (Join-Path $installRoot 'lt_direct_control_probe.dll'))) { throw 'A build is already installed: pass -PreviousVersion so its files are checked before replacing them.' }
if ($newRecord.version -ne $Version) { throw 'Version mismatch.' }
function Assert-Hash([string]$Path, [string]$Expected) {
    if ((Get-FileHash -LiteralPath $Path -Algorithm SHA256).Hash.ToLowerInvariant() -ne $Expected) { throw "Fingerprint mismatch: $Path" }
}
Assert-Hash (Join-Path $gameRoot 'TeamfightManager2.exe') $newRecord.executable_sha256
foreach ($entry in @(@('lt_direct_control_probe.dll','dll_sha256'),@('mod.mod_info','metadata_sha256'))) {
    Assert-Hash (Join-Path $packageRoot $entry[0]) $newRecord.($entry[1])
    if ($oldRecord) { Assert-Hash (Join-Path $installRoot $entry[0]) $oldRecord.($entry[1]) }
}
$graphics = Get-Content -LiteralPath (Join-Path $taskRoot 'tools\records\ui-graphics.json') -Raw | ConvertFrom-Json
foreach ($entry in $graphics.files.PSObject.Properties) {
    $source = Join-Path $packageRoot $entry.Name
    Assert-Hash $source $entry.Value
    $destination = [IO.Path]::GetFullPath((Join-Path $installRoot $entry.Name))
    if (-not $destination.StartsWith($installRoot + '\', [StringComparison]::OrdinalIgnoreCase)) { throw 'Asset destination escapes the mod folder.' }
}
New-Item -ItemType Directory -Path (Join-Path $installRoot 'ui') -Force | Out-Null
foreach ($entry in $graphics.files.PSObject.Properties) {
    New-Item -ItemType Directory -Path (Split-Path -Parent (Join-Path $installRoot $entry.Name)) -Force | Out-Null
    Copy-Item -LiteralPath (Join-Path $packageRoot $entry.Name) -Destination (Join-Path $installRoot $entry.Name) -Force
}
Copy-Item -LiteralPath (Join-Path $packageRoot 'lt_direct_control_probe.dll') -Destination (Join-Path $installRoot 'lt_direct_control_probe.dll') -Force
Copy-Item -LiteralPath (Join-Path $packageRoot 'mod.mod_info') -Destination (Join-Path $installRoot 'mod.mod_info') -Force
Assert-Hash (Join-Path $installRoot 'lt_direct_control_probe.dll') $newRecord.dll_sha256
Assert-Hash (Join-Path $installRoot 'mod.mod_info') $newRecord.metadata_sha256
foreach ($entry in $graphics.files.PSObject.Properties) { Assert-Hash (Join-Path $installRoot $entry.Name) $entry.Value }
$assetCount = @($graphics.files.PSObject.Properties).Count
Write-Output "Installed verified $Version DLL, metadata, and $assetCount UI assets; all fingerprints match."
