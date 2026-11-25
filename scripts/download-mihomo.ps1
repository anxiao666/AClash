$ErrorActionPreference = "Stop"
$version = if ($args[0]) { $args[0] } else { "v1.19.15" }
$outDir = Join-Path $PSScriptRoot "..\src-tauri\resources"
New-Item -ItemType Directory -Force -Path $outDir | Out-Null
$url = "https://github.com/MetaCubeX/mihomo/releases/download/$version/mihomo-windows-amd64-$version.zip"
$zip = Join-Path $env:TEMP "mihomo-core.zip"
$extract = Join-Path $env:TEMP "mihomo-core"
Write-Host "Downloading $url"
Invoke-WebRequest -Uri $url -OutFile $zip -UseBasicParsing
if (Test-Path $extract) { Remove-Item $extract -Recurse -Force }
Expand-Archive -Path $zip -DestinationPath $extract -Force
$exe = Get-ChildItem $extract -Recurse -Filter "*.exe" | Select-Object -First 1
Copy-Item $exe.FullName (Join-Path $outDir "mihomo.exe") -Force
Write-Host "Installed $($exe.Name) -> src-tauri/resources/mihomo.exe"
