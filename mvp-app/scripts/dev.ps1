$ErrorActionPreference='Stop'
$appRoot=Split-Path $PSScriptRoot -Parent
$toolRoot=Join-Path (Split-Path $appRoot -Parent) '.tools'
if(Test-Path -LiteralPath (Join-Path $toolRoot 'cargo\bin\cargo.exe')) {
    $env:RUSTUP_HOME=Join-Path $toolRoot 'rustup'
    $env:CARGO_HOME=Join-Path $toolRoot 'cargo'
    $env:PATH="$(Join-Path $toolRoot 'cargo\bin');$env:PATH"
}
Set-Location $appRoot
npm run tauri -- dev
