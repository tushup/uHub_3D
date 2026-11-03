# Fetch the pinned Intel Open Image Denoise 2.3.3 prebuilt (D-32).
# Downloads the CPU subset, stages the DLLs Tauri bundles next to the
# app and CLI, and prints the OIDN_DIR value `cargo build` needs.
$ErrorActionPreference = "Stop"

$Version = "2.3.3"
$RepoRoot = Split-Path -Parent $PSScriptRoot
$DownloadRoot = Join-Path $RepoRoot "src-tauri\.oidn\$Version"
$Extracted = Join-Path $DownloadRoot "oidn-$Version.x64.windows"
$StageDir = Join-Path $RepoRoot "src-tauri\oidn"
$ZipPath = Join-Path $DownloadRoot "oidn-$Version.x64.windows.zip"
$Url = "https://github.com/OpenImageDenoise/oidn/releases/download/v$Version/oidn-$Version.x64.windows.zip"

# CPU device only (D-32). Never the CUDA/HIP/SYCL/Metal devices.
$CpuDlls = @(
    "OpenImageDenoise.dll",
    "OpenImageDenoise_core.dll",
    "OpenImageDenoise_device_cpu.dll",
    "tbb12.dll",
    "tbbbind.dll",
    "tbbbind_2_0.dll",
    "tbbbind_2_5.dll"
)

New-Item -ItemType Directory -Force -Path $DownloadRoot | Out-Null
if (-not (Test-Path (Join-Path $Extracted "lib\OpenImageDenoise.lib"))) {
    if (-not (Test-Path $ZipPath)) {
        Write-Host "Downloading OIDN $Version from $Url"
        Invoke-WebRequest -Uri $Url -OutFile $ZipPath
    }
    Expand-Archive -Path $ZipPath -DestinationPath $DownloadRoot -Force
}

New-Item -ItemType Directory -Force -Path $StageDir | Out-Null
foreach ($dll in $CpuDlls) {
    $src = Join-Path $Extracted "bin\$dll"
    if (-not (Test-Path $src)) { throw "Missing OIDN DLL in prebuilt: $src" }
    Copy-Item $src (Join-Path $StageDir $dll) -Force
}

Write-Host "OIDN_DIR=$Extracted"
Write-Host "Set it before building, and put its bin folder on PATH for tests:"
Write-Host "`$env:OIDN_DIR = `"$Extracted`""
Write-Host "`$env:PATH = `"$Extracted\bin;`$env:PATH`""
if ($env:GITHUB_ENV) {
    "OIDN_DIR=$Extracted" | Out-File -FilePath $env:GITHUB_ENV -Append -Encoding utf8
    "PATH=$Extracted\bin;$env:PATH" | Out-File -FilePath $env:GITHUB_ENV -Append -Encoding utf8
}
