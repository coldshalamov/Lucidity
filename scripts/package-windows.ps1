[CmdletBinding()]
param(
    [switch] $SkipBuild,

    [ValidateSet("Debug", "Release")]
    [string] $Configuration,

    [string] $TargetDirectory,

    [string] $VsixPath
)

Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"

function Invoke-Checked {
    param(
        [Parameter(Mandatory = $true)]
        [string] $Executable,

        [Parameter(Mandatory = $true)]
        [string[]] $Arguments
    )

    & $Executable @Arguments
    if ($LASTEXITCODE -ne 0) {
        throw "Command failed with exit code ${LASTEXITCODE}: $Executable $($Arguments -join ' ')"
    }
}

function Resolve-RepositoryPath {
    param(
        [Parameter(Mandatory = $true)]
        [string] $RepositoryRoot,

        [Parameter(Mandatory = $true)]
        [string] $Path
    )

    if ([System.IO.Path]::IsPathRooted($Path)) {
        return [System.IO.Path]::GetFullPath($Path)
    }

    return [System.IO.Path]::GetFullPath((Join-Path $RepositoryRoot $Path))
}

function Copy-RequiredFile {
    param(
        [Parameter(Mandatory = $true)]
        [string] $Source,

        [Parameter(Mandatory = $true)]
        [string] $Destination
    )

    if (-not (Test-Path -LiteralPath $Source -PathType Leaf)) {
        throw "Required packaging input does not exist: $Source"
    }

    Copy-Item -LiteralPath $Source -Destination $Destination -Force
}

if ([System.Environment]::OSVersion.Platform -ne [System.PlatformID]::Win32NT) {
    throw "This packager produces the native Windows distribution and must run on Windows."
}

$scriptDirectory = Split-Path -Parent $PSCommandPath
$repositoryRoot = [System.IO.Path]::GetFullPath((Join-Path $scriptDirectory ".."))
$workspaceManifest = Join-Path $repositoryRoot "Cargo.toml"
$agentManifest = Join-Path $repositoryRoot "agent-terminal\Cargo.toml"
$extensionManifestPath = Join-Path $repositoryRoot "extensions\vscode\package.json"

foreach ($marker in @($workspaceManifest, $agentManifest, $extensionManifestPath)) {
    if (-not (Test-Path -LiteralPath $marker -PathType Leaf)) {
        throw "The script is not inside a Lucidity checkout; missing marker: $marker"
    }
}

if (-not $PSBoundParameters.ContainsKey("Configuration")) {
    $Configuration = if ($SkipBuild) { "Debug" } else { "Release" }
}

if (-not $PSBoundParameters.ContainsKey("TargetDirectory")) {
    $TargetDirectory = if ($env:CARGO_TARGET_DIR) {
        $env:CARGO_TARGET_DIR
    } else {
        "target"
    }
}

$targetRoot = Resolve-RepositoryPath -RepositoryRoot $repositoryRoot -Path $TargetDirectory
$profileName = $Configuration.ToLowerInvariant()
$artifactDirectory = Join-Path $targetRoot $profileName
$extensionRoot = Join-Path $repositoryRoot "extensions\vscode"
$extensionManifest = Get-Content -LiteralPath $extensionManifestPath -Raw | ConvertFrom-Json
$expectedVsixName = "$($extensionManifest.name)-$($extensionManifest.version).vsix"

if (-not $SkipBuild) {
    $cargo = (Get-Command cargo.exe -ErrorAction Stop).Source
    $npm = (Get-Command npm.cmd -ErrorAction Stop).Source
    $cargoProfileArguments = if ($Configuration -eq "Release") { @("--release") } else { @() }

    Push-Location $repositoryRoot
    try {
        Invoke-Checked -Executable $cargo -Arguments (@(
                "build",
                "--locked",
                "--package", "agent-terminal",
                "--bin", "agent"
            ) + $cargoProfileArguments)
        Invoke-Checked -Executable $cargo -Arguments (@(
                "build",
                "--locked",
                "--package", "agent-backends",
                "--bin", "lucidity-mock-agent"
            ) + $cargoProfileArguments)
    } finally {
        Pop-Location
    }

    Push-Location $extensionRoot
    try {
        Invoke-Checked -Executable $npm -Arguments @("ci")
        Invoke-Checked -Executable $npm -Arguments @("run", "package")
    } finally {
        Pop-Location
    }
}

$agentExecutable = Join-Path $artifactDirectory "agent.exe"
$mockExecutable = Join-Path $artifactDirectory "lucidity-mock-agent.exe"

if ($PSBoundParameters.ContainsKey("VsixPath")) {
    $resolvedVsixPath = Resolve-RepositoryPath -RepositoryRoot $repositoryRoot -Path $VsixPath
} else {
    $resolvedVsixPath = Join-Path $extensionRoot "dist\$expectedVsixName"
}

foreach ($requiredArtifact in @($agentExecutable, $mockExecutable, $resolvedVsixPath)) {
    if (-not (Test-Path -LiteralPath $requiredArtifact -PathType Leaf)) {
        $hint = if ($SkipBuild) {
            "Run a build first or supply the matching -TargetDirectory/-VsixPath to -SkipBuild."
        } else {
            "The build completed without producing the expected artifact."
        }
        throw "Missing required artifact: $requiredArtifact`n$hint"
    }
}

