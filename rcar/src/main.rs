#![no_std]
#![no_main]

use core::{any::Any, time};

use defmt::{debug, error, info, println, warn, Debug2Format};
use embassy_executor::Spawner;
use embassy_futures::select::{select, Either};
use embassy_nrf::{
    bind_interrupts,
    gpio::{AnyPin, Input, Pin},
    peripherals::{SAADC, TWISPI0},
    saadc,
    twim::{self, Twim},
};
use embassy_sync::{blocking_mutex::raw::ThreadModeRawMutex, mutex::Mutex};
use embassy_time::{Duration, Instant, Timer};
// use micromath::F32Ext;
use ringbuffer::{ConstGenericRingBuffer, RingBuffer};
use {defmt_rtt as _, panic_probe as _};

// use nrf_softdevice::ble::{gatt_server, peripheral, Connection};
// use nrf_softdevice::{raw, Softdevice};

use static_cell::ConstStaticCell;
use static_cell::StaticCell;
// type SharedCounter = Mutex<ThreadModeRawMutex, u32>;
// static COUNTER: SharedCounter = SharedCounter::new(0);

static TARGET_SPEED: rcar::SharedSpeed = Mutex::new([0.0, 0.0]);

#[embassy_executor::task]
async fn drive_servos() {}
bind_interrupts!(struct Irqs {
    TWISPI0 => twim::InterruptHandler<TWISPI0>;
});

// static SERVER: StaticCell<Server> = StaticCell::new();

#[embassy_executor::main]
async fn main(s: Spawner) {
    defmt::println!("Hello, World!");
    let p = embassy_nrf::init(rcar::config());

    info!("Initializing TWI...");
    static RAM_BUFFER: ConstStaticCell<[u8; 16]> = ConstStaticCell::new([0; 16]);
    let scl = p.P0_26;
    let sda = p.P1_00;
    let mut i2c_config = twim::Config::default();
    i2c_config.frequency = twim::Frequency::K100;
    i2c_config.sda_pullup = true;
    i2c_config.scl_pullup = true;

    let mut twi = Twim::new(p.TWISPI0, Irqs, sda, scl, i2c_config);
    let wukong_address = 0x10;

    let mut speed = 0_u8;
    loop {
        Timer::after_millis(100).await;
        info!("setting speed: {}", speed);
        let motors = [4, 5, 6, 7];
        for m in motors {
            let buf = [m, speed, 0, 0];
            let res = twi.write(wukong_address, &buf).await;
        }

        speed += 10;
        if speed > 180 {
            speed = 0;
        }
    }

    // BLE
    // Spawn the underlying softdevice task
    // let sd = rcar::enable_softdevice("Embassy rcar");

    // let mut server = Server::new(sd).unwrap();
    // let server = SERVER.init(server);

    // s.spawn(softdevice_task(sd)).unwrap();
    // Starts the bluetooth advertisement and GATT server
    // s.spawn(rcar::advertiser_task(
    //     s,
    //     sd,
    //     server,
    //     "Embassy rcar",
    //     &TARGET_SPEED,
    // ))
    // .unwrap();
}
