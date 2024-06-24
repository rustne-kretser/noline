use embassy_futures::join::join;
use embassy_rp::bind_interrupts;
use embassy_rp::peripherals::USB;
use embassy_rp::usb::{Driver, InterruptHandler};
use embassy_usb::class::cdc_acm::{CdcAcmClass, State, USB_CLASS_CDC};
use embassy_usb::{Builder, Config};

use crate::noline_async::cli;

bind_interrupts!(struct Irqs {
    USBCTRL_IRQ => InterruptHandler<USB>;
});

const USB_CDC_SUBCLASS_ACM: u8 = 0x02;
const USB_CDC_PROTOCOL_AT: u8 = 0x01;

const BUF_SIZE_DESCRIPTOR: usize = 256;
const BUF_SIZE_CONTROL: usize = 64;
const MAX_PACKET_SIZE: u16 = 64;

#[embassy_executor::task]
pub async fn usb_handler(usb: USB) {
    // Create the driver, from the HAL.
    let driver = Driver::new(usb, Irqs);

    // Create embassy-usb Config
    let mut config = Config::new(0xc0de, 0xcafe);
    config.manufacturer = Some("Embassy");
    config.product = Some("USB-serial example");
    config.serial_number = Some("TEST");
    config.max_power = 100;
    config.max_packet_size_0 = 64;

    // Required for windows compatibility.
    // https://developer.nordicsemi.com/nRF_Connect_SDK/doc/1.9.1/kconfig/CONFIG_CDC_ACM_IAD.html#help
    config.device_class = USB_CLASS_CDC;
    config.device_sub_class = USB_CDC_SUBCLASS_ACM;
    config.device_protocol = USB_CDC_PROTOCOL_AT;

    // Create embassy-usb DeviceBuilder using the driver and config.
    // It needs some buffers for building the descriptors.
    let mut config_descriptor = [0; BUF_SIZE_DESCRIPTOR];
    let mut bos_descriptor = [0; BUF_SIZE_DESCRIPTOR];
    let mut msos_descriptor = [0; BUF_SIZE_DESCRIPTOR];
    let mut control_buf = [0; BUF_SIZE_CONTROL];

    let mut state = State::new();

    let mut builder = Builder::new(
        driver,
        config,
        //&mut device_descriptor,
        &mut config_descriptor,
        &mut bos_descriptor,
        &mut msos_descriptor,
        &mut control_buf,
    );

    // Create classes on the builder.
    let serial = CdcAcmClass::new(&mut builder, &mut state, MAX_PACKET_SIZE);

    let (mut send, mut recv, mut control) = serial.split_with_control();

    let noline_fut = async {
        cli(&mut send, &mut recv, &mut control).await;
    };

    // Build the builder.
    let mut usb = builder.build();

    // Run the USB device.
    let usb_fut = usb.run();

    // Run everything concurrently.
    // If we had made everything `'static` above instead, we could do this using separate tasks instead.
    join(usb_fut, noline_fut).await;
}
