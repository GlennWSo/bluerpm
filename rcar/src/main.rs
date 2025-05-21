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

use rcar::SharedSpeed;

pub static TARGET_SPEED: SharedSpeed = SharedSpeed::new([0.0, 0.0]);

#[embassy_executor::main]
async fn main(s: Spawner) {
    println!("Hello, World!");
    let p = embassy_nrf::init(rcar::config());

    s.spawn(rcar::motor::drive_servos(
        &TARGET_SPEED,
        p.TWISPI1,
        p.P0_26,
        p.P1_00,
    ))
    .unwrap();

    s.spawn(rcar::ble::read_ble(s, "rcar", &TARGET_SPEED))
        .unwrap();
}