$distParent = [System.IO.Path]::GetFullPath((Join-Path $repositoryRoot "dist"))
$distRoot = [System.IO.Path]::GetFullPath((Join-Path $distParent "Lucidity"))
$actualDistParent = [System.IO.Path]::GetFullPath((Split-Path -Parent $distRoot))

if (-not [System.StringComparer]::OrdinalIgnoreCase.Equals($actualDistParent, $distParent)) {
    throw "Refusing to assemble outside the repository distribution directory: $distRoot"
}

if (Test-Path -LiteralPath $distRoot) {
    Remove-Item -LiteralPath $distRoot -Recurse -Force
}

$binDirectory = New-Item -ItemType Directory -Path (Join-Path $distRoot "bin") -Force
$adapterDirectory = New-Item -ItemType Directory -Path (Join-Path $distRoot "adapters") -Force
$extensionDirectory = New-Item -ItemType Directory -Path (Join-Path $distRoot "extensions") -Force
$licenseDirectory = New-Item -ItemType Directory -Path (Join-Path $distRoot "licenses") -Force

Copy-RequiredFile -Source $agentExecutable -Destination (Join-Path $binDirectory.FullName "agent.exe")
Copy-RequiredFile -Source $mockExecutable -Destination (Join-Path $binDirectory.FullName "lucidity-mock-agent.exe")

foreach ($sidecarPattern in @("*.dll", "agent.pdb", "lucidity-mock-agent.pdb")) {
    Get-ChildItem -LiteralPath $artifactDirectory -Filter $sidecarPattern -File -ErrorAction SilentlyContinue |
        Sort-Object -Property Name |
        ForEach-Object {
            Copy-Item -LiteralPath $_.FullName -Destination (Join-Path $binDirectory.FullName $_.Name) -Force
        }
}

$adapterSource = Join-Path $repositoryRoot "agent-backends\adapters"
$adapterPackages = @(Get-ChildItem -LiteralPath $adapterSource -Directory -Filter "*.agent-adapter" |
        Sort-Object -Property Name)
if ($adapterPackages.Count -eq 0) {
    throw "No adapter packages were found below $adapterSource"
}

foreach ($adapterPackage in $adapterPackages) {
    $adapterDestination = Join-Path $adapterDirectory.FullName $adapterPackage.Name
    Copy-Item -LiteralPath $adapterPackage.FullName -Destination $adapterDestination -Recurse -Force
}

Copy-RequiredFile -Source $resolvedVsixPath -Destination (Join-Path $extensionDirectory.FullName $expectedVsixName)
Copy-RequiredFile -Source (Join-Path $repositoryRoot "LICENSE.md") -Destination (Join-Path $licenseDirectory.FullName "PROJECT-LICENSE.md")
Copy-RequiredFile -Source (Join-Path $extensionRoot "LICENSE") -Destination (Join-Path $licenseDirectory.FullName "Lucidity-VSCode-MIT.txt")

$upstreamLicenseSource = Join-Path $repositoryRoot "licenses"
if (Test-Path -LiteralPath $upstreamLicenseSource -PathType Container) {
    Copy-Item -LiteralPath $upstreamLicenseSource -Destination (Join-Path $licenseDirectory.FullName "WezTerm") -Recurse -Force
}

$fontLicenseDirectory = New-Item -ItemType Directory -Path (Join-Path $licenseDirectory.FullName "Fonts") -Force
Get-ChildItem -LiteralPath (Join-Path $repositoryRoot "assets\fonts") -File -Filter "LICENSE*" |
    Sort-Object -Property Name |
    ForEach-Object {
        Copy-Item -LiteralPath $_.FullName -Destination (Join-Path $fontLicenseDirectory.FullName $_.Name) -Force
    }

Copy-RequiredFile -Source (Join-Path $repositoryRoot "packaging\Start-Lucidity.ps1") -Destination (Join-Path $distRoot "Start-Lucidity.ps1")
Copy-RequiredFile -Source (Join-Path $repositoryRoot "packaging\README.md") -Destination (Join-Path $distRoot "README.md")

$git = (Get-Command git.exe -ErrorAction Stop).Source
$commit = (& $git -C $repositoryRoot rev-parse HEAD).Trim()
if ($LASTEXITCODE -ne 0) {
    throw "Unable to record the packaged Git revision."
}

@(
    "Lucidity Windows distribution"
    "Commit: $commit"
    "Configuration: $Configuration"
    "Extension: $($extensionManifest.displayName) $($extensionManifest.version)"
) | Set-Content -LiteralPath (Join-Path $distRoot "BUILD-INFO.txt") -Encoding UTF8

$checksumPath = Join-Path $distRoot "SHA256SUMS.txt"
$checksumLines = Get-ChildItem -LiteralPath $distRoot -Recurse -File |
    Where-Object { $_.FullName -ne $checksumPath } |
    ForEach-Object {
        $relativePath = $_.FullName.Substring($distRoot.Length + 1).Replace("\", "/")
        $hash = (Get-FileHash -LiteralPath $_.FullName -Algorithm SHA256).Hash.ToLowerInvariant()
        "$hash *$relativePath"
    } |
    Sort-Object
$checksumLines | Set-Content -LiteralPath $checksumPath -Encoding ASCII

Write-Host "Lucidity distribution assembled at: $distRoot"
Write-Host "Run: powershell -ExecutionPolicy Bypass -File `"$distRoot\Start-Lucidity.ps1`""
Write-Host "Install extension: code --install-extension `"$distRoot\extensions\$expectedVsixName`""
