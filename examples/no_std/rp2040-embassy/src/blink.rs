use embassy_rp::gpio;
use embassy_time::{Duration, Timer};
use gpio::{AnyPin, Level, Output};

#[embassy_executor::task]
pub async fn blinking_led(led: AnyPin) {
    let mut led = Output::new(led, Level::Low);

    loop {
        led.set_high();
        Timer::after(Duration::from_millis(750)).await;

        led.set_low();
        Timer::after(Duration::from_millis(250)).await;
    }
}
