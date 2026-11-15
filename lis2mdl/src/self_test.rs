//! Self-test procedure, shared by the blocking and async drivers.
//!
//! The procedure follows ST's reference self-test for the LIS2MDL: measure at 100 Hz with
//! offset cancellation, average [`SELF_TEST_SAMPLES`] samples with self-test disabled and
//! enabled, and check the per-axis difference against the limits of datasheet table 2.

use crate::MagneticField;
use lis2mdl_registers::{CfgRegA, CfgRegB, CfgRegC, MILLIGAUSS_PER_LSB, Mode, OutputDataRate};

/// The minimum absolute self-test difference per axis, in milligauss (datasheet table 2).
pub const SELF_TEST_MIN_MILLIGAUSS: f32 = 15.0;

/// The maximum absolute self-test difference per axis, in milligauss (datasheet table 2).
pub const SELF_TEST_MAX_MILLIGAUSS: f32 = 500.0;

/// The number of samples averaged with self-test disabled and enabled.
pub const SELF_TEST_SAMPLES: u8 = 50;

/// The settling time after starting the measurement, in milliseconds.
pub(crate) const SETTLE_MS: u32 = 20;

/// The settling time after enabling self-test, in milliseconds.
pub(crate) const SELF_TEST_SETTLE_MS: u32 = 60;

/// The polling interval while waiting for data, in milliseconds.
pub(crate) const POLL_MS: u32 = 1;

/// The number of polls before giving up on a sample. At 100 Hz, a sample takes 10 ms.
pub(crate) const MAX_POLLS: u32 = 100;

/// An error during the self-test.
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub enum SelfTestError<E> {
    /// A bus error.
    Bus(E),
    /// The device did not signal new data in time.
    Timeout,
}

impl<E> From<E> for SelfTestError<E> {
    fn from(error: E) -> Self {
        Self::Bus(error)
    }
}

/// The result of a self-test.
#[derive(Debug, Copy, Clone, PartialEq)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct SelfTest {
    /// The absolute difference between the averages with and without self-test, per axis,
    /// in milligauss.
    pub difference_milligauss: [f32; 3],
}

impl SelfTest {
    /// Evaluates the sums of [`SELF_TEST_SAMPLES`] samples without and with self-test.
    pub(crate) fn from_sums(without: [i32; 3], with: [i32; 3]) -> Self {
        let n = f32::from(SELF_TEST_SAMPLES);
        let difference = |axis: usize| {
            let d = (with[axis] - without[axis]) as f32 / n * MILLIGAUSS_PER_LSB;
            if d < 0.0 { -d } else { d }
        };
        Self {
            difference_milligauss: [difference(0), difference(1), difference(2)],
        }
    }

    /// Whether every axis is within [`SELF_TEST_MIN_MILLIGAUSS`] and
    /// [`SELF_TEST_MAX_MILLIGAUSS`].
    pub fn passed(&self) -> bool {
        self.difference_milligauss
            .iter()
            .all(|d| (SELF_TEST_MIN_MILLIGAUSS..=SELF_TEST_MAX_MILLIGAUSS).contains(d))
    }
}

/// The configuration registers during the self-test.
///
/// Keeps the interface bits of `CFG_REG_C` (`I2C_DIS`, `4WSPI`, `BLE`) so that the bus keeps
/// working.
pub(crate) fn configuration(saved_c: CfgRegC) -> (CfgRegA, CfgRegB, CfgRegC) {
    let a = CfgRegA::new()
        .with_comp_temp_en(true)
        .with_odr(OutputDataRate::Hz100)
        .with_md(Mode::Continuous);
    let b = CfgRegB::new().with_off_canc(true);
    let c = saved_c.with_bdu(true).with_self_test(false);
    (a, b, c)
}

/// Adds a sample to a running sum.
pub(crate) fn accumulate(sum: &mut [i32; 3], sample: MagneticField) {
    sum[0] += i32::from(sample.x);
    sum[1] += i32::from(sample.y);
    sum[2] += i32::from(sample.z);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn evaluation() {
        let n = i32::from(SELF_TEST_SAMPLES);
        let result = SelfTest::from_sums([0, 0, 0], [100 * n, -100 * n, 9 * n]);
        assert_eq!(result.difference_milligauss, [150.0, 150.0, 13.5]);
        assert!(!result.passed());
        let result = SelfTest::from_sums([0, 0, 0], [10 * n, 10 * n, 10 * n]);
        assert!(result.passed());
    }
}
