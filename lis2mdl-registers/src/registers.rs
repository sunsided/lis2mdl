//! Magnetometer registers.

use crate::types::{Mode, OutputDataRate};
use bitfield_struct::bitfield;

/// Register addresses.
///
/// See also [`DEFAULT_DEVICE_ADDRESS`](crate::DEFAULT_DEVICE_ADDRESS).
#[allow(non_camel_case_types)]
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
#[repr(u8)]
pub enum RegisterAddress {
    /// See [`OffsetXL`]. Read-write.
    OFFSET_X_REG_L = 0x45,
    /// See [`OffsetXH`]. Read-write.
    OFFSET_X_REG_H = 0x46,
    /// See [`OffsetYL`]. Read-write.
    OFFSET_Y_REG_L = 0x47,
    /// See [`OffsetYH`]. Read-write.
    OFFSET_Y_REG_H = 0x48,
    /// See [`OffsetZL`]. Read-write.
    OFFSET_Z_REG_L = 0x49,
    /// See [`OffsetZH`]. Read-write.
    OFFSET_Z_REG_H = 0x4A,
    /// See [`WhoAmI`]. Read-only.
    WHO_AM_I = 0x4F,
    /// See [`CfgRegA`]. Read-write.
    CFG_REG_A = 0x60,
    /// See [`CfgRegB`]. Read-write.
    CFG_REG_B = 0x61,
    /// See [`CfgRegC`]. Read-write.
    CFG_REG_C = 0x62,
    /// See [`IntCtrlReg`]. Read-write.
    INT_CRTL_REG = 0x63,
    /// See [`IntSourceReg`]. Read-only.
    INT_SOURCE_REG = 0x64,
    /// See [`IntThsL`]. Read-write.
    INT_THS_L_REG = 0x65,
    /// See [`IntThsH`]. Read-write.
    INT_THS_H_REG = 0x66,
    /// See [`StatusReg`]. Read-only.
    STATUS_REG = 0x67,
    /// See [`OutXL`]. Read-only.
    OUTX_L_REG = 0x68,
    /// See [`OutXH`]. Read-only.
    OUTX_H_REG = 0x69,
    /// See [`OutYL`]. Read-only.
    OUTY_L_REG = 0x6A,
    /// See [`OutYH`]. Read-only.
    OUTY_H_REG = 0x6B,
    /// See [`OutZL`]. Read-only.
    OUTZ_L_REG = 0x6C,
    /// See [`OutZH`]. Read-only.
    OUTZ_H_REG = 0x6D,
    /// See [`TempOutL`]. Read-only.
    TEMP_OUT_L_REG = 0x6E,
    /// See [`TempOutH`]. Read-only.
    TEMP_OUT_H_REG = 0x6F,
}

impl RegisterAddress {
    /// Returns the address of a register.
    pub const fn addr(&self) -> u8 {
        *self as u8
    }
}

impl From<RegisterAddress> for u8 {
    fn from(value: RegisterAddress) -> Self {
        value.addr()
    }
}

