# ZenOmen Windows Inno Setup Build Script
# Compiles packaging\windows\inno\ZenOmen.iss to target\installer\ZenOmen-Setup-x64.exe

param(
    [string]$Configuration = "release",
    [string]$TargetDir = ""
)

$ErrorActionPreference = "Stop"

$ScriptDir = Split-Path -Parent $MyInvocation.MyCommand.Path
$ProjectRoot = Resolve-Path "$ScriptDir\..\.."

if (-not $TargetDir) {
    $TargetDir = "$ProjectRoot\target\$Configuration"
}

$ZenOmenExe = "$TargetDir\zenomen.exe"
$ZenOmenHwExe = "$TargetDir\zenomen-hw.exe"

Write-Host "==> Verifying binary presence in $TargetDir..."
if (-not (Test-Path $ZenOmenExe)) {
    throw "zenomen.exe not found at $ZenOmenExe. Run 'cargo build --release' first."
}
if (-not (Test-Path $ZenOmenHwExe)) {
    throw "zenomen-hw.exe not found at $ZenOmenHwExe. Run 'cargo build --release' first."
}

# Locate Inno Setup Compiler (ISCC.exe)
$Iscc = Get-Command "iscc" -ErrorAction SilentlyContinue
if (-not $Iscc) {
    $PossiblePaths = @(
        "${env:ProgramFiles(x86)}\Inno Setup 6\ISCC.exe",
        "${env:ProgramFiles}\Inno Setup 6\ISCC.exe",
        "${env:LOCALAPPDATA}\Programs\Inno Setup 6\ISCC.exe"
    )
    foreach ($p in $PossiblePaths) {
        if (Test-Path $p) {
            $Iscc = $p
            break
        }
    }
}

if (-not $Iscc) {
    throw "Inno Setup compiler (ISCC.exe) not found. Please install Inno Setup 6 or add it to PATH."
}

$IssFile = "$ScriptDir\inno\ZenOmen.iss"
$OutputDir = "$ProjectRoot\target\installer"
if (-not (Test-Path $OutputDir)) {
    New-Item -ItemType Directory -Path $OutputDir -Force | Out-Null
}

Write-Host "==> Compiling $IssFile with $Iscc..."
& $Iscc "/DSourceDir=$TargetDir" "/DProjectRoot=$ProjectRoot" "/DOutputDir=$OutputDir" $IssFile

if ($LASTEXITCODE -ne 0) {
    throw "ISCC compilation failed with code $LASTEXITCODE"
}

$InstallerPath = "$OutputDir\ZenOmen-Setup-x64.exe"
if (Test-Path $InstallerPath) {
    $Size = (Get-Item $InstallerPath).Length / 1MB
    Write-Host "==> ZenOmen setup installer created successfully: $InstallerPath ($('{0:N2}' -f $Size) MB)"
} else {
    throw "Installer binary was not produced at $InstallerPath"
}
