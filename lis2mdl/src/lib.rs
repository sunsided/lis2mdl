//! # STMicroelectronics LIS2MDL magnetometer driver
//!
//! > A platform-agnostic driver to interface with the STMicroelectronics LIS2MDL
//! > 3-axis magnetometer over I²C or SPI.
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
//! use lis2mdl::{Lis2mdl, MagneticField, OutputDataRate};
//! # let model = RefCell::new(Lis2mdlModel::new());
//! # model.borrow_mut().set_field(200, 0, -300);
//! # let i2c = FakeI2c::new(&model);
//!
//! let mut mag = Lis2mdl::new(i2c);
//! assert!(mag.identify()?);
//!
//! // Hard-iron calibration, applied by the sensor itself.
//! mag.set_hard_iron_offset(MagneticField::from_milligauss(150.0, 0.0, 0.0))?;
//! // Optional: low-pass filter (bandwidth ODR/4) and offset cancellation.
//! mag.set_low_pass_filter(true)?;
//! mag.set_offset_cancellation(true)?;
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
//! ## Coordinate frames
//!
//! The driver reports the field along the chip's own X, Y and Z axes. Which world direction
//! they point to depends on how the chip is mounted on the board, so the conversion belongs
//! in the board or application code. The [`coordinate-frame`] crate expresses it as a type.
//! The chip's axes are left-handed (DS12144 Rev 6, figure 2), so a chip mounted flat with
//! X to the north has Y to the east and Z up. The `coordinate-frame` feature names this
//! frame `SensorFrame`.
//!
//! For a chip mounted with X to the north, Y to the east and Z up:
//!
//! ```
//! # use core::cell::RefCell;
//! # use lis2mdl_sim::{FakeI2c, Lis2mdlModel};
//! use coordinate_frame::{NorthEastDown, NorthEastUp};
//! use lis2mdl::Lis2mdl;
//! # let model = RefCell::new(Lis2mdlModel::new());
//! # model.borrow_mut().set_field(200, 200, 0);
//! # let i2c = FakeI2c::new(&model);
//!
//! let mut mag = Lis2mdl::new(i2c);
//! mag.trigger_single()?;
//! let [x, y, z] = mag.read_magnetic_field()?.milligauss();
//!
//! // The mounting: the chip's axes, named in world terms.
//! let sensor = NorthEastUp::new(x, y, z);
//!
//! // Any other frame follows from that, e.g. north-east-down for sensor fusion.
//! let ned: NorthEastDown<f32> = sensor.into();
//! assert_eq!((ned.north(), ned.east(), ned.down()), (300.0, 300.0, 0.0));
//!
//! // Heading in degrees clockwise from magnetic north, for a level board.
//! let heading = ned.east().atan2(ned.north()).to_degrees().rem_euclid(360.0);
//! assert_eq!(heading, 45.0);
//! # Ok::<(), embedded_hal::i2c::ErrorKind>(())
//! ```
//!
//! The heading is only valid while the board is level; tilt compensation needs an
//! accelerometer.
//!
//! ## Byte order
//!
//! The driver tracks the `BLE` bit of `CFG_REG_C` and decodes the output registers
//! accordingly. It reads `CFG_REG_C` once before the first output read, so a device that
//! kept its configuration across a microcontroller reset is decoded correctly too.
//!
//! [`embedded-hal`]: https://crates.io/crates/embedded-hal
//! [`embedded-hal-async`]: https://crates.io/crates/embedded-hal-async
//! [`lis2mdl-registers`]: https://crates.io/crates/lis2mdl-registers
//! [`coordinate-frame`]: https://crates.io/crates/coordinate-frame

#![deny(missing_docs)]
#![deny(warnings)]
#![no_std]
#![forbid(unsafe_code)]
// Enables the `doc_cfg` feature when the `docsrs` configuration attribute is defined.
#![cfg_attr(docsrs, feature(doc_cfg))]

