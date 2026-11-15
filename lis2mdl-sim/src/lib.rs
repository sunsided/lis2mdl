//! # STMicroelectronics LIS2MDL behavioral model
//!
//! A register-level model of the LIS2MDL magnetometer, used in two places:
//!
//! - as a fake I²C bus ([`FakeI2c`]) or SPI device ([`FakeSpi`]) in host tests of the
//!   `lis2mdl` driver, with a delay ([`FakeDelay`]) that advances the model's time, and
//! - wrapped into a [Wokwi](https://wokwi.com) custom chip (`sim/wokwi-chip`).
//!
//! The model implements:
//!
//! - the I²C protocol (register pointer write followed by auto-incrementing reads or writes)
//!   and the SPI protocol (read bit plus address, then auto-incrementing data),
//! - the I²C and SPI interfaces (with the I²C disable bit),
//! - the operating modes and the output data rate,
//! - hard-iron offset subtraction, the status flags and swapped byte order (`BLE`),
//! - threshold interrupts on the X, Y and Z axes, pulsed or latched, before or after
//!   hard-iron correction, and the INT/DRDY pin,
//! - self-test, which adds [`MODEL_SELF_TEST_LSB`] to every axis,
//! - soft reset.
//!
//! ## Not modelled
//!
//! The low-pass filter, offset cancellation, low-power mode, block data update (`BDU`),
//! the internal measurement range overflow (`MROI`) and the set pulse. The bits can be
//! written and read back, but do not change the output.
//!
//! ## Assumptions
//!
//! The datasheet states that the MSB of the I²C register address enables auto-increment.
//! ST's reference driver never sets it, so the model auto-increments regardless and
//! ignores the bit.
//!
//! With the interrupt enabled ([`IntCtrlReg::ien`]), [`IntSourceReg::int`] reads as
//! "interrupt" or "no interrupt" according to the polarity bit [`IntCtrlReg::iea`]. With
//! the interrupt disabled, it reads `0`.
//!
//! The datasheet states that `BLE` swaps the low and high bytes of "the data". The model
//! applies it to the magnetic and temperature outputs, but not to the offset and threshold
//! registers.

#![no_std]
#![forbid(unsafe_code)]
#![deny(missing_docs)]

mod bus;

pub use bus::{FakeDelay, FakeI2c, FakeSpi};

use lis2mdl_registers::{
    CfgRegA, CfgRegB, CfgRegC, DEFAULT_DEVICE_ADDRESS, IntCtrlReg, IntSourceReg, LIS2MDL_WHO_AM_I,
    Mode, RegisterAddress as Reg, SPI_READ, StatusReg,
};

/// The phase of an SPI transfer.
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
enum SpiState {
    /// The next byte holds the read bit and the register address.
    Command,
    /// The transfer reads registers.
    Read,
    /// The transfer writes registers.
    Write,
}

/// The field the model adds to every axis while self-test is enabled, in LSB (150 mG).
pub const MODEL_SELF_TEST_LSB: i16 = 100;

/// Register addresses that accept writes.
const WRITABLE: &[Reg] = &[
    Reg::OFFSET_X_REG_L,
    Reg::OFFSET_X_REG_H,
    Reg::OFFSET_Y_REG_L,
    Reg::OFFSET_Y_REG_H,
    Reg::OFFSET_Z_REG_L,
    Reg::OFFSET_Z_REG_H,
    Reg::CFG_REG_A,
    Reg::CFG_REG_B,
    Reg::CFG_REG_C,
    Reg::INT_CRTL_REG,
    Reg::INT_THS_L_REG,
    Reg::INT_THS_H_REG,
];

