#!/bin/sh
set -eu
project=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
. "$project/../../scripts/apple-env.sh"
export ANDROID_HOME=${ANDROID_HOME:-$GLORY_BUILD_ROOT/sdk}
export JAVA_HOME=${JAVA_HOME:-/Applications/Android Studio.app/Contents/jbr/Contents/Home}
classes="$GLORY_BUILD_ROOT/clipboard-java-tests"
sdk="$ANDROID_HOME/platforms/android-36/android.jar"
mkdir -p "$classes"
"$JAVA_HOME/bin/javac" -Xlint:all -Werror -d "$classes" -cp "$sdk" \
    "$project/android-game/src/main/java/rs/glory/counter/ClipboardLedger.java" \
    "$project/android-game/src/main/java/rs/glory/counter/GloryClipboard.java" \
    "$project/android-game/src/main/java/rs/glory/counter/GloryClipboardProbe.java" \
    "$project/android-game/tests/rs/glory/counter/ClipboardTest.java"
"$JAVA_HOME/bin/java" -cp "$classes:$sdk" rs.glory.counter.ClipboardTest
