#!/usr/bin/env sh
# Builds the chip and places it next to lis2mdl.chip.json, as Wokwi expects.
set -eu
cd "$(dirname "$0")"
cargo build --release
cp target/wasm32-unknown-unknown/release/lis2mdl_wokwi_chip.wasm lis2mdl.chip.wasm
