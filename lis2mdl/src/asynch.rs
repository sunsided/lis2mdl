//! Async driver on `embedded-hal-async`.
//!
//! Mirrors the blocking [`Lis2mdl`](crate::Lis2mdl).

use crate::{
    DEFAULT_DEVICE_ADDRESS, LIS2MDL_WHO_AM_I, MagneticField, Mode, OutputDataRate, Temperature,
    configure_mode,
};
use embedded_hal_async::i2c::I2c;
use lis2mdl_registers::prelude::*;
use lis2mdl_registers::{CfgRegA, RegisterAddress, StatusReg, WhoAmI};

/// Async LIS2MDL driver.
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
    pub async fn who_am_i(&mut self) -> Result<u8, I2C::Error> {
        Ok(self.read_register::<WhoAmI>().await?.ident())
    }

    /// Checks whether `WHO_AM_I` matches [`LIS2MDL_WHO_AM_I`].
    pub async fn identify(&mut self) -> Result<bool, I2C::Error> {
        Ok(self.who_am_i().await? == LIS2MDL_WHO_AM_I)
    }

    /// Starts continuous measurement at the specified output data rate.
    ///
    /// Also enables temperature compensation. Keeps the power mode (`LP`).
    pub async fn start_continuous(&mut self, odr: OutputDataRate) -> Result<(), I2C::Error> {
        self.modify_register(|r| configure_mode(r, Mode::Continuous, odr))
            .await
    }

    /// Triggers a single measurement. The device returns to idle mode afterwards.
    pub async fn trigger_single(&mut self) -> Result<(), I2C::Error> {
        self.modify_register(|r: CfgRegA| configure_mode(r, Mode::Single, r.odr()))
            .await
    }

    /// Stops measuring and enters idle mode.
    pub async fn stop(&mut self) -> Result<(), I2C::Error> {
        self.modify_register(|r: CfgRegA| configure_mode(r, Mode::Idle, r.odr()))
            .await
    }

    /// Whether a new X, Y and Z sample is available.
    pub async fn data_ready(&mut self) -> Result<bool, I2C::Error> {
        Ok(self.read_register::<StatusReg>().await?.zyxda())
    }

    /// Reads the X, Y and Z outputs in a single burst.
    ///
    /// The values are hard-iron corrected by the device (see [`Self::set_hard_iron_offset`]).
    pub async fn read_magnetic_field(&mut self) -> Result<MagneticField, I2C::Error> {
        let bytes = self.read_bytes(MagneticField::OUTPUT_START).await?;
        Ok(MagneticField::from_bytes(&bytes))
    }

    /// Reads the die temperature.
    pub async fn read_temperature(&mut self) -> Result<Temperature, I2C::Error> {
        let bytes = self.read_bytes(Temperature::START).await?;
        Ok(Temperature::from_bytes(&bytes))
    }

    /// Reads the hard-iron offset registers.
    pub async fn hard_iron_offset(&mut self) -> Result<MagneticField, I2C::Error> {
        let bytes = self.read_bytes(MagneticField::OFFSET_START).await?;
        Ok(MagneticField::from_bytes(&bytes))
    }

    /// Writes the hard-iron offset registers. The device subtracts the offset from each sample.
    pub async fn set_hard_iron_offset(&mut self, offset: MagneticField) -> Result<(), I2C::Error> {
        let [a, b, c, d, e, f] = offset.to_bytes();
        self.i2c
            .write(
                self.address,
                &[MagneticField::OFFSET_START.addr(), a, b, c, d, e, f],
            )
            .await
    }

    /// Reads a single register.
    pub async fn read_register<R>(&mut self) -> Result<R, I2C::Error>
    where
        R: Register,
    {
        let mut buffer = [0];
        self.i2c
            .write_read(self.address, &[*R::REGISTER_ADDRESS], &mut buffer)
            .await?;
        Ok(R::from_bits(buffer[0]))
    }

    /// Writes a single register.
    pub async fn write_register<R>(&mut self, register: R) -> Result<(), I2C::Error>
    where
        R: WritableRegister,
    {
        self.i2c
            .write(self.address, &[*R::REGISTER_ADDRESS, register.to_bits()])
            .await
    }

    /// Reads, modifies and writes back a single register.
    pub async fn modify_register<R, F>(&mut self, f: F) -> Result<(), I2C::Error>
    where
        R: WritableRegister,
        F: FnOnce(R) -> R,
    {
        let register = self.read_register::<R>().await?;
        self.write_register(f(register)).await
    }

    /// Reads `N` consecutive registers. The device auto-increments the address on I²C.
    async fn read_bytes<const N: usize>(
        &mut self,
        start: RegisterAddress,
    ) -> Result<[u8; N], I2C::Error> {
        let mut buffer = [0; N];
        self.i2c
            .write_read(self.address, &[start.addr()], &mut buffer)
            .await?;
        Ok(buffer)
    }
}
