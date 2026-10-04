# Ramp concurrent clients against the Docker catalog until throughput stops rising.
# Usage: powershell -File scripts/max-load.ps1

param(
    [string]$Url = "http://127.0.0.1:3000",
    [int]$Duration = 8,
    [double]$ErrorStop = 0.01,
    [double]$DropStop = 0.85,
    [int[]]$Steps = @(64, 128, 256, 512, 1024, 2048, 4096, 8192, 10000)
)

$ErrorActionPreference = "Stop"
Set-Location (Join-Path $PSScriptRoot "..")

function Get-Field([string]$Text, [string]$Name) {
    $pattern = '(?m)^result\s.*\b' + $Name + '=(\S+)'
    $match = [regex]::Match($Text, $pattern)
    if (-not $match.Success) {
        throw "load output has no $Name. Output:`n$Text"
    }
    return $match.Groups[1].Value
}

try {
    $health = & curl.exe -fsS --max-time 5 "$Url/api/health"
} catch {
    Write-Host "No catalog server at $Url."
    Write-Host "Start it with: docker compose up --build"
    exit 1
}
if ($health -notmatch '"ok"') {
    Write-Host "Health check failed: $health"
    exit 1
}

Write-Host "Target $Url"
Write-Host "Each step runs ${Duration}s after connections are open."
Write-Host ""
"{0,12} {1,10} {2,8} {3,12} {4,10} {5,10} {6,10}" -f "concurrency", "requests", "errors", "rps", "p50_ms", "p95_ms", "p99_ms"

$best = $null
$stopReason = "finished every step"
$rows = @()

foreach ($step in $Steps) {
    $output = & cargo run -q --release --bin load -- --url $Url --concurrency $step --duration $Duration
    $text = ($output | Out-String)
    if ($text -notmatch "(?m)^result ") {
        Write-Host $text
        Write-Host "load tool failed at concurrency $step"
        exit 1
    }

    $requests = [int64](Get-Field $text "requests")
    $errors = [int64](Get-Field $text "errors")
    $rps = [double](Get-Field $text "rps")
    $p50 = [double](Get-Field $text "p50_us") / 1000
    $p95 = [double](Get-Field $text "p95_us") / 1000
    $p99 = [double](Get-Field $text "p99_us") / 1000
    $attempted = $requests + $errors
    $errorRate = 0.0
    if ($attempted -gt 0) {
        $errorRate = $errors / $attempted
    }

    $row = [pscustomobject]@{
        Concurrency = $step
        Requests    = $requests
        Errors      = $errors
        Rps         = $rps
        P50         = $p50
        P95         = $p95
        P99         = $p99
        ErrorRate   = $errorRate
    }
    $rows += $row
    "{0,12} {1,10} {2,8} {3,12:N1} {4,10:N0} {5,10:N0} {6,10:N0}" -f $step, $requests, $errors, $rps, $p50, $p95, $p99

    $failed = $errorRate -gt $ErrorStop -or $requests -eq 0
    $saturated = $null -ne $best -and $step -gt $best.Concurrency -and $rps -lt ($best.Rps * $DropStop)
    if ($failed -or $saturated) {
        if ($failed) {
            $pct = "{0:N1}" -f ($errorRate * 100)
            $stopReason = "error rate $pct% at $step clients"
        } else {
            $stopReason = "throughput fell below $([int]($DropStop * 100))% of the peak at $step clients"
        }
        break
    }
    if ($null -eq $best -or $rps -gt $best.Rps) {
        $best = $row
    }
}

Write-Host ""
if ($null -eq $best) {
    Write-Host "No successful step. $stopReason"
    exit 1
}

Write-Host ("Peak {0:N1} requests/sec at {1} clients (p50 {2:N0} ms, p95 {3:N0} ms, p99 {4:N0} ms)." -f $best.Rps, $best.Concurrency, $best.P50, $best.P95, $best.P99)
Write-Host "Stopped because $stopReason."
