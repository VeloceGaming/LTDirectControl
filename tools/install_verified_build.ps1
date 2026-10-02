param([Parameter(Mandatory=$true)][string]$Version)
$ErrorActionPreference = 'Stop'
if (@(Get-Process -Name TeamfightManager2 -ErrorAction SilentlyContinue).Count -ne 0) { throw 'Close the game before installing.' }
$taskRoot = Split-Path -Parent $PSScriptRoot
$packageRoot = Join-Path $taskRoot 'dist\lt_direct_control_probe'
$gameRoot = 'C:\Program Files (x86)\Steam\steamapps\common\Teamfight Manager2'
$installRoot = Join-Path $gameRoot 'mods\lt_direct_control_probe'
$newRecord = Get-Content -LiteralPath (Join-Path $taskRoot "dist\build-$Version.json") -Raw | ConvertFrom-Json
$oldRecord = Get-Content -LiteralPath (Join-Path $taskRoot 'dist\build-0.25.0.json') -Raw | ConvertFrom-Json
if ($newRecord.version -ne $Version) { throw 'Version mismatch.' }
function Assert-Hash([string]$Path, [string]$Expected) {
    if ((Get-FileHash -LiteralPath $Path -Algorithm SHA256).Hash.ToLowerInvariant() -ne $Expected) { throw "Fingerprint mismatch: $Path" }
}
Assert-Hash (Join-Path $gameRoot 'TeamfightManager2.exe') $newRecord.executable_sha256
foreach ($entry in @(@('lt_direct_control_probe.dll','dll_sha256'),@('mod.mod_info','metadata_sha256'))) {
    Assert-Hash (Join-Path $packageRoot $entry[0]) $newRecord.($entry[1])
    Assert-Hash (Join-Path $installRoot $entry[0]) $oldRecord.($entry[1])
}
$graphics = Get-Content -LiteralPath (Join-Path $taskRoot "research\ui-graphics-$Version.json") -Raw | ConvertFrom-Json
foreach ($entry in $graphics.files.PSObject.Properties) {
    $source = Join-Path $packageRoot $entry.Name
    Assert-Hash $source $entry.Value
    $destination = [IO.Path]::GetFullPath((Join-Path $installRoot $entry.Name))
    if (-not $destination.StartsWith($installRoot + '\', [StringComparison]::OrdinalIgnoreCase)) { throw 'Asset destination escapes the mod folder.' }
}
New-Item -ItemType Directory -Path (Join-Path $installRoot 'ui') -Force | Out-Null
foreach ($entry in $graphics.files.PSObject.Properties) {
    Copy-Item -LiteralPath (Join-Path $packageRoot $entry.Name) -Destination (Join-Path $installRoot $entry.Name) -Force
}
Copy-Item -LiteralPath (Join-Path $packageRoot 'lt_direct_control_probe.dll') -Destination (Join-Path $installRoot 'lt_direct_control_probe.dll') -Force
Copy-Item -LiteralPath (Join-Path $packageRoot 'mod.mod_info') -Destination (Join-Path $installRoot 'mod.mod_info') -Force
Assert-Hash (Join-Path $installRoot 'lt_direct_control_probe.dll') $newRecord.dll_sha256
Assert-Hash (Join-Path $installRoot 'mod.mod_info') $newRecord.metadata_sha256
foreach ($entry in $graphics.files.PSObject.Properties) { Assert-Hash (Join-Path $installRoot $entry.Name) $entry.Value }
$assetCount = @($graphics.files.PSObject.Properties).Count
Write-Output "Installed verified $Version DLL, metadata, and $assetCount UI assets; all fingerprints match."
