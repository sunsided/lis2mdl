//! Driver tests against the behavioral model.

use core::cell::RefCell;
use lis2mdl::registers::{CfgRegA, CfgRegB, RegisterAddress};
use lis2mdl::{Lis2mdl, MagneticField, Mode, OutputDataRate};
use lis2mdl_sim::{FakeI2c, Lis2mdlModel};

#[test]
fn continuous_mode_keeps_power_mode() {
    let model = RefCell::new(Lis2mdlModel::new());
    let mut mag = Lis2mdl::new(FakeI2c::new(&model));
    mag.modify_register(|r: CfgRegA| r.with_lp(true)).unwrap();
    mag.start_continuous(OutputDataRate::Hz100).unwrap();

    let cfg = CfgRegA::from(model.borrow().register(RegisterAddress::CFG_REG_A));
    assert!(cfg.lp() && cfg.comp_temp_en());
    assert_eq!(cfg.odr(), OutputDataRate::Hz100);
    assert_eq!(cfg.md(), Mode::Continuous);

    mag.stop().unwrap();
    assert_eq!(model.borrow().mode(), Mode::Idle);
}

#[test]
fn single_measurement() {
    let model = RefCell::new(Lis2mdlModel::new());
    model.borrow_mut().set_field(-1, 2, -3);
    model.borrow_mut().set_temperature(8);
    let mut mag = Lis2mdl::new(FakeI2c::new(&model));

    assert!(!mag.data_ready().unwrap());
    mag.trigger_single().unwrap();
    assert!(mag.data_ready().unwrap());
    assert_eq!(
        mag.read_magnetic_field().unwrap(),
        MagneticField::new(-1, 2, -3)
    );
    assert!(!mag.data_ready().unwrap());
    assert_eq!(mag.read_temperature().unwrap().celsius(), 26.0);
}

#[test]
fn hard_iron_offset_roundtrip() {
    let model = RefCell::new(Lis2mdlModel::new());
    let mut mag = Lis2mdl::new(FakeI2c::new(&model));
    let offset = MagneticField::new(-1234, 5, 32767);
    mag.set_hard_iron_offset(offset).unwrap();
    assert_eq!(mag.hard_iron_offset().unwrap(), offset);
}

#[test]
fn modify_register_preserves_other_bits() {
    let model = RefCell::new(Lis2mdlModel::new());
    let mut mag = Lis2mdl::new(FakeI2c::new(&model));
    mag.modify_register(|r: CfgRegB| r.with_off_canc(true))
        .unwrap();
    mag.modify_register(|r: CfgRegB| r.with_lpf(true)).unwrap();
    assert_eq!(mag.read_register::<CfgRegB>().unwrap(), CfgRegB::from(0b11));
}

#[test]
fn wrong_address_fails() {
    let model = RefCell::new(Lis2mdlModel::new());
    let mut mag = Lis2mdl::with_address(FakeI2c::new(&model), 0x1F);
    assert!(mag.identify().is_err());
    let _bus = mag.release();
}

#[cfg(feature = "async")]
#[test]
fn async_driver() {
    use embassy_futures::block_on;
    let model = RefCell::new(Lis2mdlModel::new());
    model.borrow_mut().set_field(10, 20, 30);
    let mut mag = lis2mdl::asynch::Lis2mdl::new(FakeI2c::new(&model));
    block_on(async {
        assert!(mag.identify().await.unwrap());
        mag.set_hard_iron_offset(MagneticField::new(1, 2, 3))
            .await
            .unwrap();
        mag.start_continuous(OutputDataRate::Hz10).await.unwrap();
        model.borrow_mut().tick(100_000);
        assert!(mag.data_ready().await.unwrap());
        assert_eq!(
            mag.read_magnetic_field().await.unwrap(),
            MagneticField::new(9, 18, 27)
        );
    });
}

#[test]
fn spi_interface() {
    use lis2mdl_sim::FakeSpi;
    let model = RefCell::new(Lis2mdlModel::new());
    model.borrow_mut().set_field(7, -8, 9);
    let mut mag = Lis2mdl::new_spi(FakeSpi::new(&model));
    assert!(mag.identify().unwrap());
    mag.set_hard_iron_offset(MagneticField::new(1, 1, 1))
        .unwrap();
    assert_eq!(mag.hard_iron_offset().unwrap(), MagneticField::new(1, 1, 1));
    mag.trigger_single().unwrap();
    assert_eq!(
        mag.read_magnetic_field().unwrap(),
        MagneticField::new(6, -9, 8)
    );
    let _spi = mag.release();
}

#[test]
fn byte_order_is_tracked() {
    use lis2mdl::registers::CfgRegC;
    let model = RefCell::new(Lis2mdlModel::new());
    model.borrow_mut().set_field(0x0102, -2, 3);
    model.borrow_mut().set_temperature(-16);
    let mut mag = Lis2mdl::new(FakeI2c::new(&model));
    mag.modify_register(|r: CfgRegC| r.with_ble(true)).unwrap();
    mag.trigger_single().unwrap();
    assert_eq!(
        mag.read_magnetic_field().unwrap(),
        MagneticField::new(0x0102, -2, 3)
    );
    assert_eq!(mag.read_temperature().unwrap().celsius(), 23.0);

    // A new driver instance (e.g. after an MCU reset) reads BLE from the device.
    let mut mag = Lis2mdl::new(mag.release());
    mag.trigger_single().unwrap();
    assert_eq!(
        mag.read_magnetic_field().unwrap(),
        MagneticField::new(0x0102, -2, 3)
    );

    // Soft reset clears BLE.
    mag.soft_reset().unwrap();
    mag.trigger_single().unwrap();
    assert_eq!(
        mag.read_magnetic_field().unwrap(),
        MagneticField::new(0x0102, -2, 3)
    );
}

