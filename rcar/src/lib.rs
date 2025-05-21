#![no_std]
#![macro_use]

pub mod ble;

pub mod motor;

use embassy_nrf::{config::Config, interrupt::Priority};
use embassy_sync::{blocking_mutex::raw::ThreadModeRawMutex, mutex::Mutex};

// pub type SharedRpm = Mutex<ThreadModeRawMutex, f32>;
pub type SharedSpeed = Mutex<ThreadModeRawMutex, [f32; 2]>;

pub fn config() -> Config {
    let mut config = Config::default();
    config.gpiote_interrupt_priority = Priority::P2;
    config.time_interrupt_priority = Priority::P2;
    config
}