/// A behavioral model of the LIS2MDL.
#[derive(Debug, Clone)]
pub struct Lis2mdlModel {
    regs: [u8; 256],
    pointer: u8,
    /// `true` until the first byte of a write transaction set the register pointer.
    expect_pointer: bool,
    /// The magnetic field seen by the sensor, in LSB (1.5 mG), before offset subtraction.
    field: [i16; 3],
    /// The die temperature, in LSB (1/8 °C) relative to 25 °C.
    temperature: i16,
    /// Time since the last sample in continuous mode.
    elapsed_us: u32,
    /// Nanoseconds not yet passed to [`Self::tick`], which works in whole microseconds.
    pending_ns: u64,
    spi: SpiState,
}

impl Default for Lis2mdlModel {
    fn default() -> Self {
        Self::new()
    }
}

impl Lis2mdlModel {
    /// Creates a model freshly out of power-on reset, in idle mode.
    pub fn new() -> Self {
        let mut model = Self {
            regs: [0; 256],
            pointer: 0,
            expect_pointer: false,
            field: [0; 3],
            temperature: 0,
            elapsed_us: 0,
            pending_ns: 0,
            spi: SpiState::Command,
        };

        model.power_on();
        model
    }

    /// Resets the model as by a power cycle.
    pub fn power_on(&mut self) {
        self.regs = [0; 256];
        self.set(Reg::WHO_AM_I, LIS2MDL_WHO_AM_I);
        self.soft_reset();
    }

    /// Resets the configuration and user registers, as by [`CfgRegA::soft_rst`].
    fn soft_reset(&mut self) {
        for &reg in WRITABLE {
            self.set(reg, 0);
        }

        self.set(Reg::CFG_REG_A, CfgRegA::new().into());
        self.set(Reg::INT_CRTL_REG, IntCtrlReg::new().into());
        self.set(Reg::STATUS_REG, 0);
        self.set(Reg::INT_SOURCE_REG, 0);
        self.pointer = 0;
        self.expect_pointer = false;
        self.elapsed_us = 0;
    }

    /// The 7-bit I²C address of the model. The LIS2MDL address is fixed.
    pub fn address(&self) -> u8 {
        DEFAULT_DEVICE_ADDRESS
    }

    /// Reads a register directly, bypassing the bus.
    pub fn register(&self, reg: Reg) -> u8 {
        self.regs[usize::from(reg.addr())]
    }

    /// The current operating mode.
    pub fn mode(&self) -> Mode {
        self.cfg_a().md()
    }

    /// Sets the magnetic field seen by the sensor, in LSB (1.5 mG per LSB).
    ///
    /// The field shows in the output registers with the next sample.
    pub fn set_field(&mut self, x: i16, y: i16, z: i16) {
        self.field = [x, y, z];
    }

    /// Sets the die temperature, in LSB (8 LSB per °C, `0` at 25 °C).
    pub fn set_temperature(&mut self, raw: i16) {
        self.temperature = raw;
    }

    /// The level of the INT/DRDY pin.
    ///
    /// With [`CfgRegC::int_on_pin`], the pin follows [`IntSourceReg::int`]. Otherwise, with
    /// [`CfgRegC::drdy_on_pin`], it is high while new data is available. Otherwise, and in
    /// 4-wire SPI mode, it is low.
    pub fn int_pin(&self) -> bool {
        let cfg = CfgRegC::from(self.register(Reg::CFG_REG_C));
        if cfg.four_wire_spi() {
            false
        } else if cfg.int_on_pin() {
            self.int_source().int()
        } else if cfg.drdy_on_pin() {
            StatusReg::from(self.register(Reg::STATUS_REG)).zyxda()
        } else {
            false
        }
    }

    /// Advances time by `us` microseconds. In continuous mode, the model samples at the
    /// configured output data rate.
    pub fn tick(&mut self, us: u32) {
        if self.mode() != Mode::Continuous {
            return;
        }

        let period_us = 1_000_000 / u32::from(self.cfg_a().odr().hz());
        self.elapsed_us = self.elapsed_us.saturating_add(us);
        while self.elapsed_us >= period_us {
            self.elapsed_us -= period_us;
            self.sample();
        }
    }

