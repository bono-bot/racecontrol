$macs = @{
    "Pod1" = "30-56-0F-05-45-88"
    "Pod3" = "30-56-0F-05-44-B3"
    "Pod4" = "30-56-0F-05-45-25"
}
foreach ($pod in $macs.GetEnumerator()) {
    $macBytes = $pod.Value -split '-' | ForEach-Object { [byte]("0x" + $_) }
    $magicPacket = [byte[]](,0xFF * 6) + ($macBytes * 16)
    $udpClient = New-Object System.Net.Sockets.UdpClient
    $udpClient.Connect([System.Net.IPAddress]::Broadcast, 9)
    $udpClient.Send($magicPacket, $magicPacket.Length) | Out-Null
    $udpClient.Close()
    Write-Host "WOL sent to $($pod.Key) ($($pod.Value))"
}
