# Export an existing RBH front/rear proof pair as timestamped EPS files.
# Usage (from any directory): .\scripts\Export-RBH-EPS.ps1
param(
    [string]$Front,
    [string]$Rear,
    [string]$OutputDirectory
)

$ErrorActionPreference = 'Stop'
$repoRoot = (Resolve-Path (Join-Path $PSScriptRoot '..')).Path
$assetsOutput = Join-Path (Split-Path $repoRoot -Parent) 'rbh-label-test-assets\output'
$converter = Join-Path $repoRoot 'target\debug\vectorcraft-cli.exe'

if (-not $Front) { $Front = Join-Path $assetsOutput 'front-verification-20261029.vectorcraft' }
if (-not $Rear) { $Rear = Join-Path $assetsOutput 'rear-verification-20261029.vectorcraft' }
if (-not $OutputDirectory) { $OutputDirectory = $assetsOutput }

foreach ($path in @($converter, $Front, $Rear)) {
    if (-not (Test-Path -LiteralPath $path -PathType Leaf)) {
        throw "Required file not found: $path"
    }
}
if (-not (Test-Path -LiteralPath $OutputDirectory -PathType Container)) {
    New-Item -ItemType Directory -Path $OutputDirectory -Force | Out-Null
}

$stamp = Get-Date -Format 'yyyyMMdd-HHmmss'
$frontEps = Join-Path $OutputDirectory "RBH-FRONT-$stamp.eps"
$rearEps = Join-Path $OutputDirectory "RBH-REAR-$stamp.eps"
foreach ($path in @($frontEps, $rearEps)) {
    if (Test-Path -LiteralPath $path) { throw "Refusing to overwrite existing file: $path" }
}

try {
    & $converter convert $Front $frontEps --artboard 0
    if ($LASTEXITCODE -ne 0 -or -not (Test-Path -LiteralPath $frontEps -PathType Leaf)) {
        throw 'Front EPS conversion failed.'
    }
    & $converter convert $Rear $rearEps --artboard 0
    if ($LASTEXITCODE -ne 0 -or -not (Test-Path -LiteralPath $rearEps -PathType Leaf)) {
        throw 'Rear EPS conversion failed.'
    }
} catch {
    # A matched pair must be complete. Never leave a misleading half-pair.
    foreach ($path in @($frontEps, $rearEps)) {
        if (Test-Path -LiteralPath $path -PathType Leaf) { Remove-Item -LiteralPath $path -Force }
    }
    throw
}

Write-Host "Created front EPS: $frontEps" -ForegroundColor Green
Write-Host "Created rear EPS:  $rearEps" -ForegroundColor Green
