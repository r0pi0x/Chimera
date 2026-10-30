# Build benchmark: one scenario per call, timed, printed as a markdown row.
# The before/after reports in docs/plans/active/build-infra-*.md are made
# with this script, so the numbers can be reproduced in any slot.
#
#   .\scripts\build-bench.ps1 cold            # empty target/, full test build
#   .\scripts\build-bench.ps1 seeded          # the same build after seeding target/
#   .\scripts\build-bench.ps1 noop            # same again (should be seconds)
#   .\scripts\build-bench.ps1 leaf-full       # edit source/formats, whole workspace
#   .\scripts\build-bench.ps1 leaf-direct     # edit source/formats, -p only
#   .\scripts\build-bench.ps1 leaf-affected   # edit source/formats, affected selection
#   .\scripts\build-bench.ps1 leaf-check      # check affected tests, test only source/formats
#   .\scripts\build-bench.ps1 bottom-full     # edit chimera/core, whole workspace
#   .\scripts\build-bench.ps1 bottom-affected # edit chimera/core, affected selection
#   .\scripts\build-bench.ps1 test-file-affected # edit one test file, affected selection
#   .\scripts\build-bench.ps1 fast-tier       # run the fast tier (no build expected)
#   .\scripts\build-bench.ps1 fast-tier-nextest
#   .\scripts\build-bench.ps1 run-leaf        # cargo run after an app edit
#
# `-Out file.md` appends the row. `-Label before|after` is the report's
# column. Edits are one appended comment line, restored afterwards, so the
# working tree is unchanged when the scenario ends.
[CmdletBinding()]
param(
    [Parameter(Mandatory = $true, Position = 0)]
    [ValidateSet('cold', 'seeded', 'noop', 'leaf-full', 'leaf-direct', 'leaf-affected', 'leaf-check',
        'bottom-full', 'bottom-affected', 'test-file-affected',
        'fast-tier', 'fast-tier-nextest', 'run-leaf')]
    [string]$Scenario,
    [string]$Label = 'run',
    [string]$Out = '',
    [int]$Jobs = 12
)

$ErrorActionPreference = 'Stop'
$root = Split-Path -Parent $PSScriptRoot
$cargo = Join-Path $env:USERPROFILE '.cargo\bin\cargo.exe'
if (-not (Test-Path $cargo)) { $cargo = 'cargo' }
$env:PATH = (Split-Path -Parent $cargo) + ';' + $env:PATH
$env:CARGO_BUILD_JOBS = "$Jobs"
$log = Join-Path $root 'target\bench-last.log'
New-Item -ItemType Directory -Force -Path (Split-Path -Parent $log) | Out-Null
Push-Location $root
trap { Pop-Location; throw }

$script:seconds = 0.0
$script:exit = 0

function Invoke-Cargo([string[]]$CargoArgs, [string]$LogPath = '') {
    if (-not $LogPath) { $LogPath = $log }
    $eap = $ErrorActionPreference
    $ErrorActionPreference = 'Continue'
    $sw = [Diagnostics.Stopwatch]::StartNew()
    try {
        & $cargo @CargoArgs *> $LogPath
        $script:exit = $LASTEXITCODE
    } finally {
        $sw.Stop()
        $ErrorActionPreference = $eap
    }
    $script:seconds = $sw.Elapsed.TotalSeconds
}

function Get-AffectedArgs([string]$File) {
    $eap = $ErrorActionPreference
    $ErrorActionPreference = 'Continue'
    try {
        $lines = & $cargo run -q -p repo_checks --bin affected -- --files $File --format cargo 2>$null
        $code = $LASTEXITCODE
    } finally {
        $ErrorActionPreference = $eap
    }
    if ($code -ne 0) { throw "affected failed for $File" }
    return @($lines | Where-Object { $_ -and $_.Trim().Length -gt 0 })
}

function Edit-And-Run([string]$Relative, [string[]]$BuildArgs, [string]$Kind) {
    $path = Join-Path $root $Relative
    if (-not (Test-Path $path)) { throw "missing $path" }
    $original = [IO.File]::ReadAllText($path)
    try {
        [IO.File]::AppendAllText($path, "`n// bench: $Kind`n")
        Invoke-Cargo $BuildArgs
    } finally {
        [IO.File]::WriteAllText($path, $original)
    }
}