#[cfg(feature = "async")]
#[cfg_attr(docsrs, doc(cfg(feature = "async")))]
pub mod asynch;
mod interface;
mod measurement;
mod self_test;

#[cfg(feature = "async")]
pub use interface::AsyncInterface;
pub use interface::{I2cInterface, Interface, SpiInterface};
pub use lis2mdl_registers as registers;
pub use lis2mdl_registers::{DEFAULT_DEVICE_ADDRESS, LIS2MDL_WHO_AM_I, Mode, OutputDataRate};
pub use measurement::{MagneticField, Temperature};
pub use self_test::{
    SELF_TEST_MAX_MILLIGAUSS, SELF_TEST_MIN_MILLIGAUSS, SELF_TEST_SAMPLES, SelfTest, SelfTestError,
};

/// A vector in the sensor's own axes, as a [`coordinate_frame`] type.
///
/// The LIS2MDL datasheet (DS12144 Rev 6, section 1.2, figure 2 "Pin connections", page 3)
/// draws +Z out of the top face, +X and +Y in the package plane. Seen from above, +Y lies
/// 90° clockwise of +X, so the axes form a **left-handed** set. With +Z as Up, this is
/// [`NorthEastUp`](coordinate_frame::NorthEastUp): X = North, Y = East, Z = Up.
///
/// Convert a reading with [`From<[T; 3]>`](From), then into your board frame with `.into()`:
///
/// ```
/// # use core::cell::RefCell;
/// # use lis2mdl_sim::{FakeI2c, Lis2mdlModel};
/// use coordinate_frame::NorthEastDown;
/// use lis2mdl::{Lis2mdl, OutputDataRate, SensorFrame};
/// # let model = RefCell::new(Lis2mdlModel::new());
/// # model.borrow_mut().set_field(200, 100, -300);
/// # let i2c = FakeI2c::new(&model);
///
/// let mut mag = Lis2mdl::new(i2c);
/// mag.start_continuous(OutputDataRate::Hz50)?;
/// # model.borrow_mut().tick(20_000);
///
/// let field = SensorFrame::from(mag.read_magnetic_field()?.microtesla());
/// let ned: NorthEastDown<f32> = field.into();
/// assert_eq!((ned.north(), ned.east(), ned.down()), (30.0, 15.0, 45.0));
/// # Ok::<(), embedded_hal::i2c::ErrorKind>(())
/// ```
#[cfg(feature = "coordinate-frame")]
#[cfg_attr(docsrs, doc(cfg(feature = "coordinate-frame")))]
pub type SensorFrame<T> = coordinate_frame::NorthEastUp<T>;

use embedded_hal::delay::DelayNs;
use lis2mdl_registers::prelude::*;
use lis2mdl_registers::{
    CfgRegA, CfgRegB, CfgRegC, IntSourceReg, RegisterAddress, StatusReg, WhoAmI,
};

/// The time the device needs after power-up or [`Lis2mdl::reboot`] before it measures, in
/// milliseconds (datasheet table 11, high-resolution mode).
pub const TURN_ON_TIME_MS: u32 = 10;

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

/// Updates the tracked byte order after a register write.
///
/// Returns the new state: `Some(ble)` if known, `None` if it must be read back.
pub(crate) fn track_byte_order(swapped: Option<bool>, address: u8, bits: u8) -> Option<bool> {
    if address == RegisterAddress::CFG_REG_C.addr() {
        Some(CfgRegC::from(bits).ble())
    } else if address == RegisterAddress::CFG_REG_A.addr() && CfgRegA::from(bits).soft_rst() {
        Some(false)
    } else {
        swapped
    }
}

/// LIS2MDL driver.
#[derive(Debug)]
pub struct Lis2mdl<IFACE> {
    iface: IFACE,
    /// The `BLE` bit, if known.
    swapped: Option<bool>,
}

