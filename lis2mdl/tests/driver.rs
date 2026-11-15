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
