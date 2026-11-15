# LIS2MDL Wokwi custom chip

Wraps the [`lis2mdl-sim`](../../lis2mdl-sim) behavioral model into a
[Wokwi custom chip](https://docs.wokwi.com/chips-api/getting-started).

```sh
./build.sh   # produces lis2mdl.chip.wasm next to lis2mdl.chip.json
```

Reference it from a project's `wokwi.toml`:

```toml
[[chip]]
name = 'lis2mdl'
binary = '../../sim/wokwi-chip/lis2mdl.chip.wasm'
```

and add a `chip-lis2mdl` part to `diagram.json`.

| Pin   | Function                                      |
|-------|-----------------------------------------------|
| `SDA` | I²C data (address `0x1E`)                     |
| `SCL` | I²C clock                                     |
| `INT` | INT/DRDY output (push-pull), as configured    |

Controls `fieldX`, `fieldY`, `fieldZ` (milligauss, before hard-iron correction) and
`temperature` (°C) are sampled every millisecond, which is also the model's time step.
Drive them from the Wokwi UI or with `set-control` in automation scenarios.

The chip models I²C only; SPI is covered by host tests with `FakeSpi`.
