#![no_std]
#![no_main]

use core::time;

use defmt::{debug, info, println, warn};
use embassy_executor::Spawner;
use embassy_futures::select::{select, Either};
use embassy_nrf::{
    bind_interrupts,
    gpio::{AnyPin, Input, Level, Output, OutputDrive, Pin},
    peripherals::{self, TWISPI1},
    spim,
};
use embassy_sync::{blocking_mutex::raw::ThreadModeRawMutex, mutex::Mutex};
use embassy_time::{Duration, Instant, Timer};
use microbit_bsp::{
    display::{Brightness, Frame},
    Config, LedMatrix, Microbit, Priority,
};
// use micromath::F32Ext;
use ringbuffer::{ConstGenericRingBuffer, RingBuffer};
use rpmsensor::{log_rpm, softdevice_task, Server, SharedRpm};
use segments::SEGS;
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

// static DISPLAY_NUMBER: Signal<ThreadModeRawMutex, u16> = Signal::new();

#[embassy_executor::task]
async fn display7(mut spim: spim::Spim<'static, TWISPI1>, mut ncs: Output<'static, AnyPin>) {
    ncs.set_low();
    Timer::after_ticks(30).await;
    spim.write(&[0xFF, 0xFF]).await.unwrap();
    ncs.set_high();
    let speed = 1337_u16;

    let mut numbers = [
        speed / 1000,
        speed % 1000 / 100,
        speed % 100 / 10,
        speed % 10,
    ]
    .map(|n| n as usize);
    loop {
        if let Ok(speed) = RPM.try_lock() {
            let speed = *speed as u16;
            info!("new speed: {}", speed);
            numbers = [
                speed / 1000,
                speed % 1000 / 100,
                speed % 100 / 10,
                speed % 10,
            ]
            .map(|n| n as usize)
        };
        for (block, num) in numbers.into_iter().rev().enumerate() {
            let mask: u8 = 1 << block;
            // info!("lit {} at char {:b}", num, mask);
            ncs.set_low();
            spim.write(&[SEGS[num], mask]).await.unwrap();
            ncs.set_high();
            Timer::after_ticks(10).await;
        }
    }
}

bind_interrupts!(struct Irqs {
    SPIM1_SPIS1_TWIM1_TWIS1_SPI1_TWI1 => spim::InterruptHandler<peripherals::TWISPI1>;
});

static SERVER: StaticCell<Server> = StaticCell::new();
#[embassy_executor::main]
async fn main(s: Spawner) {
    defmt::println!("Hello, World!");
    let board = embassy_nrf::init(rpmsensor::config());
    // Spawn the underlying softdevice task
    let sd = rpmsensor::enable_softdevice("Embassy Microbit");

    let server = SERVER.init(Server::new(sd).unwrap());

    server.bas.set(13.0).unwrap();
    s.spawn(softdevice_task(sd)).unwrap();
    // Starts the bluetooth advertisement and GATT server
    s.spawn(rpmsensor::advertiser_task(
        s,
        sd,
        server,
        "Embassy Microbit",
    ))
    .unwrap();

    // let mut display = board.display;
    // display.set_brightness(Brightness::MAX);
    let mut btn_a = Input::new(board.P0_14.degrade(), embassy_nrf::gpio::Pull::Up);
    s.spawn(rpm_sense(btn_a, &RPM)).unwrap();
    s.spawn(log_rpm(server, &RPM)).unwrap();

    let sck = board.P0_17; // edge p13
    let miso = board.P0_01; // edge p14
    let mosi = board.P0_13; // edge p15
    let mut spi_cfg = spim::Config::default();
    spi_cfg.frequency = spim::Frequency::M1;
    let mut spim = spim::Spim::new(board.TWISPI1, Irqs, sck, miso, mosi, spi_cfg);

    let mut ncs = Output::new(board.P0_12.degrade(), Level::High, OutputDrive::Standard);
    s.spawn(display7(spim, ncs));
}

mod segments {
    const S0: u8 = 0b0_011_1111;
    const S1: u8 = 0b0_000_0110;
    const S2: u8 = 0b0_101_1011;
    const S3: u8 = 0b0_100_1111;
    const S4: u8 = 0b0_110_0110;
    const S5: u8 = 0b0_110_1101;
    const S6: u8 = 0b0_111_1101;
    const S7: u8 = 0b0_000_0111;
    const S8: u8 = 0b0_111_1111;
    const S9: u8 = 0b0_110_1111;
    pub const SEGS: [u8; 10] = [
        u8::MAX - S0,
        u8::MAX - S1,
        u8::MAX - S2,
        u8::MAX - S3,
        u8::MAX - S4,
        u8::MAX - S5,
        u8::MAX - S6,
        u8::MAX - S7,
        u8::MAX - S8,
        u8::MAX - S9,
    ];
}
