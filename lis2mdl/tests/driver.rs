//! Driver tests against the behavioral model.
//!
//! Every test runs twice: against the blocking [`lis2mdl::Lis2mdl`] and, with the `async`
//! feature, against [`lis2mdl::asynch::Lis2mdl`]. Test bodies call the driver through
//! `aw!(…)`, which awaits the call in the async variant. That keeps both APIs under the
//! same tests, so a fix applied to only one of them shows up as a failing test.

use core::cell::RefCell;
use lis2mdl::registers::{CfgRegA, CfgRegB, CfgRegC, IntCtrlReg, RegisterAddress};
use lis2mdl::{MagneticField, Mode, OutputDataRate, SelfTestError};
use lis2mdl_sim::{FakeDelay, FakeI2c, FakeSpi, Lis2mdlModel, MODEL_SELF_TEST_LSB};

/// A delay provider that does not advance time, so the model never produces data.
struct NoDelay;

impl embedded_hal::delay::DelayNs for NoDelay {
    fn delay_ns(&mut self, _: u32) {}
}

#[cfg(feature = "async")]
impl embedded_hal_async::delay::DelayNs for NoDelay {
    async fn delay_ns(&mut self, _: u32) {}
}

/// Defines each test once per driver API.
macro_rules! driver_tests {
    ($($name:ident $body:block)*) => {
        mod blocking {
            use super::*;
            use lis2mdl::Lis2mdl as Driver;

            macro_rules! aw {
                ($call:expr) => {
                    $call
                };
            }

            $(
                #[test]
                fn $name() $body
            )*
        }

        #[cfg(feature = "async")]
        mod asynchronous {
            use super::*;
            use lis2mdl::asynch::Lis2mdl as Driver;

            macro_rules! aw {
                ($call:expr) => {
                    $call.await
                };
            }

            $(
                #[test]
                fn $name() {
                    embassy_futures::block_on(async $body)
                }
            )*
        }
    };
}

