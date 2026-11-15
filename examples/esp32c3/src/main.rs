//! Reads a LIS2MDL magnetometer and prints field, heading, temperature and threshold
//! interrupts over the serial console.
//!
//! Wiring: SDA = GPIO4, SCL = GPIO5, INT/DRDY = GPIO7.

#![no_std]
#![no_main]

use esp_backtrace as _;
use esp_hal::delay::Delay;
use esp_hal::gpio::{Input, InputConfig, Pull};
use esp_hal::i2c::master::{Config as I2cConfig, I2c};
use esp_hal::main;
use esp_hal::time::Rate;
use esp_println::println;
use lis2mdl::registers::{CfgRegC, IntCtrlReg};
use lis2mdl::{Lis2mdl, MagneticField, OutputDataRate, Temperature};

esp_bootloader_esp_idf::esp_app_desc!();

/// The hard-iron offset written to the device: 15 mG on X.
const HARD_IRON_OFFSET_MG: (f32, f32, f32) = (15.0, 0.0, 0.0);

/// The threshold interrupt fires when any axis exceeds ±1 gauss.
const THRESHOLD_MG: f32 = 1000.0;

/// The heading in degrees (0–360), with X pointing north and the sensor held flat.
fn heading(field: &MagneticField) -> f32 {
    let [x, y, _] = field.milligauss();
    let degrees = libm::atan2f(y, x).to_degrees();
    if degrees < 0.0 {
        degrees + 360.0
    } else {
        degrees
    }
}

#[main]
fn main() -> ! {
    let peripherals = esp_hal::init(esp_hal::Config::default());
    let mut delay = Delay::new();

    let i2c = I2c::new(
        peripherals.I2C0,
        I2cConfig::default().with_frequency(Rate::from_khz(400)),
    )
    .expect("valid I2C config")
    .with_sda(peripherals.GPIO4)
    .with_scl(peripherals.GPIO5);
    let int = Input::new(
        peripherals.GPIO7,
        InputConfig::default().with_pull(Pull::Down),
    );

    // The device needs its turn-on time after power-up.
    delay.delay_millis(lis2mdl::TURN_ON_TIME_MS);
    let mut mag = Lis2mdl::new(i2c);
    let who_am_i = mag.who_am_i().expect("device responds");
    if who_am_i != lis2mdl::LIS2MDL_WHO_AM_I {
        panic!("unexpected WHO_AM_I 0x{who_am_i:02X}");
    }

    println!("LIS2MDL ok who_am_i=0x{who_am_i:02X}");

    let st = mag.self_test(&mut delay).expect("self-test runs");
    let [dx, dy, dz] = st.difference_milligauss;
    println!(
        "self-test passed={} dx={dx:.1} dy={dy:.1} dz={dz:.1}",
        st.passed()
    );

    let (ox, oy, oz) = HARD_IRON_OFFSET_MG;
    mag.set_hard_iron_offset(MagneticField::from_milligauss(ox, oy, oz))
        .unwrap();
    mag.set_interrupt_threshold((THRESHOLD_MG / lis2mdl::registers::MILLIGAUSS_PER_LSB) as u16)
        .unwrap();
    // Pulsed, active high: INT follows the current sample.
    mag.write_register(IntCtrlReg::new().with_ien(true).with_iea(true))
        .unwrap();
    mag.modify_register(|r: CfgRegC| r.with_int_on_pin(true).with_bdu(true))
        .unwrap();
    mag.start_continuous(OutputDataRate::Hz50).unwrap();
    println!("running odr=50Hz threshold={THRESHOLD_MG:.0}mG");

    let mut last: Option<(MagneticField, Temperature)> = None;
    let mut int_active = false;
    loop {
        if mag.data_ready().unwrap() {
            let field = mag.read_magnetic_field().unwrap();
            let temperature = mag.read_temperature().unwrap();
            if last != Some((field, temperature)) {
                let [x, y, z] = field.milligauss();
                println!(
                    "mag x={x:.1} y={y:.1} z={z:.1} mG heading={:.0} temp={:.1}C",
                    heading(&field),
                    temperature.celsius()
                );
                last = Some((field, temperature));
            }
        }

        let level = int.is_high();
        if level && !int_active {
            let src = mag.interrupt_source().unwrap();
            println!("int src=0x{:02X}", u8::from(src));
        } else if !level && int_active {
            println!("int clear");
        }

        int_active = level;

        delay.delay_millis(5);
    }
}