/// Declares a register that holds one byte of a multi-byte value.
macro_rules! byte_register {
    ($(#[$meta:meta])* $type:ident, $addr:expr, $access:ident, $macro:ident) => {
        $(#[$meta])*
        #[bitfield(u8)]
        #[derive(PartialEq, Eq)]
        #[cfg_attr(feature = "defmt", derive(defmt::Format))]
        pub struct $type {
            /// The register value.
            #[bits(8, access = $access)]
            pub value: u8,
        }

        $macro!($type, $addr);
    };
}

byte_register!(
    /// [`OFFSET_X_REG_L`](RegisterAddress::OFFSET_X_REG_L) (45h): hard-iron offset for X, low byte.
    ///
    /// The offsets are 16-bit two's complement values at 1.5 mG/LSB. The device subtracts them
    /// from the measured values before writing the output registers.
    OffsetXL, RegisterAddress::OFFSET_X_REG_L, RW, writable_register
);
byte_register!(
    /// [`OFFSET_X_REG_H`](RegisterAddress::OFFSET_X_REG_H) (46h): hard-iron offset for X, high byte.
    OffsetXH, RegisterAddress::OFFSET_X_REG_H, RW, writable_register
);
byte_register!(
    /// [`OFFSET_Y_REG_L`](RegisterAddress::OFFSET_Y_REG_L) (47h): hard-iron offset for Y, low byte.
    OffsetYL, RegisterAddress::OFFSET_Y_REG_L, RW, writable_register
);
byte_register!(
    /// [`OFFSET_Y_REG_H`](RegisterAddress::OFFSET_Y_REG_H) (48h): hard-iron offset for Y, high byte.
    OffsetYH, RegisterAddress::OFFSET_Y_REG_H, RW, writable_register
);
byte_register!(
    /// [`OFFSET_Z_REG_L`](RegisterAddress::OFFSET_Z_REG_L) (49h): hard-iron offset for Z, low byte.
    OffsetZL, RegisterAddress::OFFSET_Z_REG_L, RW, writable_register
);
byte_register!(
    /// [`OFFSET_Z_REG_H`](RegisterAddress::OFFSET_Z_REG_H) (4Ah): hard-iron offset for Z, high byte.
    OffsetZH, RegisterAddress::OFFSET_Z_REG_H, RW, writable_register
);

/// [`WHO_AM_I`](RegisterAddress::WHO_AM_I) (4Fh)
#[bitfield(u8)]
#[derive(PartialEq, Eq)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct WhoAmI {
    /// The identification value. Always [`LIS2MDL_WHO_AM_I`](crate::LIS2MDL_WHO_AM_I).
    #[bits(8, access = RO, default = crate::LIS2MDL_WHO_AM_I)]
    pub ident: u8,
}

readable_register!(WhoAmI, RegisterAddress::WHO_AM_I);

/// [`CFG_REG_A`](RegisterAddress::CFG_REG_A) (60h): mode, output data rate, power mode, reset.
#[bitfield(u8)]
#[derive(PartialEq, Eq)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct CfgRegA {
    /// Mode of operation. Defaults to [`Mode::Idle`].
    #[bits(2, access = RW, default = Mode::Idle)]
    pub md: Mode,

    /// Output data rate. Defaults to [`OutputDataRate::Hz10`].
    #[bits(2, access = RW, default = OutputDataRate::Hz10)]
    pub odr: OutputDataRate,

    /// Low-power mode.
    ///
    /// * `false`: high-resolution mode.
    /// * `true`: low-power mode.
    #[bits(1, access = RW)]
    pub lp: bool,

    /// Soft reset. Resets the configuration and user registers. Flash registers keep their values.
    #[bits(1, access = RW)]
    pub soft_rst: bool,

    /// Reboots the magnetometer memory content.
    #[bits(1, access = RW)]
    pub reboot: bool,

    /// Temperature compensation. The datasheet requires this to be `true` for correct operation.
    #[bits(1, access = RW)]
    pub comp_temp_en: bool,
}

writable_register!(CfgRegA, RegisterAddress::CFG_REG_A);

/// [`CFG_REG_B`](RegisterAddress::CFG_REG_B) (61h): low-pass filter, offset cancellation.
#[bitfield(u8)]
#[derive(PartialEq, Eq)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct CfgRegB {
    /// Digital low-pass filter.
    ///
    /// * `false`: bandwidth is ODR/2.
    /// * `true`: bandwidth is ODR/4.
    #[bits(1, access = RW)]
    pub lpf: bool,

    /// Offset cancellation.
    #[bits(1, access = RW)]
    pub off_canc: bool,

    /// Set pulse frequency.
    ///
    /// * `false`: the set pulse is released every 64 ODR cycles.
    /// * `true`: the set pulse is released only at power-on after power-down.
    #[bits(1, access = RW)]
    pub set_freq: bool,

    /// If `true`, the interrupt block checks data after hard-iron correction.
    #[bits(1, access = RW)]
    pub int_on_data_off: bool,

    /// Offset cancellation in single measurement mode.
    /// [`CfgRegB::off_canc`] must be set as well.
    #[bits(1, access = RW)]
    pub off_canc_one_shot: bool,

    #[bits(3)]
    __: u8,
}

writable_register!(CfgRegB, RegisterAddress::CFG_REG_B);

