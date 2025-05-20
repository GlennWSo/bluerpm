#![no_std]
#![macro_use]

//! suggested reading: https://docs.silabs.com/bluetooth/4.0/general/adv-and-scanning/bluetooth-adv-data-basics

use core::f32;
use core::ops::Deref;

use embassy_executor::Spawner;
use embassy_sync::{blocking_mutex::raw::ThreadModeRawMutex, mutex::Mutex};
use embassy_time::Timer;
use heapless::Vec;
use microbit_bsp::*;
// use nrf_softdevice::ble::gatt_server::{notify_value, Server};
use array_concat::split_array;
use defmt::{debug, error, info, println, warn};
use nrf_softdevice::ble::advertisement_builder::{
    AdvertisementDataType, Flag, LegacyAdvertisementBuilder, LegacyAdvertisementPayload,
};
use nrf_softdevice::ble::gatt_server::Service;
use nrf_softdevice::ble::{gatt_server, get_address, peripheral, set_address, Address, Connection};
use nrf_softdevice::{raw, Softdevice};
use static_cell::StaticCell;
use {defmt_rtt as _, panic_probe as _};

pub type SharedRpm = Mutex<ThreadModeRawMutex, f32>;

#[nrf_softdevice::gatt_server]
pub struct Server {
    pub rcar: RcCarService,
}

#[nrf_softdevice::gatt_service(uuid = "8a8ec266-3ede-4a2f-a87b-aafbc55b8a30")]
pub struct RcCarService {
    ///speed forward m/s
    #[characteristic(uuid = "2C09", write, read)]
    target_velocity: [u8; 8],
}

impl RcCarService {}

#[embassy_executor::task]
pub async fn softdevice_task(sd: &'static Softdevice) {
    sd.run().await;
}

static CONN: Mutex<ThreadModeRawMutex, Option<Connection>> = Mutex::new(None);

pub type SharedSpeed = Mutex<ThreadModeRawMutex, [f32; 2]>;

#[embassy_executor::task(pool_size = "1")]
pub async fn gatt_server_task(server: &'static Server, target_speed: &'static SharedSpeed) {
    {
        let conn = {
            let lock = CONN.lock().await;
            lock.as_ref().unwrap().clone() // clone is used here so we can drop the lock
        };

        gatt_server::run(&conn, server, |e| match e {
            ServerEvent::Rcar(e) => match e {
                RcCarServiceEvent::TargetVelocityWrite(v_bytes) => {
                    let Ok(mut targe_speed) = target_speed.try_lock() else {
                        warn!("unable to set speed, lock buzy");
                        return;
                    };
                    let (x_bytes, y_bytes) = split_array!(v_bytes, 4, 4);
                    let x = f32::from_le_bytes(x_bytes);
                    let y = f32::from_le_bytes(y_bytes);
                    info!("set speed request x:{} y:{}", x, y);
                    *targe_speed = [x, y];
                }
            },
        })
        .await;
        info!("connection closed");
    }
    let mut lock = CONN.lock().await;
    lock.take();
}

/// Application must run at a lower priority than softdevice
pub fn config() -> Config {
    let mut config = Config::default();
    config.gpiote_interrupt_priority = Priority::P2;
    config.time_interrupt_priority = Priority::P2;
    config
}
pub fn enable_softdevice(name: &'static str) -> &'static mut Softdevice {
    let config = nrf_softdevice::Config {
        clock: Some(raw::nrf_clock_lf_cfg_t {
            source: raw::NRF_CLOCK_LF_SRC_RC as u8,
            rc_ctiv: 4,
            rc_temp_ctiv: 2,
            accuracy: 7,
        }),
        conn_gap: Some(raw::ble_gap_conn_cfg_t {
            conn_count: 2,
            event_length: 24,
        }),
        conn_gatt: Some(raw::ble_gatt_conn_cfg_t { att_mtu: 128 }),
        gatts_attr_tab_size: Some(raw::ble_gatts_cfg_attr_tab_size_t {
            attr_tab_size: 32768,
        }),
        gap_role_count: Some(raw::ble_gap_cfg_role_count_t {
            adv_set_count: 1,
            periph_role_count: 3,
        }),
        gap_device_name: Some(raw::ble_gap_cfg_device_name_t {
            p_value: name.as_ptr() as *const u8 as _,
            current_len: name.len() as u16,
            max_len: name.len() as u16,
            write_perm: unsafe { core::mem::zeroed() },
            _bitfield_1: raw::ble_gap_cfg_device_name_t::new_bitfield_1(
                raw::BLE_GATTS_VLOC_STACK as u8,
            ),
        }),
        ..Default::default()
    };
    let sd = Softdevice::enable(&config);
    set_address(
        sd,
        &Address::new(
            nrf_softdevice::ble::AddressType::RandomStatic,
            [0x13, 0x33, 0x33, 0x33, 0x37, 0b1100_1010],
        ),
    );
    println!("address: {:?}", get_address(&sd));
    sd
}

#[embassy_executor::task]
pub async fn advertiser_task(
    spawner: Spawner,
    sd: &'static Softdevice,
    server: &'static Server,
    name: &'static str,
    target_speed: &'static SharedSpeed,
) {
    // spec for assigned numbers: https://www.bluetooth.com/wp-content/uploads/Files/Specification/HTML/Assigned_Numbers/out/en/Assigned_Numbers.pdf?v=1715770644767

    static ADV_DATA: LegacyAdvertisementPayload = LegacyAdvertisementBuilder::new()
        .flags(&[Flag::LE_Only, Flag::GeneralDiscovery])
        .full_name("rcar")
        // .raw(
        //     AdvertisementDataType::RANDOM_TARGET_ADDRESS,
        //     &[0xf1, 0x15, 0xba, 0x1e, 0x5e, 0b0000_0011],
        // )
        // .raw(
        //     AdvertisementDataType::PUBLIC_TARGET_ADDRESS,
        //     &[0xf1, 0x15, 0xba, 0x1e, 0x5e, 0x22],
        // )
        .build();

    static SCAN_DATA: LegacyAdvertisementPayload = LegacyAdvertisementBuilder::new()
        .services_128(
            nrf_softdevice::ble::advertisement_builder::ServiceList::Complete,
            &[0x8a8ec266_3ede_4a2f_a87b_aafbc55b8a30_u128.to_le_bytes()],
        )
        .build();

    loop {
        let config = peripheral::Config::default();
        let adv = peripheral::ConnectableAdvertisement::ScannableUndirected {
            adv_data: &ADV_DATA,
            scan_data: &SCAN_DATA,
        };
        info!("advertising");
        let conn = peripheral::advertise_connectable(sd, adv, &config)
            .await
            .unwrap();

        defmt::info!("connection established");
        let mut lock = CONN.lock().await;
        lock.replace(conn);

        if let Err(e) = spawner.spawn(gatt_server_task(server, target_speed)) {
            defmt::warn!("Error spawning gatt task: {:?}", e);
        }
    }
}
#[embassy_executor::task]
pub async fn log_rpm(server: &'static Server, rpm: &'static SharedRpm) {
    // server.rcar.target_velocity_x_set(&0.0);
    // server.rcar.target_velocity_set(&);
    loop {
        Timer::after_millis(300).await;

        if let Some(conn) = CONN.lock().await.as_ref() {
            // match server.rcar.target_velocity_y_get() {
            //     Ok(y) => (),
            //     Err(e) => error!("didnt get y: {}", e),
            // };
        };
    }
}
