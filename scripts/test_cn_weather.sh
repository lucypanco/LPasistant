#!/usr/bin/env bash
# 国内天气源可达性探测(NMC中央气象台 / 中国天气网 / open-meteo geocoding对照)
UA="Mozilla/5.0 (Windows NT 10.0; Win64; x64) Chrome/124.0"
test_u() {
  echo "== $2 =="
  curl -s -m 12 -A "$UA" ${3:+-H "Referer: $3"} -w "\n[HTTP %{http_code}]\n" "$1" | head -c 400
  echo
}
test_u "http://www.nmc.cn/rest/weather?stationid=54511" "NMC·weather(北京54511)"
test_u "http://www.nmc.cn/rest/findWeather?stationid=54511" "NMC·findWeather"
test_u "http://www.nmc.cn/rest/findStationByName?name=%E5%8C%97%E4%BA%AC" "NMC·站名搜索"
test_u "http://www.weather.com.cn/data/sk/101010100.html" "中国天气网·实况sk"
test_u "http://d1.weather.com.cn/weather_index/101010100.html" "中国天气网·index(需Referer)" "http://www.weather.com.cn/"
test_u "https://geocoding-api.open-meteo.com/v1/search?name=%E4%B8%8A%E6%B5%B7&count=1" "open-meteo geocoding(对照)"
