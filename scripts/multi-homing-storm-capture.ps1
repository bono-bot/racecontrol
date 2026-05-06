# multi-homing-storm-capture.ps1
# Non-admin diagnostic: capture per-second NIC packet rates + router responsiveness
# during a Wi-Fi reconnect window. Confirms/falsifies the multi-homed broadcast-storm
# hypothesis without requiring admin / pktmon.
#
# Usage:
#   1. Run this script with Wi-Fi DISCONNECTED (current state)
#   2. Script collects 5s of baseline with Wi-Fi off
#   3. Script prompts you to RE-ENABLE Wi-Fi
#   4. Continues capture for up to 55 more seconds
#   5. If router becomes unresponsive (>5 consecutive ping fails), script flags it
#   6. You disable Wi-Fi when prompted (or it auto-times-out)
#   7. Output saved to data\multi-homing-capture-<timestamp>.csv

$ErrorActionPreference = "Continue"
$timestamp = Get-Date -Format "yyyyMMdd-HHmmss"
$outFile = "C:\Users\bono\racingpoint\racecontrol\data\multi-homing-capture-$timestamp.csv"
$gatewayIP = "192.168.31.1"

Write-Host "=== Multi-Homing Storm Capture ===" -ForegroundColor Cyan
Write-Host "Output: $outFile"
Write-Host ""
Write-Host "PRE-FLIGHT CHECK:" -ForegroundColor Yellow
$wifiState = (Get-NetAdapter -Name "Wi-Fi" -ErrorAction SilentlyContinue).Status
$ethState = (Get-NetAdapter -Name "Ethernet" -ErrorAction SilentlyContinue).Status
Write-Host "  Wi-Fi current state: $wifiState (must be Disconnected/Disabled to start)"
Write-Host "  Ethernet current state: $ethState (must be Up)"
if ($wifiState -eq "Up") {
  Write-Host ""
  Write-Host "ERROR: Wi-Fi is already Up. Disconnect it first." -ForegroundColor Red
  exit 1
}
if ($ethState -ne "Up") {
  Write-Host ""
  Write-Host "ERROR: Ethernet is not Up." -ForegroundColor Red
  exit 1
}

Write-Host ""
Write-Host "Capture begins now. CSV header is being written." -ForegroundColor Green
"timestamp,phase,wifi_status,wifi_pkts_rx,wifi_pkts_tx,wifi_bytes_rx,wifi_bytes_tx,eth_pkts_rx,eth_pkts_tx,eth_bytes_rx,eth_bytes_tx,gw_ping_ms,gw_unreachable_streak" | Out-File -FilePath $outFile -Encoding UTF8

$prevWifi = Get-NetAdapterStatistics -Name "Wi-Fi" -ErrorAction SilentlyContinue
$prevEth = Get-NetAdapterStatistics -Name "Ethernet" -ErrorAction SilentlyContinue
$gwUnreachableStreak = 0
$totalSec = 60
$baselineSec = 5

