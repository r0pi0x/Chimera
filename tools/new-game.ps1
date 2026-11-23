# Windows twin of tools/new-game: runs it with Git for Windows' bash
# (which has the perl it needs). Same arguments:
#   tools\new-game.ps1 <id> "<Display Name>" [--short <name>] [--no-roster] [--no-cargo]
$ErrorActionPreference = 'Stop'
$candidates = @(
    (Join-Path $env:ProgramFiles 'Git\bin\bash.exe'),
    (Join-Path ${env:ProgramFiles(x86)} 'Git\bin\bash.exe'),
    (Join-Path $env:LOCALAPPDATA 'Programs\Git\bin\bash.exe')
)
$git = Get-Command git -ErrorAction SilentlyContinue
if ($git) { $candidates += (Join-Path (Split-Path (Split-Path $git.Source)) 'bin\bash.exe') }
$bash = $candidates | Where-Object { $_ -and (Test-Path $_) } | Select-Object -First 1
if (-not $bash) { throw 'new-game: Git for Windows bash not found (install Git for Windows)' }
$script = Join-Path $PSScriptRoot 'new-game'
& $bash $script @args
exit $LASTEXITCODE
