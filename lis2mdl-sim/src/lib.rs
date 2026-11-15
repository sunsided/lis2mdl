//! # STMicroelectronics LIS2MDL behavioral model
//!
//! A register-level model of the LIS2MDL magnetometer, used in two places:
//!
//! - as a fake I²C bus ([`FakeI2c`]) in host tests of the `lis2mdl` driver, and
//! - wrapped into a [Wokwi](https://wokwi.com) custom chip (`sim/wokwi-chip`).
//!
//! The model implements the I²C protocol (register pointer write followed by
//! auto-incrementing reads or writes), the operating modes, the output data rate,
//! hard-iron offset subtraction, the status flags, soft reset and the I²C disable bit.
//!
//! ## Not modelled
//!
//! Interrupt generation, self-test, the low-pass filter, offset cancellation, low-power mode,
//! big-endian output (`BLE`) and block data update (`BDU`). The bits can be written and
//! read back, but do not change the output.

#![no_std]
#![forbid(unsafe_code)]
#![deny(missing_docs)]

mod fake_i2c;

pub use fake_i2c::FakeI2c;

use lis2mdl_registers::{
    CfgRegA, CfgRegC, DEFAULT_DEVICE_ADDRESS, IntCtrlReg, LIS2MDL_WHO_AM_I, Mode,
    RegisterAddress as Reg, StatusReg,
};

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

    fn set_i16(&mut self, low: Reg, value: i16) {
        let addr = usize::from(low.addr());
        self.regs[addr..addr + 2].copy_from_slice(&value.to_le_bytes());
    }

    /// Takes one measurement: output = field - hard-iron offset.
    fn sample(&mut self) {
        let offsets = [
            Reg::OFFSET_X_REG_L,
            Reg::OFFSET_Y_REG_L,
            Reg::OFFSET_Z_REG_L,
        ];
        let outputs = [Reg::OUTX_L_REG, Reg::OUTY_L_REG, Reg::OUTZ_L_REG];
        for axis in 0..3 {
            let value = self.field[axis].saturating_sub(self.get_i16(offsets[axis]));
            self.set_i16(outputs[axis], value);
        }
        self.set_i16(Reg::TEMP_OUT_L_REG, self.temperature);

        let status = StatusReg::from(self.register(Reg::STATUS_REG));
        let overrun = if status.zyxda() { 0xF0 } else { 0x00 };
        self.set(Reg::STATUS_REG, overrun | 0x0F);
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

    /// Starts a transaction addressed to the model. Returns `true` to acknowledge.
    ///
    /// `is_read` selects a read (`true`) or write (`false`) transfer.
    pub fn i2c_start(&mut self, is_read: bool) -> bool {
        self.expect_pointer = !is_read;
        !CfgRegC::from(self.register(Reg::CFG_REG_C)).i2c_dis()
    }

    /// Writes a byte within a write transfer. Returns `true` to acknowledge.
    ///
    /// The first byte sets the register pointer; subsequent bytes are written to
    /// consecutive registers. Writes to read-only registers are ignored.
    pub fn i2c_write(&mut self, byte: u8) -> bool {
        if self.expect_pointer {
            self.pointer = byte;
            self.expect_pointer = false;
            return true;
        }
        let addr = self.pointer;
        self.pointer = self.pointer.wrapping_add(1);
        if let Some(&reg) = WRITABLE.iter().find(|r| r.addr() == addr) {
            self.write_register(reg, byte);
        }
        true
    }

    /// Reads the byte at the register pointer within a read transfer and advances the pointer.
    ///
    /// Reading the last output register clears the data-ready and overrun flags.
    pub fn i2c_read(&mut self) -> u8 {
        let addr = self.pointer;
        let value = self.regs[usize::from(addr)];
        self.pointer = self.pointer.wrapping_add(1);
        if addr == Reg::OUTZ_H_REG.addr() {
            self.set(Reg::STATUS_REG, 0);
        }
        value
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
}