for ($i = 0; $i -lt $totalSec; $i++) {
  Start-Sleep -Seconds 1
  $now = Get-Date -Format "HH:mm:ss"
  $phase = if ($i -lt $baselineSec) { "BASELINE-WIFI-OFF" } else { "ACTIVE" }

  # Capture per-second deltas
  $curWifi = Get-NetAdapterStatistics -Name "Wi-Fi" -ErrorAction SilentlyContinue
  $curEth = Get-NetAdapterStatistics -Name "Ethernet" -ErrorAction SilentlyContinue
  $wifiStat = (Get-NetAdapter -Name "Wi-Fi" -ErrorAction SilentlyContinue).Status

  $wifiPktsRx = if ($curWifi -and $prevWifi) { $curWifi.ReceivedUnicastPackets - $prevWifi.ReceivedUnicastPackets + $curWifi.ReceivedMulticastPackets - $prevWifi.ReceivedMulticastPackets } else { 0 }
  $wifiPktsTx = if ($curWifi -and $prevWifi) { $curWifi.SentUnicastPackets - $prevWifi.SentUnicastPackets } else { 0 }
  $wifiBytesRx = if ($curWifi -and $prevWifi) { $curWifi.ReceivedBytes - $prevWifi.ReceivedBytes } else { 0 }
  $wifiBytesTx = if ($curWifi -and $prevWifi) { $curWifi.SentBytes - $prevWifi.SentBytes } else { 0 }
  $ethPktsRx = if ($curEth -and $prevEth) { $curEth.ReceivedUnicastPackets - $prevEth.ReceivedUnicastPackets + $curEth.ReceivedMulticastPackets - $prevEth.ReceivedMulticastPackets } else { 0 }
  $ethPktsTx = if ($curEth -and $prevEth) { $curEth.SentUnicastPackets - $prevEth.SentUnicastPackets } else { 0 }
  $ethBytesRx = if ($curEth -and $prevEth) { $curEth.ReceivedBytes - $prevEth.ReceivedBytes } else { 0 }
  $ethBytesTx = if ($curEth -and $prevEth) { $curEth.SentBytes - $prevEth.SentBytes } else { 0 }

  # Quick ping to gateway
  $ping = Test-Connection -ComputerName $gatewayIP -Count 1 -TimeoutSeconds 1 -Quiet -ErrorAction SilentlyContinue
  $gwMs = "(none)"
  if ($ping) {
    $gwUnreachableStreak = 0
  } else {
    $gwUnreachableStreak++
  }

  $line = "$now,$phase,$wifiStat,$wifiPktsRx,$wifiPktsTx,$wifiBytesRx,$wifiBytesTx,$ethPktsRx,$ethPktsTx,$ethBytesRx,$ethBytesTx,$gwMs,$gwUnreachableStreak"
  $line | Out-File -FilePath $outFile -Append -Encoding UTF8

  # Live console display
  $alert = if ($gwUnreachableStreak -ge 3) { "  !!! GATEWAY UNREACHABLE x$gwUnreachableStreak !!!" } else { "" }
  Write-Host ("[{0}] {1,-18} | wifi={2,-12} TX={3,5}p/s RX={4,5}p/s | eth TX={5,5}p/s RX={6,5}p/s | gw_streak={7}{8}" -f $now, $phase, $wifiStat, $wifiPktsTx, $wifiPktsRx, $ethPktsTx, $ethPktsRx, $gwUnreachableStreak, $alert)

  if ($i -eq ($baselineSec - 1)) {
    Write-Host ""
    Write-Host "  >>> BASELINE COMPLETE. NOW RE-ENABLE WI-FI (or let it auto-connect). <<<" -ForegroundColor Yellow
    Write-Host "  >>> Capture continues for 55 more seconds. <<<" -ForegroundColor Yellow
    Write-Host ""
  }

  $prevWifi = $curWifi
  $prevEth = $curEth

  # Auto-stop if gateway is dead for 10 consecutive seconds (router crashed)
  if ($gwUnreachableStreak -ge 10) {
    Write-Host ""
    Write-Host "  >>> Gateway dead for 10s. Stopping capture. Now please DISCONNECT WI-FI. <<<" -ForegroundColor Red
    break
  }
}

Write-Host ""
Write-Host "=== Capture complete. Analyzing... ===" -ForegroundColor Cyan
Write-Host ""

# Quick analysis
$rows = Import-Csv $outFile
$baseline = $rows | Where-Object { $_.phase -eq "BASELINE-WIFI-OFF" }
$active = $rows | Where-Object { $_.phase -eq "ACTIVE" }

if ($baseline.Count -gt 0) {
  $bAvgEthTx = ($baseline | Measure-Object -Property eth_pkts_tx -Average).Average
  $bAvgEthRx = ($baseline | Measure-Object -Property eth_pkts_rx -Average).Average
  Write-Host "BASELINE (Wi-Fi off, $($baseline.Count) samples):" -ForegroundColor Green
  Write-Host ("  Ethernet  avg TX={0:N1} pkt/s, avg RX={1:N1} pkt/s" -f $bAvgEthTx, $bAvgEthRx)
}
if ($active.Count -gt 0) {
  $aAvgEthTx = ($active | Measure-Object -Property eth_pkts_tx -Average).Average
  $aAvgEthRx = ($active | Measure-Object -Property eth_pkts_rx -Average).Average
  $aAvgWifiTx = ($active | Measure-Object -Property wifi_pkts_tx -Average).Average
  $aAvgWifiRx = ($active | Measure-Object -Property wifi_pkts_rx -Average).Average
  Write-Host "ACTIVE (Wi-Fi on, $($active.Count) samples):" -ForegroundColor Yellow
  Write-Host ("  Ethernet  avg TX={0:N1} pkt/s, avg RX={1:N1} pkt/s" -f $aAvgEthTx, $aAvgEthRx)
  Write-Host ("  Wi-Fi     avg TX={0:N1} pkt/s, avg RX={1:N1} pkt/s" -f $aAvgWifiTx, $aAvgWifiRx)
  $maxStreak = ($active | Measure-Object -Property gw_unreachable_streak -Maximum).Maximum
  Write-Host ("  Max gateway-unreachable streak: $maxStreak seconds")
  if ($maxStreak -ge 5) {
    Write-Host "  HYPOTHESIS SUPPORTED: gateway became unreachable while Wi-Fi was up" -ForegroundColor Red
  } elseif ($aAvgEthTx -gt $bAvgEthTx * 2) {
    Write-Host "  Ethernet TX more than 2x baseline — suggests duplicated traffic on multi-home"
  }
}

Write-Host ""
Write-Host "Full data: $outFile"
Write-Host "Compress for analysis: rate spike on ETH TX/RX during ACTIVE phase = multi-homed broadcast doubling"