/// [`CFG_REG_C`](RegisterAddress::CFG_REG_C) (62h): interface, data update and pin configuration.
#[bitfield(u8)]
#[derive(PartialEq, Eq)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct CfgRegC {
    /// Drives the data-ready signal ([`StatusReg::zyxda`]) on the INT/DRDY pin (push-pull).
    #[bits(1, access = RW)]
    pub drdy_on_pin: bool,

    /// Self-test.
    #[bits(1, access = RW)]
    pub self_test: bool,

    /// Enables the SDO line on pin 7 (4-wire SPI). This disables the interrupt and data-ready
    /// signals on the pin.
    #[bits(1, access = RW)]
    pub four_wire_spi: bool,

    /// If `true`, the low and high bytes of the data are swapped.
    #[bits(1, access = RW)]
    pub ble: bool,

    /// Block data update. If `true`, the low and high bytes of an output register are not updated
    /// separately, which avoids reading incoherent values during asynchronous reads.
    #[bits(1, access = RW)]
    pub bdu: bool,

    /// Disables the I²C interface. Only SPI can be used afterwards.
    #[bits(1, access = RW)]
    pub i2c_dis: bool,

    /// Drives the interrupt signal ([`IntSourceReg::int`]) on the INT/DRDY pin (push-pull).
    #[bits(1, access = RW)]
    pub int_on_pin: bool,

    #[bits(1)]
    __: u8,
}

writable_register!(CfgRegC, RegisterAddress::CFG_REG_C);

/// [`INT_CRTL_REG`](RegisterAddress::INT_CRTL_REG) (63h): interrupt configuration.
#[bitfield(u8)]
#[derive(PartialEq, Eq)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct IntCtrlReg {
    /// Enables interrupt generation.
    #[bits(1, access = RW)]
    pub ien: bool,

    /// Latches [`IntSourceReg::int`] until [`IntSourceReg`] is read. If `false`, it is pulsed.
    #[bits(1, access = RW)]
    pub iel: bool,

    /// Polarity of [`IntSourceReg::int`] (and the INT/DRDY pin).
    ///
    /// * `false`: `INT = 0` signals an interrupt.
    /// * `true`: `INT = 1` signals an interrupt.
    #[bits(1, access = RW)]
    pub iea: bool,

    #[bits(2)]
    __: u8,

    /// Enables interrupt recognition on the Z axis.
    #[bits(1, access = RW, default = true)]
    pub zien: bool,

    /// Enables interrupt recognition on the Y axis.
    #[bits(1, access = RW, default = true)]
    pub yien: bool,

    /// Enables interrupt recognition on the X axis.
    #[bits(1, access = RW, default = true)]
    pub xien: bool,
}

writable_register!(IntCtrlReg, RegisterAddress::INT_CRTL_REG);

/// [`INT_SOURCE_REG`](RegisterAddress::INT_SOURCE_REG) (64h): interrupt source.
///
/// When the interrupt is latched ([`IntCtrlReg::iel`]), reading this register resets all bits.
#[bitfield(u8)]
#[derive(PartialEq, Eq)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct IntSourceReg {
    /// Signals an interrupt event, with the polarity set by [`IntCtrlReg::iea`].
    #[bits(1, access = RO)]
    pub int: bool,

    /// Internal measurement range overflow. Always enabled; reset by reading this register.
    #[bits(1, access = RO)]
    pub mroi: bool,

    /// The Z value exceeded the negative threshold.
    #[bits(1, access = RO)]
    pub n_th_s_z: bool,

    /// The Y value exceeded the negative threshold.
    #[bits(1, access = RO)]
    pub n_th_s_y: bool,

    /// The X value exceeded the negative threshold.
    #[bits(1, access = RO)]
    pub n_th_s_x: bool,

    /// The Z value exceeded the positive threshold.
    #[bits(1, access = RO)]
    pub p_th_s_z: bool,

    /// The Y value exceeded the positive threshold.
    #[bits(1, access = RO)]
    pub p_th_s_y: bool,

    /// The X value exceeded the positive threshold.
    #[bits(1, access = RO)]
    pub p_th_s_x: bool,
}

readable_register!(IntSourceReg, RegisterAddress::INT_SOURCE_REG);

byte_register!(
    /// [`INT_THS_L_REG`](RegisterAddress::INT_THS_L_REG) (65h): interrupt threshold, low byte.
    ///
    /// The threshold is an unsigned 16-bit value at 1.5 mG/LSB. It applies to all axes as an
    /// absolute value: crossings are detected on both the positive and the negative side.
    IntThsL, RegisterAddress::INT_THS_L_REG, RW, writable_register
);
byte_register!(
    /// [`INT_THS_H_REG`](RegisterAddress::INT_THS_H_REG) (66h): interrupt threshold, high byte.
    IntThsH, RegisterAddress::INT_THS_H_REG, RW, writable_register
);

