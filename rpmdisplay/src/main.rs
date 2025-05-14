#![no_std]
#![no_main]

use embassy_executor::Spawner;
use embassy_futures::select::{Either, select};
use embassy_nrf::{
    bind_interrupts,
    gpio::{AnyPin, Input, Level, Output, OutputDrive, Pin},
    peripherals, spim,
};
use embassy_time::{Duration, Timer};
use microbit_bsp::*;
use nrf_softdevice;
use {defmt_rtt as _, panic_probe as _};

type Btn = Input<'static, AnyPin>;
type Led = Output<'static, AnyPin>;

#[embassy_executor::task]
async fn run_btns(mut a: Btn, mut b: Btn, mut display: display::LedMatrix<Led, 5, 5>) {
    loop {
        match select(a.wait_for_low(), b.wait_for_low()).await {
            Either::First(_) => {
                defmt::info!("A pressed");
                display
                    .display(display::fonts::ARROW_LEFT, Duration::from_secs(1))
                    .await;
            }
            Either::Second(_) => {
                defmt::info!("B pressed");
                display
                    .display(display::fonts::ARROW_RIGHT, Duration::from_secs(1))
                    .await;
            }
        }
    }
}

bind_interrupts!(struct Irqs {
    SPIM0_SPIS0_TWIM0_TWIS0_SPI0_TWI0 => spim::InterruptHandler<peripherals::TWISPI0>;
});

#[embassy_executor::main]
async fn main(spawner: Spawner) {
    let board = Microbit::default();

    let mut btn_a = board.btn_a;
    let mut display = board.display;
    let mut btn_b = board.btn_b;
    let miso = board.p14;
    let mosi = board.p15;
    let clk = board.p13;
    let mut cs_segment_display = Output::new(board.p8, Level::High, OutputDrive::Standard);

    let mut spi_config = spim::Config::default();
    spi_config.frequency = spim::Frequency::M2;
    let mut segment_spi = spim::Spim::new(board.twispi0, Irqs, clk, miso, mosi, spi_config);

    Timer::after_millis(10).await;
    cs_segment_display.is_set_low();
    Timer::after_millis(10).await;
    let tx = [];

    display.set_brightness(display::Brightness::MAX);
    display.scroll("Hello").await;
    defmt::info!("Application started, press buttons!");
    spawner.spawn(run_btns(btn_a, btn_b, display));
}
