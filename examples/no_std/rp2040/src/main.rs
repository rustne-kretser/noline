#![no_std]
#![no_main]

use embedded_io::{ErrorKind, ErrorType, Read, ReadReady, Write, WriteReady};
use rp_pico as bsp;

use bsp::entry;
use defmt::*;
use {defmt_rtt as _, panic_probe as _};

use bsp::hal::{clocks::init_clocks_and_plls, pac, usb::UsbBus, watchdog::Watchdog};

use core::fmt::Write as FmtWrite;

use noline::builder::EditorBuilder;
use noline::error::NolineError;

use usb_device::bus::UsbBusAllocator;
use usb_device::prelude::*;
use usbd_serial::{DefaultBufferStore, SerialPort, USB_CLASS_CDC};

type SP<'a> = SerialPort<'a, UsbBus, DefaultBufferStore, DefaultBufferStore>;

struct SerialWrapper<'a> {
    device: UsbDevice<'a, UsbBus>,
    serial: SP<'a>,
    ready: bool,
}

impl<'a> SerialWrapper<'a> {
    fn new(device: UsbDevice<'a, UsbBus>, serial: SP<'a>) -> Self {
        Self {
            device,
            serial,
            ready: false,
        }
    }

    fn poll(&mut self) -> bool {
        self.device.poll(&mut [&mut self.serial])
    }

    fn is_ready(&mut self) -> bool {
        self.ready = self.ready | self.poll();
        self.ready
    }
}

#[derive(Debug)]
struct Error(UsbError);

impl From<UsbError> for Error {
    fn from(value: UsbError) -> Self {
        Self(value)
    }
}

impl embedded_io::Error for Error {
    fn kind(&self) -> ErrorKind {
        ErrorKind::Other
    }
}

impl<'a> ErrorType for SerialWrapper<'a> {
    type Error = Error;
}

impl<'a> ReadReady for SerialWrapper<'a> {
    fn read_ready(&mut self) -> Result<bool, Self::Error> {
        Ok(self.is_ready() | self.serial.dtr() | self.serial.rts())
    }
}

impl<'a> Read for SerialWrapper<'a> {
    fn read(&mut self, buf: &mut [u8]) -> Result<usize, Self::Error> {
        loop {
            while !self.read_ready()? {
                continue;
            }

            let res = self.serial.read(buf);
            if res == Err(UsbError::WouldBlock) {
                self.ready = false;
                continue;
            }

            break Ok(res?);
        }
    }
}

impl<'a> WriteReady for SerialWrapper<'a> {
    fn write_ready(&mut self) -> Result<bool, Self::Error> {
        Ok(self.is_ready() | self.serial.dtr() | self.serial.rts())
    }
}
impl<'a> Write for SerialWrapper<'a> {
    fn write(&mut self, buf: &[u8]) -> Result<usize, Self::Error> {
        loop {
            while !self.write_ready()? {
                continue;
            }

            let res = self.serial.write(buf);
            if res == Err(UsbError::WouldBlock) {
                self.ready = false;
                continue;
            }

            break Ok(res?);
        }
    }

    fn flush(&mut self) -> Result<(), Self::Error> {
        Ok(self.serial.flush()?)
    }
}

#[entry]
fn main() -> ! {
    info!("Starting...");

    // Grab our singleton objects
    let mut pac = pac::Peripherals::take().unwrap();

    // Set up the watchdog driver - needed by the clock setup code
    let mut watchdog = Watchdog::new(pac.WATCHDOG);

    // Configure the clocks
    //
    // The default is to generate a 125 MHz system clock
    let clocks = init_clocks_and_plls(
        rp_pico::XOSC_CRYSTAL_FREQ,
        pac.XOSC,
        pac.CLOCKS,
        pac.PLL_SYS,
        pac.PLL_USB,
        &mut pac.RESETS,
        &mut watchdog,
    )
    .ok()
    .unwrap();

    // Set up the USB driver
    let usb_bus = UsbBusAllocator::new(UsbBus::new(
        pac.USBCTRL_REGS,
        pac.USBCTRL_DPRAM,
        clocks.usb_clock,
        true,
        &mut pac.RESETS,
    ));

    // Set up the USB Communications Class Device driver
    let serial = SerialPort::new(&usb_bus);

    // Create a USB device with a fake VID and PID
    let usb_dev = UsbDeviceBuilder::new(&usb_bus, UsbVidPid(0x16c0, 0x27dd))
        .strings(&[StringDescriptors::default()
            .manufacturer("Fake company")
            .product("Serial port")
            .serial_number("TEST")])
        .unwrap()
        .device_class(USB_CLASS_CDC)
        .build();

    let prompt = "> ";

    let mut io = SerialWrapper::new(usb_dev, serial);

    info!("Waiting for connection");

    let mut buffer = [0; 128];
    let mut editor = EditorBuilder::from_slice(&mut buffer)
        .with_static_history::<128>()
        .build_sync(&mut io)
        .unwrap();

    loop {
        match editor.readline(prompt, &mut io) {
            Ok(s) => {
                if s.len() > 0 {
                    writeln!(io, "Echo: {}\r", s).unwrap();
                } else {
                    // Writing emtpy slice causes panic
                    writeln!(io, "Echo: \r").unwrap();
                }
            }
            Err(err) => {
                let error = match err {
                    NolineError::IoError(_) => "IoError",
                    NolineError::ParserError => "ParserError",
                    NolineError::Aborted => "Aborted",
                };
                writeln!(io, "Error: {}\r", error).unwrap();
            }
        }
    }
}
