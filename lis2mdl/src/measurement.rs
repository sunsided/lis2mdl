//! Decoding of the output and offset registers.

use lis2mdl_registers::{
    MILLIGAUSS_PER_LSB, RegisterAddress, TEMPERATURE_LSB_PER_CELSIUS, TEMPERATURE_OFFSET_CELSIUS,
};

/// A magnetic field vector in raw sensor units (1.5 mG per LSB).
///
/// Used for both the output registers and the hard-iron offset registers.
#[derive(Debug, Default, Copy, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct MagneticField {
    /// The X component.
    pub x: i16,
    /// The Y component.
    pub y: i16,
    /// The Z component.
    pub z: i16,
}

impl MagneticField {
    /// The first output register of the burst read.
    pub(crate) const OUTPUT_START: RegisterAddress = RegisterAddress::OUTX_L_REG;

    /// The first hard-iron offset register.
    pub(crate) const OFFSET_START: RegisterAddress = RegisterAddress::OFFSET_X_REG_L;

    /// The number of bytes of a vector (three little-endian 16-bit values).
    pub const LEN: usize = 6;

    /// Creates a vector from raw components.
    pub const fn new(x: i16, y: i16, z: i16) -> Self {
        Self { x, y, z }
    }

    /// Creates a vector from components in milligauss, rounding to the nearest LSB.
    ///
    /// Values out of range saturate.
    pub fn from_milligauss(x: f32, y: f32, z: f32) -> Self {
        // `as` saturates on overflow; rounding without `std` via an offset of ±0.5.
        let lsb = |v: f32| {
            let v = v / MILLIGAUSS_PER_LSB;
            (if v < 0.0 { v - 0.5 } else { v + 0.5 }) as i16
        };
        Self::new(lsb(x), lsb(y), lsb(z))
    }

    /// Decodes registers `X_L` through `Z_H` (little-endian, `BLE = 0`).
    pub const fn from_bytes(bytes: &[u8; Self::LEN]) -> Self {
        Self {
            x: i16::from_le_bytes([bytes[0], bytes[1]]),
            y: i16::from_le_bytes([bytes[2], bytes[3]]),
            z: i16::from_le_bytes([bytes[4], bytes[5]]),
        }
    }

    /// Encodes the vector for registers `X_L` through `Z_H` (little-endian).
    pub const fn to_bytes(&self) -> [u8; Self::LEN] {
        let [x0, x1] = self.x.to_le_bytes();
        let [y0, y1] = self.y.to_le_bytes();
        let [z0, z1] = self.z.to_le_bytes();
        [x0, x1, y0, y1, z0, z1]
    }

    /// The components in milligauss.
    pub fn milligauss(&self) -> [f32; 3] {
        [self.x, self.y, self.z].map(|v| f32::from(v) * MILLIGAUSS_PER_LSB)
    }

    /// The components in microtesla (1 µT = 10 mG).
    pub fn microtesla(&self) -> [f32; 3] {
        self.milligauss().map(|v| v / 10.0)
    }
}

/// A die temperature reading in raw sensor units (8 LSB per °C, `0` at 25 °C).
///
/// The temperature sensor is meant for compensation, not as an absolute thermometer.
#[derive(Debug, Default, Copy, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct Temperature {
    /// The raw register value.
    pub raw: i16,
}

impl Temperature {
    /// The first register of the burst read.
    pub(crate) const START: RegisterAddress = RegisterAddress::TEMP_OUT_L_REG;

    /// The number of bytes in the burst read.
    pub const LEN: usize = 2;

    /// Decodes registers `TEMP_OUT_L_REG` and `TEMP_OUT_H_REG`.
    pub const fn from_bytes(bytes: &[u8; Self::LEN]) -> Self {
        Self {
            raw: i16::from_le_bytes(*bytes),
        }
    }

    /// The temperature in °C.
    pub fn celsius(&self) -> f32 {
        f32::from(self.raw) / TEMPERATURE_LSB_PER_CELSIUS + TEMPERATURE_OFFSET_CELSIUS
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn field_roundtrip_and_units() {
        let field = MagneticField::from_bytes(&[0x64, 0x00, 0x38, 0xFF, 0x2C, 0x01]);
        assert_eq!(field, MagneticField::new(100, -200, 300));
        assert_eq!(field.to_bytes(), [0x64, 0x00, 0x38, 0xFF, 0x2C, 0x01]);
        assert_eq!(field.milligauss(), [150.0, -300.0, 450.0]);
        assert_eq!(field.microtesla(), [15.0, -30.0, 45.0]);
    }

    #[test]
    fn from_milligauss_rounds_and_saturates() {
        assert_eq!(
            MagneticField::from_milligauss(150.0, -2.0, 1.0e9),
            MagneticField::new(100, -1, i16::MAX)
        );
    }

    #[test]
    fn temperature() {
        assert_eq!(Temperature::from_bytes(&[0x00, 0x00]).celsius(), 25.0);
        assert_eq!(Temperature::from_bytes(&[0xF0, 0xFF]).celsius(), 23.0);
    }
}
