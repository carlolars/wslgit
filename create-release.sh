#!/usr/bin/env bash

if [[ -d "release/" ]]; then
    rm -rf release/* || exit 1
fi

[[ ! -f "target/release/wslgit.exe" ]] && echo "Release not built!" && exit 1

if ! mkdir -p release/Git/cmd ; then echo "Failed to create output directory"; exit 1; fi

if ! cp resources/Fork.RI target/release/wslgit.exe release/Git/cmd/ ; then echo "Failed to copy release files"; exit 1; fi

# cp resources/install.bat release/Git/ || echo "Failed to copy install.bat"; exit 1
if ! cp resources/install.bat release/Git/ ; then echo "Failed to copy install.bat"; exit 1; fi

cd release
zip -r wslgit-fork-patch.zip ./*