switch ($Scenario) {
    'cold' {
        Invoke-Cargo @('test', '--features', 'dev', '--no-run')
    }
    'seeded' {
        # Same build, but target/ was copied from a warm slot first.
        Invoke-Cargo @('test', '--features', 'dev', '--no-run')
    }
    'noop' {
        # The first run absorbs any pending work; the second is the no-op.
        Invoke-Cargo @('test', '--features', 'dev', '--no-run')
        Invoke-Cargo @('test', '--features', 'dev', '--no-run')
    }
    'leaf-full' {
        Edit-And-Run 'source/formats/src/lib.rs' @('test', '--features', 'dev', '--no-run') 'leaf-full'
    }
    'leaf-direct' {
        Edit-And-Run 'source/formats/src/lib.rs' @('test', '-p', 'source_formats', '-p', 'repo_checks', '--features', 'dev', '--no-run') 'leaf-direct'
    }
    'leaf-affected' {
        $path = Join-Path $root 'source/formats/src/lib.rs'
        $original = [IO.File]::ReadAllText($path)
        try {
            [IO.File]::AppendAllText($path, "`n// bench: leaf-affected`n")
            $selected = Get-AffectedArgs 'source/formats/src/lib.rs'
            Invoke-Cargo (@('test') + $selected + @('--features', 'dev', '--no-run'))
        } finally {
            [IO.File]::WriteAllText($path, $original)
        }
    }
    'leaf-check' {
        # The inner-loop alternative: type-check the affected packages'
        # tests (no codegen or link), then build and run only the edited
        # crate's tests. It does not replace the pre-commit build of the
        # dependents' test binaries.
        $path = Join-Path $root 'source/formats/src/lib.rs'
        $original = [IO.File]::ReadAllText($path)
        $checkLog = Join-Path $root 'target\bench-check.log'
        try {
            [IO.File]::AppendAllText($path, "`n// bench: leaf-check`n")
            $selected = Get-AffectedArgs 'source/formats/src/lib.rs'
            Invoke-Cargo (@('check', '--tests') + $selected + @('--features', 'dev')) $checkLog
            $checkSeconds = $script:seconds
            $checkExit = $script:exit
            Invoke-Cargo @('test', '-p', 'source_formats')
            $testSeconds = $script:seconds
            $testExit = $script:exit
            $script:seconds = $checkSeconds + $testSeconds
            if ($checkExit -ne 0) { $script:exit = $checkExit }
            elseif ($testExit -ne 0) { $script:exit = $testExit }
            else { $script:exit = 0 }
            $combined = @(Get-Content $checkLog) + @(Get-Content $log)
            $combined | Set-Content $log
        } finally {
            [IO.File]::WriteAllText($path, $original)
        }
    }
    'bottom-full' {
        Edit-And-Run 'chimera/core/src/lib.rs' @('test', '--features', 'dev', '--no-run') 'bottom-full'
    }
    'bottom-affected' {
        $path = Join-Path $root 'chimera/core/src/lib.rs'
        $original = [IO.File]::ReadAllText($path)
        try {
            [IO.File]::AppendAllText($path, "`n// bench: bottom-affected`n")
            $selected = Get-AffectedArgs 'chimera/core/src/lib.rs'
            Invoke-Cargo (@('test') + $selected + @('--features', 'dev', '--no-run'))
        } finally {
            [IO.File]::WriteAllText($path, $original)
        }
    }
    'test-file-affected' {
        $rel = 'chimera/client/tests/it/metrics.rs'
        $path = Join-Path $root $rel
        $original = [IO.File]::ReadAllText($path)
        try {
            [IO.File]::AppendAllText($path, "`n// bench: test-file-affected`n")
            $selected = Get-AffectedArgs $rel
            Invoke-Cargo (@('test') + $selected + @('--features', 'dev', '--no-fail-fast', '--', '--skip', 'heavy::'))
        } finally {
            [IO.File]::WriteAllText($path, $original)
        }
    }
    'fast-tier' {
        Invoke-Cargo @('test', '--features', 'dev', '--no-fail-fast', '--', '--skip', 'heavy::')
    }
    'fast-tier-nextest' {
        Invoke-Cargo @('nextest', 'run', '--features', 'dev', '-E', 'not test(/^heavy::/)')
    }
    'run-leaf' {
        Edit-And-Run 'apps/lucker_party/src/main.rs' @('run', '--features', 'dev', '--bin', 'lucker_party', '--', '--help') 'run-leaf'
    }
}

$compiled = 0
if (Test-Path $log) {
    # `cargo check` prints "Checking"; the build scenarios print "Compiling".
    $compiled = (Select-String -Path $log -Pattern '^\s*(Compiling|Checking) ' -ErrorAction SilentlyContinue).Count
}
$row = "| {0} | {1} | {2:N1} | {3} | {4} | {5} |" -f $Label, $Scenario, $script:seconds, $compiled, $Jobs, $script:exit
Write-Output $row
if ($Out) { Add-Content -Path $Out -Value $row }
if ($script:exit -ne 0 -and (Test-Path $log)) {
    Write-Warning "exit $($script:exit); last lines of $log`:"
    Get-Content $log -Tail 12 | ForEach-Object { Write-Warning $_ }
}
Pop-Location
