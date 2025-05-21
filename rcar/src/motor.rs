#![no_std]
#![no_main]

use core::{any::Any, time};

use crate::SharedSpeed;
use defmt::{debug, error, info, println, warn, Debug2Format};
use embassy_executor::Spawner;
use embassy_futures::select::{select, Either};
use embassy_nrf::{
    bind_interrupts,
    gpio::{AnyPin, Input, Pin},
    interrupt::{self, typelevel, InterruptExt},
    peripherals::{self, P0_26, P1_00, SAADC, TWISPI0, TWISPI1},
    saadc,
    twim::{self, Twim},
};

use embassy_sync::{blocking_mutex::raw::ThreadModeRawMutex, mutex::Mutex};
use embassy_time::{Duration, Instant, Timer};
// use micromath::F32Ext;
use {defmt_rtt as _, panic_probe as _};

// use nrf_softdevice::ble::{gatt_server, peripheral, Connection};
// use nrf_softdevice::{raw, Softdevice};

use static_cell::ConstStaticCell;
use static_cell::StaticCell;
// type SharedCounter = Mutex<ThreadModeRawMutex, u32>;
// static COUNTER: SharedCounter = SharedCounter::new(0);

bind_interrupts!(struct Irqs {
    SPIM1_SPIS1_TWIM1_TWIS1_SPI1_TWI1 => twim::InterruptHandler<peripherals::TWISPI1>;
});

#[embassy_executor::task]
pub async fn drive_servos(
    target_speed: &'static SharedSpeed,
    twi1: TWISPI1,
    scl: P0_26,
    sda: P1_00,
) {
    info!("Initializing TWI...");

    let mut i2c_config = twim::Config::default();
    i2c_config.frequency = twim::Frequency::K100;
    // i2c_config.sda_pullup = true;
    // i2c_config.scl_pullup = true;

    // interrupt::TWISPI1.set_priority(interrupt::Priority::P7);
    let mut twim = Twim::new(twi1, Irqs, sda, scl, i2c_config);
    let wukong_address = 0x10;

    // let mut speed = 0_u8;
    let speed = 40;
    info!("entering speed ctrl loop");
    loop {
        Timer::after_millis(300).await;

        // info!("setting speed to: {}", speed);
        let motors = [4, 5, 6, 7];
        for m in motors {
            let buf = [m, speed, 0, 0];
            // let res = twim.write(wukong_address, &buf).await;
        }
    }
    return;
}
