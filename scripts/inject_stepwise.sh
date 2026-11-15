#!/usr/bin/env bash
ADB=/mnt/c/qq-bot/release/scrcpy/adb.exe
S=/mnt/c/qq-bot/notes/真机验证截图
"$ADB" shell ime enable com.android.adbkeyboard/.AdbIME
"$ADB" shell ime set com.android.adbkeyboard/.AdbIME
sleep 3
"$ADB" shell am start -n com.lenovo.lpasistant/.MainActivity && sleep 3
"$ADB" shell input tap 480 2226
sleep 3
"$ADB" exec-out screencap -p > "$S/tmp_s1_focus.png"
"$ADB" shell am broadcast -a ADB_INPUT_TEXT --es msg '下周末前把报告写完' >/dev/null 2>&1
sleep 3
"$ADB" exec-out screencap -p > "$S/tmp_s2_text.png"
echo captured
