#!/bin/sh
set -eu
project=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
. "$project/../../scripts/apple-env.sh"
export ANDROID_HOME=${ANDROID_HOME:-$GLORY_BUILD_ROOT/sdk}
export JAVA_HOME=${JAVA_HOME:-/Applications/Android Studio.app/Contents/jbr/Contents/Home}
classes="$GLORY_BUILD_ROOT/files-java-tests"
sdk="$ANDROID_HOME/platforms/android-36/android.jar"
mkdir -p "$classes"
cat > "$classes/BuildConfig.java" <<'JAVA'
package rs.glory.counter;
final class BuildConfig { static final boolean GLORY_FILES_PROBE=true; }
JAVA
"$JAVA_HOME/bin/javac" -Xlint:all -Werror -d "$classes" -cp "$sdk" \
    "$classes/BuildConfig.java" \
    "$project/android-game/src/main/java/rs/glory/counter/FilesLedger.java" \
    "$project/android-game/src/main/java/rs/glory/counter/GloryFiles.java" \
    "$project/android-game/src/main/java/rs/glory/counter/GloryFilesSelectionProbe.java" \
    "$project/android-game/src/main/java/rs/glory/counter/GloryFilesProbeProvider.java" \
    "$project/android-game/tests/rs/glory/counter/FilesTest.java"
"$JAVA_HOME/bin/java" -cp "$classes:$sdk" rs.glory.counter.FilesTest
