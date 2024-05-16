#![no_std]

extern crate alloc;

use core::{any::Any, ops::RangeInclusive};
use alloc::vec::Vec;

use constants::{io::RtcTime, AlienResult};

pub trait DeviceBase: Sync + Send {
    fn handle_irq(&self);
}

pub trait BlockDevice: DeviceBase {
    fn read(&self, buf: &mut [u8], offset: usize) -> AlienResult<usize>;
    fn write(&self, buf: &[u8], offset: usize) -> AlienResult<usize>;
    fn size(&self) -> usize;
    fn flush(&self) -> AlienResult<()>;
}
pub trait LowBlockDevice {
    fn read_block(&self, block_id: usize, buf: &mut [u8]) -> AlienResult<()>;
    fn write_block(&self, block_id: usize, buf: &[u8]) -> AlienResult<()>;
    fn capacity(&self) -> usize;
    fn read_block_async(&self, block_id: usize, buf: &mut [u8]) -> AlienResult<()>;
    fn write_block_async(&self, block_id: usize, buf: &[u8]) -> AlienResult<()>;
    fn handle_irq(&self);
    fn flush(&self) {}
}

pub trait GpuDevice: Any + DeviceBase {
    fn update_cursor(&self);
    fn get_framebuffer(&self) -> &mut [u8];
    fn flush(&self);
    fn resolution(&self) -> (u32, u32);
}

pub trait InputDevice: DeviceBase {
    fn is_empty(&self) -> bool;
    fn read_event_async(&self) -> u64;
    fn read_event_without_block(&self) -> Option<u64>;
}

pub trait RtcDevice: DeviceBase {
    fn read_time(&self) -> RtcTime;
}

pub trait UartDevice: DeviceBase {
    fn put(&self, c: u8);
    fn get(&self) -> Option<u8>;
    fn put_bytes(&self, bytes: &[u8]);
    fn have_data_to_get(&self) -> bool;
    fn have_space_to_put(&self) -> bool;
}

pub trait SoundDevice: DeviceBase {
    fn jack_remap(&self, jack_id: u32, association: u32, sequence: u32) -> bool;
    fn pcm_set_params(
        &self,
        stream_id: u32,
        buffer_bytes: u32,
        period_bytes: u32,
        features: u32,
        channels: u8,
        format: u64,
        rate: u64,
    ) -> bool;
    fn pcm_prepare(&self, stream_id: u32) -> bool;
    fn pcm_release(&self, stream_id: u32) -> bool;
    fn pcm_start(&self, stream_id: u32) -> bool;
    fn pcm_stop(&self, stream_id: u32) -> bool;
    fn pcm_xfer(&self, stream_id: u32, frames: &[u8]) -> bool;
    fn pcm_xfer_nb(&self, stream_id: u32, frames: &[u8]) -> AlienResult<u16>;
    fn pcm_xfer_ok(&self, token: u16) -> bool;
    fn output_streams(&self) -> Vec<u32>;
    fn input_streams(&self) -> Vec<u32>;
    fn rates_supported(&self, stream_id: u32) -> AlienResult<u64>;
    fn formats_supported(&self, stream_id: u32) -> AlienResult<u64>;
    fn channel_range_supported(&self, stream_id: u32) -> AlienResult<RangeInclusive<u8>>;
    fn features_supported(&self, stream_id: u32) -> AlienResult<u32>;
}

pub trait NetDevice: DeviceBase {}
