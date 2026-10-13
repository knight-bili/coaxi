#!/usr/bin/env bash
set -Eeuo pipefail
root="$GITHUB_WORKSPACE"
role=codeg-android
evidence="$FOUNDATION_CI_EVIDENCE"
mkdir -p "$evidence"
stage=toolchain
mark() { python3 "$root/tools/integration/record-ci-stage.py" "$role" "$stage" "$1"; }
trap 'mark failed' ERR
{ java -version; echo "SDK=$ANDROID_HOME"; } > "$evidence/toolchain.txt" 2>&1
mark passed
cd "$RUNNER_TEMP/codeg-android"
cp foundation-source.json "$evidence/source.json"
export GRADLE_USER_HOME="$RUNNER_TEMP/foundation-gradle"
chmod +x gradlew
stage=compile_and_unit_tests
./gradlew --no-daemon --console=plain :app:assembleDebug :app:testDebugUnitTest :app:lintDebug > "$evidence/gradle.log" 2>&1
cp app/build/outputs/apk/debug/app-debug.apk "$evidence/app-debug.apk"
cp -R app/build/test-results/testDebugUnitTest "$evidence/unit-tests"
cp app/build/reports/lint-results-debug.* "$evidence/"
python3 - "$evidence/unit-tests" "$evidence/unit-counts.json" <<'PY'
from pathlib import Path
import json,sys,xml.etree.ElementTree as E
counts={k:0 for k in ['tests','failures','errors','skipped']}
for p in Path(sys.argv[1]).glob('TEST-*.xml'):
    suite=E.parse(p).getroot()
    for k in counts: counts[k]+=int(suite.get(k,'0'))
assert counts['tests']>0 and counts['failures']==counts['errors']==0,counts
Path(sys.argv[2]).write_text(json.dumps(counts,indent=2))
PY
mark passed
stage=emulator_launch
"$ANDROID_HOME/cmdline-tools/latest/bin/sdkmanager" 'system-images;android-36;google_apis;x86_64' 'emulator' > "$evidence/sdk-install.log" 2>&1
echo no | "$ANDROID_HOME/cmdline-tools/latest/bin/avdmanager" create avd -n foundation-api36 -k 'system-images;android-36;google_apis;x86_64' --device pixel_6 > "$evidence/avd.log" 2>&1
test -e /dev/kvm
sudo chmod 666 /dev/kvm
"$ANDROID_HOME/emulator/emulator" -avd foundation-api36 -port 5554 -no-window -no-audio -no-snapshot -no-boot-anim -gpu swiftshader_indirect > "$evidence/emulator.log" 2>&1 &
emulator_pid=$!
trap 'kill "$emulator_pid" >/dev/null 2>&1 || true' EXIT
export ANDROID_SERIAL=emulator-5554
timeout 300s bash -c 'adb wait-for-device; until [[ "$(adb shell getprop sys.boot_completed | tr -d "\r")" == 1 ]]; do sleep 2; done'
adb install "$evidence/app-debug.apk" > "$evidence/install.txt"
adb shell am start -W -n app.codeg.android.debug/app.codeg.android.MainActivity > "$evidence/launch.txt"
sleep 5
adb shell pidof app.codeg.android.debug > "$evidence/app-process.txt"
adb shell uiautomator dump /sdcard/foundation-window.xml > "$evidence/ui-dump.log"
adb pull /sdcard/foundation-window.xml "$evidence/startup.xml" > "$evidence/ui-pull.log" 2>&1
python3 - "$evidence/startup.xml" <<'PY'
import sys,xml.etree.ElementTree as E
assert any(n.get('package')=='app.codeg.android.debug' for n in E.parse(sys.argv[1]).iter('node')),'Application is not foreground'
PY
adb exec-out screencap -p > "$evidence/startup.png"
mark passed
