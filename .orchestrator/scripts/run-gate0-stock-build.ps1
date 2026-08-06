$ErrorActionPreference = 'Stop'

$vcvars = 'C:\Program Files (x86)\Microsoft Visual Studio\18\BuildTools\VC\Auxiliary\Build\vcvars64.bat'
$repo = 'C:\Users\93rob\Documents\GitHub\Lucidity'
$exitFile = 'C:\Users\93rob\.codex\delegations\lucidity-hackathon-20260806\gate0-stock-build.exit'
$strawberry = 'C:\Users\93rob\.codex\delegations\lucidity-hackathon-20260806\strawberry-perl-5.42.2.1-portable'

Set-Location -LiteralPath $repo
$env:PATH = @(
    (Join-Path $strawberry 'perl\site\bin')
    (Join-Path $strawberry 'perl\bin')
    (Join-Path $strawberry 'c\bin')
    $env:PATH
) -join ';'
& "$env:SystemRoot\System32\cmd.exe" /d /s /c "call `"$vcvars`" >nul && cargo build --release -p wezterm-gui"
$code = $LASTEXITCODE
Set-Content -LiteralPath $exitFile -Value $code -NoNewline
exit $code
