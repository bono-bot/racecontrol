$manifests = Get-ChildItem "C:\Program Files (x86)\Steam\steamapps\appmanifest_*.acf"
foreach ($m in $manifests) {
    $content = Get-Content $m.FullName
    $name = ($content | Select-String '"name"').Line.Trim()
    $dir = ($content | Select-String '"installdir"').Line.Trim()
    $appid = $m.Name -replace 'appmanifest_|\.acf',''
    Write-Output "$appid | $name | $dir"
}
