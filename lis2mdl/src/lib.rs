//! # STMicroelectronics LIS2MDL magnetometer driver
//!
//! > A platform-agnostic driver to interface with the STMicroelectronics LIS2MDL
//! > 3-axis magnetometer over I²C.
//!
//! This driver was built using [`embedded-hal`] 1.0 and [`lis2mdl-registers`]. Enable the
//! `async` feature for an equivalent driver on [`embedded-hal-async`] in [`asynch`].
//!
//! The driver takes the bus by value. To share the bus with other devices, pass a
//! bus handle such as `embedded_hal_bus::i2c::RefCellDevice`, or `&mut` to a bus.
//!
//! ## Example
//!
//! ```
//! # use core::cell::RefCell;
//! # use lis2mdl_sim::{FakeI2c, Lis2mdlModel};
//! use lis2mdl::{Lis2mdl, MagneticField, OutputDataRate, registers::CfgRegB};
//! # let model = RefCell::new(Lis2mdlModel::new());
//! # model.borrow_mut().set_field(200, 0, -300);
//! # let i2c = FakeI2c::new(&model);
//!
//! let mut mag = Lis2mdl::new(i2c);
//! assert!(mag.identify()?);
//!
//! // Hard-iron calibration, applied by the sensor itself.
//! mag.set_hard_iron_offset(MagneticField::from_milligauss(150.0, 0.0, 0.0))?;
//! // Optional: low-pass filter (ODR/4) and offset cancellation.
//! mag.modify_register(|r: CfgRegB| r.with_lpf(true).with_off_canc(true))?;
//! mag.start_continuous(OutputDataRate::Hz50)?;
//! # model.borrow_mut().tick(20_000);
//!
//! if mag.data_ready()? {
//!     let [x, y, z] = mag.read_magnetic_field()?.milligauss();
//!     assert_eq!((x, y, z), (150.0, 0.0, -450.0));
//!     let _celsius = mag.read_temperature()?.celsius();
//! }
//! # Ok::<(), embedded_hal::i2c::ErrorKind>(())
//! ```
//!
//! [`embedded-hal`]: https://crates.io/crates/embedded-hal
//! [`embedded-hal-async`]: https://crates.io/crates/embedded-hal-async
//! [`lis2mdl-registers`]: https://crates.io/crates/lis2mdl-registers

#![deny(missing_docs)]
#![deny(warnings)]
#![no_std]
#![forbid(unsafe_code)]
// Enables the `doc_cfg` feature when the `docsrs` configuration attribute is defined.
#![cfg_attr(docsrs, feature(doc_cfg))]

#[cfg(feature = "async")]
#[cfg_attr(docsrs, doc(cfg(feature = "async")))]
pub mod asynch;
mod measurement;

pub use lis2mdl_registers as registers;
pub use lis2mdl_registers::{DEFAULT_DEVICE_ADDRESS, LIS2MDL_WHO_AM_I, Mode, OutputDataRate};
pub use measurement::{MagneticField, Temperature};

use embedded_hal::i2c::I2c;
use lis2mdl_registers::prelude::*;
use lis2mdl_registers::{CfgRegA, StatusReg, WhoAmI};

/// Returns `CFG_REG_A` with the specified mode and output data rate, keeping the power mode.
///
/// Also sets `COMP_TEMP_EN`, which the datasheet requires for correct operation.
pub(crate) fn configure_mode(reg: CfgRegA, mode: Mode, odr: OutputDataRate) -> CfgRegA {
    reg.with_comp_temp_en(true)
        .with_soft_rst(false)
        .with_reboot(false)
        .with_odr(odr)
        .with_md(mode)
}

/// LIS2MDL driver.
#[derive(Debug)]
pub struct Lis2mdl<I2C> {
    i2c: I2C,
    address: u8,
}

