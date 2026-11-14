#!/usr/bin/env bash
# 测试各天气模式源与集合API的可达性
for m in icon_seamless gfs_seamless meteofrance_seamless jma_seamless gem_seamless ecmwf_ifs0424; do
  echo "== $m =="
  curl -s -m 12 -w '\n[HTTP %{http_code}]\n' \
    "https://api.open-meteo.com/v1/forecast?latitude=39.9&longitude=116.4&hourly=temperature_2m,precipitation&models=$m&forecast_hours=1&timezone=auto" \
    | head -c 320
  echo
done
echo "== ensemble(gfs_seamless) =="
curl -s -m 12 -w '\n[HTTP %{http_code}]\n' \
  "https://ensemble-api.open-meteo.com/v1/ensemble?latitude=39.9&longitude=116.4&hourly=temperature_2m&models=gfs_seamless&forecast_hours=1&timezone=auto" \
  | head -c 400
