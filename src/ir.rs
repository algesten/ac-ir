use std::fs::File;
use std::io;
use std::io::Error;
use std::os::fd::AsRawFd;
use std::time::Duration;
use std::time::Instant;

use crate::{Fan, Mode, Power, Settings};

// Stuff to avoid having libc crate

#[cfg(unix)]
unsafe extern "C" {
    fn ioctl(fd: i32, request: u64, ...) -> i32;
}

#[cfg(unix)]
#[allow(non_camel_case_types)]
type c_void = std::ffi::c_void;

// GPIO ioctl definitions
const GPIO_GET_LINEHANDLE_IOCTL: u32 = 0xc040b403;
const GPIOHANDLE_SET_LINE_VALUES_IOCTL: u32 = 0xc040b409;

#[repr(C)]
struct GpioHandleRequest {
    lineoffsets: [u32; 64],
    flags: u32,
    default_values: [u32; 64],
    consumer_label: [u8; 32],
    lines: u32,
    fd: i32,
}

#[repr(C)]
struct GpioHandleData {
    values: [u32; 64],
}

// HVAC timing constants
const HVAC_MITSUBISHI_HDR_MARK: Duration = Duration::from_micros(3400);
const HVAC_MITSUBISHI_HDR_SPACE: Duration = Duration::from_micros(1750);
const HVAC_MITSUBISHI_BIT_MARK: Duration = Duration::from_micros(340);
const HVAC_MITSUBISHI_ONE_SPACE: Duration = Duration::from_micros(1300);
const HVAC_MITSUBISHI_ZERO_SPACE: Duration = Duration::from_micros(420);
const HVAC_MITSUBISHI_RPT_MARK: Duration = Duration::from_micros(440);
const HVAC_MITSUBISHI_RPT_SPACE: Duration = Duration::from_micros(17100);

#[repr(C)]
pub union Payload {
    data: [u8; 18],
    fields: PayloadFields,
}

#[derive(Copy, Clone)]
#[repr(C)]
struct PayloadFields {
    magic: [u8; 5],
    onoff: u8,
    hvac_mode: u8,
    temperature: u8,
    hvac_mode2: u8,
    fan_speed: u8,
    clock: u8,
    endclock: u8,
    startclock: u8,
    progmode: u8,
    zero: [u8; 3],
    checksum: u8,
}

fn set_line(fd: &File, value: u32, delay: Duration, target: &mut Instant) -> io::Result<()> {
    let mut data = GpioHandleData {
        values: [value; 64],
    };

    unsafe {
        if ioctl(
            fd.as_raw_fd(),
            GPIOHANDLE_SET_LINE_VALUES_IOCTL as u64,
            &mut data as *mut _ as *mut c_void,
        ) < 0
        {
            return Err(Error::last_os_error());
        }
    }

    *target += delay;
    let now = Instant::now();
    if now < *target {
        std::thread::sleep(*target - now);
    }
    Ok(())
}

fn send_byte(fd: &File, byte: u8, target: &mut Instant) -> io::Result<()> {
    println!("byte:{:x}", byte);
    for i in 0..8 {
        set_line(fd, 1, HVAC_MITSUBISHI_BIT_MARK, target)?;
        set_line(
            fd,
            0,
            if (1 << i) & byte != 0 {
                HVAC_MITSUBISHI_ONE_SPACE
            } else {
                HVAC_MITSUBISHI_ZERO_SPACE
            },
            target,
        )?;
    }
    Ok(())
}

fn send_msg(fd: &File, msg: &[u8; 18], target: &mut Instant) -> io::Result<()> {
    for &byte in msg.iter() {
        send_byte(fd, byte, target)?;
    }
    Ok(())
}

// Vane
// 0x48
// 0x50
// 0x58
// 0x60
// 0x68
// 0x78                    0b0111_1000
// 0x80                    0b1000_0000

impl From<Settings> for Payload {
    fn from(settings: Settings) -> Self {
        let mut p = Payload { data: [0; 18] };

        p.fields.magic = [0x23, 0xcb, 0x26, 0x01, 0x00];

        p.fields.onoff = match settings.power {
            Power::On => 0x20,
            Power::Off => 0x00,
        };

        let (m1, m2) = match settings.mode {
            Mode::Heat => (0x08, 0x30),
            Mode::Dry => (0x10, 0x32),
            Mode::Cool => (0x18, 0x36),
            Mode::Fan => (0x38, 0x30),
        };

        p.fields.hvac_mode = m1;
        p.fields.hvac_mode2 = m2;

        p.fields.temperature = settings.temp - 16;

        p.fields.fan_speed = match settings.fan {
            Fan::Auto => 0xb8,
            Fan::Low => 0x79,
            Fan::Medium => 0x7a,
            Fan::High => 0x7b,
            Fan::Higher => 0x7c,
        };

        // Calculate checksum
        let mut acc: u8 = 0;
        for i in 0..17 {
            acc += unsafe { p.data[i] };
        }
        p.fields.checksum = acc;

        p
    }
}

pub fn send_settings(settings: impl Into<Payload>) -> io::Result<()> {
    let fd = File::open("/dev/gpiochip0")?;

    // Request GPIO line
    let mut req = GpioHandleRequest {
        lineoffsets: [4; 64],
        default_values: [0; 64],
        lines: 1,
        flags: 0x00000001, // GPIOHANDLE_REQUEST_OUTPUT
        consumer_label: [0; 32],
        fd: 0,
    };

    let consumer_label = b"AC\0";
    req.consumer_label[..consumer_label.len()].copy_from_slice(consumer_label);

    unsafe {
        if ioctl(
            fd.as_raw_fd(),
            GPIO_GET_LINEHANDLE_IOCTL as u64,
            &mut req as *mut _ as *mut c_void,
        ) < 0
        {
            return Err(Error::last_os_error());
        }
    }

    let line_fd = File::open(format!("/dev/gpiochip{}", req.fd))?;

    let p: Payload = settings.into();

    let mut target = Instant::now();

    // Send IR signal
    set_line(&line_fd, 1, HVAC_MITSUBISHI_HDR_MARK, &mut target)?;
    set_line(&line_fd, 0, HVAC_MITSUBISHI_HDR_SPACE, &mut target)?;

    send_msg(&line_fd, unsafe { &p.data }, &mut target)?;

    set_line(&line_fd, 1, HVAC_MITSUBISHI_RPT_MARK, &mut target)?;
    set_line(&line_fd, 0, HVAC_MITSUBISHI_RPT_SPACE, &mut target)?;

    set_line(&line_fd, 1, HVAC_MITSUBISHI_HDR_MARK, &mut target)?;
    set_line(&line_fd, 0, HVAC_MITSUBISHI_HDR_SPACE, &mut target)?;

    send_msg(&line_fd, unsafe { &p.data }, &mut target)?;

    Ok(())
}
