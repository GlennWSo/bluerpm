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
    LedMatrix, Microbit,
};
use micromath::F32Ext;
use ringbuffer::{ConstGenericRingBuffer, RingBuffer};
use {defmt_rtt as _, panic_probe as _};

// type SharedCounter = Mutex<ThreadModeRawMutex, u32>;
// static COUNTER: SharedCounter = SharedCounter::new(0);

type SharedRpm = Mutex<ThreadModeRawMutex, f32>;
static RPM: SharedRpm = Mutex::new(0.0);

type Btn = Input<'static, AnyPin>;
#[embassy_executor::task]
async fn btn_log(mut a: Btn, mut b: Btn, rpm: &'static SharedRpm) {
    let mut running_rpm = ConstGenericRingBuffer::<f32, 3>::default();
    running_rpm.fill_default();
    let mut avg_rpm = 0.0;
    loop {
        let t0 = Instant::now();
        let time_out = 1;
        match select(a.wait_for_rising_edge(), Timer::after_secs(time_out)).await {
            Either::First(_) => {
                let elapsed = Instant::now() - t0;
                let dt = (elapsed.as_micros() as f32) / 1_000_000.0;
                if dt == 0.0 {
                    println!("dt == 0.0"); // TODO warn!?
                    continue;
                }

                let latest_rpm = 60.0 / dt;
                running_rpm.push(latest_rpm);
                avg_rpm = running_rpm.iter().sum::<f32>() / running_rpm.len() as f32;
            }
            Either::Second(_) => {
                running_rpm.push(0.0);
                avg_rpm = running_rpm.iter().sum::<f32>() / running_rpm.len() as f32;
                if avg_rpm == 0.0 {
                    println!("rpm period timeout of: {} secs reached", 5);
                }
            }
        };
        *rpm.lock().await = avg_rpm;
    }
}

#[embassy_executor::task]
async fn compute_rpm(rpm: &'static SharedRpm) {
    loop {
        Timer::after_millis(200).await;
        let dt = *rpm.lock().await;
        println!("rpm {}  ", dt);
    }
}

#[embassy_executor::main]
async fn main(spawner: Spawner) {
    defmt::println!("Hello, World!");
    let board = Microbit::default();

    let mut display = board.display;
    display.set_brightness(Brightness::MAX);
    spawner
        .spawn(btn_log(board.btn_a, board.btn_b, &RPM))
        .unwrap();
    spawner.spawn(compute_rpm(&RPM)).unwrap();
}
