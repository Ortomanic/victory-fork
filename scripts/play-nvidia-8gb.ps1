param(
    [Parameter(Mandatory)][string]$Exe,
    [ValidateRange(0, 4)][int]$DropMips = 0,
    [string]$Resolution = '1920x1080',
    [switch]$Borderless,
    [switch]$Water,
    [switch]$NoOverlay,
    # Lower idle pool and window sizes for low-memory runs (see docs/NVIDIA-INTEGRATION.md).
    [switch]$LowMemory,
    # Free physical memory in MiB under which idle memory is trimmed (0: off).
    [ValidateRange(0, 262144)][int]$RamTrimMB = 0,
    # Log a memory census every N frames (0: off).
    [ValidateRange(0, 100000)][int]$CensusFrames = 0
)

$repoRoot = Split-Path -Parent $PSScriptRoot
$runner = Join-Path $repoRoot 'target\release\victory.exe'
if (-not (Test-Path -LiteralPath $runner -PathType Leaf)) {
    throw "Release executable missing at $runner. Run cargo build --release --locked -p victory."
}
if (-not (Test-Path -LiteralPath $Exe -PathType Leaf)) {
    throw "Title executable missing at $Exe."
}
if ($DropMips -gt 0) {
    Write-Warning 'This build uses mip residency on demand. The DropMips setting has no effect.'
}

$settings = @{
    VICTORY_TITLE_EXE = (Resolve-Path -LiteralPath $Exe).ProviderPath
    VICTORY_RUNNER = $runner
    VICTORY_CONSOLE = 'series-x'
    VICTORY_RESOLUTION = $Resolution
}
if ($Borderless) { $settings.VICTORY_BORDERLESS = '1' }
if ($Water) { $settings.VICTORY_TEXTURE_BACK = '1' }
if ($LowMemory) {
    $lowMemory = @{
        VICTORY_WINDOW_KB = '512'
        VICTORY_WINDOW_TAILS = '1'
        VICTORY_STAGING_POOL_MB = '64'
        VICTORY_SCRATCH_SPARE_MB = '64'
        VICTORY_SNAPSHOT_POOL_MB = '128'
        VICTORY_TEXTURE_POOL_SPARE_MB = '256'
        VICTORY_VIEW_COPIES_MB = '32'
    }
    foreach ($key in $lowMemory.Keys) {
        if (-not (Test-Path "Env:$key")) { $settings[$key] = $lowMemory[$key] }
    }
}
if ($RamTrimMB -gt 0) { $settings.VICTORY_RAM_TRIM_MB = "$RamTrimMB" }
if ($CensusFrames -gt 0) { $settings.VICTORY_CENSUS_FRAMES = "$CensusFrames" }
if ($NoOverlay) {
    $settings.VICTORY_OVERLAY = '0'
    $settings.VICTORY_PANEL = '0'
} elseif (-not (Test-Path Env:VICTORY_OVERLAY)) {
    $settings.VICTORY_OVERLAY = '1'
}

$saved = @{}
try {
    foreach ($key in $settings.Keys) {
        $saved[$key] = [Environment]::GetEnvironmentVariable($key)
        [Environment]::SetEnvironmentVariable($key, $settings[$key])
    }
    & (Join-Path $repoRoot 'Play-VIctory.cmd') pareto-residency
} finally {
    foreach ($key in $saved.Keys) {
        [Environment]::SetEnvironmentVariable($key, $saved[$key])
    }
}
