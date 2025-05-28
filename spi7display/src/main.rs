#![no_std]
#![no_main]

use core::{any::Any, ops::ShlAssign};

use defmt::{info, println};
use embassy_executor::Spawner;
use embassy_nrf::{
    bind_interrupts,
    gpio::{AnyPin, Input, Level, Output, OutputDrive, Pin},
    peripherals::{self, TWISPI0, TWISPI1},
    spim,
};
use embassy_sync::{blocking_mutex::raw::ThreadModeRawMutex, signal::Signal};
use embassy_time::Timer;
use segments::SEGS;
use {defmt_rtt as _, panic_probe as _};

use nrf_softdevice::ble::advertisement_builder::{
    AdvertisementDataType, Flag, LegacyAdvertisementBuilder, LegacyAdvertisementPayload,
};
use nrf_softdevice::ble::gatt_server::Service;
use nrf_softdevice::ble::{gatt_server, get_address, peripheral, set_address, Address, Connection};
use nrf_softdevice::{raw, Softdevice};

// use rcar::SharedSpeed;

// pub static TARGET_SPEED: SharedSpeed = SharedSpeed::new();
//

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

bind_interrupts!(struct Irqs {
    SPIM1_SPIS1_TWIM1_TWIS1_SPI1_TWI1 => spim::InterruptHandler<peripherals::TWISPI1>;
});

static DISPLAY_NUMBER: Signal<ThreadModeRawMutex, u16> = Signal::new();

#[embassy_executor::task]
async fn display7(mut spim: spim::Spim<'static, TWISPI1>, mut ncs: Output<'static, AnyPin>) {
    ncs.set_low();
    cortex_m::asm::delay(30);
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
        if let Some(speed) = DISPLAY_NUMBER.try_take() {
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
            Timer::after_nanos(1000).await;
        }
    }
}

#[embassy_executor::main]
async fn main(s: Spawner) {
    println!("Hello, World!");
    let p = embassy_nrf::init(spi7display::config());

    let mut spi_cfg = spim::Config::default();
    spi_cfg.frequency = spim::Frequency::M1;

    let sck = p.P0_17; // edge p13
    let miso = p.P0_01; // edge p14
    let mosi = p.P0_13; // edge p15
    let mut spim = spim::Spim::new(p.TWISPI1, Irqs, sck, miso, mosi, spi_cfg);

    let mut ncs = Output::new(p.P0_12.degrade(), Level::High, OutputDrive::Standard);
    Timer::after_millis(100);
    s.spawn(display7(spim, ncs)).unwrap();

    let mut btn_b = Input::new(p.P0_23, embassy_nrf::gpio::Pull::Up);
    let mut counter = 0_u16;
    info!("entering buzy loop");
    loop {
        btn_b.wait_for_rising_edge().await;
        info!("btn_b");
        DISPLAY_NUMBER.signal(counter);
        counter += 1;
    }
}