/// [`STATUS_REG`](RegisterAddress::STATUS_REG) (67h): data status.
#[bitfield(u8)]
#[derive(PartialEq, Eq)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct StatusReg {
    /// New X data is available.
    #[bits(1, access = RO)]
    pub xda: bool,

    /// New Y data is available.
    #[bits(1, access = RO)]
    pub yda: bool,

    /// New Z data is available.
    #[bits(1, access = RO)]
    pub zda: bool,

    /// New X, Y and Z data is available.
    #[bits(1, access = RO)]
    pub zyxda: bool,

    /// X data was overwritten before it was read.
    #[bits(1, access = RO)]
    pub xor: bool,

    /// Y data was overwritten before it was read.
    #[bits(1, access = RO)]
    pub yor: bool,

    /// Z data was overwritten before it was read.
    #[bits(1, access = RO)]
    pub zor: bool,

    /// X, Y or Z data was overwritten before it was read.
    #[bits(1, access = RO)]
    pub zyxor: bool,
}

readable_register!(StatusReg, RegisterAddress::STATUS_REG);

byte_register!(
    /// [`OUTX_L_REG`](RegisterAddress::OUTX_L_REG) (68h): X output, low byte.
    ///
    /// The outputs are 16-bit two's complement values at 1.5 mG/LSB.
    OutXL, RegisterAddress::OUTX_L_REG, RO, readable_register
);
byte_register!(
    /// [`OUTX_H_REG`](RegisterAddress::OUTX_H_REG) (69h): X output, high byte.
    OutXH, RegisterAddress::OUTX_H_REG, RO, readable_register
);
byte_register!(
    /// [`OUTY_L_REG`](RegisterAddress::OUTY_L_REG) (6Ah): Y output, low byte.
    OutYL, RegisterAddress::OUTY_L_REG, RO, readable_register
);
byte_register!(
    /// [`OUTY_H_REG`](RegisterAddress::OUTY_H_REG) (6Bh): Y output, high byte.
    OutYH, RegisterAddress::OUTY_H_REG, RO, readable_register
);
byte_register!(
    /// [`OUTZ_L_REG`](RegisterAddress::OUTZ_L_REG) (6Ch): Z output, low byte.
    OutZL, RegisterAddress::OUTZ_L_REG, RO, readable_register
);
byte_register!(
    /// [`OUTZ_H_REG`](RegisterAddress::OUTZ_H_REG) (6Dh): Z output, high byte.
    OutZH, RegisterAddress::OUTZ_H_REG, RO, readable_register
);
byte_register!(
    /// [`TEMP_OUT_L_REG`](RegisterAddress::TEMP_OUT_L_REG) (6Eh): temperature, low byte.
    ///
    /// The temperature is a 16-bit two's complement value at 8 LSB/°C, reading `0` at 25 °C.
    TempOutL, RegisterAddress::TEMP_OUT_L_REG, RO, readable_register
);
byte_register!(
    /// [`TEMP_OUT_H_REG`](RegisterAddress::TEMP_OUT_H_REG) (6Fh): temperature, high byte.
    TempOutH, RegisterAddress::TEMP_OUT_H_REG, RO, readable_register
);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn power_on_defaults() {
        assert_eq!(u8::from(CfgRegA::new()), 0x03);
        assert_eq!(u8::from(CfgRegB::new()), 0x00);
        assert_eq!(u8::from(CfgRegC::new()), 0x00);
        assert_eq!(u8::from(IntCtrlReg::new()), 0xE0);
        assert_eq!(u8::from(WhoAmI::new()), 0x40);
    }

    #[test]
    fn cfg_reg_a_layout() {
        let reg = CfgRegA::new()
            .with_comp_temp_en(true)
            .with_odr(OutputDataRate::Hz100)
            .with_md(Mode::Continuous);
        assert_eq!(u8::from(reg), 0b1000_1100);
        assert!(CfgRegA::from(0b0001_0101).lp());
        assert_eq!(CfgRegA::from(0b0001_0101).odr(), OutputDataRate::Hz20);
        assert_eq!(CfgRegA::from(0b0001_0101).md(), Mode::Single);
    }

    #[test]
    fn cfg_reg_b_layout() {
        let reg = CfgRegB::new().with_off_canc(true).with_lpf(true);
        assert_eq!(u8::from(reg), 0b0000_0011);
        assert!(CfgRegB::from(0b0001_0000).off_canc_one_shot());
    }

    #[test]
    fn status_layout() {
        let s = StatusReg::from(0b1000_1000);
        assert!(s.zyxda() && s.zyxor());
        assert!(!s.xda() && !s.xor());
    }
}
