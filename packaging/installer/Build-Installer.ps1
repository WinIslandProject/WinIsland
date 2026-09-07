param(
    [Parameter(Mandatory)]
    [ValidateSet('stable', 'nightly')]
    [string]$Channel,

    [Parameter(Mandatory)]
    [string]$Version,

    [Parameter(Mandatory)]
    [string]$CertificatePath,

    [Parameter(Mandatory)]
    [string]$CertificatePassword,

    [Parameter(Mandatory)]
    [string]$OutputDirectory
)

$ErrorActionPreference = 'Stop'
$OutputDirectory = [IO.Path]::GetFullPath($OutputDirectory)

function Get-InnoCompiler {
    $command = Get-Command ISCC.exe -ErrorAction SilentlyContinue
    if ($null -ne $command) {
        return $command.Source
    }

    $paths = @(
        (Join-Path ${env:ProgramFiles(x86)} 'Inno Setup 6\ISCC.exe'),
        (Join-Path $env:LOCALAPPDATA 'Programs\Inno Setup 6\ISCC.exe')
    )
    foreach ($path in $paths) {
        if (Test-Path $path) {
            return $path
        }
    }

    throw 'ISCC.exe was not found. Install Inno Setup 6.'
}

function Get-SignTool {
    $roots = @(
        "${env:ProgramFiles(x86)}\Windows Kits\10\bin",
        "$env:ProgramFiles\Windows Kits\10\bin"
    )
    foreach ($root in $roots) {
        $tool = Get-ChildItem -Path $root -Filter SignTool.exe -Recurse -ErrorAction SilentlyContinue |
            Where-Object { $_.FullName -match '\\x64\\' } |
            Sort-Object FullName -Descending |
            Select-Object -First 1
        if ($null -ne $tool) {
            return $tool.FullName
        }
    }
    throw 'SignTool.exe was not found. Install the Windows 10 or Windows 11 SDK.'
}

$repositoryRoot = (Resolve-Path (Join-Path $PSScriptRoot '..\..')).Path
$releaseDirectory = Join-Path $repositoryRoot 'target\release'
$stageDirectory = Join-Path $OutputDirectory 'stage'
$identityDirectory = Join-Path $stageDirectory 'identity'
$resourceDirectory = Join-Path $stageDirectory 'resources'
$executablePath = Join-Path $releaseDirectory 'WinIsland.exe'

if (-not (Test-Path $executablePath)) {
    throw "Release executable was not found: $executablePath"
}

Remove-Item -Recurse -Force -ErrorAction SilentlyContinue $stageDirectory
New-Item -ItemType Directory -Force -Path $identityDirectory, $resourceDirectory, $OutputDirectory | Out-Null

Copy-Item -Path $executablePath -Destination (Join-Path $stageDirectory 'WinIsland.exe') -Force
Copy-Item -Path (Join-Path $repositoryRoot 'resources\icon-dark.png') -Destination (Join-Path $resourceDirectory 'icon-dark.png') -Force
Copy-Item -Path (Join-Path $repositoryRoot 'resources\icon-dark.ico') -Destination (Join-Path $resourceDirectory 'icon-dark.ico') -Force
Copy-Item -Path (Join-Path $repositoryRoot 'resources\licenses') -Destination $resourceDirectory -Recurse -Force
Copy-Item -Path (Join-Path $repositoryRoot 'packaging\identity\Install-Identity.ps1') -Destination (Join-Path $identityDirectory 'Install-Identity.ps1') -Force
Copy-Item -Path (Join-Path $repositoryRoot 'packaging\identity\Remove-Identity.ps1') -Destination (Join-Path $identityDirectory 'Remove-Identity.ps1') -Force

& (Join-Path $repositoryRoot 'packaging\identity\Build-IdentityPackage.ps1') `
    -Channel $Channel `
    -Version $Version `
    -CertificatePath $CertificatePath `
    -CertificatePassword $CertificatePassword `
    -OutputDirectory $identityDirectory

$installerOutput = Join-Path $OutputDirectory 'installer'
New-Item -ItemType Directory -Force -Path $installerOutput | Out-Null

$compiler = Get-InnoCompiler
& $compiler `
    "/DChannel=$Channel" `
    "/DSourceDirectory=$stageDirectory" `
    "/DOutputDirectory=$installerOutput" `
    "/DVersion=$Version" `
    (Join-Path $repositoryRoot 'packaging\installer\WinIsland.iss')
if ($LASTEXITCODE -ne 0) {
    throw 'Inno Setup failed to build the installer.'
}

$installerName = if ($Channel -eq 'stable') {
    'WinIsland-Setup.exe'
} else {
    'WinIsland-Nightly-Setup.exe'
}
$installerPath = Join-Path $installerOutput $installerName
$signTool = Get-SignTool
& $signTool sign /fd SHA256 /f $CertificatePath /p $CertificatePassword $installerPath
if ($LASTEXITCODE -ne 0) {
    throw 'SignTool failed to sign the installer.'
}