    fn cfg_a(&self) -> CfgRegA {
        CfgRegA::from(self.register(Reg::CFG_REG_A))
    }

    fn set(&mut self, reg: Reg, value: u8) {
        self.regs[usize::from(reg.addr())] = value;
    }

    fn get_i16(&self, low: Reg) -> i16 {
        let addr = usize::from(low.addr());
        i16::from_le_bytes([self.regs[addr], self.regs[addr + 1]])
    }

    fn set_i16(&mut self, low: Reg, value: i16, swap: bool) {
        let addr = usize::from(low.addr());
        let bytes = if swap {
            value.to_be_bytes()
        } else {
            value.to_le_bytes()
        };

        self.regs[addr..addr + 2].copy_from_slice(&bytes);
    }

    /// Takes one measurement: output = field (+ self-test) - hard-iron offset.
    fn sample(&mut self) {
        let cfg_c = CfgRegC::from(self.register(Reg::CFG_REG_C));
        let self_test = if cfg_c.self_test() {
            MODEL_SELF_TEST_LSB
        } else {
            0
        };

        let offsets = [
            Reg::OFFSET_X_REG_L,
            Reg::OFFSET_Y_REG_L,
            Reg::OFFSET_Z_REG_L,
        ];
        let outputs = [Reg::OUTX_L_REG, Reg::OUTY_L_REG, Reg::OUTZ_L_REG];
        let mut raw = [0; 3];
        let mut corrected = [0; 3];
        for axis in 0..3 {
            raw[axis] = self.field[axis].saturating_add(self_test);
            corrected[axis] = raw[axis].saturating_sub(self.get_i16(offsets[axis]));
            self.set_i16(outputs[axis], corrected[axis], cfg_c.ble());
        }

        self.set_i16(Reg::TEMP_OUT_L_REG, self.temperature, cfg_c.ble());

        let status = StatusReg::from(self.register(Reg::STATUS_REG));
        let overrun = if status.zyxda() { 0xF0 } else { 0x00 };
        self.set(Reg::STATUS_REG, overrun | 0x0F);

        let cfg_b = CfgRegB::from(self.register(Reg::CFG_REG_B));
        let data = if cfg_b.int_on_data_off() {
            corrected
        } else {
            raw
        };

        self.update_interrupt(data);
    }

    /// Compares a sample against the threshold and updates [`IntSourceReg`].
    fn update_interrupt(&mut self, data: [i16; 3]) {
        let ctrl = IntCtrlReg::from(self.register(Reg::INT_CRTL_REG));
        if !ctrl.ien() {
            self.set(Reg::INT_SOURCE_REG, 0);
            return;
        }

        let threshold = i32::from(self.get_i16(Reg::INT_THS_L_REG) as u16);
        let enabled = [ctrl.xien(), ctrl.yien(), ctrl.zien()];
        // Positive flags in bits 7..5 (X, Y, Z), negative flags in bits 4..2.
        let mut flags = 0u8;
        for axis in 0..3 {
            let value = i32::from(data[axis]);
            if enabled[axis] && value > threshold {
                flags |= 0x80 >> axis;
            }

            if enabled[axis] && value < -threshold {
                flags |= 0x10 >> axis;
            }
        }

        if flags != 0 {
            flags |= 0x01;
        }

        let previous = self.register(Reg::INT_SOURCE_REG);
        let value = if ctrl.iel() { previous | flags } else { flags };
        self.set(Reg::INT_SOURCE_REG, value);
    }

    fn write_register(&mut self, reg: Reg, value: u8) {
        match reg {
            Reg::CFG_REG_A => {
                let cfg = CfgRegA::from(value);
                if cfg.soft_rst() {
                    self.soft_reset();
                    return;
                }
                // SOFT_RST and REBOOT clear themselves.
                self.set(reg, cfg.with_reboot(false).into());
                self.elapsed_us = 0;
                if cfg.md() == Mode::Single {
                    self.sample();
                    self.set(reg, cfg.with_reboot(false).with_md(Mode::Idle).into());
                }
            }
            _ => self.set(reg, value),
        }
    }

