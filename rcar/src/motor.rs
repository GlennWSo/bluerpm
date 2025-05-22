#![no_std]
#![no_main]

use core::{any::Any, ops::Mul, time};

use crate::{ble, SharedSpeed};
use defmt::{debug, error, info, println, trace, warn, Debug2Format, Format};
use embassy_executor::Spawner;
use embassy_futures::select::{select, Either};
use embassy_nrf::{
    bind_interrupts,
    gpio::{AnyPin, Input, Pin},
    interrupt::{self, typelevel, InterruptExt},
    peripherals::{self, P0_26, P1_00, SAADC, TWISPI0, TWISPI1},
    saadc,
    twim::{self, Twim},
};

use embassy_sync::{blocking_mutex::raw::ThreadModeRawMutex, mutex::Mutex};
use embassy_time::{Duration, Instant, Timer};
use micromath::{
    vector::{F32x2, Vector},
    F32Ext,
};
use {defmt_rtt as _, panic_probe as _};
type Vec2 = F32x2;

// use nrf_softdevice::ble::{gatt_server, peripheral, Connection};
// use nrf_softdevice::{raw, Softdevice};

use static_cell::ConstStaticCell;
use static_cell::StaticCell;
// type SharedCounter = Mutex<ThreadModeRawMutex, u32>;
// static COUNTER: SharedCounter = SharedCounter::new(0);
use core::ops::Add;

bind_interrupts!(struct Irqs {
    SPIM1_SPIS1_TWIM1_TWIS1_SPI1_TWI1 => twim::InterruptHandler<peripherals::TWISPI1>;
});

#[derive(Clone, Default, Copy, Debug)]
struct WheelSpeed {
    front_left: f32,
    front_right: f32,
    back_left: f32,
    back_right: f32,
}
impl WheelSpeed {
    fn drive_y(y: f32) -> WheelSpeed {
        WheelSpeed {
            front_left: y,
            back_left: y,
            front_right: -y,
            back_right: -y,
        }
    }
    fn drive_x(x: f32) -> WheelSpeed {
        WheelSpeed {
            front_left: x,
            back_left: -x,
            front_right: x,
            back_right: -x,
        }
    }
    fn drive_z(z: f32) -> WheelSpeed {
        WheelSpeed {
            front_left: -z,
            back_left: -z,
            front_right: -z,
            back_right: -z,
        }
    }

    fn translate(x: f32, y: f32) -> WheelSpeed {
        (WheelSpeed::drive_x(x) + WheelSpeed::drive_y(y)).clamp1()
    }
    fn trans_rotate(x: f32, y: f32, z: f32) -> WheelSpeed {
        let xy_mag = Vec2 { x, y }.magnitude();
        let z = z / (2.0 - xy_mag);
        (WheelSpeed::drive_x(x) + WheelSpeed::drive_y(y) + WheelSpeed::drive_z(z)).clamp1()
    }

    fn to_array(&self) -> [f32; 4] {
        [
            self.front_left,
            self.back_left,
            self.front_right,
            self.back_right,
        ]
    }
    fn absmax(&self) -> f32 {
        self.to_array()
            .map(|e| e.abs())
            .iter()
            .fold(0.0, |acc, v| acc.max(*v))
    }

    fn clamp1(mut self) -> WheelSpeed {
        let max = self.absmax();
        if max < 1.0 {
            return self;
        }
        self * (1.0 / max)
    }
}

impl Add<f32> for WheelSpeed {
    type Output = WheelSpeed;

    fn add(mut self, rhs: f32) -> Self::Output {
        self.front_left += rhs;
        self.back_left += rhs;
        self.front_right += rhs;
        self.back_right += rhs;
        self
    }
}
impl Mul<f32> for WheelSpeed {
    type Output = WheelSpeed;

    fn mul(mut self, rhs: f32) -> Self::Output {
        self.front_left *= rhs;
        self.back_left *= rhs;
        self.front_right *= rhs;
        self.back_right *= rhs;
        self
    }
}

impl Add for WheelSpeed {
    type Output = WheelSpeed;

    fn add(mut self, rhs: Self) -> Self::Output {
        self.front_left += rhs.front_left;
        self.back_left += rhs.back_left;
        self.front_right += rhs.front_right;
        self.back_right += rhs.back_right;
        self
    }
}

#[derive(Clone, Copy, defmt::Format)]
struct WheelMan {
    front_left: u8,
    front_right: u8,
    back_left: u8,
    back_right: u8,
}

type MotorWriteBufs = [[u8; 2]; 4];

impl WheelMan {
    fn transform_bufs(&self, x: f32, y: f32) -> MotorWriteBufs {
        let speeds = WheelSpeed::translate(x, y);
        [
            [self.front_left, (speeds.front_left * 90.0 + 90.0) as u8],
            [self.front_right, (speeds.front_right * 90.0 + 90.0) as u8],
            [self.back_left, (speeds.back_left * 90.0 + 90.0) as u8],
            [self.back_right, (speeds.back_right * 90.0 + 90.0) as u8],
        ]
    }
    fn trans_rotate_bufs(&self, x: f32, y: f32, z: f32) -> MotorWriteBufs {
        let speeds = WheelSpeed::trans_rotate(x, y, z);
        [
            [self.front_left, (speeds.front_left * 90.0 + 90.0) as u8],
            [self.front_right, (speeds.front_right * 90.0 + 90.0) as u8],
            [self.back_left, (speeds.back_left * 90.0 + 90.0) as u8],
            [self.back_right, (speeds.back_right * 90.0 + 90.0) as u8],
        ]
    }
}

#[embassy_executor::task]
pub async fn drive_servos(
    target_speed: &'static SharedSpeed,
    twi1: TWISPI1,
    scl: P0_26,
    sda: P1_00,
) {
    info!("Initializing TWI...");

    let mut i2c_config = twim::Config::default();
    i2c_config.frequency = twim::Frequency::K100;
    i2c_config.sda_pullup = true;
    i2c_config.scl_pullup = true;
    i2c_config.sda_high_drive = true;
    i2c_config.scl_high_drive = true;

    interrupt::SPIM1_SPIS1_TWIM1_TWIS1_SPI1_TWI1.set_priority(interrupt::Priority::P5);
    let mut twim = Twim::new(twi1, Irqs, sda, scl, i2c_config);
    let wukong_address = 0x10;
    let wheel_cfg = WheelMan {
        front_left: 4,
        back_left: 5,
        front_right: 6,
        back_right: 7,
    };

    // let mut speed = 0_u8;
    info!("entering speed ctrl loop");
    loop {
        let [x, y, z] = target_speed.wait().await;
        let mut motor_speeds = wheel_cfg.trans_rotate_bufs(x, y, z);
        trace!("new speed: x:{}, y:{}, z:{}", x, y, z);

        for (i, [motor, speed]) in motor_speeds.iter().copied().enumerate() {
            let buf = [motor, speed, 0, 0];
            let res = twim.write(wukong_address, &buf).await;
            match res {
                _ => {}
                Err(e) => {
                    error!(
                        "failed to write twi_buff: {}:{:?} \n\te:{}",
                        wukong_address, buf, e
                    );
                }
            }
        }
    }
    return;
}
