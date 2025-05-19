#![no_std]
#![no_main]

use defmt::{info, println};
use embassy_executor::Spawner;
use embassy_futures::select::{Either, select};
use embassy_nrf::{
    bind_interrupts,
    gpio::{AnyPin, Input, Level, Output, OutputDrive, Pin},
    peripherals::{self, P0_00, P0_02, P0_03, P0_04, P0_05, SAADC},
    saadc::{self, Saadc},
    spim,
};
use micromath::F32Ext;

use embassy_time::{Duration, Timer};
// use microbit_bsp::*;
use nrf_softdevice;
use rctrl::read_ble;
use {defmt_rtt as _, panic_probe as _};

type Btn = Input<'static, AnyPin>;
type Led = Output<'static, AnyPin>;

bind_interrupts!(struct Irqs {
    SAADC => saadc::InterruptHandler;
});

struct Joystick<'a> {
    raw: &'a [i16],
    offsets: [i16; 2],
    minv: i16,
    maxv: i16,
}

impl<'a> Joystick<'a> {
    fn clamp(&self, v: i16) -> i16 {
        if v.abs() < self.minv {
            return 0;
        }
        if v.abs() > self.maxv {
            return v.signum() * self.maxv;
        }
        v
    }
    fn x(&self) -> i16 {
        let v = self.raw[0] - self.offsets[0];
        self.clamp(v)
    }
    fn y(&self) -> i16 {
        let v = self.raw[1] - self.offsets[1];
        self.clamp(v)
    }
    /// normalized vector
    fn vec2(&self) -> [f32; 2] {
        if (self.x() == 0) && (self.y() == 0) {
            return [0.0; 2];
        };
        let x = self.x() as f32;
        let y = self.y() as f32;
        let mag2 = (x.powi(2) + y.powi(2));
        let maxv = self.maxv as f32;
        let maxmag2 = maxv.powi(2);
        let clamp_mag2 = mag2.min(maxmag2);
        let clamper = (clamp_mag2 / mag2).sqrt() / maxv;
        [x * clamper, -y * clamper]
    }
}

#[embassy_executor::task]
async fn analog_read(adc: SAADC, a0: P0_02, a1: P0_03, a2: P0_04, a3: P0_05) {
    let config = saadc::Config::default();
    println!("adc  res {:#?}", config.resolution as u8);

    let ain1 = saadc::ChannelConfig::single_ended(a0);
    let ain2 = saadc::ChannelConfig::single_ended(a1);
    let ain3 = saadc::ChannelConfig::single_ended(a2);
    let ain4 = saadc::ChannelConfig::single_ended(a3);
    let mut saadc = Saadc::new(adc, Irqs, config, [ain1, ain2, ain3, ain4]);

    Timer::after_millis(300).await;
    saadc.calibrate().await;
    Timer::after_millis(300).await;
    let mut buf = [0; 4];

    saadc.sample(&mut buf).await;
    let offsets = [buf[0], buf[1]];
    let minv = 60;
    let maxv = 1500;
    loop {
        saadc.sample(&mut buf).await;
        let joy = Joystick {
            raw: &buf[0..2],
            offsets,
            minv,
            maxv,
        };
        println!("joy: {:?}", joy.vec2());
        Timer::after_millis(300).await;
    }
}

#[embassy_executor::main]
async fn main(s: Spawner) {
    let mut p = embassy_nrf::init(rctrl::config());
    // s.spawn(analog_read(p.SAADC, p.P0_02, p.P0_03, p.P0_04, p.P0_05))
    //     .unwrap();
    s.spawn(read_ble(s)).unwrap();
}