impl<I2C> Lis2mdl<I2cInterface<I2C>> {
    /// Creates a driver for a device on I²C at [`DEFAULT_DEVICE_ADDRESS`].
    /// Performs no bus access.
    pub fn new(i2c: I2C) -> Self {
        Self::from_interface(I2cInterface::new(i2c))
    }

    /// Creates a driver for a device on I²C at the specified 7-bit address.
    /// Performs no bus access.
    pub fn with_address(i2c: I2C, address: u8) -> Self {
        Self::from_interface(I2cInterface::with_address(i2c, address))
    }

    /// Consumes the driver and returns the I²C bus.
    pub fn release(self) -> I2C {
        self.iface.release()
    }
}

impl<SPI> Lis2mdl<SpiInterface<SPI>> {
    /// Creates a driver for a device on SPI. Performs no bus access.
    ///
    /// See [`SpiInterface`] for 3-wire and 4-wire mode.
    pub fn new_spi(spi: SPI) -> Self {
        Self::from_interface(SpiInterface::new(spi))
    }

    /// Consumes the driver and returns the SPI device.
    pub fn release(self) -> SPI {
        self.iface.release()
    }
}

impl<IFACE> Lis2mdl<IFACE> {
    /// Creates a driver on the specified interface. Performs no bus access.
    pub fn from_interface(iface: IFACE) -> Self {
        Self {
            iface,
            swapped: None,
        }
    }

    /// Consumes the driver and returns the interface.
    pub fn into_interface(self) -> IFACE {
        self.iface
    }
}

