#![no_std]
#![no_main]

use embedded_io::{ErrorKind, ErrorType, Read, ReadReady, Write, WriteReady};
use rp_pico as bsp;

use bsp::entry;
use defmt::*;
use {defmt_rtt as _, panic_probe as _};

use bsp::hal::{clocks::init_clocks_and_plls, pac, usb::UsbBus, watchdog::Watchdog};

use noline::builder::EditorBuilder;
use noline::error::NolineError;

use usb_device::bus::UsbBusAllocator;
use usb_device::prelude::*;
use usbd_serial::{DefaultBufferStore, SerialPort, USB_CLASS_CDC};

type SP<'a> = SerialPort<'a, UsbBus, DefaultBufferStore, DefaultBufferStore>;

struct SerialWrapper<'a> {
    device: UsbDevice<'a, UsbBus>,
    serial: SP<'a>,
}

impl<'a> SerialWrapper<'a> {
    fn new(device: UsbDevice<'a, UsbBus>, serial: SP<'a>) -> Self {
        Self { device, serial }
    }

    fn poll(&mut self) -> bool {
        self.device.poll(&mut [&mut self.serial])
    }
}

#[derive(Debug)]
struct Error(UsbError);

impl From<UsbError> for Error {
    fn from(value: UsbError) -> Self {
        Self(value)
    }
}

impl core::error::Error for Error {}

impl core::fmt::Display for Error {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        core::write!(f, "an error occurred")
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
        // not used in this example
        Ok(true)
    }
}

impl<'a> Read for SerialWrapper<'a> {
    fn read(&mut self, buf: &mut [u8]) -> Result<usize, Self::Error> {
        loop {
            self.poll();
            match self.serial.read(buf) {
                Ok(0) => continue,
                Ok(n) => return Ok(n),
                Err(UsbError::WouldBlock) => continue,
                Err(e) => return Err(Error(e)),
            }
        }
    }
}

impl<'a> WriteReady for SerialWrapper<'a> {
    fn write_ready(&mut self) -> Result<bool, Self::Error> {
        // not used in this example
        Ok(true)
    }
}

impl<'a> Write for SerialWrapper<'a> {
    fn write(&mut self, buf: &[u8]) -> Result<usize, Self::Error> {
        loop {
            self.poll();
            match self.serial.write(buf) {
                Ok(n) => return Ok(n),
                Err(UsbError::WouldBlock) => continue,
                Err(e) => return Err(Error(e)),
            }
        }
    }

    fn flush(&mut self) -> Result<(), Self::Error> {
        loop {
            self.poll();
            match self.serial.flush() {
                Ok(()) => return Ok(()),
                Err(UsbError::WouldBlock) => continue,
                Err(e) => return Err(Error(e)),
            }
        }
    }
}

#[entry]
fn main() -> ! {
    info!("Starting...");

    info!("Grabbing PAC");
    // Grab our singleton objects
    let mut pac = pac::Peripherals::take().unwrap();

    info!("Setting up watchdog");
    // Set up the watchdog driver - needed by the clock setup code
    let mut watchdog = Watchdog::new(pac.WATCHDOG);

    info!("Setting up clock");
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

    info!("Setting up usb driver");
    // Set up the USB driver
    let usb_bus = UsbBusAllocator::new(UsbBus::new(
        pac.USBCTRL_REGS,
        pac.USBCTRL_DPRAM,
        clocks.usb_clock,
        true,
        &mut pac.RESETS,
    ));

    info!("Setting up serial driver");
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

    // Wait for host to open the port (DTR signal)
    loop {
        io.poll();
        if io.serial.dtr() {
            break;
        }
    }

    info!("Connected");

    let mut buffer = [0; 128];
    let mut history = [0; 128];
    let mut editor = EditorBuilder::from_slice(&mut buffer)
        .with_slice_history(&mut history)
        .build();

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
                let error;

                match err {
                    NolineError::IoError(_) => error = "IoError",
                    NolineError::ParserError => error = "ParserError",
                    NolineError::Aborted => error = "Aborted",
                };

                error!("{}", error);
                writeln!(io, "Error: {}\r", error).unwrap();
            }
        }
    }
}
