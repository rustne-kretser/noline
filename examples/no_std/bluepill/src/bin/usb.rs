//! CDC-ACM serial port example using polling in a busy loop.
//! Target board: Blue Pill
//!
//! Note:
//! When building this since this is a larger program,
//! one would need to build it using release profile
//! since debug profiles generates artifacts that
//! cause FLASH overflow errors due to their size
#![no_std]
#![no_main]

use embedded_hal::serial::{Read, Write};
use nb::block;
use noline::error::Error;
use noline::line_buffer::StaticBuffer;
use noline::sync::embedded::Editor;
use panic_halt as _;

use cortex_m::asm::delay;
use cortex_m_rt::entry;
use stm32f1xx_hal::pac;
use stm32f1xx_hal::prelude::*;
use stm32f1xx_hal::usb::{Peripheral, UsbBus};
use usb_device::prelude::*;
use usbd_serial::{SerialPort, USB_CLASS_CDC};

// The usb-device API doesn't play well with the `block!` from
// `nb`. Added a simple wrapper to be able to use a shared
// implementation for both the UART and USB examples.
struct Wrapper<'a> {
    device: &'a mut UsbDevice<'a, UsbBus<Peripheral>>,
    serial: &'a mut SerialPort<'a, UsbBus<Peripheral>>,
    ready: bool,
}

impl<'a> Wrapper<'a> {
    fn new(
        device: &'a mut UsbDevice<'a, UsbBus<Peripheral>>,
        serial: &'a mut SerialPort<'a, UsbBus<Peripheral>>,
    ) -> Self {
        Self {
            device,
            serial,
            ready: false,
        }
    }

    fn poll(&mut self) -> bool {
        self.device.poll(&mut [self.serial])
    }

    fn is_ready(&mut self) -> bool {
        if !self.ready {
            self.ready = self.poll();
        }

        self.ready
    }

    fn try_op<'b, T, E>(
        &'b mut self,
        f: impl FnOnce(&'b mut SerialPort<'a, UsbBus<Peripheral>>) -> nb::Result<T, E>,
    ) -> nb::Result<T, E> {
        if self.is_ready() {
            let res = f(self.serial);

            match res {
                Err(nb::Error::WouldBlock) => self.ready = false,
                _ => (),
            }

            res
        } else {
            Err(nb::Error::WouldBlock)
        }
    }
}

impl<'a> Read<u8> for Wrapper<'a> {
    type Error = UsbError;

    fn read(&mut self) -> nb::Result<u8, Self::Error> {
        self.try_op(|serial| Read::read(serial))
    }
}

impl<'a> Write<u8> for Wrapper<'a> {
    type Error = UsbError;

    fn write(&mut self, word: u8) -> nb::Result<(), Self::Error> {
        self.try_op(|serial| Write::write(serial, word))
    }

    fn flush(&mut self) -> nb::Result<(), Self::Error> {
        self.try_op(|serial| Write::flush(serial))
    }
}

#[entry]
fn main() -> ! {
    let dp = pac::Peripherals::take().unwrap();

    let mut flash = dp.FLASH.constrain();
    let rcc = dp.RCC.constrain();

    let clocks = rcc
        .cfgr
        .use_hse(8.mhz())
        .sysclk(48.mhz())
        .pclk1(24.mhz())
        .freeze(&mut flash.acr);

    assert!(clocks.usbclk_valid());

    // Configure the on-board LED (PC13, green)
    let mut gpioc = dp.GPIOC.split();
    let mut led = gpioc.pc13.into_push_pull_output(&mut gpioc.crh);
    led.set_high(); // Turn off

    let mut gpioa = dp.GPIOA.split();

    // BluePill board has a pull-up resistor on the D+ line.
    // Pull the D+ pin down to send a RESET condition to the USB bus.
    // This forced reset is needed only for development, without it host
    // will not reset your device when you upload new firmware.
    let mut usb_dp = gpioa.pa12.into_push_pull_output(&mut gpioa.crh);
    usb_dp.set_low();
    delay(clocks.sysclk().0 / 100);

    let usb = Peripheral {
        usb: dp.USB,
        pin_dm: gpioa.pa11,
        pin_dp: usb_dp.into_floating_input(&mut gpioa.crh),
    };
    let usb_bus = UsbBus::new(usb);

    let mut serial = SerialPort::new(&usb_bus);

    let mut usb_dev = UsbDeviceBuilder::new(&usb_bus, UsbVidPid(0x16c0, 0x27dd))
        .manufacturer("Rustne kretser AS")
        .product("Serial port")
        .serial_number("TEST")
        .device_class(USB_CLASS_CDC)
        .build();

    let prompt = "> ";

    let mut wrapper = Wrapper::new(&mut usb_dev, &mut serial);

    let mut editor: Editor<StaticBuffer<128>> = loop {
        if !wrapper.poll() || !wrapper.serial.dtr() || !wrapper.serial.rts() {
            continue;
        }

        // If attempting to write before reading, the next read will
        // get occasional garbage input. I'm not sure where the
        // garbage comes from, but it could be a bug in usb-device or
        // usbd-serial. Becase noline needs to write during
        // initialization, I've added this blocking read here to wait
        // for user input before proceeding.
        block!(wrapper.read()).unwrap();
        break Editor::new(prompt, &mut wrapper).unwrap();
    };

    loop {
        match editor.readline(&mut wrapper) {
            Ok(s) => {
                wrapper.serial.write("Echo: ".as_bytes()).unwrap();

                // Writing emtpy slice causes panic
                if s.len() > 0 {
                    wrapper.serial.write(s.as_bytes()).unwrap();
                }

                wrapper.serial.write("\n\r".as_bytes()).unwrap();
            }
            Err(err) => {
                let error = match err {
                    Error::WriteError(err) | Error::ReadError(err) => match err {
                        UsbError::WouldBlock => "Wouldblock",
                        UsbError::ParseError => "ParseEror",
                        UsbError::BufferOverflow => "BufferOverflow",
                        UsbError::EndpointOverflow => "EndpointOverflow",
                        UsbError::EndpointMemoryOverflow => "EndpointMemoryOverflow",
                        UsbError::InvalidEndpoint => "InvalidEndpoint",
                        UsbError::Unsupported => "Unsupported",
                        UsbError::InvalidState => "InvalidState",
                    },
                    Error::ParserError => "ParserError",
                    Error::Aborted => "Aborted",
                };

                wrapper.serial.write("Failed: ".as_bytes()).unwrap();
                wrapper.serial.write(error.as_bytes()).unwrap();
                wrapper.serial.write("\n\r".as_bytes()).unwrap();
            }
        }
    }
}