    /// Writes a data byte at the register pointer and advances the pointer.
    /// Writes to read-only registers are ignored.
    fn write_data(&mut self, byte: u8) {
        let addr = self.pointer;
        self.pointer = self.pointer.wrapping_add(1);
        if let Some(&reg) = WRITABLE.iter().find(|r| r.addr() == addr) {
            self.write_register(reg, byte);
        }
    }

    /// [`IntSourceReg`] as read from the bus, with the polarity of the `INT` bit applied.
    ///
    /// Internally, the register holds the `INT` bit as "interrupt active".
    pub fn int_source(&self) -> IntSourceReg {
        let ctrl = IntCtrlReg::from(self.register(Reg::INT_CRTL_REG));
        let src = IntSourceReg::from(self.register(Reg::INT_SOURCE_REG));
        let int = ctrl.ien() && (src.int() == ctrl.iea());
        IntSourceReg::from((u8::from(src) & !0x01) | u8::from(int))
    }

    /// Reads the byte at the register pointer and advances the pointer.
    fn read_data(&mut self) -> u8 {
        let addr = self.pointer;
        let mut value = self.regs[usize::from(addr)];
        self.pointer = self.pointer.wrapping_add(1);
        if addr == Reg::OUTZ_H_REG.addr() {
            self.set(Reg::STATUS_REG, 0);
        }

        if addr == Reg::INT_SOURCE_REG.addr() {
            value = self.int_source().into();
            if IntCtrlReg::from(self.register(Reg::INT_CRTL_REG)).iel() {
                self.set(Reg::INT_SOURCE_REG, 0);
            }
        }

        value
    }

    /// Starts an I²C transaction addressed to the model. Returns `true` to acknowledge.
    ///
    /// `is_read` selects a read (`true`) or write (`false`) transfer.
    pub fn i2c_start(&mut self, is_read: bool) -> bool {
        self.expect_pointer = !is_read;
        !CfgRegC::from(self.register(Reg::CFG_REG_C)).i2c_dis()
    }

    /// Writes a byte within an I²C write transfer. Returns `true` to acknowledge.
    ///
    /// The first byte sets the register pointer (the auto-increment bit is ignored);
    /// subsequent bytes are written to consecutive registers. Writes to read-only registers
    /// are ignored.
    pub fn i2c_write(&mut self, byte: u8) -> bool {
        if self.expect_pointer {
            self.pointer = byte & !lis2mdl_registers::I2C_AUTO_INCREMENT;
            self.expect_pointer = false;
        } else {
            self.write_data(byte);
        }

        true
    }

    /// Reads the byte at the register pointer within an I²C read transfer and advances
    /// the pointer.
    ///
    /// Reading the last output register clears the data-ready and overrun flags. Reading
    /// [`IntSourceReg`] clears a latched interrupt.
    pub fn i2c_read(&mut self) -> u8 {
        self.read_data()
    }

    /// Starts an SPI transfer (chip select goes low).
    pub fn spi_select(&mut self) {
        self.spi = SpiState::Command;
    }

