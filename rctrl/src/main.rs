#![no_std]
#![no_main]

use defmt::{info, println, trace};
use embassy_executor::Spawner;
use embassy_futures::select::{Either, select};
use embassy_nrf::{
    bind_interrupts,
    gpio::{AnyPin, Input, Level, Output, OutputDrive, Pin},
    interrupt::{self, InterruptExt, Priority},
    peripherals::{self, P0_00, P0_02, P0_03, P0_04, P0_05, P0_31, SAADC},
    saadc::{self, Saadc},
    spim,
};
use embassy_sync::{blocking_mutex::raw::ThreadModeRawMutex, mutex::Mutex};
use micromath::F32Ext;

use embassy_time::{Duration, Timer};
// use microbit_bsp::*;
use nrf_softdevice;
use rctrl::{SharedSpeed, Vec2, Vec3, write_ble};
use {defmt_rtt as _, panic_probe as _};

type Btn = Input<'static, AnyPin>;
type Led = Output<'static, AnyPin>;

static TARGET_SPEED: SharedSpeed = SharedSpeed::new();

bind_interrupts!(struct Irqs {
    SAADC => saadc::InterruptHandler;
});

struct Joystick<'a> {
    raw: &'a [i16],
    offsets: [i16; 3],
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
        -self.clamp(v)
    }
    fn z(&self) -> i16 {
        let v = self.raw[2] - self.offsets[2];
        self.clamp(v)
    }

    /// normalized vector
    fn vec3(&self) -> Vec3 {
        if (self.x() == 0) && (self.y() == 0) && (self.z() == 0) {
            return Vec3::default();
        };
        let maxv = self.maxv as f32;
        let x = self.x() as f32 / maxv;
        let y = self.y() as f32 / maxv;
        let z = self.z() as f32 / maxv;
        Vec3 { x, y, z }
    }

    /// normalized vector
    fn vec2(&self) -> Vec2 {
        if (self.x() == 0) && (self.y() == 0) {
            return Vec2::default();
        };
        let maxv = self.maxv as f32;
        let x = self.x() as f32 / maxv;
        let y = self.y() as f32 / maxv;
        Vec2 { x, y }
    }
}

#[embassy_executor::task]
async fn analog_read(
    target_speed: &'static SharedSpeed,
    adc: SAADC,
    a0: P0_02,
    a1: P0_03,
    a2: P0_04,
    a3: P0_31,
) {
    let config = saadc::Config::default();
    println!("adc  res {:#?}", config.resolution as u8);

    let ain1 = saadc::ChannelConfig::single_ended(a0);
    let ain2 = saadc::ChannelConfig::single_ended(a1);
    let ain3 = saadc::ChannelConfig::single_ended(a2);
    let ain4 = saadc::ChannelConfig::single_ended(a3);

    interrupt::SAADC.set_priority(Priority::P5);
    let mut saadc = Saadc::new(adc, Irqs, config, [ain1, ain2, ain3, ain4]);

    Timer::after_millis(300).await;
    saadc.calibrate().await;
    Timer::after_millis(300).await;
    let mut buf = [0; 4];

    saadc.sample(&mut buf).await;
    let offsets = [buf[0], buf[1], buf[2]];
    let minv = 60;
    let maxv = 1500;
    loop {
        saadc.sample(&mut buf).await;
        let joy = Joystick {
            raw: &buf[0..3],
            offsets,
            minv,
            maxv,
        };
        let speed = joy.vec3();
        target_speed.signal(speed);

        trace!("speed: {:?}", speed.to_array());
        Timer::after_millis(5).await;
    }
}

#[embassy_executor::main]
async fn main(s: Spawner) {
    let mut p = embassy_nrf::init(rctrl::config());
    s.spawn(analog_read(
        &TARGET_SPEED,
        p.SAADC,
        p.P0_02,
        p.P0_03,
        p.P0_04,
        p.P0_31,
    ))
    .unwrap();
    s.spawn(write_ble(&TARGET_SPEED, s)).unwrap();
}
