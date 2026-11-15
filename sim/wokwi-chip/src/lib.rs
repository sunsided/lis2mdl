//! Wokwi custom chip wrapping the `lis2mdl-sim` behavioral model.
//!
//! Pins: `VCC`, `GND`, `SDA`, `SCL` (I²C at `0x1E`), `INT` (the INT/DRDY output).
//!
//! Controls (also settable as attributes and via `set-control` in scenarios):
//! `fieldX`, `fieldY`, `fieldZ` in milligauss and `temperature` in °C. They are sampled
//! every [`TICK_US`], which is also the time step of the model.

use lis2mdl_sim::Lis2mdlModel;
use std::cell::RefCell;
use std::ffi::{CString, c_void};
use wokwi_chip_ll::*;

/// The model's time step and control sampling period, in microseconds.
const TICK_US: u32 = 1_000;

struct Chip {
    model: Lis2mdlModel,
    int_pin: PinId,
    field: [AttrId; 3],
    temperature: AttrId,
}

thread_local! {
    static CHIP: RefCell<Option<Chip>> = const { RefCell::new(None) };
}

fn with_chip<R>(f: impl FnOnce(&mut Chip) -> R) -> R {
    CHIP.with(|chip| f(chip.borrow_mut().as_mut().expect("chipInit not called")))
}

fn cstr(s: &str) -> CString {
    CString::new(s).expect("no interior NUL")
}

fn debug(message: &str) {
    let message = cstr(message);
    // SAFETY: `message` is a valid NUL-terminated string for the duration of the call.
    unsafe { debugPrint(message.as_ptr()) };
}

/// Converts milligauss to LSB (1.5 mG per LSB), rounding to the nearest LSB.
fn milligauss_to_lsb(mg: f32) -> i16 {
    (mg / 1.5).round() as i16
}

/// Converts °C to the temperature register (8 LSB per °C, `0` at 25 °C).
fn celsius_to_lsb(celsius: f32) -> i16 {
    ((celsius - 25.0) * 8.0).round() as i16
}

impl Chip {
    fn update_pin(&self) {
        let level = if self.model.int_pin() { HIGH } else { LOW };
        // SAFETY: The pin ID was obtained from the simulator in `chipInit`.
        unsafe { pinWrite(self.int_pin, level) };
    }

    fn tick(&mut self) {
        // SAFETY: Attribute IDs were obtained from the simulator in `chipInit`.
        // `attrRead` returns `u32`, which cannot hold negative slider values.
        let ([x, y, z], t) = unsafe {
            (
                self.field.map(|attr| attrReadFloat(attr) as f32),
                attrReadFloat(self.temperature) as f32,
            )
        };
        self.model.set_field(
            milligauss_to_lsb(x),
            milligauss_to_lsb(y),
            milligauss_to_lsb(z),
        );
        self.model.set_temperature(celsius_to_lsb(t));
        self.model.tick(TICK_US);
        self.update_pin();
    }
}

extern "C" fn on_connect(_: *mut c_void, _address: u32, is_read: bool) -> bool {
    with_chip(|chip| chip.model.i2c_start(is_read))
}

extern "C" fn on_read(_: *mut c_void) -> u8 {
    with_chip(|chip| chip.model.i2c_read())
}

extern "C" fn on_write(_: *mut c_void, data: u8) -> bool {
    with_chip(|chip| chip.model.i2c_write(data))
}

extern "C" fn on_disconnect(_: *mut c_void) {
    // Reads and writes can change the pin: data-ready, interrupt source, configuration.
    with_chip(|chip| chip.update_pin());
}

extern "C" fn on_tick(_: *mut c_void) {
    with_chip(Chip::tick);
}

/// Entry point called by the simulator.
#[unsafe(no_mangle)]
pub extern "C" fn chipInit() {
    // SAFETY: All pointers passed to the simulator are valid for the duration of each
    // call; configs are copied by the simulator. Init functions are only called here.
    unsafe {
        let int_pin = pinInit(cstr("INT").as_ptr(), OUTPUT_LOW);
        let sda = pinInit(cstr("SDA").as_ptr(), INPUT);
        let scl = pinInit(cstr("SCL").as_ptr(), INPUT);

        let model = Lis2mdlModel::new();
        i2cInit(&I2CConfig {
            user_data: std::ptr::null(),
            address: u32::from(model.address()),
            scl,
            sda,
            connect: on_connect,
            read: on_read,
            write: on_write,
            disconnect: on_disconnect,
        });

        let tick_timer = timerInit(&TimerConfig {
            user_data: std::ptr::null(),
            callback: on_tick,
        });

        let chip = Chip {
            model,
            int_pin,
            field: ["fieldX", "fieldY", "fieldZ"].map(|id| attrInit(cstr(id).as_ptr(), 0.0)),
            temperature: attrInit(cstr("temperature").as_ptr(), 25.0),
        };
        CHIP.with(|c| *c.borrow_mut() = Some(chip));

        timerStart(tick_timer, TICK_US, true);
    }
    debug("lis2mdl: chip initialized");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn conversions() {
        assert_eq!(milligauss_to_lsb(315.0), 210);
        assert_eq!(milligauss_to_lsb(-285.0), -190);
        assert_eq!(celsius_to_lsb(35.0), 80);
        assert_eq!(celsius_to_lsb(25.0), 0);
    }
}
