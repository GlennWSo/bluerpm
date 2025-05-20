#![no_std]
#![no_main]

use core::{any::Any, time};

use defmt::{debug, info, println, warn};
use embassy_executor::Spawner;
use embassy_futures::select::{select, Either};
use embassy_nrf::gpio::{AnyPin, Input, Pin};
use embassy_sync::{blocking_mutex::raw::ThreadModeRawMutex, mutex::Mutex};
use embassy_time::{Duration, Instant, Timer};
use microbit_bsp::{
    display::{Brightness, Frame},
    Config, LedMatrix, Microbit, Priority,
};
// use micromath::F32Ext;
use rcar::{log_rpm, softdevice_task, Server, SharedRpm};
use ringbuffer::{ConstGenericRingBuffer, RingBuffer};
use {defmt_rtt as _, panic_probe as _};

use nrf_softdevice::ble::{gatt_server, peripheral, Connection};
use nrf_softdevice::{raw, Softdevice};

use static_cell::StaticCell;
// type SharedCounter = Mutex<ThreadModeRawMutex, u32>;
// static COUNTER: SharedCounter = SharedCounter::new(0);

static TARGET_SPEED: rcar::SharedSpeed = Mutex::new([0.0, 0.0]);

static SERVER: StaticCell<Server> = StaticCell::new();
#[embassy_executor::main]
async fn main(s: Spawner) {
    defmt::println!("Hello, World!");
    let board = Microbit::new(rcar::config());
    // Spawn the underlying softdevice task
    let sd = rcar::enable_softdevice("Embassy rcar");

    let mut server = Server::new(sd).unwrap();
    let server = SERVER.init(server);

    s.spawn(softdevice_task(sd)).unwrap();
    // Starts the bluetooth advertisement and GATT server
    s.spawn(rcar::advertiser_task(
        s,
        sd,
        server,
        "Embassy rcar",
        &TARGET_SPEED,
    ))
    .unwrap();
}
