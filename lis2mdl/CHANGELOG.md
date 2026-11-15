# Changelog

All notable changes to this project will be documented in this file.
This project uses [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.2.0] - 2026-11-15

[0.2.0]: https://github.com/sunsided/lis2mdl/releases/tag/v0.2.0

### Changed

- Updated to `lis2mdl-registers` with `hardware-registers` 0.4.

## [0.1.0] - 2026-11-15

[0.1.0]: https://github.com/sunsided/lis2mdl/releases/tag/v0.1.0

### Added

- Blocking driver on embedded-hal 1.0 and async driver on embedded-hal-async (`async` feature).
- Magnetic field, temperature and hard-iron offset access with unit conversions.
- I²C and SPI interfaces, with I²C address auto-increment on multi-byte transfers.
- Configuration helpers for low-power mode, low-pass filter, offset cancellation,
  block data update, soft reset and reboot.
- Threshold interrupt support.
- Self-test following ST's reference procedure and the datasheet limits.
- Tracking of the `BLE` byte order bit.
