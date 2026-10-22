$ErrorActionPreference='Stop'
$appRoot=Split-Path $PSScriptRoot -Parent
$buildOutput=Join-Path $appRoot 'src-tauri\target\debug'
if(Test-Path -LiteralPath $buildOutput) {
    $resolvedOutput=(Resolve-Path -LiteralPath $buildOutput).Path
    if($resolvedOutput -ne [IO.Path]::GetFullPath($buildOutput)) {
        throw 'Unexpected build output path; cleanup stopped'
    }
    Get-ChildItem -LiteralPath $resolvedOutput -File | Where-Object {
        ($_.Name -like 'esh-chat*.d') -or ($_.Name -like 'esh_chat*.pdb')
    } | ForEach-Object { Remove-Item -LiteralPath $_.FullName -Force -ErrorAction Stop }
}
Push-Location $appRoot
try {
    npm run build
    if($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
    & (Join-Path $PSScriptRoot 'rust.ps1') build --bin esh-chat-mvp4 --features custom-protocol
    exit $LASTEXITCODE
} finally { Pop-Location }
