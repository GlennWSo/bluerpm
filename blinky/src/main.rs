#![no_std]
#![no_main]

use core::ptr::write_volatile;

use cortex_m::asm::nop;
use cortex_m_rt::entry;
// use nrf52833_pac as _; // just so we dont have to remove it from Cargo.toml

use defmt::*;
use {defmt_rtt as _, panic_probe as _};

const GPIO: u32 = 0x50_000_000;
const OUT: u32 = 0x504; // write to gpio port

const P0: u32 = GPIO; // port 0
const PIN21: u32 = 0x754;
const PIN28: u32 = 0x770;

// const PIN_CNF21= c

#[entry]
fn main() -> ! {
    const GPIO0_PNCNF21_ROW1_ADDR: *mut u32 = (GPIO + PIN21) as *mut u32;
    const GPIO0_PNCNF28_ROW1_ADDR: *mut u32 = (GPIO + PIN28) as *mut u32;
    const DIR_OUTPUT_POS: u32 = 0;
    const PINCNF_DRIVE_LED: u32 = 1 << DIR_OUTPUT_POS;
    unsafe {
        write_volatile(GPIO0_PNCNF21_ROW1_ADDR, PINCNF_DRIVE_LED);
        write_volatile(GPIO0_PNCNF28_ROW1_ADDR, PINCNF_DRIVE_LED);
    }

    const GPIO0_OUT_ROW1_POS: u32 = 21;
    const GPIO0_OUT_ADDR: *mut u32 = (GPIO + OUT) as *mut u32;
    let mut is_on = false;
    println!("entering blinky loop");
    loop {
        unsafe {
            write_volatile(GPIO0_OUT_ADDR, (is_on as u32) << GPIO0_OUT_ROW1_POS);
        }
        for _ in 0..400_000 {
            nop();
        }
        is_on = !is_on;
    }
}