    /// Exchanges one byte within an SPI transfer and returns the byte shifted out.
    ///
    /// The first byte after [`Self::spi_select`] holds the read bit and the register
    /// address. On a read transfer, the following bytes read consecutive registers;
    /// on a write transfer, they write them.
    pub fn spi_transfer(&mut self, mosi: u8) -> u8 {
        match self.spi {
            SpiState::Command => {
                self.pointer = mosi & !SPI_READ;
                self.spi = if mosi & SPI_READ != 0 {
                    SpiState::Read
                } else {
                    SpiState::Write
                };

                0
            }
            SpiState::Read => self.read_data(),
            SpiState::Write => {
                self.write_data(mosi);
                0
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write(model: &mut Lis2mdlModel, bytes: &[u8]) -> bool {
        model.i2c_start(false) && bytes.iter().all(|&b| model.i2c_write(b))
    }

    fn read<const N: usize>(model: &mut Lis2mdlModel, reg: u8) -> [u8; N] {
        assert!(write(model, &[reg]));
        assert!(model.i2c_start(true));
        core::array::from_fn(|_| model.i2c_read())
    }

    #[test]
    fn power_on_state() {
        let mut model = Lis2mdlModel::new();
        assert_eq!(read::<1>(&mut model, 0x4F), [0x40]);
        assert_eq!(read::<4>(&mut model, 0x60), [0x03, 0x00, 0x00, 0xE0]);
        assert_eq!(model.mode(), Mode::Idle);
    }

    #[test]
    fn continuous_sampling_at_odr() {
        let mut model = Lis2mdlModel::new();
        model.set_field(100, -200, 300);
        model.set_temperature(-16);
        // COMP_TEMP_EN, ODR = 50 Hz, continuous.
        assert!(write(&mut model, &[0x60, 0x88]));
        model.tick(19_999);
        assert_eq!(read::<1>(&mut model, 0x67), [0x00]);
        model.tick(1);
        assert_eq!(read::<1>(&mut model, 0x67), [0x0F]);
        assert_eq!(
            read::<8>(&mut model, 0x68),
            [100, 0, 0x38, 0xFF, 0x2C, 0x01, 0xF0, 0xFF]
        );
        assert_eq!(read::<1>(&mut model, 0x67), [0x00]);
    }

    #[test]
    fn overrun() {
        let mut model = Lis2mdlModel::new();
        assert!(write(&mut model, &[0x60, 0x8C]));
        model.tick(20_000);
        assert_eq!(read::<1>(&mut model, 0x67), [0xFF]);
    }

    #[test]
    fn single_measurement_returns_to_idle() {
        let mut model = Lis2mdlModel::new();
        model.set_field(1, 2, 3);
        assert!(write(&mut model, &[0x60, 0x81]));
        assert_eq!(model.mode(), Mode::Idle);
        assert_eq!(read::<6>(&mut model, 0x68), [1, 0, 2, 0, 3, 0]);
        model.tick(1_000_000);
        assert_eq!(read::<1>(&mut model, 0x67), [0x00]);
    }

    #[test]
    fn hard_iron_offset_is_subtracted() {
        let mut model = Lis2mdlModel::new();
        model.set_field(100, 100, 100);
        assert!(write(&mut model, &[0x45, 10, 0, 0xF6, 0xFF, 0, 0]));
        assert!(write(&mut model, &[0x60, 0x01]));
        assert_eq!(read::<6>(&mut model, 0x68), [90, 0, 110, 0, 100, 0]);
    }

    #[test]
    fn soft_reset_clears_configuration() {
        let mut model = Lis2mdlModel::new();
        assert!(write(&mut model, &[0x45, 10, 0]));
        assert!(write(&mut model, &[0x60, 0x8C, 0x03]));
        assert!(write(&mut model, &[0x60, 0x20]));
        assert_eq!(read::<2>(&mut model, 0x60), [0x03, 0x00]);
        assert_eq!(read::<1>(&mut model, 0x45), [0]);
    }

    #[test]
    fn read_only_registers_ignore_writes() {
        let mut model = Lis2mdlModel::new();
        assert!(write(&mut model, &[0x4F, 0x00]));
        assert_eq!(model.register(Reg::WHO_AM_I), LIS2MDL_WHO_AM_I);
    }

    #[test]
    fn i2c_disable_nacks() {
        let mut model = Lis2mdlModel::new();
        assert!(write(&mut model, &[0x62, 0x20]));
        assert!(!model.i2c_start(true));
    }

    #[test]
    fn i2c_auto_increment_bit_is_ignored() {
        let mut model = Lis2mdlModel::new();
        assert_eq!(read::<1>(&mut model, 0x80 | 0x4F), [0x40]);
    }

    #[test]
    fn ble_swaps_output_bytes() {
        let mut model = Lis2mdlModel::new();
        model.set_field(0x0102, 0, 0);
        model.set_temperature(0x0304);
        assert!(write(&mut model, &[0x62, 0x08]));
        assert!(write(&mut model, &[0x60, 0x01]));
        assert_eq!(read::<2>(&mut model, 0x68), [0x01, 0x02]);
        assert_eq!(read::<2>(&mut model, 0x6E), [0x03, 0x04]);
    }

    #[test]
    fn self_test_adds_field() {
        let mut model = Lis2mdlModel::new();
        assert!(write(&mut model, &[0x62, 0x02]));
        assert!(write(&mut model, &[0x60, 0x01]));
        let expected = MODEL_SELF_TEST_LSB.to_le_bytes();
        assert_eq!(read::<2>(&mut model, 0x6C), expected);
    }

    #[test]
    fn threshold_interrupt_pulsed() {
        let mut model = Lis2mdlModel::new();
        // Threshold 100 LSB, interrupt enabled on all axes, active high, pulsed, on the pin.
        assert!(write(&mut model, &[0x65, 100, 0]));
        assert!(write(&mut model, &[0x63, 0xE5]));
        assert!(write(&mut model, &[0x62, 0x40]));

        model.set_field(101, -101, 100);
        assert!(write(&mut model, &[0x60, 0x01]));
        assert_eq!(read::<1>(&mut model, 0x64), [0b1000_1001]);
        assert!(model.int_pin());
        // Pulsed: reading does not clear, the next sample does.
        assert!(model.int_pin());
        model.set_field(0, 0, 0);
        assert!(write(&mut model, &[0x60, 0x01]));
        assert_eq!(read::<1>(&mut model, 0x64), [0]);
        assert!(!model.int_pin());
    }

    #[test]
    fn threshold_interrupt_latched_active_low() {
        let mut model = Lis2mdlModel::new();
        // Threshold 10 LSB, only Z, active low, latched.
        assert!(write(&mut model, &[0x65, 10, 0]));
        assert!(write(&mut model, &[0x63, 0x23]));
        assert!(write(&mut model, &[0x62, 0x40]));
        model.set_field(100, 100, 0);
        assert!(write(&mut model, &[0x60, 0x01]));
        // Active low: no interrupt reads as `INT = 1`.
        assert!(model.int_source().int());
        assert!(model.int_pin());

        model.set_field(0, 0, 20);
        assert!(write(&mut model, &[0x60, 0x01]));
        model.set_field(0, 0, 0);
        assert!(write(&mut model, &[0x60, 0x01]));
        assert!(!model.int_pin());
        assert_eq!(read::<1>(&mut model, 0x64), [0b0010_0000]);
        assert!(model.int_pin());
    }

    #[test]
    fn interrupt_after_hard_iron_correction() {
        let mut model = Lis2mdlModel::new();
        assert!(write(&mut model, &[0x45, 50, 0]));
        assert!(write(&mut model, &[0x65, 30, 0]));
        assert!(write(&mut model, &[0x63, 0x85]));
        model.set_field(60, 0, 0);
        assert!(write(&mut model, &[0x60, 0x01]));
        assert_eq!(read::<1>(&mut model, 0x64), [0b1000_0001]);
        assert!(write(&mut model, &[0x61, 0x08]));
        assert!(write(&mut model, &[0x60, 0x01]));
        assert_eq!(read::<1>(&mut model, 0x64), [0]);
    }

    #[test]
    fn four_wire_spi_disables_pin() {
        let mut model = Lis2mdlModel::new();
        assert!(write(&mut model, &[0x62, 0x05]));
        assert!(write(&mut model, &[0x60, 0x01]));
        assert!(!model.int_pin());
        assert!(write(&mut model, &[0x62, 0x01]));
        assert!(model.int_pin());
    }
}
