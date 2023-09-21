#![no_std]
#![no_main]
#![feature(type_alias_impl_trait)]
#![feature(async_fn_in_trait)]

mod noline_async;
mod blink;
mod usb;

use embassy_executor::Spawner;
use {defmt_rtt as _, panic_probe as _};

use blink::blinking_led;
use usb::usb_handler;

#[embassy_executor::main]
async fn main(spawner: Spawner) {
    let p = embassy_rp::init(Default::default());

    spawner.spawn(blinking_led(p.PIN_25.into())).unwrap();
    spawner.spawn(usb_handler(p.USB)).unwrap();
}
