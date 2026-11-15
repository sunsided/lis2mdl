# STMicroelectronics LIS2MDL Magnetometer

> Platform-agnostic Rust support for the STMicroelectronics LIS2MDL 3-axis magnetometer.

> **Status:** tested against a behavioral model of the chip (host tests and Wokwi), not yet
> verified on real hardware. See [#6](https://github.com/sunsided/lis2mdl/issues/6).

| Crate                                    | Description                                                |
|------------------------------------------|------------------------------------------------------------|
| [`lis2mdl-registers`](lis2mdl-registers) | A typed map of the LIS2MDL's I²C/SPI registers.            |
| [`lis2mdl`](lis2mdl)                     | An `embedded-hal` 1.0 I²C/SPI driver (blocking and async). |
| [`lis2mdl-sim`](lis2mdl-sim)             | A behavioral model of the chip for tests and emulation.    |

The register map follows the LIS2MDL datasheet (DS12144). ST's official driver,
[`lis2mdl-rs`](https://crates.io/crates/lis2mdl-rs), served as a cross-check; no code was taken from it.

## Emulation

The behavioral model in `lis2mdl-sim` is used as a fake I²C bus and SPI device in host tests.

**[Try it in the browser](https://wokwi.com/projects/480780742750157825)**: an ESP32-C3 talking to the emulated LIS2MDL on
wokwi.com. Click the chip and use its magnetic-field and temperature sliders.

### Use the chip in your own Wokwi project

Every [GitHub Release](https://github.com/sunsided/lis2mdl/releases) carries the chip as
`chip.zip`. Add a `chip-lis2mdl` part and this dependency to your `diagram.json`, with the
latest release version written without the tag's `v` (`@v0.2.1` fails to load):

```json
"dependencies": { "chip-lis2mdl": "github:sunsided/lis2mdl@0.2.1" }
```

This works on [wokwi.com](https://wokwi.com), in the Wokwi VS Code extension and with
`wokwi-cli`; nothing needs to be built.

For a browser demo, [`examples/arduino-esp32c3`](examples/arduino-esp32c3/README.md) has an
Arduino sketch and `diagram.json` to paste into a new wokwi.com project.

### Run the example locally

Needs [`wokwi-cli`](https://docs.wokwi.com/wokwi-ci/cli-installation) and a
`WOKWI_CLI_TOKEN` from the [Wokwi CI dashboard](https://wokwi.com/dashboard/ci):

```sh
task wokwi:test
```

[`sim/wokwi-chip`](sim/wokwi-chip/README.md) describes building the chip and its pins;
[`examples/esp32c3`](examples/esp32c3/README.md) the wiring and the scenario.

## Code of Conduct

We abide by the [Contributor Covenant][cc] and ask that you do as well.

## License

Copyright © 2026 Markus Mayer

Triple licensed under your choice of either of:

- European Union Public Licence, Version 1.2, ([LICENSE-EUPL](LICENSE-EUPL)
  or https://joinup.ec.europa.eu/collection/eupl/eupl-text-eupl-12)
- Apache License, Version 2.0, ([LICENSE-APACHE](LICENSE-APACHE) or http://www.apache.org/licenses/LICENSE-2.0)
- MIT license ([LICENSE-MIT](LICENSE-MIT) or http://opensource.org/licenses/MIT)

[cc]: https://contributor-covenant.org
