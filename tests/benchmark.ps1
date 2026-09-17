# Run from PowerShell 7 on Windows. Creates only disposable NTFS junctions.
$ErrorActionPreference = 'Stop'
$projectRoot = Split-Path $PSScriptRoot -Parent
$fixture = [IO.Path]::GetFullPath((Join-Path $projectRoot '.benchmark-fixture'))
if (Test-Path -LiteralPath $fixture) { throw 'Fixture already exists; refusing to overwrite it.' }
New-Item -ItemType Directory -Path $fixture | Out-Null
$previous = $env:SYMLINK_BENCH_ROOT
try {
    $targets = Join-Path $fixture 'targets'
    New-Item -ItemType Directory -Path $targets | Out-Null
    for ($i = 0; $i -lt 3000; $i++) {
        $target = Join-Path $targets "target-$i"
        New-Item -ItemType Directory -Path $target | Out-Null
        New-Item -ItemType Junction -Path (Join-Path $fixture "link-$i") -Target $target | Out-Null
        if ($i % 10 -eq 0) { Remove-Item -LiteralPath $target }
    }
    $env:SYMLINK_BENCH_ROOT = $fixture
    cargo test --manifest-path (Join-Path $projectRoot 'src-tauri/Cargo.toml') --lib benchmark_scan_3000 -- --ignored --nocapture
    if ($LASTEXITCODE -ne 0) { throw 'Benchmark failed.' }
} finally {
    $env:SYMLINK_BENCH_ROOT = $previous
    if ($fixture -ne [IO.Path]::GetFullPath((Join-Path $projectRoot '.benchmark-fixture'))) { throw 'Unexpected cleanup path.' }
    # Unlink junctions first; never traverse their targets during cleanup.
    Get-ChildItem -LiteralPath $fixture -Force | Where-Object { $_.Attributes -band [IO.FileAttributes]::ReparsePoint } | ForEach-Object { Remove-Item -LiteralPath $_.FullName -Force }
    Remove-Item -LiteralPath $fixture -Recurse -Force
}
