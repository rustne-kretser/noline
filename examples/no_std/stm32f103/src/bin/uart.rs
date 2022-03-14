//! Serial interface loopback test
//!
//! You have to short the TX and RX pins to make this program work

#![allow(clippy::empty_loop)]
#![no_main]
#![no_std]

use heapless::spsc::{Consumer, Producer, Queue};
use noline::{
    error::Error,
    line_buffer::StaticBuffer,
    sync::{embedded::IO, Editor},
};
use panic_halt as _;

use cortex_m::asm;

use cortex_m_rt::entry;
use stm32f1xx_hal::{
    device::USART3,
    pac,
    pac::interrupt,
    prelude::*,
    serial::{Config, Rx, Serial, Tx},
};

use core::{convert::Infallible, fmt::Write as FmtWrite};
use embedded_hal::serial::{Read, Write};

static mut RX: Option<Rx<USART3>> = None;
static mut RX_PRODUCER: Option<Producer<u8, 16>> = None;

struct SerialWrapper<'a> {
    tx: Tx<USART3>,
    rx: Consumer<'a, u8, 16>,
}

impl<'a> SerialWrapper<'a> {
    fn new(tx: Tx<USART3>, rx: Consumer<'a, u8, 16>) -> Self {
        Self { tx, rx }
    }
}

impl<'a> Write<u8> for SerialWrapper<'a> {
    type Error = Infallible;

    fn write(&mut self, word: u8) -> nb::Result<(), Self::Error> {
        self.tx.write(word)
    }

    fn flush(&mut self) -> nb::Result<(), Self::Error> {
        Ok(())
    }
}

impl<'a> Read<u8> for SerialWrapper<'a> {
    type Error = ();

    fn read(&mut self) -> nb::Result<u8, Self::Error> {
        if let Some(word) = self.rx.dequeue() {
            Ok(word)
        } else {
            Err(nb::Error::WouldBlock)
        }
    }
}

#[entry]
fn main() -> ! {
    // Get access to the device specific peripherals from the peripheral access crate
    let p = pac::Peripherals::take().unwrap();

    // Take ownership over the raw flash and rcc devices and convert them into the corresponding
    // HAL structs
    let mut flash = p.FLASH.constrain();
    let rcc = p.RCC.constrain();

    // Freeze the configuration of all the clocks in the system and store the frozen frequencies in
    // `clocks`
    // let clocks = rcc.cfgr.freeze(&mut flash.acr);
    let clocks = rcc
        .cfgr
        .use_hse(8.mhz())
        .sysclk(48.mhz())
        .pclk1(24.mhz())
        .freeze(&mut flash.acr);

    assert!(clocks.usbclk_valid());

    // Prepare the alternate function I/O registers
    let mut afio = p.AFIO.constrain();

    // Prepare the GPIOB peripheral
    let mut gpiob = p.GPIOB.split();

    // USART3
    // Configure pb10 as a push_pull output, this will be the tx pin
    let tx = gpiob.pb10.into_alternate_push_pull(&mut gpiob.crh);
    // Take ownership over pb11
    let rx = gpiob.pb11;

    // Set up the usart device. Taks ownership over the USART register and tx/rx pins. The rest of
    // the registers are used to enable and configure the device.
    let serial = Serial::usart3(
        p.USART3,
        (tx, rx),
        &mut afio.mapr,
        Config::default().baudrate(9600.bps()),
        clocks,
    );

    let (tx, mut rx) = serial.split();

    static mut QUEUE: Queue<u8, 16> = Queue::new();

    let (rx_producer, rx_consumer) = unsafe { QUEUE.split() };

    rx.listen();

    cortex_m::interrupt::free(|_| unsafe {
        RX.replace(rx);
        RX_PRODUCER.replace(rx_producer);
    });

    unsafe {
        cortex_m::peripheral::NVIC::unmask(pac::Interrupt::USART3);
    }

    let mut io = IO::new(SerialWrapper::new(tx, rx_consumer));

    let prompt = "> ";
    let mut editor: Editor<StaticBuffer<128>, _> = loop {
        match Editor::new(&mut io) {
            Ok(editor) => break editor,
            Err(err) => {
                let error = match err {
                    Error::ParserError => "ParserError",
                    Error::Aborted => "Aborted",
                    Error::ReadError(_) => "ReadError",
                    Error::WriteError(_) => "WriteError",
                };

                writeln!(io, "Failed: {}\r", error).unwrap();

                loop {
                    asm::nop();
                }
            }
        }
    };

    loop {
        match editor.readline(prompt, &mut io) {
            Ok(s) => {
                writeln!(io, "Echo: {}\r", s).unwrap();
            }
            Err(err) => {
                let error = match err {
                    Error::ParserError => "ParserError",
                    Error::Aborted => "Aborted",
                    Error::ReadError(_) => "ReadError",
                    Error::WriteError(_) => "WriteError",
                };

                writeln!(io, "Failed: {}\r", error).unwrap();
            }
        }
    }
}

#[interrupt]
unsafe fn USART3() {
    cortex_m::interrupt::free(|_| {
        if let Some(rx) = RX.as_mut() {
            if rx.is_rx_not_empty() {
                if let Ok(w) = nb::block!(rx.read()) {
                    if let Some(producer) = RX_PRODUCER.as_mut() {
                        let _ = producer.enqueue(w);
                    }
                }
            }
        }
    })
}
