#Requires -Version 7.4
# Runs on Linux or Windows. Fixtures are synthetic packaging inputs, never app build artifacts.
. "$PSScriptRoot/common.ps1"
$root = (Resolve-Path "$PSScriptRoot/../../..").Path
$scripts = @(Get-ChildItem "$root/shells/windows" -Filter '*.ps1' -Recurse)
$scripts += Get-Item "$root/tools/zed-vendor.ps1"
foreach ($script in $scripts) {
    $tokens = $null; $errors = $null
    $null = [System.Management.Automation.Language.Parser]::ParseFile($script.FullName, [ref]$tokens, [ref]$errors)
    if ($errors.Count) { throw "$($script.FullName): $($errors -join '; ')" }
}
$scratch = Join-Path $root "build/windows/fixtures-$([guid]::NewGuid())"
$app = Join-Path $scratch 'OptChat'
$null = New-Item -ItemType Directory -Force "$app/notices"
function Expect-Failure {
    param([scriptblock]$Action)
    $failed = $false
    try { & $Action } catch { $failed = $true }
    if (-not $failed) { throw 'An invalid package input was accepted.' }
}
try {
    foreach ($arch in @('x64', 'arm64')) {
        $bytes = [byte[]]::new(256)
        [BitConverter]::GetBytes([UInt16]0x5a4d).CopyTo($bytes, 0)
        [BitConverter]::GetBytes([int]0x80).CopyTo($bytes, 0x3c)
        [BitConverter]::GetBytes([UInt32]0x4550).CopyTo($bytes, 0x80)
        $machine = if ($arch -eq 'x64') { 0x8664 } else { 0xaa64 }
        [BitConverter]::GetBytes([UInt16]$machine).CopyTo($bytes, 0x84)
        [IO.File]::WriteAllBytes("$app/optchat.exe", $bytes)
        if ((Get-PeArchitecture "$app/optchat.exe") -ne $arch) { throw 'PE architecture detection failed.' }
        Write-PackageManifest "$root/shells/windows/packaging/AppxManifest.xml.in" "$scratch/AppxManifest.xml" $arch '0.1.0.42' 'CN=Test & Co'
        [xml]$manifest = Get-Content "$scratch/AppxManifest.xml" -Raw
        if ($manifest.Package.Identity.Publisher -cne 'CN=Test & Co' -or $manifest.Package.Identity.ProcessorArchitecture -ne $arch) { throw 'Manifest substitution failed.' }
        $min = if ($arch -eq 'arm64') { '10.0.22000.0' } else { '10.0.19045.0' }
        if ($manifest.Package.Dependencies.TargetDeviceFamily.MinVersion -ne $min) { throw 'OS minimum mismatch.' }
        Write-Utf8 "$app/build-info.json" (@{architecture=$arch; static_crt=$true; version='0.1.0.42'; publisher='CN=Test & Co'} | ConvertTo-Json)
        Write-Utf8 "$app/notices/fixture.txt" 'Synthetic test fixture'
        $zip = Join-Path $scratch "$arch.zip"
        Compress-Archive -Path $app -DestinationPath $zip
        Write-Checksum $zip
        & "$PSScriptRoot/check-package.ps1" -Package $zip -Architecture $arch
        $wrongArch = if ($arch -eq 'x64') { 'arm64' } else { 'x64' }
        Expect-Failure { & "$PSScriptRoot/check-package.ps1" -Package $zip -Architecture $wrongArch }
        # Exercise MSIX layout without claiming MakeAppx schema/signing validation on Linux.
        Copy-Item "$scratch/AppxManifest.xml" $app
        Copy-Item "$root/shells/windows/packaging/Assets" $app -Recurse -Force
        $msix = Join-Path $scratch "$arch.msix"
        [IO.Compression.ZipFile]::CreateFromDirectory($app, $msix)
        Write-Checksum $msix
        & "$PSScriptRoot/check-package.ps1" -Package $msix -Architecture $arch
        Remove-Item "$app/AppxManifest.xml", "$app/Assets" -Recurse -Force
        [IO.File]::AppendAllText($zip, 'corruption')
        Expect-Failure { & "$PSScriptRoot/check-package.ps1" -Package $zip -Architecture $arch }
    }
    [IO.File]::WriteAllBytes("$app/optchat.exe", [byte[]]::new(256))
    Expect-Failure { Get-PeArchitecture "$app/optchat.exe" }
    Expect-Failure { Write-PackageManifest "$root/shells/windows/packaging/AppxManifest.xml.in" "$scratch/bad.xml" 'x86' '0.1.0.0' 'CN=Test' }
    Expect-Failure { Write-PackageManifest "$root/shells/windows/packaging/AppxManifest.xml.in" "$scratch/bad.xml" 'x64' '0.1.0-beta' 'CN=Test' }
    Expect-Failure { Write-PackageManifest "$root/shells/windows/packaging/AppxManifest.xml.in" "$scratch/bad.xml" 'x64' '0.1.0.65536' 'CN=Test' }
    Write-Host 'PASS: PowerShell parsing, both architecture layouts, manifest escaping/minimum OS, wrong-architecture and corruption rejection.'
} finally { Remove-Item -LiteralPath $scratch -Recurse -Force }
