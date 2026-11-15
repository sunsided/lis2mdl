//! Value types used in register fields.

/// The operating mode, as set in [`CfgRegA::md`](crate::CfgRegA).
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
#[repr(u8)]
pub enum Mode {
    /// Continuous mode: the device measures continuously at the configured output data rate.
    Continuous = 0b00,
    /// Single mode: the device performs one measurement, sets `DRDY` and returns to idle mode.
    Single = 0b01,
    /// Idle mode. The device is placed in idle mode with the I²C and SPI interfaces active.
    ///
    /// The datasheet assigns both `10` and `11` to idle mode. `11` is the power-on default.
    Idle = 0b11,
}

impl Mode {
    /// Converts the value into its register representation.
    pub const fn into_bits(self) -> u8 {
        self as u8
    }

    /// Converts the two lowest bits of a register value into a mode.
    pub const fn from_bits(value: u8) -> Self {
        match value & 0b11 {
            0b00 => Self::Continuous,
            0b01 => Self::Single,
            _ => Self::Idle,
        }
    }
}

/// The output data rate, as set in [`CfgRegA::odr`](crate::CfgRegA).
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
#[repr(u8)]
pub enum OutputDataRate {
    /// 10 Hz. The power-on default.
    Hz10 = 0b00,
    /// 20 Hz.
    Hz20 = 0b01,
    /// 50 Hz.
    Hz50 = 0b10,
    /// 100 Hz.
    Hz100 = 0b11,
}

impl OutputDataRate {
    /// Converts the value into its register representation.
    pub const fn into_bits(self) -> u8 {
        self as u8
    }

    /// Converts the two lowest bits of a register value into an output data rate.
    pub const fn from_bits(value: u8) -> Self {
        match value & 0b11 {
            0b00 => Self::Hz10,
            0b01 => Self::Hz20,
            0b10 => Self::Hz50,
            _ => Self::Hz100,
        }
    }

    /// Returns the output data rate in Hz.
    pub const fn hz(self) -> u8 {
        match self {
            Self::Hz10 => 10,
            Self::Hz20 => 20,
            Self::Hz50 => 50,
            Self::Hz100 => 100,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mode_roundtrip() {
        for m in [Mode::Continuous, Mode::Single, Mode::Idle] {
            assert_eq!(Mode::from_bits(m.into_bits()), m);
        }
        assert_eq!(Mode::from_bits(0b10), Mode::Idle);
    }

    #[test]
    fn odr_roundtrip() {
        for odr in [
            OutputDataRate::Hz10,
            OutputDataRate::Hz20,
            OutputDataRate::Hz50,
            OutputDataRate::Hz100,
        ] {
            assert_eq!(OutputDataRate::from_bits(odr.into_bits()), odr);
        }
    }
}
