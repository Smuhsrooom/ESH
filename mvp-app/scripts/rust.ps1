param([Parameter(ValueFromRemainingArguments=$true)][string[]]$CargoArgs)
$ErrorActionPreference='Stop'
$appRoot=Split-Path $PSScriptRoot -Parent
$toolRoot=Join-Path (Split-Path $appRoot -Parent) '.tools'
$localCargo=Join-Path $toolRoot 'cargo\bin\cargo.exe'
if(Test-Path -LiteralPath $localCargo) {
    $env:RUSTUP_HOME=Join-Path $toolRoot 'rustup'
    $env:CARGO_HOME=Join-Path $toolRoot 'cargo'
    $env:PATH="$(Join-Path $toolRoot 'cargo\bin');$env:PATH"
    $cargoBin=$localCargo
} else { $cargoBin=(Get-Command cargo -ErrorAction Stop).Source }
Push-Location (Join-Path $appRoot 'src-tauri')
try { & $cargoBin @CargoArgs; exit $LASTEXITCODE } finally { Pop-Location }
