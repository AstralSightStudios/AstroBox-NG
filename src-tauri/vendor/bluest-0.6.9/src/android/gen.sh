#!/bin/bash

set -euxo pipefail

if [[ -z "$ANDROID_HOME" ]] then 
    echo ANDROID_HOME is not set!
    exit 1
fi

rm -rf build
mkdir build
javac -cp "$ANDROID_HOME"/platforms/android-33/android.jar -source 1.7 -target 1.7 java/*.java -d build
jar cvf build/bluest.jar -C build .
java-spaghetti-gen generate --verbose
rm -rf build
rustfmt bindings.rs
