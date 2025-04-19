#![no_std]
#![no_main]

use defmt::{debug, info, println};
use embassy_executor::Spawner;
use embassy_futures::select::{select, Either};
use embassy_nrf::gpio::{AnyPin, Input};
use embassy_sync::{blocking_mutex::raw::ThreadModeRawMutex, mutex::Mutex};
use embassy_time::{Duration, Timer};
use microbit_bsp::{
    display::{Brightness, Frame},
    LedMatrix, Microbit,
};
use micromath::F32Ext;
use {defmt_rtt as _, panic_probe as _};

type SharedCounter = Mutex<ThreadModeRawMutex, u32>;
static COUNTER: SharedCounter = SharedCounter::new(0);

type Btn = Input<'static, AnyPin>;
#[embassy_executor::task]
async fn btn_log(mut a: Btn, mut b: Btn, counter: &'static SharedCounter) {
    loop {
        match select(a.wait_for_rising_edge(), b.wait_for_rising_edge()).await {
            Either::First(_) => {
                let mut c = counter.lock().await;
                *c += 1;
                // println!("a rising {}", *c);
            }
            Either::Second(_) => println!("b rising"),
        }
    }
}

#[embassy_executor::task]
async fn compute_rpm(counter: &'static SharedCounter) {
    let dt_ms: u64 = 2000;
    let time_factor = 60.0 * 1000.0 / (dt_ms as f32);
    let mut damp_rpm: f32 = 0.0;
    let damp_factor = 0.3;
    loop {
        let timer = Timer::after_millis(dt_ms).await;
        let mut c = (*counter.lock().await) as f32;
        *counter.lock().await = 0;
        let latest_rpm = c * time_factor;

        damp_rpm = latest_rpm * damp_factor + (1.0 - damp_factor) * damp_rpm;
        println!("rpm: {} damp: {}", latest_rpm, damp_rpm);
    }
}

#[embassy_executor::main]
async fn main(spawner: Spawner) {
    defmt::println!("Hello, World!");
    let board = Microbit::default();

    let mut display = board.display;
    display.set_brightness(Brightness::MAX);
    spawner
        .spawn(btn_log(board.btn_a, board.btn_b, &COUNTER))
        .unwrap();
    spawner.spawn(compute_rpm(&COUNTER)).unwrap();
}
