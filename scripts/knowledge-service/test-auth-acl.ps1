# Runs the Knowledge Service auth/ACL lifecycle against an isolated pgvector
# Docker container. It never connects to ripple.db or a host PostgreSQL service.

[CmdletBinding()]
param(
    [int]$Port = 55432
)

$ErrorActionPreference = "Stop"
$projectRoot = (Resolve-Path "$PSScriptRoot\..\..").Path
$composeFile = Join-Path $projectRoot "docker-compose.knowledge-test.yml"
$dataRoot = Join-Path $projectRoot ".knowledge-service-test-data"
$servicePort = 18789
$bootstrapToken = "test-bootstrap-token-not-for-production-0123456789"
$serviceProcess = $null
$serverLog = Join-Path $dataRoot "server.log"

function Invoke-JsonRequest {
    param(
        [Parameter(Mandatory = $true)][string]$Method,
        [Parameter(Mandatory = $true)][string]$Path,
        [object]$Body,
        [string]$BearerToken,
        [int[]]$ExpectedStatus = @(200)
    )

    $headers = @{ "x-request-id" = [guid]::NewGuid().ToString() }
    if ($BearerToken) { $headers.Authorization = "Bearer $BearerToken" }
    $request = @{
        Method = $Method
        Uri = "http://127.0.0.1:$servicePort$Path"
        Headers = $headers
        SkipHttpErrorCheck = $true
    }
    if ($null -ne $Body) {
        $request.ContentType = "application/json"
        $request.Body = ($Body | ConvertTo-Json -Depth 8 -Compress)
    }
    $response = Invoke-WebRequest @request
    if ($response.StatusCode -notin $ExpectedStatus) {
        throw "Unexpected HTTP status $($response.StatusCode) for $Method $Path"
    }
    if (-not $response.Headers["x-request-id"]) {
        throw "Missing x-request-id for $Method $Path"
    }
    if ([string]::IsNullOrWhiteSpace($response.Content)) { return $null }
    return $response.Content | ConvertFrom-Json
}

function Assert-Equal {
    param([object]$Actual, [object]$Expected, [string]$Message)
    if ($Actual -ne $Expected) { throw "$Message. Expected '$Expected', got '$Actual'." }
}

$dockerAvailable = $false
try {
    docker version --format '{{.Server.Version}}' | Out-Null
    $dockerAvailable = $LASTEXITCODE -eq 0
} catch {}
if (-not $dockerAvailable) {
    Write-Host "SKIP: Docker daemon is unavailable; auth/ACL integration was not run."
    exit 0
}

