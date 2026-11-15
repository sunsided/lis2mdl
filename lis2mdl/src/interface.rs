//! Bus interfaces: I²C and SPI.

use lis2mdl_registers::{DEFAULT_DEVICE_ADDRESS, I2C_AUTO_INCREMENT, SPI_READ};

/// Register access over a bus.
///
/// Implemented by [`I2cInterface`] and [`SpiInterface`]. Transfers of more than one byte
/// address consecutive registers.
pub trait Interface {
    /// The bus error type.
    type Error;

    /// Reads consecutive registers, starting at `start`.
    fn read(&mut self, start: u8, buffer: &mut [u8]) -> Result<(), Self::Error>;

    /// Writes consecutive registers, starting at `start`.
    fn write(&mut self, start: u8, data: &[u8]) -> Result<(), Self::Error>;
}

/// Async register access over a bus.
///
/// Implemented by [`I2cInterface`] and [`SpiInterface`]. Transfers of more than one byte
/// address consecutive registers.
#[cfg(feature = "async")]
#[cfg_attr(docsrs, doc(cfg(feature = "async")))]
// The trait is meant for single-threaded executors; no `Send` bound on the futures.
#[allow(async_fn_in_trait)]
pub trait AsyncInterface {
    /// The bus error type.
    type Error;

    /// Reads consecutive registers, starting at `start`.
    async fn read(&mut self, start: u8, buffer: &mut [u8]) -> Result<(), Self::Error>;

    /// Writes consecutive registers, starting at `start`.
    async fn write(&mut self, start: u8, data: &[u8]) -> Result<(), Self::Error>;
}

/// The LIS2MDL on an I²C bus.
#[derive(Debug)]
pub struct I2cInterface<I2C> {
    i2c: I2C,
    address: u8,
}

impl<I2C> I2cInterface<I2C> {
    /// Creates an interface for a device at [`DEFAULT_DEVICE_ADDRESS`].
    pub fn new(i2c: I2C) -> Self {
        Self::with_address(i2c, DEFAULT_DEVICE_ADDRESS)
    }

    /// Creates an interface for a device at the specified 7-bit address.
    pub fn with_address(i2c: I2C, address: u8) -> Self {
        Self { i2c, address }
    }

    /// Returns the I²C bus.
    pub fn release(self) -> I2C {
        self.i2c
    }

    /// The register address byte (`SUB`), with auto-increment for multi-byte transfers.
    fn sub(start: u8, len: usize) -> u8 {
        if len > 1 {
            start | I2C_AUTO_INCREMENT
        } else {
            start
        }
    }
}

impl<I2C> Interface for I2cInterface<I2C>
where
    I2C: embedded_hal::i2c::I2c,
{
    type Error = I2C::Error;

    fn read(&mut self, start: u8, buffer: &mut [u8]) -> Result<(), Self::Error> {
        let sub = Self::sub(start, buffer.len());
        self.i2c.write_read(self.address, &[sub], buffer)
    }

    fn write(&mut self, start: u8, data: &[u8]) -> Result<(), Self::Error> {
        use embedded_hal::i2c::Operation;
        let sub = Self::sub(start, data.len());
        // Adjacent writes are sent as one transfer, without a repeated start.
        self.i2c.transaction(
            self.address,
            &mut [Operation::Write(&[sub]), Operation::Write(data)],
        )
    }
}

#[cfg(feature = "async")]
impl<I2C> AsyncInterface for I2cInterface<I2C>
where
    I2C: embedded_hal_async::i2c::I2c,
{
    type Error = I2C::Error;

    async fn read(&mut self, start: u8, buffer: &mut [u8]) -> Result<(), Self::Error> {
        let sub = Self::sub(start, buffer.len());
        self.i2c.write_read(self.address, &[sub], buffer).await
    }

    async fn write(&mut self, start: u8, data: &[u8]) -> Result<(), Self::Error> {
        use embedded_hal_async::i2c::Operation;
        let sub = Self::sub(start, data.len());
        self.i2c
            .transaction(
                self.address,
                &mut [Operation::Write(&[sub]), Operation::Write(data)],
            )
            .await
    }
}

/// The LIS2MDL on an SPI bus.
///
/// The device starts in 3-wire mode, where SDI/SDO share one line. Use a HAL that supports
/// half-duplex transfers on that line, or switch the device to 4-wire mode by setting
/// [`CfgRegC::four_wire_spi`](crate::registers::CfgRegC) with a write (writes work in
/// both modes). In 4-wire mode, the INT/DRDY pin carries SDO and signals nothing.
///
/// When only SPI is used, the datasheet allows inhibiting I²C with
/// [`CfgRegC::i2c_dis`](crate::registers::CfgRegC).
#[derive(Debug)]
pub struct SpiInterface<SPI> {
    spi: SPI,
}

impl<SPI> SpiInterface<SPI> {
    /// Creates an interface on the specified SPI device (bus plus chip select).
    pub fn new(spi: SPI) -> Self {
        Self { spi }
    }

    /// Returns the SPI device.
    pub fn release(self) -> SPI {
        self.spi
    }
}

impl<SPI> Interface for SpiInterface<SPI>
where
    SPI: embedded_hal::spi::SpiDevice,
{
    type Error = SPI::Error;

    fn read(&mut self, start: u8, buffer: &mut [u8]) -> Result<(), Self::Error> {
        use embedded_hal::spi::Operation;
        self.spi.transaction(&mut [
            Operation::Write(&[start | SPI_READ]),
            Operation::Read(buffer),
        ])
    }

    fn write(&mut self, start: u8, data: &[u8]) -> Result<(), Self::Error> {
        use embedded_hal::spi::Operation;
        self.spi.transaction(&mut [
            Operation::Write(&[start & !SPI_READ]),
            Operation::Write(data),
        ])
    }
}

#[cfg(feature = "async")]
impl<SPI> AsyncInterface for SpiInterface<SPI>
where
    SPI: embedded_hal_async::spi::SpiDevice,
{
    type Error = SPI::Error;

    async fn read(&mut self, start: u8, buffer: &mut [u8]) -> Result<(), Self::Error> {
        use embedded_hal_async::spi::Operation;
        self.spi
            .transaction(&mut [
                Operation::Write(&[start | SPI_READ]),
                Operation::Read(buffer),
            ])
            .await
    }

    async fn write(&mut self, start: u8, data: &[u8]) -> Result<(), Self::Error> {
        use embedded_hal_async::spi::Operation;
        self.spi
            .transaction(&mut [
                Operation::Write(&[start & !SPI_READ]),
                Operation::Write(data),
            ])
            .await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn i2c_auto_increment_on_multi_byte_transfers() {
        assert_eq!(I2cInterface::<()>::sub(0x68, 1), 0x68);
        assert_eq!(I2cInterface::<()>::sub(0x68, 6), 0xE8);
    }
}