impl<IFACE> Lis2mdl<IFACE>
where
    IFACE: Interface,
{
    /// Reads the `WHO_AM_I` register.
    pub fn who_am_i(&mut self) -> Result<u8, IFACE::Error> {
        Ok(self.read_register::<WhoAmI>()?.ident())
    }

    /// Checks whether `WHO_AM_I` matches [`LIS2MDL_WHO_AM_I`].
    pub fn identify(&mut self) -> Result<bool, IFACE::Error> {
        Ok(self.who_am_i()? == LIS2MDL_WHO_AM_I)
    }

    /// Starts continuous measurement at the specified output data rate.
    ///
    /// Also enables temperature compensation. Keeps the power mode (`LP`).
    pub fn start_continuous(&mut self, odr: OutputDataRate) -> Result<(), IFACE::Error> {
        self.modify_register(|r| configure_mode(r, Mode::Continuous, odr))
    }

    /// Triggers a single measurement. The device returns to idle mode afterwards.
    pub fn trigger_single(&mut self) -> Result<(), IFACE::Error> {
        self.modify_register(|r: CfgRegA| configure_mode(r, Mode::Single, r.odr()))
    }

    /// Stops measuring and enters idle mode.
    pub fn stop(&mut self) -> Result<(), IFACE::Error> {
        self.modify_register(|r: CfgRegA| configure_mode(r, Mode::Idle, r.odr()))
    }

    /// Selects low-power mode (`true`) or high-resolution mode (`false`).
    pub fn set_low_power(&mut self, enabled: bool) -> Result<(), IFACE::Error> {
        self.modify_register(|r: CfgRegA| r.with_lp(enabled))
    }

    /// Enables the digital low-pass filter: the bandwidth drops from ODR/2 to ODR/4.
    pub fn set_low_pass_filter(&mut self, enabled: bool) -> Result<(), IFACE::Error> {
        self.modify_register(|r: CfgRegB| r.with_lpf(enabled))
    }

    /// Enables offset cancellation in continuous mode, and in single mode as well.
    pub fn set_offset_cancellation(&mut self, enabled: bool) -> Result<(), IFACE::Error> {
        self.modify_register(|r: CfgRegB| r.with_off_canc(enabled).with_off_canc_one_shot(enabled))
    }

    /// Enables block data update, which prevents reading the low and high bytes of
    /// different samples.
    pub fn set_block_data_update(&mut self, enabled: bool) -> Result<(), IFACE::Error> {
        self.modify_register(|r: CfgRegC| r.with_bdu(enabled))
    }

    /// Resets the configuration and user registers (including the hard-iron offset)
    /// to their defaults.
    pub fn soft_reset(&mut self) -> Result<(), IFACE::Error> {
        self.write_register(CfgRegA::new().with_soft_rst(true))
    }

    /// Reloads the factory calibration from non-volatile memory. Keeps the configuration
    /// otherwise. Wait [`TURN_ON_TIME_MS`] afterwards.
    pub fn reboot(&mut self) -> Result<(), IFACE::Error> {
        self.modify_register(|r: CfgRegA| r.with_reboot(true))
    }

    /// Reads the status register.
    pub fn status(&mut self) -> Result<StatusReg, IFACE::Error> {
        self.read_register()
    }

    /// Whether a new X, Y and Z sample is available.
    pub fn data_ready(&mut self) -> Result<bool, IFACE::Error> {
        Ok(self.status()?.zyxda())
    }

    /// Reads the X, Y and Z outputs in a single burst.
    ///
    /// The values are hard-iron corrected by the device (see [`Self::set_hard_iron_offset`]).
    pub fn read_magnetic_field(&mut self) -> Result<MagneticField, IFACE::Error> {
        let swapped = self.byte_order()?;
        let bytes = self.read_bytes(RegisterAddress::OUTX_L_REG)?;
        Ok(MagneticField::from_output(&bytes, swapped))
    }

    /// Reads the die temperature.
    pub fn read_temperature(&mut self) -> Result<Temperature, IFACE::Error> {
        let swapped = self.byte_order()?;
        let bytes = self.read_bytes(RegisterAddress::TEMP_OUT_L_REG)?;
        Ok(Temperature::from_output(&bytes, swapped))
    }

    /// Reads the hard-iron offset registers.
    pub fn hard_iron_offset(&mut self) -> Result<MagneticField, IFACE::Error> {
        let bytes = self.read_bytes(RegisterAddress::OFFSET_X_REG_L)?;
        Ok(MagneticField::from_le_bytes(&bytes))
    }

    /// Writes the hard-iron offset registers. The device subtracts the offset from each sample.
    pub fn set_hard_iron_offset(&mut self, offset: MagneticField) -> Result<(), IFACE::Error> {
        self.iface.write(
            RegisterAddress::OFFSET_X_REG_L.addr(),
            &offset.to_le_bytes(),
        )
    }

    /// Reads the interrupt threshold (unsigned, 1.5 mG per LSB, applied to all axes).
    pub fn interrupt_threshold(&mut self) -> Result<u16, IFACE::Error> {
        let bytes = self.read_bytes(RegisterAddress::INT_THS_L_REG)?;
        Ok(u16::from_le_bytes(bytes))
    }

    /// Writes the interrupt threshold (unsigned, 1.5 mG per LSB, applied to all axes).
    ///
    /// Configure the interrupt with [`IntCtrlReg`](registers::IntCtrlReg) and route it to
    /// the pin with [`CfgRegC::int_on_pin`](registers::CfgRegC).
    pub fn set_interrupt_threshold(&mut self, threshold: u16) -> Result<(), IFACE::Error> {
        self.iface.write(
            RegisterAddress::INT_THS_L_REG.addr(),
            &threshold.to_le_bytes(),
        )
    }

    /// Reads the interrupt source. Resets a latched interrupt.
    pub fn interrupt_source(&mut self) -> Result<IntSourceReg, IFACE::Error> {
        self.read_register()
    }

    /// Runs the self-test and restores the configuration afterwards.
    ///
    /// Takes about 1.2 s: 50 samples at 100 Hz without and with self-test, plus settling
    /// time. The magnetic field must stay constant during the test.
    pub fn self_test<D>(&mut self, delay: &mut D) -> Result<SelfTest, SelfTestError<IFACE::Error>>
    where
        D: DelayNs,
    {
        let saved_a = self.read_register::<CfgRegA>()?;
        let saved_b = self.read_register::<CfgRegB>()?;
        let saved_c = self.read_register::<CfgRegC>()?;

        let result = self.run_self_test(self_test::configuration(saved_c), delay);

        // Restore, even if the test failed.
        self.write_register(saved_c)?;
        self.write_register(saved_b)?;
        self.write_register(saved_a)?;
        result
    }

    fn run_self_test<D>(
        &mut self,
        (a, b, c): (CfgRegA, CfgRegB, CfgRegC),
        delay: &mut D,
    ) -> Result<SelfTest, SelfTestError<IFACE::Error>>
    where
        D: DelayNs,
    {
        self.write_register(c)?;
        self.write_register(b)?;
        self.write_register(a)?;
        delay.delay_ms(self_test::SETTLE_MS);
        let without = self.self_test_sum(delay)?;

        self.write_register(c.with_self_test(true))?;
        delay.delay_ms(self_test::SELF_TEST_SETTLE_MS);
        let with = self.self_test_sum(delay)?;

        Ok(SelfTest::from_sums(without, with))
    }

    /// Discards one sample, then sums up [`SELF_TEST_SAMPLES`] samples.
    fn self_test_sum<D>(&mut self, delay: &mut D) -> Result<[i32; 3], SelfTestError<IFACE::Error>>
    where
        D: DelayNs,
    {
        self.wait_for_data(delay)?;
        self.read_magnetic_field()?;

        let mut sum = [0; 3];
        for _ in 0..SELF_TEST_SAMPLES {
            self.wait_for_data(delay)?;
            self_test::accumulate(&mut sum, self.read_magnetic_field()?);
        }

        Ok(sum)
    }

    fn wait_for_data<D>(&mut self, delay: &mut D) -> Result<(), SelfTestError<IFACE::Error>>
    where
        D: DelayNs,
    {
        for _ in 0..self_test::MAX_POLLS {
            if self.data_ready()? {
                return Ok(());
            }

            delay.delay_ms(self_test::POLL_MS);
        }

        Err(SelfTestError::Timeout)
    }

    /// Reads a single register.
    pub fn read_register<R>(&mut self) -> Result<R, IFACE::Error>
    where
        R: Register,
    {
        let mut buffer = [0];
        self.iface.read(*R::REGISTER_ADDRESS, &mut buffer)?;
        Ok(R::from_bits(buffer[0]))
    }

    /// Writes a single register.
    pub fn write_register<R>(&mut self, register: R) -> Result<(), IFACE::Error>
    where
        R: WritableRegister,
    {
        let bits = register.to_bits();
        self.iface.write(*R::REGISTER_ADDRESS, &[bits])?;
        self.swapped = track_byte_order(self.swapped, *R::REGISTER_ADDRESS, bits);
        Ok(())
    }

    /// Reads, modifies and writes back a single register.
    pub fn modify_register<R, F>(&mut self, f: F) -> Result<(), IFACE::Error>
    where
        R: WritableRegister,
        F: FnOnce(R) -> R,
    {
        let register = self.read_register::<R>()?;
        self.write_register(f(register))
    }

    /// The `BLE` bit, read from the device if unknown.
    fn byte_order(&mut self) -> Result<bool, IFACE::Error> {
        match self.swapped {
            Some(swapped) => Ok(swapped),
            None => {
                let swapped = self.read_register::<CfgRegC>()?.ble();
                self.swapped = Some(swapped);
                Ok(swapped)
            }
        }
    }

    /// Reads `N` consecutive registers.
    fn read_bytes<const N: usize>(
        &mut self,
        start: RegisterAddress,
    ) -> Result<[u8; N], IFACE::Error> {
        let mut buffer = [0; N];
        self.iface.read(start.addr(), &mut buffer)?;
        Ok(buffer)
    }
}
