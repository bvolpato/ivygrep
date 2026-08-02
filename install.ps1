$ErrorActionPreference = "Stop"
Set-StrictMode -Version Latest

$repository = "bvolpato/ivygrep"
$configuredInstallDir = [Environment]::GetEnvironmentVariable("IVYGREP_INSTALL_DIR")
$configuredVersion = [Environment]::GetEnvironmentVariable("IVYGREP_VERSION")
$installDir = if ($configuredInstallDir) {
    $configuredInstallDir
} else {
    Join-Path $env:LOCALAPPDATA "ivygrep\bin"
}
$version = if ($configuredVersion) { $configuredVersion } else { "latest" }

function Get-GitHubHeaders {
    $token = [Environment]::GetEnvironmentVariable("GITHUB_TOKEN")
    if (-not $token) {
        $token = [Environment]::GetEnvironmentVariable("GH_TOKEN")
    }

    $headers = @{
        "User-Agent" = "ivygrep-installer"
    }
    if ($token) {
        $headers["Authorization"] = "Bearer $token"
        $headers["X-GitHub-Api-Version"] = "2022-11-28"
    }
    return $headers
}

function Invoke-WithRetry {
    param(
        [Parameter(Mandatory = $true)]
        [scriptblock]$Operation,
        [Parameter(Mandatory = $true)]
        [string]$Description,
        [int]$MaxAttempts = 5
    )

    for ($attempt = 1; $attempt -le $MaxAttempts; $attempt++) {
        try {
            return & $Operation
        } catch {
            if ($attempt -eq $MaxAttempts) {
                throw
            }

            $delaySeconds = [int][Math]::Pow(2, $attempt - 1)
            Write-Warning "$Description failed (attempt $attempt/$MaxAttempts). Retrying in $delaySeconds seconds: $($_.Exception.Message)"
            Start-Sleep -Seconds $delaySeconds
        }
    }
}

if ($version -eq "latest") {
    $release = Invoke-WithRetry -Description "Release metadata request" -Operation {
        Invoke-RestMethod `
            -Uri "https://api.github.com/repos/$repository/releases/latest" `
            -Headers (Get-GitHubHeaders)
    }
    $tag = $release.tag_name
} elseif ($version.StartsWith("v")) {
    $tag = $version
} else {
    $tag = "v$version"
}

$asset = "ivygrep-$tag-windows-x86_64.zip"
$baseUrl = if ($env:IVYGREP_BASE_URL) {
    $env:IVYGREP_BASE_URL.TrimEnd('/')
} else {
    "https://github.com/$repository/releases/download/$tag"
}
$localArchive = [Environment]::GetEnvironmentVariable("IVYGREP_INSTALL_ARCHIVE")
$localChecksum = [Environment]::GetEnvironmentVariable("IVYGREP_INSTALL_CHECKSUM")
$tempDir = Join-Path ([System.IO.Path]::GetTempPath()) "ivygrep-$([guid]::NewGuid())"

try {
    Write-Host "ivygrep installer: selected portable archive (windows-x86_64)"
    New-Item $tempDir -ItemType Directory -Force | Out-Null
    $archivePath = Join-Path $tempDir $asset
    $checksumPath = "$archivePath.sha256"
    if ($localArchive) {
        if (-not (Test-Path -LiteralPath $localArchive -PathType Leaf)) {
            throw "IVYGREP_INSTALL_ARCHIVE does not exist: $localArchive"
        }
        if (-not $localChecksum) {
            $localChecksum = "$localArchive.sha256"
        }
        if (-not (Test-Path -LiteralPath $localChecksum -PathType Leaf)) {
            throw "IVYGREP_INSTALL_CHECKSUM does not exist: $localChecksum"
        }
        Copy-Item -LiteralPath $localArchive -Destination $archivePath -Force
        Copy-Item -LiteralPath $localChecksum -Destination $checksumPath -Force
    } else {
        Invoke-WithRetry -Description "Archive download" -Operation {
            Invoke-WebRequest "$baseUrl/$asset" -OutFile $archivePath
        } | Out-Null
        Invoke-WithRetry -Description "Checksum download" -Operation {
            Invoke-WebRequest "$baseUrl/$asset.sha256" -OutFile $checksumPath
        } | Out-Null
    }

    $expected = (Get-Content $checksumPath -Raw).Split()[0].ToLowerInvariant()
    $actual = (Get-FileHash $archivePath -Algorithm SHA256).Hash.ToLowerInvariant()
    if ($actual -ne $expected) {
        throw "SHA-256 mismatch for $asset"
    }

    Expand-Archive $archivePath -DestinationPath $tempDir -Force
    New-Item $installDir -ItemType Directory -Force | Out-Null
    $source = Join-Path $tempDir "ivygrep-$tag-windows-x86_64\ig.exe"
    Copy-Item $source (Join-Path $installDir "ig.exe") -Force

    $userPath = [Environment]::GetEnvironmentVariable("Path", "User")
    $pathEntries = @($userPath -split ";" | Where-Object { $_ })
    if ($pathEntries -notcontains $installDir) {
        $newPath = (@($pathEntries) + $installDir) -join ";"
        [Environment]::SetEnvironmentVariable("Path", $newPath, "User")
    }
    $env:Path = "$installDir;$env:Path"

    Write-Host "Installed ivygrep $tag to $installDir\ig.exe"
    & (Join-Path $installDir "ig.exe") --version
} finally {
    Remove-Item $tempDir -Recurse -Force -ErrorAction SilentlyContinue
}
