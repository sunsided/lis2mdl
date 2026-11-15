//! [`fake_bus`] adapters for the [`Lis2mdlModel`].

use crate::Lis2mdlModel;
use core::cell::RefCell;
use fake_bus::{Clock, I2cTarget, SpiTarget};

/// An I²C bus with a single [`Lis2mdlModel`] attached.
///
/// The model is shared through a [`RefCell`] so that tests can change the field while a
/// driver owns the bus. For a bus shared with other models, use [`fake_bus::FakeI2c`]
/// with an array of targets.
pub type FakeI2c<'a> = fake_bus::FakeI2c<&'a RefCell<Lis2mdlModel>>;

/// An SPI device (bus plus chip select) with a [`Lis2mdlModel`] attached.
///
/// Every transaction is one chip-select cycle. The model does not distinguish between
/// 3-wire and 4-wire mode.
pub type FakeSpi<'a> = fake_bus::FakeSpi<'a, Lis2mdlModel>;

/// A delay provider that advances the time of a [`Lis2mdlModel`] instead of waiting.
pub type FakeDelay<'a> = fake_bus::FakeDelay<'a, Lis2mdlModel>;

impl I2cTarget for Lis2mdlModel {
    fn address(&self) -> u8 {
        Lis2mdlModel::address(self)
    }

    fn start(&mut self, is_read: bool) -> bool {
        self.i2c_start(is_read)
    }

    fn write(&mut self, byte: u8) -> bool {
        self.i2c_write(byte)
    }

    fn read(&mut self) -> u8 {
        self.i2c_read()
    }
}

impl SpiTarget for Lis2mdlModel {
    fn select(&mut self) {
        self.spi_select();
    }

    fn transfer(&mut self, mosi: u8) -> u8 {
        self.spi_transfer(mosi)
    }
}

impl Clock for Lis2mdlModel {
    fn advance_ns(&mut self, ns: u64) {
        // The model works in whole microseconds; carry the rest to the next call.
        let total = self.pending_ns + ns;
        self.pending_ns = total % 1_000;
        let mut us = total / 1_000;

        while us > 0 {
            let step = us.min(u64::from(u32::MAX));
            self.tick(step as u32);
            us -= step;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use embedded_hal::delay::DelayNs;
    use embedded_hal::i2c::{ErrorKind, I2c, NoAcknowledgeSource, Operation};
    use embedded_hal::spi::SpiDevice;

    #[test]
    fn i2c_write_read() {
        let model = RefCell::new(Lis2mdlModel::new());
        let mut bus = FakeI2c::new(&model);
        let mut id = [0];
        bus.write_read(0x1E, &[0x4F], &mut id).unwrap();
        assert_eq!(id, [0x40]);
    }

    #[test]
    fn i2c_wrong_address_nacks() {
        let model = RefCell::new(Lis2mdlModel::new());
        let mut bus = FakeI2c::new(&model);
        assert_eq!(
            bus.write(0x1F, &[0x4F]),
            Err(ErrorKind::NoAcknowledge(NoAcknowledgeSource::Address))
        );
    }

    #[test]
    fn i2c_adjacent_writes_form_one_transfer() {
        let model = RefCell::new(Lis2mdlModel::new());
        let mut bus = FakeI2c::new(&model);
        bus.transaction(
            0x1E,
            &mut [Operation::Write(&[0x45]), Operation::Write(&[7, 8])],
        )
        .unwrap();

        let mut offset = [0; 2];
        bus.write_read(0x1E, &[0x45], &mut offset).unwrap();
        assert_eq!(offset, [7, 8]);
    }

    #[test]
    fn i2c_async_write_read() {
        let model = RefCell::new(Lis2mdlModel::new());
        let mut bus = FakeI2c::new(&model);
        let mut id = [0];
        embassy_futures::block_on(embedded_hal_async::i2c::I2c::write_read(
            &mut bus,
            0x1E,
            &[0x4F],
            &mut id,
        ))
        .unwrap();
        assert_eq!(id, [0x40]);
    }

    #[test]
    fn spi_read_who_am_i() {
        let model = RefCell::new(Lis2mdlModel::new());
        let mut spi = FakeSpi::new(&model);
        let mut buf = [0x80 | 0x4F, 0];
        spi.transfer_in_place(&mut buf).unwrap();
        assert_eq!(buf[1], 0x40);
    }

    #[test]
    fn spi_write_then_read_burst() {
        use embedded_hal::spi::Operation;
        let model = RefCell::new(Lis2mdlModel::new());
        let mut spi = FakeSpi::new(&model);
        spi.write(&[0x45, 1, 2, 3, 4, 5, 6]).unwrap();

        let mut buf = [0; 6];
        spi.transaction(&mut [Operation::Write(&[0x80 | 0x45]), Operation::Read(&mut buf)])
            .unwrap();
        assert_eq!(buf, [1, 2, 3, 4, 5, 6]);
    }

    #[test]
    fn delay_carries_sub_microsecond_remainder() {
        let model = RefCell::new(Lis2mdlModel::new());
        // Continuous mode at 100 Hz: one sample every 10 ms.
        model.borrow_mut().i2c_start(false);
        model.borrow_mut().i2c_write(0x60);
        model.borrow_mut().i2c_write(0x8C);
        let mut delay = FakeDelay::new(&model);

        // 10 ms in steps of 999 ns plus one short step: no time may get lost.
        for _ in 0..10_010 {
            delay.delay_ns(999);
        }
        delay.delay_ns(10);
        let status = model
            .borrow()
            .register(lis2mdl_registers::RegisterAddress::STATUS_REG);
        assert_eq!(status & 0x08, 0x08);
    }
}