#[test]
fn soft_reset_clears_offset() {
    let model = RefCell::new(Lis2mdlModel::new());
    let mut mag = Lis2mdl::new(FakeI2c::new(&model));
    mag.set_hard_iron_offset(MagneticField::new(5, 5, 5))
        .unwrap();
    mag.start_continuous(OutputDataRate::Hz20).unwrap();
    mag.soft_reset().unwrap();
    assert_eq!(mag.hard_iron_offset().unwrap(), MagneticField::default());
    assert_eq!(model.borrow().mode(), Mode::Idle);
}

#[test]
fn configuration_helpers() {
    use lis2mdl::registers::CfgRegC;
    let model = RefCell::new(Lis2mdlModel::new());
    let mut mag = Lis2mdl::new(FakeI2c::new(&model));
    mag.set_low_power(true).unwrap();
    mag.set_low_pass_filter(true).unwrap();
    mag.set_offset_cancellation(true).unwrap();
    mag.set_block_data_update(true).unwrap();
    mag.reboot().unwrap();
    assert!(mag.read_register::<CfgRegA>().unwrap().lp());
    assert_eq!(u8::from(mag.read_register::<CfgRegB>().unwrap()), 0b1_0011);
    assert!(mag.read_register::<CfgRegC>().unwrap().bdu());
    mag.set_offset_cancellation(false).unwrap();
    assert_eq!(u8::from(mag.read_register::<CfgRegB>().unwrap()), 0b0_0001);
}

#[test]
fn threshold_interrupt() {
    use lis2mdl::registers::{CfgRegC, IntCtrlReg};
    let model = RefCell::new(Lis2mdlModel::new());
    let mut mag = Lis2mdl::new(FakeI2c::new(&model));
    mag.set_interrupt_threshold(40_000).unwrap();
    assert_eq!(mag.interrupt_threshold().unwrap(), 40_000);
    mag.set_interrupt_threshold(100).unwrap();
    mag.write_register(
        IntCtrlReg::new()
            .with_ien(true)
            .with_iea(true)
            .with_iel(true),
    )
    .unwrap();
    mag.modify_register(|r: CfgRegC| r.with_int_on_pin(true))
        .unwrap();

    model.borrow_mut().set_field(0, -150, 0);
    mag.trigger_single().unwrap();
    assert!(model.borrow().int_pin());
    let src = mag.interrupt_source().unwrap();
    assert!(src.int() && src.n_th_s_y() && !src.p_th_s_y() && !src.n_th_s_x());
    assert!(!model.borrow().int_pin());
}

#[test]
fn self_test_passes_and_restores_configuration() {
    use lis2mdl::registers::CfgRegC;
    use lis2mdl_sim::{FakeDelay, MODEL_SELF_TEST_LSB};
    let model = RefCell::new(Lis2mdlModel::new());
    model.borrow_mut().set_field(300, -200, 100);
    let mut mag = Lis2mdl::new(FakeI2c::new(&model));
    mag.set_low_power(true).unwrap();
    let config = [
        RegisterAddress::CFG_REG_A,
        RegisterAddress::CFG_REG_B,
        RegisterAddress::CFG_REG_C,
    ];
    let before = config.map(|r| model.borrow().register(r));

    let result = mag.self_test(&mut FakeDelay::new(&model)).unwrap();
    let expected = f32::from(MODEL_SELF_TEST_LSB) * 1.5;
    assert_eq!(result.difference_milligauss, [expected; 3]);
    assert!(result.passed());

    let after = config.map(|r| model.borrow().register(r));
    assert_eq!(before, after);
    assert!(!mag.read_register::<CfgRegC>().unwrap().self_test());
}

#[test]
fn self_test_times_out_without_data() {
    use lis2mdl::SelfTestError;
    struct NoDelay;
    impl embedded_hal::delay::DelayNs for NoDelay {
        fn delay_ns(&mut self, _: u32) {}
    }
    let model = RefCell::new(Lis2mdlModel::new());
    let mut mag = Lis2mdl::new(FakeI2c::new(&model));
    assert_eq!(mag.self_test(&mut NoDelay), Err(SelfTestError::Timeout));
    assert_eq!(model.borrow().mode(), Mode::Idle);
}

#[cfg(feature = "async")]
#[test]
fn async_spi_and_self_test() {
    use embassy_futures::block_on;
    use lis2mdl_sim::{FakeDelay, FakeSpi};
    let model = RefCell::new(Lis2mdlModel::new());
    let mut mag = lis2mdl::asynch::Lis2mdl::new_spi(FakeSpi::new(&model));
    block_on(async {
        assert!(mag.identify().await.unwrap());
        let result = mag.self_test(&mut FakeDelay::new(&model)).await.unwrap();
        assert!(result.passed());
        mag.set_interrupt_threshold(5).await.unwrap();
        assert_eq!(mag.interrupt_threshold().await.unwrap(), 5);
        assert_eq!(model.borrow().mode(), Mode::Idle);
    });
}
