//! Async driver on `embedded-hal-async`.
//!
//! Mirrors the blocking [`Lis2mdl`](crate::Lis2mdl) on [`AsyncInterface`].

use crate::interface::{AsyncInterface, I2cInterface, SpiInterface};
use crate::{
    LIS2MDL_WHO_AM_I, MagneticField, Mode, OutputDataRate, SELF_TEST_SAMPLES, SelfTest,
    SelfTestError, Temperature, configure_mode, self_test, track_byte_order,
};

use embedded_hal_async::delay::DelayNs;
use lis2mdl_registers::prelude::*;
use lis2mdl_registers::{
    CfgRegA, CfgRegB, CfgRegC, IntSourceReg, RegisterAddress, StatusReg, WhoAmI,
};

/// Async LIS2MDL driver.
#[derive(Debug)]
pub struct Lis2mdl<IFACE> {
    iface: IFACE,
    /// The `BLE` bit, if known.
    swapped: Option<bool>,
}

impl<I2C> Lis2mdl<I2cInterface<I2C>> {
    /// Creates a driver for a device on I²C at [`DEFAULT_DEVICE_ADDRESS`](crate::DEFAULT_DEVICE_ADDRESS).
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
    IFACE: AsyncInterface,
{
    /// Reads the `WHO_AM_I` register.
    pub async fn who_am_i(&mut self) -> Result<u8, IFACE::Error> {
        Ok(self.read_register::<WhoAmI>().await?.ident())
    }

    /// Checks whether `WHO_AM_I` matches [`LIS2MDL_WHO_AM_I`].
    pub async fn identify(&mut self) -> Result<bool, IFACE::Error> {
        Ok(self.who_am_i().await? == LIS2MDL_WHO_AM_I)
    }

    /// Starts continuous measurement at the specified output data rate.
    ///
    /// Also enables temperature compensation. Keeps the power mode (`LP`).
    pub async fn start_continuous(&mut self, odr: OutputDataRate) -> Result<(), IFACE::Error> {
        self.modify_register(|r| configure_mode(r, Mode::Continuous, odr))
            .await
    }

    /// Triggers a single measurement. The device returns to idle mode afterwards.
    pub async fn trigger_single(&mut self) -> Result<(), IFACE::Error> {
        self.modify_register(|r: CfgRegA| configure_mode(r, Mode::Single, r.odr()))
            .await
    }

    /// Stops measuring and enters idle mode.
    pub async fn stop(&mut self) -> Result<(), IFACE::Error> {
        self.modify_register(|r: CfgRegA| configure_mode(r, Mode::Idle, r.odr()))
            .await
    }

    /// Selects low-power mode (`true`) or high-resolution mode (`false`).
    pub async fn set_low_power(&mut self, enabled: bool) -> Result<(), IFACE::Error> {
        self.modify_register(|r: CfgRegA| r.with_lp(enabled)).await
    }

    /// Enables the digital low-pass filter: the bandwidth drops from ODR/2 to ODR/4.
    pub async fn set_low_pass_filter(&mut self, enabled: bool) -> Result<(), IFACE::Error> {
        self.modify_register(|r: CfgRegB| r.with_lpf(enabled)).await
    }

    /// Enables offset cancellation in continuous mode, and in single mode as well.
    pub async fn set_offset_cancellation(&mut self, enabled: bool) -> Result<(), IFACE::Error> {
        self.modify_register(|r: CfgRegB| r.with_off_canc(enabled).with_off_canc_one_shot(enabled))
            .await
    }

    /// Enables block data update, which prevents reading the low and high bytes of
    /// different samples.
    pub async fn set_block_data_update(&mut self, enabled: bool) -> Result<(), IFACE::Error> {
        self.modify_register(|r: CfgRegC| r.with_bdu(enabled)).await
    }

    /// Resets the configuration and user registers (including the hard-iron offset)
    /// to their defaults.
    pub async fn soft_reset(&mut self) -> Result<(), IFACE::Error> {
        self.write_register(CfgRegA::new().with_soft_rst(true))
            .await
    }

    /// Reloads the factory calibration from non-volatile memory. Keeps the configuration
    /// otherwise. Wait [`TURN_ON_TIME_MS`](crate::TURN_ON_TIME_MS) afterwards.
    pub async fn reboot(&mut self) -> Result<(), IFACE::Error> {
        self.modify_register(|r: CfgRegA| r.with_reboot(true)).await
    }

    /// Reads the status register.
    pub async fn status(&mut self) -> Result<StatusReg, IFACE::Error> {
        self.read_register().await
    }

    /// Whether a new X, Y and Z sample is available.
    pub async fn data_ready(&mut self) -> Result<bool, IFACE::Error> {
        Ok(self.status().await?.zyxda())
    }

    /// Reads the X, Y and Z outputs in a single burst.
    ///
    /// The values are hard-iron corrected by the device (see [`Self::set_hard_iron_offset`]).
    pub async fn read_magnetic_field(&mut self) -> Result<MagneticField, IFACE::Error> {
        let swapped = self.byte_order().await?;
        let bytes = self.read_bytes(RegisterAddress::OUTX_L_REG).await?;
        Ok(MagneticField::from_output(&bytes, swapped))
    }

    /// Reads the die temperature.
    pub async fn read_temperature(&mut self) -> Result<Temperature, IFACE::Error> {
        let swapped = self.byte_order().await?;
        let bytes = self.read_bytes(RegisterAddress::TEMP_OUT_L_REG).await?;
        Ok(Temperature::from_output(&bytes, swapped))
    }

    /// Reads the hard-iron offset registers.
    pub async fn hard_iron_offset(&mut self) -> Result<MagneticField, IFACE::Error> {
        let bytes = self.read_bytes(RegisterAddress::OFFSET_X_REG_L).await?;
        Ok(MagneticField::from_le_bytes(&bytes))
    }

    /// Writes the hard-iron offset registers. The device subtracts the offset from each sample.
    pub async fn set_hard_iron_offset(
        &mut self,
        offset: MagneticField,
    ) -> Result<(), IFACE::Error> {
        self.iface
            .write(
                RegisterAddress::OFFSET_X_REG_L.addr(),
                &offset.to_le_bytes(),
            )
            .await
    }

    /// Reads the interrupt threshold (unsigned, 1.5 mG per LSB, applied to all axes).
    pub async fn interrupt_threshold(&mut self) -> Result<u16, IFACE::Error> {
        let bytes = self.read_bytes(RegisterAddress::INT_THS_L_REG).await?;
        Ok(u16::from_le_bytes(bytes))
    }

    /// Writes the interrupt threshold (unsigned, 1.5 mG per LSB, applied to all axes).
    ///
    /// Configure the interrupt with [`IntCtrlReg`](crate::registers::IntCtrlReg) and route it to
    /// the pin with [`CfgRegC::int_on_pin`](crate::registers::CfgRegC).
    pub async fn set_interrupt_threshold(&mut self, threshold: u16) -> Result<(), IFACE::Error> {
        self.iface
            .write(
                RegisterAddress::INT_THS_L_REG.addr(),
                &threshold.to_le_bytes(),
            )
            .await
    }

    /// Reads the interrupt source. Resets a latched interrupt.
    pub async fn interrupt_source(&mut self) -> Result<IntSourceReg, IFACE::Error> {
        self.read_register().await
    }

    /// Runs the self-test and restores the configuration afterwards.
    ///
    /// Takes about 1.2 s: 50 samples at 100 Hz without and with self-test, plus settling
    /// time. The magnetic field must stay constant during the test.
    pub async fn self_test<D>(
        &mut self,
        delay: &mut D,
    ) -> Result<SelfTest, SelfTestError<IFACE::Error>>
    where
        D: DelayNs,
    {
        let saved_a = self.read_register::<CfgRegA>().await?;
        let saved_b = self.read_register::<CfgRegB>().await?;
        let saved_c = self.read_register::<CfgRegC>().await?;

        let result = self
            .run_self_test(self_test::configuration(saved_c), delay)
            .await;

        // Restore, even if the test failed.
        self.write_register(saved_c).await?;
        self.write_register(saved_b).await?;
        self.write_register(saved_a).await?;
        result
    }

    async fn run_self_test<D>(
        &mut self,
        (a, b, c): (CfgRegA, CfgRegB, CfgRegC),
        delay: &mut D,
    ) -> Result<SelfTest, SelfTestError<IFACE::Error>>
    where
        D: DelayNs,
    {
        self.write_register(c).await?;
        self.write_register(b).await?;
        self.write_register(a).await?;
        delay.delay_ms(self_test::SETTLE_MS).await;
        let without = self.self_test_sum(delay).await?;

        self.write_register(c.with_self_test(true)).await?;
        delay.delay_ms(self_test::SELF_TEST_SETTLE_MS).await;
        let with = self.self_test_sum(delay).await?;

        Ok(SelfTest::from_sums(without, with))
    }

    /// Discards one sample, then sums up [`SELF_TEST_SAMPLES`] samples.
    async fn self_test_sum<D>(
        &mut self,
        delay: &mut D,
    ) -> Result<[i32; 3], SelfTestError<IFACE::Error>>
    where
        D: DelayNs,
    {
        self.wait_for_data(delay).await?;
        self.read_magnetic_field().await?;

        let mut sum = [0; 3];
        for _ in 0..SELF_TEST_SAMPLES {
            self.wait_for_data(delay).await?;
            self_test::accumulate(&mut sum, self.read_magnetic_field().await?);
        }

        Ok(sum)
    }

    async fn wait_for_data<D>(&mut self, delay: &mut D) -> Result<(), SelfTestError<IFACE::Error>>
    where
        D: DelayNs,
    {
        for _ in 0..self_test::MAX_POLLS {
            if self.data_ready().await? {
                return Ok(());
            }

            delay.delay_ms(self_test::POLL_MS).await;
        }

        Err(SelfTestError::Timeout)
    }

    /// Reads a single register.
    pub async fn read_register<R>(&mut self) -> Result<R, IFACE::Error>
    where
        R: Register,
    {
        let mut buffer = [0];
        self.iface.read(*R::REGISTER_ADDRESS, &mut buffer).await?;
        Ok(R::from_bits(buffer[0]))
    }

    /// Writes a single register.
    pub async fn write_register<R>(&mut self, register: R) -> Result<(), IFACE::Error>
    where
        R: WritableRegister,
    {
        let bits = register.to_bits();
        self.iface.write(*R::REGISTER_ADDRESS, &[bits]).await?;
        self.swapped = track_byte_order(self.swapped, *R::REGISTER_ADDRESS, bits);
        Ok(())
    }

    /// Reads, modifies and writes back a single register.
    pub async fn modify_register<R, F>(&mut self, f: F) -> Result<(), IFACE::Error>
    where
        R: WritableRegister,
        F: FnOnce(R) -> R,
    {
        let register = self.read_register::<R>().await?;
        self.write_register(f(register)).await
    }

    /// The `BLE` bit, read from the device if unknown.
    async fn byte_order(&mut self) -> Result<bool, IFACE::Error> {
        match self.swapped {
            Some(swapped) => Ok(swapped),
            None => {
                let swapped = self.read_register::<CfgRegC>().await?.ble();
                self.swapped = Some(swapped);
                Ok(swapped)
            }
        }
    }

    /// Reads `N` consecutive registers.
    async fn read_bytes<const N: usize>(
        &mut self,
        start: RegisterAddress,
    ) -> Result<[u8; N], IFACE::Error> {
        let mut buffer = [0; N];
        self.iface.read(start.addr(), &mut buffer).await?;
        Ok(buffer)
    }
}
