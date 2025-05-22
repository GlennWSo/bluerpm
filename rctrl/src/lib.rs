#![no_std]
#![no_main]

use embassy_executor::{SpawnError, Spawner};
use embassy_nrf::config::Config;
use embassy_nrf::interrupt::Priority;
use embassy_sync::signal::Signal;
use embassy_sync::{blocking_mutex::raw::ThreadModeRawMutex, mutex::Mutex};
use embassy_time::Timer;
use micromath::F32;
use nrf_softdevice::ble::{Address, AddressType, central, gatt_client};
use nrf_softdevice::{Softdevice, raw};

use array_concat::*;
use core::mem;
use defmt::{info, *};
use micromath::F32Ext;

/// Application must run at a lower priority than softdevice
pub fn config() -> Config {
    let mut config = Config::default();
    config.gpiote_interrupt_priority = Priority::P2;
    config.time_interrupt_priority = Priority::P2;
    config
}

#[embassy_executor::task]
async fn softdevice_task(sd: &'static Softdevice) -> ! {
    info!("running softdevice");
    sd.run().await
}

// #[nrf_softdevice::gatt_client(uuid = "180f")]
// struct BatteryServiceClient {
//     #[characteristic(uuid = "2a19", read, write, notify)]
//     battery_level: u8,
// }

#[nrf_softdevice::gatt_client(uuid = "8a8ec266-3ede-4a2f-a87b-aafbc55b8a30")]
struct RcCarClient {
    ///speed forward m/s
    #[characteristic(uuid = "2C09", write, read)]
    target_velocity: [u8; 8],
}

fn sd_config() -> &'static Softdevice {
    info!("Hello World!");

    let config = nrf_softdevice::Config {
        clock: Some(raw::nrf_clock_lf_cfg_t {
            source: raw::NRF_CLOCK_LF_SRC_RC as u8,
            rc_ctiv: 16,
            rc_temp_ctiv: 2,
            accuracy: raw::NRF_CLOCK_LF_ACCURACY_500_PPM as u8,
        }),
        conn_gap: Some(raw::ble_gap_conn_cfg_t {
            conn_count: 6,
            event_length: 6,
        }),
        conn_gatt: Some(raw::ble_gatt_conn_cfg_t { att_mtu: 128 }),
        gatts_attr_tab_size: Some(raw::ble_gatts_cfg_attr_tab_size_t {
            attr_tab_size: raw::BLE_GATTS_ATTR_TAB_SIZE_DEFAULT,
        }),
        gap_role_count: Some(raw::ble_gap_cfg_role_count_t {
            adv_set_count: 1,
            periph_role_count: 3,
            central_sec_count: 0,
            central_role_count: 3,
            _bitfield_1: raw::ble_gap_cfg_role_count_t::new_bitfield_1(0),
        }),
        gap_device_name: Some(raw::ble_gap_cfg_device_name_t {
            p_value: b"HelloRust" as *const u8 as _,
            current_len: 9,
            max_len: 9,
            write_perm: unsafe { mem::zeroed() },
            _bitfield_1: raw::ble_gap_cfg_device_name_t::new_bitfield_1(
                raw::BLE_GATTS_VLOC_STACK as u8,
            ),
        }),
        ..Default::default()
    };

    Softdevice::enable(&config)
}

pub type Vec2 = micromath::vector::F32x2;

pub type SharedSpeed = Signal<ThreadModeRawMutex, Vec2>;

#[embassy_executor::task]
pub async fn write_ble(target_speed: &'static SharedSpeed, s: Spawner) {
    let sd = sd_config();
    s.spawn(softdevice_task(&sd)).unwrap();

    let addrs = &[&Address::new(
        AddressType::RandomStatic,
        [0x13, 0x33, 0x33, 0x33, 0x37, 0b1100_1010],
        // [0x06, 0x6b, 0x71, 0x2c, 0xf5, 0xc0],
    )];
    let mut config = central::ConnectConfig::default();
    // info!("central config: {:#?}", config.);
    info!("looking for device: {}", addrs);
    config.scan_config.whitelist = Some(addrs);
    central::scan(
        sd,
        &central::ScanConfig::default(),
        |report: &raw::ble_gap_evt_adv_report_t| {
            info!(
                "scanned: {:#?} \t {:#?}",
                report.direct_addr.addr, report.peer_addr.addr
            );
            Some(1_u32)
        },
    )
    .await
    .unwrap();
    let conn = central::connect(sd, &config).await.unwrap();
    info!("connected");

    let client: RcCarClient = unwrap!(gatt_client::discover(&conn).await);
    let mut last_speed = Vec2::default();
    let epsillon = 0.04_f32.powi(2);
    loop {
        let speed = target_speed.wait().await;

        let diff_speed = (speed - last_speed);
        let dlen2 = diff_speed[0].powi(2) + diff_speed[1].powi(2);
        if dlen2 < epsillon {
            continue;
        }
        info!(
            "new speed: {:?}, old: {:?}",
            speed.to_array(),
            last_speed.to_array()
        );

        last_speed = speed;

        let x_bytes = speed.x.to_le_bytes();
        let y_bytes = speed.y.to_le_bytes();
        let v_bytes = concat_arrays!(x_bytes, y_bytes);

        match client.target_velocity_write(&v_bytes).await {
            Ok(()) => trace!("sent speed: {:?}", speed),
            Err(e) => error!("failed to send speedy: {}", e),
        };
    }
}
