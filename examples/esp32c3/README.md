# LIS2MDL on ESP32-C3

Reads a LIS2MDL with the blocking `lis2mdl` driver: runs the self-test, sets a hard-iron
offset and a ±1 gauss threshold interrupt, then prints field, heading and temperature
whenever they change, and the interrupt source when INT rises.

| ESP32-C3 | LIS2MDL  |
|----------|----------|
| GPIO4    | SDA      |
| GPIO5    | SCL      |
| GPIO7    | INT/DRDY |

## Run in Wokwi

Requires `rustup target add riscv32imc-unknown-none-elf wasm32-unknown-unknown`,
[`wokwi-cli`](https://docs.wokwi.com/wokwi-ci/cli-installation) and `WOKWI_CLI_TOKEN`.

```sh
../../sim/wokwi-chip/build.sh
cargo build --release
wokwi-cli . --scenario scenarios/compass.yaml --timeout 20000
```

Or open this directory in VS Code with the Wokwi extension and drag the field sliders.

## Run on hardware

```sh
cargo run --release   # uses espflash
```
