# STMicroelectronics LIS2MDL Magnetometer

> Platform-agnostic Rust support for the STMicroelectronics LIS2MDL 3-axis magnetometer.

| Crate                                    | Description                                                |
|------------------------------------------|------------------------------------------------------------|
| [`lis2mdl-registers`](lis2mdl-registers) | A typed map of the LIS2MDL's I²C/SPI registers.            |
| [`lis2mdl`](lis2mdl)                     | An `embedded-hal` 1.0 I²C/SPI driver (blocking and async). |
| [`lis2mdl-sim`](lis2mdl-sim)             | A behavioral model of the chip for tests and emulation.    |

The register map follows the LIS2MDL datasheet (DS12144). ST's official driver,
[`lis2mdl-rs`](https://crates.io/crates/lis2mdl-rs), served as a cross-check; no code was taken from it.

## Emulation

The behavioral model in `lis2mdl-sim` is used as a fake I²C bus and SPI device in host tests.

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
