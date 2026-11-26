# fchsync.ps1: upload changed Valheim character files (.fch) to the tracker. Read-only on the game folder.
#   .\fchsync.ps1 -Server http://host:8088 -Token SECRET [-Dir <characters folder>] [-Once]
# Env alternatives: TRACKER_SERVER, TRACKER_TOKEN, FCH_DIR. Works in Windows PowerShell 5.1 and PowerShell 7.
param(
    [string]$Server = $(if ($env:TRACKER_SERVER) { $env:TRACKER_SERVER } else { 'http://localhost:8088' }),
    [string]$Token = $env:TRACKER_TOKEN,
    [string]$Dir = $env:FCH_DIR,
    [switch]$Once
)
$ErrorActionPreference = 'Stop'
if (-not $Token) { Write-Error 'set -Token or $env:TRACKER_TOKEN' }
if (-not $Dir) {
    $cands = @("$env:USERPROFILE\AppData\LocalLow\IronGate\Valheim\characters_local")
    $cands += @(Get-ChildItem "${env:ProgramFiles(x86)}\Steam\userdata\*\892970\remote\characters" -ErrorAction SilentlyContinue | ForEach-Object FullName)
    $cands += @(Get-ChildItem "$env:ProgramFiles\Steam\userdata\*\892970\remote\characters" -ErrorAction SilentlyContinue | ForEach-Object FullName)
    foreach ($c in $cands) {
        if ((Test-Path $c) -and (Get-ChildItem "$c\*.fch" -ErrorAction SilentlyContinue)) { $Dir = $c; break }
    }
}
if (-not $Dir -or -not (Test-Path $Dir)) { Write-Error 'characters folder not found, pass -Dir' }
[Console]::Error.WriteLine("watching $Dir -> $Server")
$Server = $Server.TrimEnd('/')
$headers = @{ 'X-Token' = $Token }

# POST and return the HTTP status code (and body) without throwing on 4xx/5xx; 0 = unreachable.
function Send([string]$Path, $Body) {
    try {
        $p = @{ Uri = "$Server$Path"; Method = 'Post'; Headers = $headers; TimeoutSec = 15; UseBasicParsing = $true }
        if ($null -ne $Body) { $p.Body = $Body; $p.ContentType = 'application/octet-stream' }
        $r = Invoke-WebRequest @p
        return @{ Code = [int]$r.StatusCode; Text = '' }
    } catch {
        $resp = $_.Exception.Response
        if ($null -eq $resp) { return @{ Code = 0; Text = $_.Exception.Message } }
        $text = ''
        if ($_.ErrorDetails -and $_.ErrorDetails.Message) { $text = $_.ErrorDetails.Message }   # PowerShell 7
        else { try { $text = (New-Object IO.StreamReader($resp.GetResponseStream())).ReadToEnd() } catch {} }   # 5.1
        return @{ Code = [int]$resp.StatusCode; Text = $text }
    }
}
function Sig($f) { $f.Refresh(); "$($f.LastWriteTimeUtc.Ticks)-$($f.Length)" }

$sig = @{}; $beat = @{}
while ($true) {
    foreach ($f in Get-ChildItem "$Dir\*.fch" -ErrorAction SilentlyContinue) {
        $name = $f.BaseName
        if ($name -like '*_backup_*') { continue }   # the game's own backups
        $s = Sig $f
        $now = Get-Date
        if ($sig[$name] -eq $s) {
            if (($now - $beat[$name]).TotalSeconds -ge 30) {
                $q = [uri]::EscapeDataString($name)
                $r = Send "/heartbeat?name=$q" $null
                if ($r.Code -eq 404) { $sig.Remove($name) } elseif ($r.Code -ne 0) { $beat[$name] = $now }
            }
            continue
        }
        Start-Sleep -Milliseconds 700                 # the game writes .fch.new then renames: wait until stable
        if ((Sig $f) -ne $s) { continue }
        try {
            $fs = [IO.File]::Open($f.FullName, 'Open', 'Read', 'ReadWrite')
            try { $ms = New-Object IO.MemoryStream; $fs.CopyTo($ms); $bytes = $ms.ToArray() } finally { $fs.Dispose() }
        } catch { [Console]::Error.WriteLine("${name}: cannot read ($($_.Exception.Message)), retry"); continue }
        $r = Send "/upload?name=$([uri]::EscapeDataString($name))" $bytes
        if ($r.Code -eq 200) { $sig[$name] = $s; $beat[$name] = $now; [Console]::Error.WriteLine("${name}: uploaded") }
        elseif ($r.Code -eq 400) { $sig[$name] = $s; $beat[$name] = $now; [Console]::Error.WriteLine("${name}: rejected: $($r.Text) (skipped until the file changes)") }
        elseif ($r.Code -eq 0) { [Console]::Error.WriteLine("${name}: server unreachable (retry)") }
        else { [Console]::Error.WriteLine("${name}: server said $($r.Code) $($r.Text)") }
    }
    if ($Once) { break }
    Start-Sleep -Seconds 2
}