impl<I2C> Lis2mdl<I2C>
where
    I2C: I2c,
{
    /// Creates a driver for a device at [`DEFAULT_DEVICE_ADDRESS`]. Performs no bus access.
    pub fn new(i2c: I2C) -> Self {
        Self::with_address(i2c, DEFAULT_DEVICE_ADDRESS)
    }

    /// Creates a driver for a device at the specified 7-bit address. Performs no bus access.
    pub fn with_address(i2c: I2C, address: u8) -> Self {
        Self { i2c, address }
    }

    /// Consumes the driver and returns the I²C bus.
    pub fn release(self) -> I2C {
        self.i2c
    }

    /// Reads the `WHO_AM_I` register.
    pub fn who_am_i(&mut self) -> Result<u8, I2C::Error> {
        Ok(self.read_register::<WhoAmI>()?.ident())
    }

    /// Checks whether `WHO_AM_I` matches [`LIS2MDL_WHO_AM_I`].
    pub fn identify(&mut self) -> Result<bool, I2C::Error> {
        Ok(self.who_am_i()? == LIS2MDL_WHO_AM_I)
    }

    /// Starts continuous measurement at the specified output data rate.
    ///
    /// Also enables temperature compensation. Keeps the power mode (`LP`).
    pub fn start_continuous(&mut self, odr: OutputDataRate) -> Result<(), I2C::Error> {
        self.modify_register(|r| configure_mode(r, Mode::Continuous, odr))
    }

    /// Triggers a single measurement. The device returns to idle mode afterwards.
    pub fn trigger_single(&mut self) -> Result<(), I2C::Error> {
        self.modify_register(|r: CfgRegA| configure_mode(r, Mode::Single, r.odr()))
    }

    /// Stops measuring and enters idle mode.
    pub fn stop(&mut self) -> Result<(), I2C::Error> {
        self.modify_register(|r: CfgRegA| configure_mode(r, Mode::Idle, r.odr()))
    }

    /// Whether a new X, Y and Z sample is available.
    pub fn data_ready(&mut self) -> Result<bool, I2C::Error> {
        Ok(self.read_register::<StatusReg>()?.zyxda())
    }

    /// Reads the X, Y and Z outputs in a single burst.
    ///
    /// The values are hard-iron corrected by the device (see [`Self::set_hard_iron_offset`]).
    pub fn read_magnetic_field(&mut self) -> Result<MagneticField, I2C::Error> {
        let bytes = self.read_bytes(MagneticField::OUTPUT_START)?;
        Ok(MagneticField::from_bytes(&bytes))
    }

    /// Reads the die temperature.
    pub fn read_temperature(&mut self) -> Result<Temperature, I2C::Error> {
        let bytes = self.read_bytes(Temperature::START)?;
        Ok(Temperature::from_bytes(&bytes))
    }

    /// Reads the hard-iron offset registers.
    pub fn hard_iron_offset(&mut self) -> Result<MagneticField, I2C::Error> {
        let bytes = self.read_bytes(MagneticField::OFFSET_START)?;
        Ok(MagneticField::from_bytes(&bytes))
    }

    /// Writes the hard-iron offset registers. The device subtracts the offset from each sample.
    pub fn set_hard_iron_offset(&mut self, offset: MagneticField) -> Result<(), I2C::Error> {
        let [a, b, c, d, e, f] = offset.to_bytes();
        self.i2c.write(
            self.address,
            &[MagneticField::OFFSET_START.addr(), a, b, c, d, e, f],
        )
    }

    /// Reads a single register.
    pub fn read_register<R>(&mut self) -> Result<R, I2C::Error>
    where
        R: Register,
    {
        let mut buffer = [0];
        self.i2c
            .write_read(self.address, &[*R::REGISTER_ADDRESS], &mut buffer)?;
        Ok(R::from_bits(buffer[0]))
    }

    /// Writes a single register.
    pub fn write_register<R>(&mut self, register: R) -> Result<(), I2C::Error>
    where
        R: WritableRegister,
    {
        self.i2c
            .write(self.address, &[*R::REGISTER_ADDRESS, register.to_bits()])
    }

    /// Reads, modifies and writes back a single register.
    pub fn modify_register<R, F>(&mut self, f: F) -> Result<(), I2C::Error>
    where
        R: WritableRegister,
        F: FnOnce(R) -> R,
    {
        let register = self.read_register::<R>()?;
        self.write_register(f(register))
    }

    /// Reads `N` consecutive registers. The device auto-increments the address on I²C.
    fn read_bytes<const N: usize>(
        &mut self,
        start: registers::RegisterAddress,
    ) -> Result<[u8; N], I2C::Error> {
        let mut buffer = [0; N];
        self.i2c
            .write_read(self.address, &[start.addr()], &mut buffer)?;
        Ok(buffer)
    }
}
