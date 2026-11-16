<#
.SYNOPSIS
    Compiles and packages the Universal AI Assistant desktop release for Windows.
.DESCRIPTION
    1. Builds the Rust core shared library (aissistant_ffi.dll) in release mode.
    2. Builds the Flutter Windows desktop application in release mode.
    3. Verifies native binary bundling in the output directory.
    4. Creates a portable ZIP archive and generates SHA-256 checksum manifests.
#>

[CmdletBinding()]
param(
    [string]$Version = "1.0.0",
    [string]$OutputDir = "$PSScriptRoot\..\..\..\dist"
)

$ErrorActionPreference = "Stop"

$RepoRoot = Resolve-Path "$PSScriptRoot\..\..\.."
Write-Host "=== Universal AI Assistant: Windows Packaging Pipeline (v$Version) ===" -ForegroundColor Cyan
Write-Host "Repository root: $RepoRoot"

# 1. Ensure output directory exists
if (-not (Test-Path $OutputDir)) {
    New-Item -ItemType Directory -Path $OutputDir -Force | Out-Null
}

# 2. Compile Rust Core (aissistant-ffi) in release mode
Write-Host "`n[1/4] Compiling Rust Core engine (aissistant-ffi)..." -ForegroundColor Yellow
$CargoToml = Join-Path $RepoRoot "core\Cargo.toml"
cargo build --manifest-path $CargoToml -p aissistant-ffi --release
if ($LASTEXITCODE -ne 0) {
    Write-Error "Rust core build failed with exit code $LASTEXITCODE"
}

$RustDll = Join-Path $RepoRoot "core\target\release\aissistant_ffi.dll"
if (-not (Test-Path $RustDll)) {
    Write-Error "Expected Rust library not found at: $RustDll"
}
Write-Host "Compiled Rust core: $RustDll" -ForegroundColor Green

# 3. Compile Flutter Windows Application in release mode
Write-Host "`n[2/4] Compiling Flutter Windows application..." -ForegroundColor Yellow
Push-Location (Join-Path $RepoRoot "app")
try {
    flutter build windows --release
    if ($LASTEXITCODE -ne 0) {
        Write-Error "Flutter Windows build failed with exit code $LASTEXITCODE"
    }
} finally {
    Pop-Location
}

$FlutterReleaseDir = Join-Path $RepoRoot "app\build\windows\x64\runner\Release"
if (-not (Test-Path $FlutterReleaseDir)) {
    # Fallback to single-config or alternate build folder if present
    $FlutterReleaseDir = Join-Path $RepoRoot "app\build\windows\runner\Release"
}

# 4. Copy and verify aissistant_ffi.dll in the bundle
Write-Host "`n[3/4] Verifying and bundling native libraries..." -ForegroundColor Yellow
$DestDll = Join-Path $FlutterReleaseDir "aissistant_ffi.dll"
if (-not (Test-Path $DestDll)) {
    Copy-Item -Path $RustDll -Destination $DestDll -Force
    Write-Host "Copied aissistant_ffi.dll to release bundle" -ForegroundColor Green
} else {
    Write-Host "aissistant_ffi.dll already bundled by CMake" -ForegroundColor Green
}

# 5. Assemble Portable Distribution ZIP and Checksums
Write-Host "`n[4/4] Assembling portable release ZIP and checksums..." -ForegroundColor Yellow
$ZipFileName = "aissistant-windows-x64-v$Version.zip"
$ZipPath = Join-Path $OutputDir $ZipFileName

if (Test-Path $ZipPath) {
    Remove-Item -Path $ZipPath -Force
}

Compress-Archive -Path "$FlutterReleaseDir\*" -DestinationPath $ZipPath -CompressionLevel Optimal
Write-Host "Created distribution archive: $ZipPath" -ForegroundColor Green

# Calculate SHA-256
$Hash = (Get-FileHash -Path $ZipPath -Algorithm SHA256).Hash.ToLower()
$ShaFile = "$ZipPath.sha256"
"$Hash *$ZipFileName" | Out-File -FilePath $ShaFile -Encoding ascii
Write-Host "SHA-256: $Hash" -ForegroundColor Cyan
Write-Host "Wrote checksum to: $ShaFile" -ForegroundColor Cyan

Write-Host "`nWindows packaging complete!" -ForegroundColor Green