$env:RIPPLE_KNOWLEDGE_TEST_PORT = "$Port"
try {
    docker compose -f $composeFile up -d --wait | Out-Host
    docker compose -f $composeFile exec -T postgres psql -U ripple_test -d ripple_knowledge_test -c "CREATE EXTENSION IF NOT EXISTS vector" | Out-Null

    Remove-Item -Recurse -Force $dataRoot -ErrorAction SilentlyContinue
    New-Item -ItemType Directory -Path $dataRoot -Force | Out-Null
    $env:RIPPLE_KNOWLEDGE_DATABASE_URL = "postgres://ripple_test:ripple_test_only@127.0.0.1:$Port/ripple_knowledge_test"
    $env:RIPPLE_KNOWLEDGE_BOOTSTRAP_TOKEN = $bootstrapToken
    $env:RIPPLE_KNOWLEDGE_DATA_ROOT = $dataRoot
    $env:RIPPLE_KNOWLEDGE_LISTEN_ADDR = "127.0.0.1:$servicePort"
    $env:RIPPLE_KNOWLEDGE_LOG = "ripple_knowledge_server=warn"

    $binary = Join-Path $projectRoot "src-tauri\target\debug\ripple-knowledge-server.exe"
    cargo build --manifest-path (Join-Path $projectRoot "src-tauri\Cargo.toml") -p ripple-knowledge-server | Out-Host
    $serviceProcess = Start-Process -FilePath $binary -RedirectStandardOutput $serverLog -RedirectStandardError $serverLog -PassThru -WindowStyle Hidden

    $ready = $false
    for ($attempt = 0; $attempt -lt 30; $attempt++) {
        try {
            $health = Invoke-JsonRequest -Method GET -Path "/health/ready" -ExpectedStatus @(200)
            if ($health.status -eq "ready") { $ready = $true; break }
        } catch { Start-Sleep -Milliseconds 500 }
    }
    if (-not $ready) { throw "Knowledge Service did not become ready." }

    $admin = Invoke-JsonRequest -Method POST -Path "/api/v1/bootstrap" -Body @{
        bootstrap_token = $bootstrapToken
        username = "admin_user"
        password = "secure-password-123"
        device_name = "integration-admin"
    } -ExpectedStatus @(200)
    if (-not $admin.access_token -or -not $admin.refresh_token) { throw "Bootstrap did not issue opaque credentials." }

    $bootstrapAgain = Invoke-JsonRequest -Method POST -Path "/api/v1/bootstrap" -Body @{
        bootstrap_token = $bootstrapToken
        username = "other_admin"
        password = "secure-password-123"
        device_name = "integration-admin"
    } -ExpectedStatus @(409)
    Assert-Equal $bootstrapAgain.error.code "conflict" "Second bootstrap must conflict"

    $identity = Invoke-JsonRequest -Method GET -Path "/api/v1/auth/me" -BearerToken $admin.access_token -ExpectedStatus @(200)
    Assert-Equal $identity.username "admin_user" "Authenticated identity mismatch"

    $member = Invoke-JsonRequest -Method POST -Path "/api/v1/users" -BearerToken $admin.access_token -Body @{
        username = "viewer_user"
        password = "secure-password-456"
    } -ExpectedStatus @(201)

    $collectionOne = Invoke-JsonRequest -Method POST -Path "/api/v1/collections" -BearerToken $admin.access_token -Body @{ name = "Integration One"; description = "test" } -ExpectedStatus @(201)
    $collectionTwo = Invoke-JsonRequest -Method POST -Path "/api/v1/collections" -BearerToken $admin.access_token -Body @{ name = "Integration Two"; description = "test" } -ExpectedStatus @(201)

    Invoke-JsonRequest -Method PUT -Path "/api/v1/collections/$($collectionOne.id)/members" -BearerToken $admin.access_token -Body @{ user_id = $member.id; role = "viewer" } -ExpectedStatus @(204) | Out-Null

    $viewerLogin = Invoke-JsonRequest -Method POST -Path "/api/v1/auth/login" -Body @{ username = "viewer_user"; password = "secure-password-456"; device_name = "integration-viewer" } -ExpectedStatus @(200)
    $visibleCollections = Invoke-JsonRequest -Method GET -Path "/api/v1/collections" -BearerToken $viewerLogin.access_token -ExpectedStatus @(200)
    Assert-Equal @($visibleCollections).Count 1 "Viewer must see exactly one collection"
    Assert-Equal $visibleCollections[0].id $collectionOne.id "Viewer saw unauthorized collection"

    $viewerMutation = Invoke-JsonRequest -Method PUT -Path "/api/v1/collections/$($collectionOne.id)/members" -BearerToken $viewerLogin.access_token -Body @{ user_id = $member.id; role = "editor" } -ExpectedStatus @(403)
    Assert-Equal $viewerMutation.error.code "forbidden" "Viewer mutation must be denied"

    $rotated = Invoke-JsonRequest -Method POST -Path "/api/v1/auth/refresh" -Body @{ refresh_token = $viewerLogin.refresh_token } -ExpectedStatus @(200)
    $oldRefresh = Invoke-JsonRequest -Method POST -Path "/api/v1/auth/refresh" -Body @{ refresh_token = $viewerLogin.refresh_token } -ExpectedStatus @(401)
    Assert-Equal $oldRefresh.error.code "unauthenticated" "Old refresh token must be rejected"

    Invoke-JsonRequest -Method POST -Path "/api/v1/auth/logout" -BearerToken $rotated.access_token -ExpectedStatus @(204) | Out-Null
    $revoked = Invoke-JsonRequest -Method GET -Path "/api/v1/auth/me" -BearerToken $rotated.access_token -ExpectedStatus @(401)
    Assert-Equal $revoked.error.code "unauthenticated" "Revoked access token must be rejected"

    $sensitive = Select-String -Path $serverLog -Pattern "secure-password|$bootstrapToken|$($admin.access_token)|$($admin.refresh_token)" -Quiet
    if ($sensitive) { throw "Sensitive credential material appeared in service log." }

    Write-Host "PASS: isolated pgvector auth/ACL integration lifecycle completed."
}
finally {
    if ($serviceProcess -and -not $serviceProcess.HasExited) { Stop-Process -Id $serviceProcess.Id -Force }
    docker compose -f $composeFile down --volumes --remove-orphans | Out-Host
    Remove-Item -Recurse -Force $dataRoot -ErrorAction SilentlyContinue
}
