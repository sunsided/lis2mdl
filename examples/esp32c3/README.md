# LIS2MDL on ESP32-C3

Reads a LIS2MDL with the blocking `lis2mdl` driver: runs the self-test, sets a hard-iron
offset and a ±1 gauss threshold interrupt, then prints field, heading and temperature
whenever they change, and the interrupt source when INT rises.

The heading assumes the chip is mounted with X north, Y east and Z down, and the board is
level. To change the mounting, change the `Mounting` type in `src/main.rs`.

| ESP32-C3 | LIS2MDL  |
|----------|----------|
| GPIO4    | SDA      |
| GPIO5    | SCL      |
| GPIO7    | INT/DRDY |

## Run in Wokwi

Requires `rustup target add riscv32imc-unknown-none-elf wasm32-unknown-unknown`,
[`wokwi-cli`](https://docs.wokwi.com/wokwi-ci/cli-installation) and `WOKWI_CLI_TOKEN`.

```sh
task wokwi:test          # from the repository root: builds the chip and the firmware
task wokwi:test          # from this directory, after `task chip:build` in the root
task wokwi -- --timeout 0   # just run it and watch the serial output
```

Or open this directory in VS Code with the Wokwi extension and drag the field sliders.

## Run on hardware

```sh
task run   # flashes with espflash and opens the serial monitor
```
