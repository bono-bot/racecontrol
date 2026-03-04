$updates = @{
    "exp_trial" = "ks_ferrari_sf15t"
    "exp_spa_f1_30" = "ks_ferrari_sf15t"
    "exp_spa_f1_60" = "ks_ferrari_sf15t"
    "exp_spa_gt3_30" = "ks_mclaren_p1_gtr"
    "exp_spa_gt4_30" = "ks_audi_r8_lms"
    "exp_spa_road_30" = "ks_lotus_3_eleven"
}
foreach ($id in $updates.Keys) {
    $car = $updates[$id]
    $body = "{`"car`": `"$car`"}"
    $r = Invoke-RestMethod -Uri "http://localhost:8080/api/v1/kiosk/experiences/$id" -Method Put -ContentType "application/json" -Body $body
    Write-Host "$id -> $car : ok=$($r.ok)"
}
