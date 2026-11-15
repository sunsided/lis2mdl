# Changelog

All notable changes to this project will be documented in this file.
This project uses [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.2.2] - 2026-11-15

[0.2.2]: https://github.com/sunsided/lis2mdl/releases/tag/v0.2.2

### Changed

- Dropped `#![deny(warnings)]`, so a warning added by a newer compiler can no longer
  fail the docs.rs build.

## [0.2.1] - 2026-11-15

[0.2.1]: https://github.com/sunsided/lis2mdl/releases/tag/v0.2.1

### Changed

- With the `defmt` feature, registers log their fields (e.g. `CfgRegA { md: Idle, odr: Hz10, … }`)
  instead of the raw byte.

## [0.2.0] - 2026-11-15

[0.2.0]: https://github.com/sunsided/lis2mdl/releases/tag/v0.2.0

### Changed

- [**breaking**] Updated to `hardware-registers` 0.4.1. `Register` now requires
  `ReadableI2CRegister8` and `FromBits<u8>`, as 0.4 no longer implies readability.

## [0.1.0] - 2026-11-15

[0.1.0]: https://github.com/sunsided/lis2mdl/releases/tag/v0.1.0

### Added

- Register map following the LIS2MDL datasheet (DS12144 Rev 6), with I²C and SPI register traits.
