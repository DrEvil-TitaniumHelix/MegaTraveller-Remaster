[CmdletBinding()]
param(
    [Parameter(Mandatory)]
    [ValidatePattern('^[A-Za-z0-9][A-Za-z0-9._-]{0,127}$')]
    [string]$Release,
    [Alias("OutDir")]
    [string]$Out = ""
)
$ErrorActionPreference = "Stop"
$root = Split-Path $PSScriptRoot -Parent
function Assert-NoBridge([string]$Executable) {
    $ascii = [Text.Encoding]::ASCII.GetString([IO.File]::ReadAllBytes($Executable))
    foreach ($marker in @("MT_DEV_ONLY_BRIDGE_IMPLEMENTATION_V1", "BUGBRIDGE v1", "Send to the fixer", "MT_FOREMAN_PANE", "Fix branch discarded")) {
        if ($ascii.Contains($marker)) { throw "Dev bridge marker in player executable: $marker" }
    }
}
function Assert-NoBannedTerms([string]$Directory) {
    $terms = @($env:USERNAME, $env:COMPUTERNAME) | Where-Object { $_ }
    if ($env:MT_BANNED_TERMS) { $terms += Get-Content -LiteralPath $env:MT_BANNED_TERMS | ForEach-Object { $_.Trim() } | Where-Object { $_ } }
    $pattern = (@($terms | ForEach-Object { [regex]::Escape($_) }) + 'Users[\\/][A-Za-z0-9]') -join '|'
    $options = [Text.RegularExpressions.RegexOptions]::IgnoreCase
    $hits = foreach ($file in Get-ChildItem -LiteralPath $Directory -Recurse -File) {
        $name = $file.FullName.Substring($Directory.Length + 1)
        $bytes = [IO.File]::ReadAllBytes($file.FullName)
        $texts = @($name, [Text.Encoding]::GetEncoding(28591).GetString($bytes), [Text.Encoding]::Unicode.GetString($bytes))
        if ($bytes.Length -gt 1) { $texts += [Text.Encoding]::Unicode.GetString($bytes, 1, $bytes.Length - 1) }
        foreach ($text in $texts) { foreach ($match in [regex]::Matches($text, $pattern, $options)) { "{0}: {1}" -f $name, $match.Value } }
    }
    if ($hits) {
        $hits | Sort-Object -Unique | ForEach-Object { Write-Host "BANNED TERM in $_" }
        throw "Package refused: a staged file contains a banned term."
    }
}
if (-not $Out) { $Out = Join-Path $root "local/rel" }
$Out = $ExecutionContext.SessionState.Path.GetUnresolvedProviderPathFromPSPath($Out)
$packageName = "MegaTraveller-1-Plus-$Release-windows-x64"
$buildArgs = @("build", "--release", "--no-default-features", "-p", "megatraveller", "--manifest-path", "$root\Cargo.toml", "--target-dir", "$root\target", "--target", "x86_64-pc-windows-msvc")
$executable = "$root\target\x86_64-pc-windows-msvc\release\megatraveller.exe"
$commit = & git -C $root rev-parse HEAD
if ($LASTEXITCODE -ne 0 -or [string]::IsNullOrWhiteSpace($commit)) { throw "Cannot package without the git commit." }
$commit = $commit.Trim().Substring(0, 12)
$cargoHome = if ($env:CARGO_HOME) { $env:CARGO_HOME } else { Join-Path $env:USERPROFILE ".cargo" }
$rustFlags = @("-C", "target-feature=+crt-static", "--remap-path-prefix=$($env:USERPROFILE)=home", "--remap-path-prefix=$cargoHome=cargo", "--remap-path-prefix=$root=.")
$oldMilestone, $oldHash, $oldFlags = $env:MT_MILESTONE, $env:MT_BUILD_HASH, $env:CARGO_ENCODED_RUSTFLAGS
try {
    $env:MT_MILESTONE = $Release
    $env:MT_BUILD_HASH = $commit
    $env:CARGO_ENCODED_RUSTFLAGS = $rustFlags -join [char]0x1f
    Push-Location $root
    & cargo @buildArgs
    if ($LASTEXITCODE) { throw "Release build failed (exit $LASTEXITCODE)." }
} finally {
    Pop-Location
    $env:MT_MILESTONE, $env:MT_BUILD_HASH, $env:CARGO_ENCODED_RUSTFLAGS = $oldMilestone, $oldHash, $oldFlags
}
$builtHash = (& $executable --build-id | Out-String).Trim()
if ($LASTEXITCODE -ne 0 -or $builtHash -ne $commit) { throw "Executable build id does not match the package commit." }
Assert-NoBridge $executable
if ([Text.Encoding]::ASCII.GetString([IO.File]::ReadAllBytes($executable)) -match '(?i)vcruntime140|msvcp140') { throw "Executable needs the Visual C++ runtime DLLs (crt-static not applied)." }
$stage = Join-Path $root ("local/package-stage/{0}-{1}" -f $packageName, [guid]::NewGuid().ToString("N"))
foreach ($folder in @("gamefiles", "fonts", "licences", "scenarios", "data", "schema", "data\tests\atoms", "docs")) { New-Item -ItemType Directory -Force "$stage\$folder" | Out-Null }
Copy-Item -LiteralPath $executable -Destination $stage
Copy-Item "$root\recomplauncher.json", "$root\README.md", "$root\LICENSE", "$root\RELEASE_NOTES.md" $stage
Set-Content "$stage\BUILD.txt" @("Release: $Release", "Commit: $builtHash")
foreach ($folder in @("scenarios", "data", "schema")) { Copy-Item "$root\$folder\*" "$stage\$folder\" }
Copy-Item "$root\tests\atoms\*.md" "$stage\data\tests\atoms\"
Copy-Item "$root\docs\KEYS.md" "$stage\docs\"
Copy-Item "$root\crates\app\fonts\OFL_*.txt", "$root\crates\app\fonts\CC-BY-SA-4.0_Px437.txt", "$root\crates\app\fonts\README_Px437.txt" "$stage\licences\"
Set-Content "$stage\README.txt" @(
    "MegaTraveller 1 Plus $Release (Windows x64)",
    "Read README.md for setup, saving, mouse and keys, Look settings, how to report a bug and credits; RELEASE_NOTES.md for known issues.",
    "Unzip, start megatraveller.exe, then drop the folder of your original MegaTraveller 1 game (the folder that contains the file TRAV.COM) onto the window, or press Choose folder... and pick it. Or copy the original files into gamefiles beside the exe. Original game files are never included.",
    "Mouse and keyboard both work. Press ? for help and F12 for the Plus window (journal, log, map, bug report). Ctrl+Shift+L opens Testing scenarios.",
    "Code: MIT, Copyright (c) 2026 DrEvil; see LICENSE. Fonts keep their own licences in licences: Px437 IBM EGA 8x14 by VileR / int10h.org, CC BY-SA 4.0; IBM Plex Mono and VT323, SIL OFL 1.1."
)
$manifest = Select-String -Path "$root\crates\mt1\src\gamefiles_manifest.rs" -Pattern '^\s*\("([^"]+)", (\d+), "' | ForEach-Object { "{0,-12} {1,7} bytes" -f $_.Matches[0].Groups[1].Value, $_.Matches[0].Groups[2].Value }
if ($manifest.Count -ne 64) { throw "gamefiles manifest: expected 64 rows, found $($manifest.Count)" }
Add-Content "$stage\README.txt" (@("", "The 64 original files (1990 MegaTraveller 1):") + $manifest)
Assert-NoBannedTerms $stage
New-Item -ItemType Directory -Force $Out | Out-Null
$zip = Join-Path $Out "$packageName.zip"
if (Test-Path -LiteralPath $zip) { Remove-Item -LiteralPath $zip }
Compress-Archive -Path "$stage\*" -DestinationPath $zip
Add-Type -AssemblyName System.IO.Compression.FileSystem
$z = [IO.Compression.ZipFile]::Open($zip, "Update"); $z.CreateEntry("gamefiles/") | Out-Null; $z.CreateEntry("fonts/") | Out-Null; $z.Dispose()
$z = [IO.Compression.ZipFile]::OpenRead($zip); $z.Entries | ForEach-Object { "{0,10}  {1}" -f $_.Length, $_.FullName }; $z.Dispose()
"wrote $zip"
