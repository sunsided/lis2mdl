//! An `embedded-hal` SPI device backed by a [`Lis2mdlModel`].

use crate::Lis2mdlModel;
use core::cell::RefCell;
use embedded_hal::spi::{ErrorKind, ErrorType, Operation};

/// An SPI device (bus plus chip select) with a [`Lis2mdlModel`] attached.
///
/// Every transaction is one chip-select cycle. The model does not distinguish between
/// 3-wire and 4-wire mode.
#[derive(Debug, Clone, Copy)]
pub struct FakeSpi<'a> {
    model: &'a RefCell<Lis2mdlModel>,
}

impl<'a> FakeSpi<'a> {
    /// Creates a device with the specified model attached.
    pub fn new(model: &'a RefCell<Lis2mdlModel>) -> Self {
        Self { model }
    }

    fn run(&mut self, operations: &mut [Operation<'_, u8>]) -> Result<(), ErrorKind> {
        let mut model = self.model.borrow_mut();
        model.spi_select();
        for operation in operations {
            match operation {
                Operation::Write(bytes) => bytes.iter().for_each(|&b| {
                    model.spi_transfer(b);
                }),
                Operation::Read(buffer) => {
                    buffer.iter_mut().for_each(|b| *b = model.spi_transfer(0))
                }
                Operation::Transfer(read, write) => {
                    for i in 0..read.len().max(write.len()) {
                        let miso = model.spi_transfer(write.get(i).copied().unwrap_or(0));
                        if let Some(b) = read.get_mut(i) {
                            *b = miso;
                        }
                    }
                }
                Operation::TransferInPlace(buffer) => {
                    buffer.iter_mut().for_each(|b| *b = model.spi_transfer(*b))
                }
                Operation::DelayNs(_) => {}
            }
        }

        Ok(())
    }
}

impl ErrorType for FakeSpi<'_> {
    type Error = ErrorKind;
}

impl embedded_hal::spi::SpiDevice for FakeSpi<'_> {
    fn transaction(&mut self, operations: &mut [Operation<'_, u8>]) -> Result<(), Self::Error> {
        self.run(operations)
    }
}

impl embedded_hal_async::spi::SpiDevice for FakeSpi<'_> {
    async fn transaction(
        &mut self,
        operations: &mut [Operation<'_, u8>],
    ) -> Result<(), Self::Error> {
        self.run(operations)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use embedded_hal::spi::SpiDevice;

    #[test]
    fn read_who_am_i() {
        let model = RefCell::new(Lis2mdlModel::new());
        let mut spi = FakeSpi::new(&model);
        let mut buf = [0x80 | 0x4F, 0];
        spi.transfer_in_place(&mut buf).unwrap();
        assert_eq!(buf[1], 0x40);
    }

    #[test]
    fn write_then_read_burst() {
        let model = RefCell::new(Lis2mdlModel::new());
        let mut spi = FakeSpi::new(&model);
        spi.write(&[0x45, 1, 2, 3, 4, 5, 6]).unwrap();
        let mut buf = [0; 6];
        spi.transaction(&mut [Operation::Write(&[0x80 | 0x45]), Operation::Read(&mut buf)])
            .unwrap();
        assert_eq!(buf, [1, 2, 3, 4, 5, 6]);
    }
}
