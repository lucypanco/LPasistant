#!/usr/bin/env bash
ADB=/mnt/c/qq-bot/release/scrcpy/adb.exe
S=/mnt/c/qq-bot/notes/真机验证截图
"$ADB" shell input tap 480 2226
sleep 5
"$ADB" shell dumpsys input_method | grep -iE "mInputShown" | head -2
"$ADB" exec-out screencap -p > "$S/tmp_f1.png"
"$ADB" shell input tap 480 2226
sleep 3
"$ADB" shell dumpsys input_method | grep -iE "mInputShown" | head -2
"$ADB" shell am broadcast -a ADB_INPUT_TEXT --es msg '下周末前把报告写完' >/dev/null 2>&1
sleep 3
"$ADB" exec-out screencap -p > "$S/tmp_f2.png"
echo done