driver_tests! {
    identify {
        let model = RefCell::new(Lis2mdlModel::new());
        let mut mag = Driver::new(FakeI2c::new(&model));
        assert_eq!(aw!(mag.who_am_i()).unwrap(), lis2mdl::LIS2MDL_WHO_AM_I);
        assert!(aw!(mag.identify()).unwrap());
    }

    continuous_mode_keeps_power_mode {
        let model = RefCell::new(Lis2mdlModel::new());
        let mut mag = Driver::new(FakeI2c::new(&model));
        aw!(mag.modify_register(|r: CfgRegA| r.with_lp(true))).unwrap();
        aw!(mag.start_continuous(OutputDataRate::Hz100)).unwrap();

        let cfg = CfgRegA::from(model.borrow().register(RegisterAddress::CFG_REG_A));
        assert!(cfg.lp() && cfg.comp_temp_en());
        assert_eq!(cfg.odr(), OutputDataRate::Hz100);
        assert_eq!(cfg.md(), Mode::Continuous);

        model.borrow_mut().tick(10_000);
        assert!(aw!(mag.status()).unwrap().zyxda());

        aw!(mag.stop()).unwrap();
        assert_eq!(model.borrow().mode(), Mode::Idle);
    }

    single_measurement {
        let model = RefCell::new(Lis2mdlModel::new());
        model.borrow_mut().set_field(-1, 2, -3);
        model.borrow_mut().set_temperature(8);
        let mut mag = Driver::new(FakeI2c::new(&model));

        assert!(!aw!(mag.data_ready()).unwrap());
        aw!(mag.trigger_single()).unwrap();
        assert!(aw!(mag.data_ready()).unwrap());
        assert_eq!(
            aw!(mag.read_magnetic_field()).unwrap(),
            MagneticField::new(-1, 2, -3)
        );
        assert!(!aw!(mag.data_ready()).unwrap());
        assert_eq!(aw!(mag.read_temperature()).unwrap().celsius(), 26.0);
    }

    hard_iron_offset_roundtrip {
        let model = RefCell::new(Lis2mdlModel::new());
        let mut mag = Driver::new(FakeI2c::new(&model));
        let offset = MagneticField::new(-1234, 5, 32767);
        aw!(mag.set_hard_iron_offset(offset)).unwrap();
        assert_eq!(aw!(mag.hard_iron_offset()).unwrap(), offset);
    }

    modify_register_preserves_other_bits {
        let model = RefCell::new(Lis2mdlModel::new());
        let mut mag = Driver::new(FakeI2c::new(&model));
        aw!(mag.modify_register(|r: CfgRegB| r.with_off_canc(true))).unwrap();
        aw!(mag.modify_register(|r: CfgRegB| r.with_lpf(true))).unwrap();
        assert_eq!(
            aw!(mag.read_register::<CfgRegB>()).unwrap(),
            CfgRegB::from(0b11)
        );
    }

    wrong_address_fails {
        let model = RefCell::new(Lis2mdlModel::new());
        let mut mag = Driver::with_address(FakeI2c::new(&model), 0x1F);
        assert!(aw!(mag.identify()).is_err());
        let _bus = mag.release();
    }

    spi_interface {
        let model = RefCell::new(Lis2mdlModel::new());
        model.borrow_mut().set_field(7, -8, 9);
        let mut mag = Driver::new_spi(FakeSpi::new(&model));
        assert!(aw!(mag.identify()).unwrap());
        aw!(mag.set_hard_iron_offset(MagneticField::new(1, 1, 1))).unwrap();
        assert_eq!(
            aw!(mag.hard_iron_offset()).unwrap(),
            MagneticField::new(1, 1, 1)
        );

        aw!(mag.trigger_single()).unwrap();
        assert_eq!(
            aw!(mag.read_magnetic_field()).unwrap(),
            MagneticField::new(6, -9, 8)
        );
        let _spi = mag.release();
    }

    byte_order_is_tracked {
        let model = RefCell::new(Lis2mdlModel::new());
        model.borrow_mut().set_field(0x0102, -2, 3);
        model.borrow_mut().set_temperature(-16);
        let mut mag = Driver::new(FakeI2c::new(&model));
        aw!(mag.modify_register(|r: CfgRegC| r.with_ble(true))).unwrap();
        aw!(mag.trigger_single()).unwrap();
        assert_eq!(
            aw!(mag.read_magnetic_field()).unwrap(),
            MagneticField::new(0x0102, -2, 3)
        );
        assert_eq!(aw!(mag.read_temperature()).unwrap().celsius(), 23.0);

        // A new driver instance (e.g. after an MCU reset) reads BLE from the device.
        let mut mag = Driver::new(mag.release());
        aw!(mag.trigger_single()).unwrap();
        assert_eq!(
            aw!(mag.read_magnetic_field()).unwrap(),
            MagneticField::new(0x0102, -2, 3)
        );

        // Soft reset clears BLE.
        aw!(mag.soft_reset()).unwrap();
        aw!(mag.trigger_single()).unwrap();
        assert_eq!(
            aw!(mag.read_magnetic_field()).unwrap(),
            MagneticField::new(0x0102, -2, 3)
        );
    }

    soft_reset_clears_offset {
        let model = RefCell::new(Lis2mdlModel::new());
        let mut mag = Driver::new(FakeI2c::new(&model));
        aw!(mag.set_hard_iron_offset(MagneticField::new(5, 5, 5))).unwrap();
        aw!(mag.start_continuous(OutputDataRate::Hz20)).unwrap();
        aw!(mag.soft_reset()).unwrap();
        assert_eq!(aw!(mag.hard_iron_offset()).unwrap(), MagneticField::default());
        assert_eq!(model.borrow().mode(), Mode::Idle);
    }

    configuration_helpers {
        let model = RefCell::new(Lis2mdlModel::new());
        let mut mag = Driver::new(FakeI2c::new(&model));
        aw!(mag.set_low_power(true)).unwrap();
        aw!(mag.set_low_pass_filter(true)).unwrap();
        aw!(mag.set_offset_cancellation(true)).unwrap();
        aw!(mag.set_block_data_update(true)).unwrap();
        aw!(mag.reboot()).unwrap();
        assert!(aw!(mag.read_register::<CfgRegA>()).unwrap().lp());
        assert_eq!(u8::from(aw!(mag.read_register::<CfgRegB>()).unwrap()), 0b1_0011);
        assert!(aw!(mag.read_register::<CfgRegC>()).unwrap().bdu());

        aw!(mag.set_offset_cancellation(false)).unwrap();
        assert_eq!(u8::from(aw!(mag.read_register::<CfgRegB>()).unwrap()), 0b0_0001);
    }

    threshold_interrupt {
        let model = RefCell::new(Lis2mdlModel::new());
        let mut mag = Driver::new(FakeI2c::new(&model));
        aw!(mag.set_interrupt_threshold(40_000)).unwrap();
        assert_eq!(aw!(mag.interrupt_threshold()).unwrap(), 40_000);

        aw!(mag.set_interrupt_threshold(100)).unwrap();
        aw!(mag.write_register(
            IntCtrlReg::new().with_ien(true).with_iea(true).with_iel(true)
        ))
        .unwrap();
        aw!(mag.modify_register(|r: CfgRegC| r.with_int_on_pin(true))).unwrap();

        model.borrow_mut().set_field(0, -150, 0);
        aw!(mag.trigger_single()).unwrap();
        assert!(model.borrow().int_pin());

        let src = aw!(mag.interrupt_source()).unwrap();
        assert!(src.int() && src.n_th_s_y() && !src.p_th_s_y() && !src.n_th_s_x());
        assert!(!model.borrow().int_pin());
    }

    self_test_passes_and_restores_configuration {
        let model = RefCell::new(Lis2mdlModel::new());
        model.borrow_mut().set_field(300, -200, 100);
        let mut mag = Driver::new(FakeI2c::new(&model));
        aw!(mag.set_low_power(true)).unwrap();
        let config = [
            RegisterAddress::CFG_REG_A,
            RegisterAddress::CFG_REG_B,
            RegisterAddress::CFG_REG_C,
        ];
        let before = config.map(|r| model.borrow().register(r));

        let result = aw!(mag.self_test(&mut FakeDelay::new(&model))).unwrap();
        let expected = f32::from(MODEL_SELF_TEST_LSB) * 1.5;
        assert_eq!(result.difference_milligauss, [expected; 3]);
        assert!(result.passed());

        let after = config.map(|r| model.borrow().register(r));
        assert_eq!(before, after);
        assert!(!aw!(mag.read_register::<CfgRegC>()).unwrap().self_test());
    }

    self_test_over_spi {
        let model = RefCell::new(Lis2mdlModel::new());
        let mut mag = Driver::new_spi(FakeSpi::new(&model));
        let result = aw!(mag.self_test(&mut FakeDelay::new(&model))).unwrap();
        assert!(result.passed());
        assert_eq!(model.borrow().mode(), Mode::Idle);
    }

    self_test_times_out_without_data {
        let model = RefCell::new(Lis2mdlModel::new());
        let mut mag = Driver::new(FakeI2c::new(&model));
        assert_eq!(aw!(mag.self_test(&mut NoDelay)), Err(SelfTestError::Timeout));
        assert_eq!(model.borrow().mode(), Mode::Idle);
    }
}
