param(
    [ValidateSet("all", "x86_64", "arm64", "windows")]
    [string]$Target = "all",

    [switch]$NoSync
)

$ErrorActionPreference = "Stop"

# --- Configuration ---
$WslDistro   = "AlmaLinux-Rust"
$WslUser     = "echo"
$AARCH64_SYSROOT = "/home/$WslUser/sysroot-aarch64"

function Get-CargoField {
    param([string]$FieldName, [string]$Default)
    $cargoFile = "Cargo.toml"
    if (!(Test-Path $cargoFile)) { return $Default }
    $inPackage = $false
    foreach ($line in Get-Content $cargoFile) {
        if ($line -match '^\s*\[package\]') { $inPackage = $true; continue }
        if ($line -match '^\s*\[') { $inPackage = $false }
        if ($inPackage -and $line -match "$FieldName\s*=\s*`"([^`"]+)`"") { return $Matches[1] }
    }
    return $Default
}
$Version     = Get-CargoField -FieldName "version" -Default "0.0.0"
$ProjectName = Get-CargoField -FieldName "name" -Default "app"

$WslProject = "/home/$WslUser/Projet/$ProjectName"
$WslUncRoot = "\\wsl.localhost\$WslDistro\home\$WslUser\Projet\$ProjectName"

function ConvertTo-WslPath {
    param([string]$WindowsPath)
    $resolved = (Resolve-Path $WindowsPath).ProviderPath
    $drive = $resolved.Substring(0, 1).ToLower()
    $rest = $resolved.Substring(2) -replace '\\', '/'
    return "/mnt/$drive$rest"
}

function Invoke-WslScript {
    # Passe par un fichier temporaire plutot qu'un argument inline : evite
    # les bugs d'echappement de guillemets de PowerShell vers un
    # executable natif (wsl.exe).
    param([string]$Script)
    $tempFile = [System.IO.Path]::GetTempFileName()
    $tempFile = Rename-Item -Path $tempFile -NewName ([System.IO.Path]::GetFileName($tempFile) + ".sh") -PassThru | Select-Object -ExpandProperty FullName
    [System.IO.File]::WriteAllText($tempFile, ($Script -replace "`r`n", "`n"))
    try {
        wsl --distribution $WslDistro --user $WslUser -- bash (ConvertTo-WslPath $tempFile)
        $exitCode = $LASTEXITCODE
    }
    finally {
        Remove-Item $tempFile -ErrorAction SilentlyContinue
    }
    if ($exitCode -ne 0) { throw "Script WSL en echec (code $exitCode)." }
}

function Sync-ToWsl {
    Write-Host "=== Synchronisation vers WSL ==="
    Invoke-WslScript "mkdir -p '$WslProject'"
    robocopy "." "$WslUncRoot" /MIR /XD target dist Deploy .git /NFL /NDL /NJH | Out-Null
}

function Build-Linux-x86_64 {
    Write-Host "=== Compilation x86_64 ==="
    Invoke-WslScript @"
set -euo pipefail
source "`$HOME/.cargo/env"
cd '$WslProject'
cargo build --release --target x86_64-unknown-linux-gnu
"@
    New-Item -ItemType Directory -Force -Path "Deploy/Linux-x86_64" | Out-Null
    $srcBin = "$WslUncRoot\target\x86_64-unknown-linux-gnu\release\$ProjectName"
    if (!(Test-Path $srcBin)) { throw "Binaire introuvable : $srcBin" }
    Copy-Item $srcBin "Deploy/Linux-x86_64/$ProjectName" -Force
	
	Make-Tar -Arch "x86_64"
	
    Write-Host "-> Deploy/Linux-x86_64/$ProjectName"
}

function Build-Linux-Arm64 {
    Write-Host "=== Compilation aarch64 (cross via zig) ==="
    Invoke-WslScript @"
set -euo pipefail
source "`$HOME/.cargo/env"
export PATH="`$HOME/.local/bin:`$PATH"
cd '$WslProject'
export RUSTFLAGS="-L native=$AARCH64_SYSROOT/usr/lib64 -L native=$AARCH64_SYSROOT/lib64"
cargo zigbuild --release --target aarch64-unknown-linux-gnu
"@
    New-Item -ItemType Directory -Force -Path "Deploy/Linux-arm64" | Out-Null
    $srcBin = "$WslUncRoot\target\aarch64-unknown-linux-gnu\release\$ProjectName"
    if (!(Test-Path $srcBin)) { throw "Binaire introuvable : $srcBin" }
    Copy-Item $srcBin "Deploy/Linux-arm64/$ProjectName" -Force
	
	Make-Tar -Arch "Arm64"
	
    Write-Host "-> Deploy/Linux-arm64/$ProjectName"
}

function Build-Windows {
    Write-Host "=== Compilation Windows ==="
    cargo build --release
    New-Item -ItemType Directory -Force -Path "Deploy/Windows" | Out-Null
    $srcBin = "target\release\$ProjectName.exe"
    if (!(Test-Path $srcBin)) { throw "Binaire introuvable : $srcBin" }
    Copy-Item $srcBin "Deploy/Windows/$ProjectName.exe" -Force
    Write-Host "-> Deploy/Windows/$ProjectName.exe"
}

function Make-Tar {
    param(
        [string]$Arch
    )

    Write-Host "=== Création du tar ($Arch) ==="
	mkdir "Deploy/Linux-$Arch/${ProjectName}Install"
	
	$deployDir = "Deploy/Linux-$Arch/${ProjectName}Install"
    $tarName   = "$ProjectName-$Arch.tar"

    # Copie des fichiers supplémentaires
	mv "Deploy/Linux-$Arch/$ProjectName" "$deployDir/$ProjectName" -Force
    Copy-Item "ressources\install.sh" "$deployDir/install.sh" -Force
    Copy-Item "ressources\uninstall.sh" "$deployDir/uninstall.sh" -Force
    Copy-Item "ressources\Icone.png" "$deployDir/Icone.png" -Force

    # Création du tar
	tar -cvf "Deploy/$tarName" -C "Deploy/Linux-$Arch/" "${ProjectName}Install"
    Write-Host "-> Deploy/$tarName"
}

# --- Execution ---
if (Test-Path "Deploy") { Remove-Item "Deploy" -Recurse -Force }
New-Item -ItemType Directory -Force -Path "Deploy" | Out-Null

Write-Host "Version detectee : $Version"

if (-not $NoSync) { Sync-ToWsl }

switch ($Target) {
    "all"    { Build-Linux-x86_64; Build-Linux-Arm64; Build-Windows }
    "x86_64" { Build-Linux-x86_64 }
    "arm64"  { Build-Linux-Arm64 }
	"windows" { Build-Windows }
}

Write-Host ""
Write-Host "=== Build termine (v$Version) ==="
Get-ChildItem -Recurse "Deploy"
