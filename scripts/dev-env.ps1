<#
.SYNOPSIS
  Configure la session PowerShell pour développer mc-core sous Windows SANS
  Visual Studio (hôte rustup x86_64-pc-windows-gnu + llvm-mingw).

.DESCRIPTION
  Le mingw « self-contained » de rustup ne contient qu'un driver de LINK
  (ni cc1, ni as) : il faut un vrai compilateur C (clang de llvm-mingw) et
  un dlltool autonome (llvm-dlltool, requis par les crates raw-dylib).

  Usage :  . .\scripts\dev-env.ps1 [-LlvmMingwDir <dossier extrait>]
  (le « . » initial est important : dot-sourcing pour exporter les variables)

  Ensuite :  cargo test -p mc-core

.NOTES
  Poste durci (EDR) : la compilation de mc-desktop peut échouer localement
  (les très gros build scripts fraîchement linkés sont tués) — la CI MSVC la
  couvre à chaque push. mc-core se développe et se teste intégralement ici.
#>
param([string]$LlvmMingwDir = $env:LLVM_MINGW_DIR)

$ErrorActionPreference = "Stop"

# cargo (rustup, hôte gnu)
$cargoBin = "$env:USERPROFILE\.cargo\bin"
if (-not (Test-Path "$cargoBin\cargo.exe")) {
    Write-Error "cargo introuvable : installer rustup avec l'hôte gnu — irm https://win.rustup.rs/x86_64 -OutFile rustup-init.exe ; .\rustup-init.exe -y --default-host x86_64-pc-windows-gnu"
}

# llvm-mingw (clang + llvm-dlltool + llvm-ar)
if (-not $LlvmMingwDir) {
    $candidates = @("$env:LOCALAPPDATA\llvm-mingw", "C:\llvm-mingw") +
        @(Get-ChildItem "$env:LOCALAPPDATA", "C:\" -Directory -Filter "llvm-mingw*" -ErrorAction SilentlyContinue | ForEach-Object FullName)
    $LlvmMingwDir = $candidates |
        Where-Object { $_ -and (Test-Path (Join-Path $_ "bin\x86_64-w64-mingw32-clang.exe")) } |
        Select-Object -First 1
}
if (-not $LlvmMingwDir) {
    Write-Error "llvm-mingw introuvable. Télécharger llvm-mingw-<ver>-ucrt-x86_64.zip (https://github.com/mstorsjo/llvm-mingw/releases), l'extraire, puis relancer :  . .\scripts\dev-env.ps1 -LlvmMingwDir <dossier extrait>"
}

$bin = Join-Path $LlvmMingwDir "bin"

# rustc invoque « dlltool.exe » par ce nom exact ; llvm-dlltool est autonome
# (le dlltool GNU du self-contained exige un `as` absent).
if (-not (Test-Path (Join-Path $bin "dlltool.exe"))) {
    Copy-Item (Join-Path $bin "llvm-dlltool.exe") (Join-Path $bin "dlltool.exe")
}

$selfContained = "$env:USERPROFILE\.rustup\toolchains\stable-x86_64-pc-windows-gnu\lib\rustlib\x86_64-pc-windows-gnu\bin\self-contained"

$env:Path = "$bin;$selfContained;$cargoBin;$env:Path"
$env:CC = Join-Path $bin "x86_64-w64-mingw32-clang.exe"
$env:AR = Join-Path $bin "llvm-ar.exe"
$env:MC_SECRETS_MODE = "memory"   # tests : coffre en mémoire (pas de pollution du Credential Manager)

Write-Host "Environnement prêt — lancer :  cargo test -p mc-core" -ForegroundColor Green
