param(
    [switch]$CheckOnly
)

$ErrorActionPreference = "Stop"

$ProjectRoot = (Resolve-Path (Join-Path $PSScriptRoot "..\..")).Path
$Target = "x86_64-pc-windows-msvc"
$DistDir = Join-Path $ProjectRoot "dist"
$StageDir = Join-Path $DistDir "packaging-windows-x64"
$InstallerScript = Join-Path $PSScriptRoot "installer.nsi"

function Require-Command {
    param([string]$Name)

    $Command = Get-Command $Name -ErrorAction SilentlyContinue
    if (-not $Command) {
        throw "Missing command '$Name'. Install it manually, then retry."
    }
    return $Command.Source
}

function Invoke-Checked {
    param(
        [string]$Description,
        [scriptblock]$Command
    )

    & $Command
    if ($LASTEXITCODE -ne 0) {
        throw "$Description failed with exit code $LASTEXITCODE."
    }
}

if ($env:OS -ne "Windows_NT") {
    throw "Windows packages must be built on Windows."
}

$Cargo = Require-Command "cargo"
$Rustup = Require-Command "rustup"
$Magick = Require-Command "magick"

$InstalledTargets = & $Rustup target list --installed
if ($LASTEXITCODE -ne 0) {
    throw "Unable to list installed Rust targets."
}
if ($InstalledTargets -notcontains $Target) {
    throw "Missing Rust target '$Target'. Run: rustup target add $Target"
}

if ($env:NSIS_PATH) {
    $MakeNsis = if (Test-Path $env:NSIS_PATH -PathType Container) {
        Join-Path $env:NSIS_PATH "makensis.exe"
    } else {
        $env:NSIS_PATH
    }
} else {
    $NsisCommand = Get-Command "makensis.exe" -ErrorAction SilentlyContinue
    $MakeNsis = if ($NsisCommand) { $NsisCommand.Source } else { $null }
}
if (-not $MakeNsis -or -not (Test-Path $MakeNsis -PathType Leaf)) {
    throw "NSIS was not found. Install NSIS or set NSIS_PATH to makensis.exe or its directory."
}

$MetadataJson = & $Cargo metadata `
    --manifest-path (Join-Path $ProjectRoot "Cargo.toml") `
    --no-deps `
    --format-version 1
if ($LASTEXITCODE -ne 0) {
    throw "cargo metadata failed."
}
$Metadata = $MetadataJson | ConvertFrom-Json
$Package = $Metadata.packages `
    | Where-Object { $_.name -eq "legend_shot" } `
    | Select-Object -First 1
if (-not $Package) {
    throw "cargo metadata did not return the legend_shot package."
}
$Version = $Package.version

$SignValues = @(
    $env:WINDOWS_CERT_PFX,
    $env:WINDOWS_CERT_PASSWORD,
    $env:WINDOWS_TIMESTAMP_URL
)
$ConfiguredSignValues = @(
    $SignValues | Where-Object { -not [string]::IsNullOrWhiteSpace($_) }
)
if ($ConfiguredSignValues.Count -ne 0 -and $ConfiguredSignValues.Count -ne 3) {
    throw "Set all or none of WINDOWS_CERT_PFX, WINDOWS_CERT_PASSWORD, and WINDOWS_TIMESTAMP_URL."
}
$SigningEnabled = $ConfiguredSignValues.Count -eq 3
$SignTool = $null
if ($SigningEnabled) {
    if (-not (Test-Path $env:WINDOWS_CERT_PFX -PathType Leaf)) {
        throw "WINDOWS_CERT_PFX does not point to a certificate file."
    }
    $SignTool = Require-Command "signtool.exe"
}

if ($CheckOnly) {
    $SignState = if ($SigningEnabled) { "enabled" } else { "disabled (unsigned test packages)" }
    Write-Host "Windows packaging preflight passed."
    Write-Host "Version: $Version"
    Write-Host "Target: $Target"
    Write-Host "Signing: $SignState"
    exit 0
}

Push-Location $ProjectRoot
try {
    Invoke-Checked "cargo build" {
        & $Cargo build --release --locked --target $Target
    }
} finally {
    Pop-Location
}

$Binary = Join-Path $ProjectRoot "target\$Target\release\legend_shot.exe"
if (-not (Test-Path $Binary -PathType Leaf) -or (Get-Item $Binary).Length -eq 0) {
    throw "Release binary is missing or empty: $Binary"
}

New-Item -ItemType Directory -Force $DistDir | Out-Null
if (Test-Path $StageDir) {
    Remove-Item -Recurse -Force $StageDir
}
$PortableDir = Join-Path $StageDir "portable"
New-Item -ItemType Directory -Force $PortableDir | Out-Null

$PreferredIcon = Join-Path $ProjectRoot "packaging\assets\legend-shot-1024.png"
$FallbackIcon = Join-Path $ProjectRoot "src\icon\tray-focus-128.png"
$IconSource = if (Test-Path $PreferredIcon -PathType Leaf) {
    $PreferredIcon
} else {
    Write-Warning "Using the 128x128 tray icon. Replace it with packaging/assets/legend-shot-1024.png before public release."
    $FallbackIcon
}
$IconFile = Join-Path $StageDir "legend-shot.ico"
Invoke-Checked "ICO generation" {
    & $Magick $IconSource -define "icon:auto-resize=256,128,64,48,32,16" $IconFile
}

$PortableBinary = Join-Path $PortableDir "LegendShot.exe"
Copy-Item $Binary $PortableBinary

function Sign-Artifact {
    param([string]$Path)

    Invoke-Checked "Signing '$Path'" {
        & $SignTool sign `
            /fd SHA256 `
            /td SHA256 `
            /tr $env:WINDOWS_TIMESTAMP_URL `
            /f $env:WINDOWS_CERT_PFX `
            /p $env:WINDOWS_CERT_PASSWORD `
            $Path
    }
}

if ($SigningEnabled) {
    Sign-Artifact $PortableBinary
}

$PortableOutput = Join-Path $DistDir "LegendShot-$Version-Portable-x64.zip"
if (Test-Path $PortableOutput) {
    Remove-Item -Force $PortableOutput
}
Compress-Archive -Path (Join-Path $PortableDir "*") -DestinationPath $PortableOutput

$SetupOutput = Join-Path $DistDir "LegendShot-$Version-Setup-x64.exe"
if (Test-Path $SetupOutput) {
    Remove-Item -Force $SetupOutput
}
Invoke-Checked "NSIS packaging" {
    & $MakeNsis `
        "/DVERSION=$Version" `
        "/DSOURCE_EXE=$PortableBinary" `
        "/DOUTPUT_EXE=$SetupOutput" `
        "/DICON_FILE=$IconFile" `
        $InstallerScript
}

if ($SigningEnabled) {
    Sign-Artifact $SetupOutput
}

if (-not (Test-Path $PortableOutput -PathType Leaf) -or (Get-Item $PortableOutput).Length -eq 0) {
    throw "Portable ZIP was not created."
}
if (-not (Test-Path $SetupOutput -PathType Leaf) -or (Get-Item $SetupOutput).Length -eq 0) {
    throw "Setup EXE was not created."
}

$FinalSignState = if ($SigningEnabled) { "signed" } else { "unsigned test packages" }
Write-Host "Windows packaging completed for Legend Shot $Version ($FinalSignState)."
Write-Host "Portable: $PortableOutput"
Write-Host "Setup:    $SetupOutput"
