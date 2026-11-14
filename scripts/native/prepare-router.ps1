param()
$ErrorActionPreference = 'Stop'
$repositoryRoot = [IO.Path]::GetFullPath((Join-Path $PSScriptRoot '../..'))
$routerDirectory = Join-Path $repositoryRoot 'target/native-router'
$sourcesDirectory = Join-Path $routerDirectory 'SOURCES'
New-Item -ItemType Directory -Path $sourcesDirectory -Force | Out-Null

function Get-VerifiedFile($Url, $Destination, $Sha256) {
    if (Test-Path -LiteralPath $Destination) {
        if ((Get-FileHash -Algorithm SHA256 -LiteralPath $Destination).Hash -eq $Sha256) { return }
    }
    Invoke-WebRequest -UseBasicParsing -Uri $Url -OutFile $Destination
    if ((Get-FileHash -Algorithm SHA256 -LiteralPath $Destination).Hash -ne $Sha256) {
        throw "SHA256 mismatch: $Destination"
    }
}

Get-VerifiedFile 'https://github.com/TypeFox/libavoid-server/releases/download/v0.3.1/libavoid-server-win.exe' `
    (Join-Path $routerDirectory 'libavoid-server.exe') '4676B8B4FD6F37AC2A74A5D8D11B023E45F34EB6BD8887F217BC776F4ACAD74D'
Get-VerifiedFile 'https://codeload.github.com/TypeFox/libavoid-server/tar.gz/refs/tags/v0.3.1' `
    (Join-Path $sourcesDirectory 'libavoid-server-v0.3.1.tar.gz') 'F34567F66D67BDB58F1557C45B89F5247EEC9008492000AA2FBA30B20EF1192F'
Get-VerifiedFile 'https://codeload.github.com/mjwybrow/adaptagrams/tar.gz/fcac991f9f7d959e8326ae0567ce122839fbd4be' `
    (Join-Path $sourcesDirectory 'adaptagrams-fcac991.tar.gz') '3EA743821E2CAA3BF5E6D7A700368EE84547CE8BEFE8C3A5E61377240C1A81F6'

& tar -xOf (Join-Path $sourcesDirectory 'libavoid-server-v0.3.1.tar.gz') 'libavoid-server-0.3.1/LICENSE.md' |
    Set-Content -Encoding utf8 (Join-Path $routerDirectory 'LICENSE-libavoid-server.txt')
if ($LASTEXITCODE -ne 0) { throw 'Cannot extract libavoid-server license' }
& tar -xOf (Join-Path $sourcesDirectory 'adaptagrams-fcac991.tar.gz') 'adaptagrams-fcac991f9f7d959e8326ae0567ce122839fbd4be/cola/LICENSE' |
    Set-Content -Encoding utf8 (Join-Path $routerDirectory 'LICENSE-libavoid.txt')
if ($LASTEXITCODE -ne 0) { throw 'Cannot extract libavoid license' }
Copy-Item -LiteralPath (Join-Path $repositoryRoot 'docs/ROUTER-NOTICES.txt') -Destination $routerDirectory -Force
Copy-Item -LiteralPath $PSCommandPath -Destination $sourcesDirectory -Force
Copy-Item -LiteralPath (Join-Path $PSScriptRoot 'prepare-router.sh') -Destination $sourcesDirectory -Force
Write-Output "Prepared ELK Libavoid router: $routerDirectory"
