#!/usr/bin/env bash
# 搜二次元音乐电台(anime/j-pop/アニメ),按票数排序
B="https://de1.api.radio-browser.info/json/stations"
for q in "tagList=anime&order=votes&reverse=true&limit=8&hidebroken=true" \
         "name=アニメ&order=votes&reverse=true&limit=6&hidebroken=true" \
         "tagList=j-pop&order=votes&reverse=true&limit=6&hidebroken=true" \
         "name=anime%20radio&order=votes&reverse=true&limit=6&hidebroken=true"; do
  echo "══ $q ══"
  curl -sS -m 15 "$B/search?$q" 2>/dev/null | python3 -c "
import sys, json
try:
    data = json.load(sys.stdin)
    for s in data[:8]:
        v = s.get('votes', 0)
        cc = s.get('countrycode', '')
        print(f\"  [{v:>4}票] {s['name'][:30]} | {cc} | {s.get('codec','')} {s.get('bitrate','')}k\")
        print(f\"         {s['url_resolved'][:90]}\")
except Exception as e:
    print('  解析失败:', e)
"
done
