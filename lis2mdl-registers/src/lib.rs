//! # STMicroelectronics LIS2MDL Magnetometer Registers
//!
//! This crate provides a typed map of the LIS2MDL's I²C registers, following
//! datasheet DS12144.
//!
//! Multi-byte values (hard-iron offsets, output data, temperature) are split into
//! one register per byte. On I²C, the LIS2MDL increments the register address
//! automatically, so consecutive registers can be read or written in one transfer.

#![deny(missing_docs)]
#![deny(warnings)]
#![no_std]
#![forbid(unsafe_code)]
// bitfield-struct generates `with_*_checked` setters that return `Result<_, ()>`.
#![allow(clippy::result_unit_err)]
// Enables the `doc_cfg` feature when the `docsrs` configuration attribute is defined.
#![cfg_attr(docsrs, feature(doc_cfg))]

/// Exports commonly used traits.
pub mod prelude {
    pub use crate::{Register, WritableRegister};
    pub use hardware_registers::i2c::*;
    pub use hardware_registers::register_address::RegisterAddress8;
    pub use hardware_registers::sizes::R1;
    pub use hardware_registers::{FromBits, HardwareRegister, ToBits, WritableHardwareRegister};
}

macro_rules! readable_register {
    ($type:ident, $addr:expr) => {
        impl $crate::Register for $type {}
        impl $crate::prelude::HardwareRegister<$crate::prelude::R1> for $type {}

        impl
            $crate::prelude::I2CRegister<
                $crate::prelude::DeviceAddress7,
                $crate::prelude::RegisterAddress8,
                $crate::prelude::R1,
            > for $type
        {
            type Backing = u8;

            const DEFAULT_DEVICE_ADDRESS: $crate::prelude::DeviceAddress7 =
                $crate::prelude::DeviceAddress7::new($crate::DEFAULT_DEVICE_ADDRESS);
            const REGISTER_ADDRESS: $crate::prelude::RegisterAddress8 =
                $crate::prelude::RegisterAddress8::new(($addr).addr());
        }

        impl $crate::prelude::ToBits for $type {
            type Target = u8;

            #[inline]
            fn to_bits(&self) -> Self::Target {
                (*self).into()
            }
        }

        impl $crate::prelude::FromBits<u8> for $type {
            #[inline]
            fn from_bits(value: u8) -> Self {
                value.into()
            }

            #[inline]
            fn from_bits_ref(value: &u8) -> Self {
                (*value).into()
            }
        }
    };
}

macro_rules! writable_register {
    ($type:ident, $addr:expr) => {
        readable_register!($type, $addr);
        impl $crate::prelude::WritableHardwareRegister<$crate::prelude::R1> for $type {}
        impl $crate::WritableRegister for $type {}
    };
}

mod registers;
mod types;

pub use registers::*;
pub use types::*;

/// The I²C bus address.
///
/// The LIS2MDL has a fixed 7-bit device address of `0011110b`
/// (write address `0x3C`, read address `0x3D`).
pub const DEFAULT_DEVICE_ADDRESS: u8 = 0x1E;

/// The value of the [`WhoAmI`] register for a LIS2MDL.
pub const LIS2MDL_WHO_AM_I: u8 = 0x40;

/// The magnetic sensitivity of the output and offset registers, in milligauss per LSB.
pub const MILLIGAUSS_PER_LSB: f32 = 1.5;

/// The temperature sensitivity of the temperature registers, in LSB per °C.
///
/// A raw value of `0` corresponds to [`TEMPERATURE_OFFSET_CELSIUS`].
pub const TEMPERATURE_LSB_PER_CELSIUS: f32 = 8.0;

/// The temperature at which the temperature registers read `0`, in °C.
pub const TEMPERATURE_OFFSET_CELSIUS: f32 = 25.0;

/// A magnetometer register.
pub trait Register: prelude::I2CRegister8<prelude::DeviceAddress7> + From<u8> + Into<u8> {}

/// A writable magnetometer register.
pub trait WritableRegister:
    prelude::WritableI2CRegister8<prelude::DeviceAddress7> + Register
{
}
