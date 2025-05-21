#![no_std]
#![no_main]

use defmt::{info, println};
use embassy_executor::Spawner;
use embassy_nrf::gpio::{Level, Output, OutputDrive};
use embassy_time::Timer;
use {defmt_rtt as _, panic_probe as _};

use nrf_softdevice::ble::advertisement_builder::{
    AdvertisementDataType, Flag, LegacyAdvertisementBuilder, LegacyAdvertisementPayload,
};
use nrf_softdevice::ble::gatt_server::Service;
use nrf_softdevice::ble::{gatt_server, get_address, peripheral, set_address, Address, Connection};
use nrf_softdevice::{raw, Softdevice};

#[embassy_executor::main]
async fn main(s: Spawner) {
    println!("Hello, World!");
    let p = embassy_nrf::init(rcar::config());
    // BLE
    // Spawn the underlying softdevice task
    for _ in 0..10 {
        info!("wait");
        Timer::after_millis(100).await;
        info!("ready");
    }
}
