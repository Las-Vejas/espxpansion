#![no_std]
extern crate alloc;

use xpanse_api::{
    bus::{
        allocator::BusAllocator,
        uart::{UartBusHandle, UartBusVersion, UartError},
    },
    driver::{Driver, DriverError, DriverMeta},
    gpio_bank::{BankPins, GpioBank},
    metadata::{ModuleDetectResistor, ModuleID, ModuleSlot},
    registry::Registry,
    reexports::{
        embassy_rp::gpio::{Flex, Pull},
        embassy_time::{Duration, Timer},
        embedded_io_async,
    },
};

pub const DEFAULT_BAUD_RATE: u32 = 115_200;

/// How long EN is held low to reset the chip.
const RESET_PULSE: Duration = Duration::from_millis(20);

const BOOT_SETTLE: Duration = Duration::from_millis(100);

pub struct EspxpansionDriver;

impl DriverMeta for EspxpansionDriver {
    const ID: ModuleID = ModuleID {
        md0: ModuleDetectResistor::R1K,
        md1: ModuleDetectResistor::R20K,
    };
}

impl<G: BankPins> Driver<G> for EspxpansionDriver {
    async fn create(
        bank: GpioBank<G>,
        slot: ModuleSlot,
        registry: &mut Registry,
        buses: &mut BusAllocator,
    ) -> Result<(), DriverError> {
        let mut en = Flex::new(bank.gpio0);
        release(&mut en);
        let mut boot = Flex::new(bank.gpio9);
        release(&mut boot);

        let mut io1 = Flex::new(bank.gpio1);
        io1.set_pull(Pull::None);
        io1.set_as_input();

        let uart = buses
            .create_uart_no_hardware(bank.gpio5, bank.gpio6, DEFAULT_BAUD_RATE)
            .map_err(|_| DriverError::InitFailed)?;

        registry.register(
            slot,
            Self::ID,
            Esp32C6 {
                uart,
                en,
                boot,
                io1,
            },
        );
        Ok(())
    }
}

pub struct Esp32C6 {
    uart: UartBusHandle,
    en: Flex<'static>,
    boot: Flex<'static>,
    io1: Flex<'static>,
}

impl Esp32C6 {
    pub async fn reset(&mut self) {
        release(&mut self.boot);
        self.pulse_reset().await;
    }

    pub async fn enter_bootloader(&mut self) {
        pull_low(&mut self.boot);
        self.pulse_reset().await;
        release(&mut self.boot);
    }

    pub fn hold_reset(&mut self) {
        pull_low(&mut self.en);
    }
    pub fn release_reset(&mut self) {
        release(&mut self.en);
    }
    pub fn in_reset(&self) -> bool {
        self.en.is_low()
    }
    pub fn io1(&mut self) -> &mut Flex<'static> {
        &mut self.io1
    }

    pub fn uart_version(&self) -> UartBusVersion {
        self.uart.version()
    }

    async fn pulse_reset(&mut self) {
        pull_low(&mut self.en);
        Timer::after(RESET_PULSE).await;
        release(&mut self.en);
        Timer::after(BOOT_SETTLE).await;
    }
}

impl embedded_io_async::ErrorType for Esp32C6 {
    type Error = UartError;
}

impl embedded_io_async::Read for Esp32C6 {
    async fn read(&mut self, buf: &mut [u8]) -> Result<usize, Self::Error> {
        self.uart.read(buf).await
    }
}

impl embedded_io_async::Write for Esp32C6 {
    async fn write(&mut self, buf: &[u8]) -> Result<usize, Self::Error> {
        self.uart.write(buf).await
    }

    async fn flush(&mut self) -> Result<(), Self::Error> {
        self.uart.flush().await
    }
}

fn pull_low(pin: &mut Flex<'static>) {
    pin.set_low();
    pin.set_as_output();
}

fn release(pin: &mut Flex<'static>) {
    pin.set_pull(Pull::None);
    pin.set_as_input();
}