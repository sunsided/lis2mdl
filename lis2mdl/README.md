# STMicroelectronics LIS2MDL Magnetometer Driver

[![Crates.io][crates-image]][crates-link]
[![Docs][docs-image]][docs-link]
[![Build Status][build-image]][build-link]
[![Coverage][codecov-image]][codecov-link]
[![Safety Dance][safety-image]][safety-link]
![MSRV][msrv-image]
[![EUPL 1.2 licensed][license-eupl-image]][license-eupl-link]
[![Apache 2.0 licensed][license-apache-image]][license-apache-link]
[![MIT licensed][license-mit-image]][license-mit-link]

> A platform-agnostic `embedded-hal` 1.0 driver for the LIS2MDL 3-axis magnetometer.

> **Status:** tested against a behavioral model of the chip (host tests and Wokwi), not yet
> verified on real hardware. See [#6](https://github.com/sunsided/lis2mdl/issues/6).

- `no_std`, no `unsafe`, I²C or SPI (3-wire or 4-wire). The bus is taken by value, so
  `embedded-hal-bus` sharing works.
- Blocking API, plus an equivalent async API on `embedded-hal-async` behind the `async` feature
- `WHO_AM_I` check, continuous and single measurement modes, data-ready polling
- Magnetic field in raw LSB, milligauss or microtesla; die temperature in °C
- Hard-iron offset registers, so the device itself applies the calibration
- Low-power mode, low-pass filter, offset cancellation, block data update, soft reset, reboot
- Threshold interrupts (threshold, source, INT/DRDY pin configuration)
- Self-test with the datasheet limits (15 to 500 mG per axis)
- Byte order (`BLE`) tracking, so swapped output registers are decoded correctly
- Typed access to every register via `read_register`, `write_register` and `modify_register`

```rust,ignore
use lis2mdl::{Lis2mdl, OutputDataRate};

let mut mag = Lis2mdl::new(i2c); // or Lis2mdl::new_spi(spi)
assert!(mag.identify()?);
mag.set_low_pass_filter(true)?;
mag.set_offset_cancellation(true)?;
mag.start_continuous(OutputDataRate::Hz50)?;

if mag.data_ready()? {
    let [x, y, z] = mag.read_magnetic_field()?.microtesla();
}
```

## Features

- `async`: adds `lis2mdl::asynch::Lis2mdl` on `embedded-hal-async`.
- `defmt`: derives `defmt::Format` for the public types.
- `coordinate-frame`: adds `lis2mdl::SensorFrame`, see below.

## Coordinate frame

With the `coordinate-frame` feature, `SensorFrame<T>` names the sensor's axes as a
[`coordinate-frame`](https://crates.io/crates/coordinate-frame) type. The datasheet
(DS12144 Rev 6, figure 2) draws a left-handed set with +Z out of the top face, so
`SensorFrame` is `NorthEastUp` (X = North, Y = East, Z = Up). Convert into your board frame:

```rust,ignore
use coordinate_frame::NorthEastDown;
use lis2mdl::SensorFrame;

let field = SensorFrame::from(mag.read_magnetic_field()?.microtesla());
let ned: NorthEastDown<f32> = field.into();
```

ST also provides an official driver, [`lis2mdl-rs`](https://crates.io/crates/lis2mdl-rs).
This crate follows the register-crate pattern of its sibling drivers instead.

## License

Copyright © 2026 Markus Mayer

Triple licensed under your choice of either of:

- European Union Public Licence, Version 1.2, ([LICENSE-EUPL](LICENSE-EUPL)
  or https://joinup.ec.europa.eu/collection/eupl/eupl-text-eupl-12)
- Apache License, Version 2.0, ([LICENSE-APACHE](LICENSE-APACHE) or http://www.apache.org/licenses/LICENSE-2.0)
- MIT license ([LICENSE-MIT](LICENSE-MIT) or http://opensource.org/licenses/MIT)

[crates-image]: https://img.shields.io/crates/v/lis2mdl
[crates-link]: https://crates.io/crates/lis2mdl
[docs-image]: https://docs.rs/lis2mdl/badge.svg
[docs-link]: https://docs.rs/lis2mdl/
[build-image]: https://github.com/sunsided/lis2mdl/workflows/Rust/badge.svg
[build-link]: https://github.com/sunsided/lis2mdl/actions
[codecov-image]: https://codecov.io/gh/sunsided/lis2mdl/graph/badge.svg
[codecov-link]: https://codecov.io/gh/sunsided/lis2mdl
[safety-image]: https://img.shields.io/badge/unsafe-forbidden-success.svg
[safety-link]: https://github.com/rust-secure-code/safety-dance/
[msrv-image]: https://img.shields.io/badge/rustc-1.85+-blue.svg
[license-eupl-image]: https://img.shields.io/badge/license-EUPL_1.2-blue.svg
[license-eupl-link]: https://joinup.ec.europa.eu/collection/eupl/eupl-text-eupl-12
[license-apache-image]: https://img.shields.io/badge/license-Apache_2.0-blue.svg
[license-apache-link]: https://www.apache.org/licenses/LICENSE-2.0
[license-mit-image]: https://img.shields.io/badge/license-MIT-blue.svg
[license-mit-link]: https://opensource.org/licenses/MIT
