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

#[embassy_executor::task]
async fn log_rpm(rpm: &'static SharedRpm) {
    loop {
        Timer::after_millis(500).await;
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
    spawner.spawn(rpm_sense(board.btn_a, &RPM)).unwrap();
    spawner.spawn(log_rpm(&RPM)).unwrap();
}
