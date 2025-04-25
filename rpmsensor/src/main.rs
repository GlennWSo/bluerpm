#![no_std]
#![no_main]

use core::time;

use defmt::{debug, info, println, warn};
use embassy_executor::Spawner;
use embassy_futures::select::{select, Either};
use embassy_nrf::gpio::{AnyPin, Input};
use embassy_sync::{blocking_mutex::raw::ThreadModeRawMutex, mutex::Mutex};
use embassy_time::{Duration, Instant, Timer};
use microbit_bsp::{
    display::{Brightness, Frame},
    Config, LedMatrix, Microbit, Priority,
};
// use micromath::F32Ext;
use ringbuffer::{ConstGenericRingBuffer, RingBuffer};
use rpmsensor::{log_rpm, softdevice_task, Server, SharedRpm};
use {defmt_rtt as _, panic_probe as _};

use nrf_softdevice::ble::{gatt_server, peripheral, Connection};
use nrf_softdevice::{raw, Softdevice};

use static_cell::StaticCell;
// type SharedCounter = Mutex<ThreadModeRawMutex, u32>;
// static COUNTER: SharedCounter = SharedCounter::new(0);

static RPM: SharedRpm = Mutex::new(0.0);

type Btn = Input<'static, AnyPin>;
#[embassy_executor::task]
async fn rpm_sense(mut a: Btn, shared_rpm: &'static SharedRpm) {
    let max_dt: f32 = 5.0;
    let target_dt = 1.0;
    let min_rpm = 60.0 / max_dt;
    const BUFF_SIZE: usize = 10;
    let mut running_dt = ConstGenericRingBuffer::<f32, BUFF_SIZE>::new();
    running_dt.fill(max_dt);

    let mut t0 = Instant::now();
    loop {
        let event = select(
            a.wait_for_rising_edge(),
            Timer::after_secs(target_dt as u64),
        )
        .await;
        match event {
            Either::First(_) => {
                let elapsed = Instant::now() - t0;
                let dt = (elapsed.as_micros() as f32) / 1_000_000.0;
                running_dt.push(dt);
                t0 = Instant::now();
            }
            Either::Second(_) => {
                let dt = running_dt.back_mut().unwrap();
                *dt += target_dt;
            }
        };
        let mut t = 0_f32;
        let mut c = 0_u8;
        for dt in running_dt.iter().rev() {
            t += dt;
            c += 1;
            if t > target_dt {
                break;
            }
        }
        let rpm = c as f32 * 60.0 / t;
        *shared_rpm.lock().await = if rpm <= min_rpm { 0.0 } else { rpm };
    }
}

static SERVER: StaticCell<Server> = StaticCell::new();
#[embassy_executor::main]
async fn main(s: Spawner) {
    defmt::println!("Hello, World!");
    let board = Microbit::new(rpmsensor::config());
    // Spawn the underlying softdevice task
    let sd = rpmsensor::enable_softdevice("Embassy Microbit");

    let server = SERVER.init(Server::new(sd).unwrap());

    server.bas.set(13).unwrap();
    s.spawn(softdevice_task(sd)).unwrap();
    // Starts the bluetooth advertisement and GATT server
    s.spawn(rpmsensor::advertiser_task(
        s,
        sd,
        server,
        "Embassy Microbit",
    ))
    .unwrap();

    let mut display = board.display;
    display.set_brightness(Brightness::MAX);
    s.spawn(rpm_sense(board.btn_a, &RPM)).unwrap();
    s.spawn(log_rpm(server, &RPM)).unwrap();
}
