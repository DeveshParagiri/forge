# Install or update Forge from a checksummed GitHub Release binary on Windows.
#
#   irm https://raw.githubusercontent.com/DeveshParagiri/forge/main/scripts/install.ps1 | iex
#   $env:FORGE_VERSION = "0.5.0"; irm https://raw.githubusercontent.com/DeveshParagiri/forge/main/scripts/install.ps1 | iex
#   & ([scriptblock]::Create((irm https://raw.githubusercontent.com/DeveshParagiri/forge/main/scripts/install.ps1))) -Version 0.5.0

param(
    [Parameter(Position = 0)]
    [string]$Version
)

$ErrorActionPreference = 'Stop'
[Net.ServicePointManager]::SecurityProtocol = [Net.ServicePointManager]::SecurityProtocol -bor [Net.SecurityProtocolType]::Tls12
$ProgressPreference = 'SilentlyContinue'

if ($PSVersionTable.Platform -and $PSVersionTable.Platform -ne 'Win32NT') {
    Write-Error "This installer is for Windows. On macOS/Linux, use: curl -fsSL https://raw.githubusercontent.com/DeveshParagiri/forge/main/scripts/install | sh"
    exit 1
}

if (-not $Version -and $env:FORGE_VERSION) {
    $Version = $env:FORGE_VERSION
}

$Repo = if ($env:FORGE_REPO) { $env:FORGE_REPO } else { 'DeveshParagiri/forge' }
$HomeDir = if ($env:USERPROFILE) { $env:USERPROFILE } else { $env:HOME }
$InstallBin = if ($env:GROK_INSTALL_BIN) { $env:GROK_INSTALL_BIN } else { Join-Path $HomeDir '.grok\bin\grok.exe' }
$Updater = if ($env:GROK_UPDATER) { $env:GROK_UPDATER } else { Join-Path $HomeDir 'bin\grok-update-from-source.ps1' }

function Fail([string]$Message) {
    Write-Error "BLOCKED: $Message"
    exit 1
}

$arch = switch ($env:PROCESSOR_ARCHITECTURE) {
    'AMD64' { 'x86_64' }
    'x86'   { 'x86_64' }
    default { $null }
}

if (-not $arch) {
    Fail "no Forge release binary is available for Windows/$($env:PROCESSOR_ARCHITECTURE) (supported: Windows x86_64)"
}

$target = 'x86_64-pc-windows-msvc'
$asset = "forge-$target.zip"

if (-not $Version -or $Version -eq 'latest') {
    $base = "https://github.com/$Repo/releases/latest/download"
    $label = 'latest'
} else {
    $tag = if ($Version -like 'forge-v*') { $Version } else { "forge-v$Version" }
    $base = "https://github.com/$Repo/releases/download/$tag"
    $label = $tag
}

$tmp = Join-Path ([System.IO.Path]::GetTempPath()) ("forge-install-" + [guid]::NewGuid().ToString('N'))
New-Item -ItemType Directory -Path $tmp -Force | Out-Null

try {
    Write-Host "==> downloading Forge $label for $target"
    $zipPath = Join-Path $tmp $asset
    $shaPath = Join-Path $tmp "$asset.sha256"
    Invoke-WebRequest -Uri "$base/$asset" -OutFile $zipPath -UseBasicParsing
    Invoke-WebRequest -Uri "$base/$asset.sha256" -OutFile $shaPath -UseBasicParsing

    $expected = ((Get-Content -Raw $shaPath) -split '\s+')[0]
    $actual = (Get-FileHash -Path $zipPath -Algorithm SHA256).Hash
    if ($expected.ToLowerInvariant() -ne $actual.ToLowerInvariant()) {
        Fail "SHA-256 mismatch for $asset"
    }

    Expand-Archive -Path $zipPath -DestinationPath $tmp -Force
    $extracted = Join-Path $tmp 'grok.exe'
    if (-not (Test-Path $extracted)) {
        Fail "release archive did not contain grok.exe"
    }

    $binDir = Split-Path -Parent $InstallBin
    New-Item -ItemType Directory -Path $binDir -Force | Out-Null
    $staging = "$InstallBin.new.$PID"
    Copy-Item -Path $extracted -Destination $staging -Force
    $old = "$InstallBin.old"
    if (Test-Path $old) { Remove-Item $old -Force -ErrorAction SilentlyContinue }
    try {
        Move-Item -Path $staging -Destination $InstallBin -Force
    } catch {
        if (Test-Path $InstallBin) { Rename-Item $InstallBin $old -Force }
        Move-Item -Path $staging -Destination $InstallBin -Force
    }

    $updaterDir = Split-Path -Parent $Updater
    New-Item -ItemType Directory -Path $updaterDir -Force | Out-Null
    $updaterSrc = "https://raw.githubusercontent.com/$Repo/main/scripts/install.ps1"
    $updaterTmp = "$Updater.new.$PID"
    Invoke-WebRequest -Uri $updaterSrc -OutFile $updaterTmp -UseBasicParsing
    Move-Item -Path $updaterTmp -Destination $Updater -Force

    $userPath = [Environment]::GetEnvironmentVariable('Path', 'User')
    $entries = if ($userPath) { $userPath -split ';' | Where-Object { $_ -ne '' } } else { @() }
    if ($entries -notcontains $binDir) {
        [Environment]::SetEnvironmentVariable('Path', (@($binDir) + $entries) -join ';', 'User')
        if ($env:Path -notlike "*$binDir*") {
            $env:Path = "$binDir;$env:Path"
        }
    }

    $reported = 'Forge'
    try { $reported = & $InstallBin --version } catch {}
    Write-Host "Installed: $reported"
    Write-Host "  binary: $InstallBin"
    Write-Host "  update: grok update"
} finally {
    if (Test-Path $tmp) { Remove-Item $tmp -Recurse -Force -ErrorAction SilentlyContinue }
}
