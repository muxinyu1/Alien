use alloc::vec::Vec;
use constants::{AlienError, AlienResult};
use device_interface::{DeviceBase, SoundDevice};
use ksync::Mutex;
use virtio_drivers::{device::sound::{PcmFeatures, PcmFormats, VirtIOSound}, transport::mmio::MmioTransport};

use crate::hal::HalImpl;

pub struct VirtIOSoundWrapper {
    sound: Mutex<VirtIOSound<HalImpl, MmioTransport>>,
}

unsafe impl Sync for VirtIOSoundWrapper {}

unsafe impl Send for VirtIOSoundWrapper {}

impl VirtIOSoundWrapper {
    pub fn from_mmio(mmio: MmioTransport) -> Self {
        let sound = VirtIOSound::new(mmio).unwrap();
        Self::__new(sound)
    }

    fn __new(sound: VirtIOSound<HalImpl, MmioTransport>) -> Self {
        Self {
            sound: Mutex::new(sound),
        }
    }
}

impl DeviceBase for VirtIOSoundWrapper {
    fn handle_irq(&self) {
        self.sound.lock().ack_interrupt();
    }
}

impl SoundDevice for VirtIOSoundWrapper {
    fn jack_remap(&self, jack_id: u32, association: u32, sequence: u32) -> bool {
        match self.sound.lock().jack_remap(jack_id, association, sequence) {
            Ok(_) => true,
            Err(_) => false,
        }
    }

    fn pcm_set_params(
        &mut self,
        stream_id: u32,
        buffer_bytes: u32,
        period_bytes: u32,
        features: u32,
        channels: u8,
        format: u64,
        rate: u64,
    ) -> bool {
        match self.sound.lock().pcm_set_params(
            stream_id,
            buffer_bytes,
            period_bytes,
            features.into(),
            channels,
            format.into(),
            rate.into(),
        ) {
            Ok(_) => true,
            Err(_) => false,
        }
    }

    fn pcm_prepare(&self, stream_id: u32) -> bool {
        match self.sound.lock().pcm_prepare(stream_id) {
            Ok(_) => true,
            Err(_) => false,
        }
    }

    fn pcm_release(&self, stream_id: u32) -> bool {
        match self.sound.lock().pcm_release(stream_id) {
            Ok(_) => true,
            Err(_) => false,
        }
    }

    fn pcm_start(&self, stream_id: u32) -> bool {
        match self.sound.lock().pcm_start(stream_id) {
            Ok(_) => true,
            Err(_) => false,
        }
    }

    fn pcm_stop(&self, stream_id: u32) -> bool {
        match self.sound.lock().pcm_stop(stream_id) {
            Ok(_) => true,
            Err(_) => false,
        }
    }

    fn pcm_xfer(&self, stream_id: u32, frames: &[u8]) -> bool {
        match self.sound.lock().pcm_xfer(stream_id, frames) {
            Ok(_) => true,
            Err(_) => false,
        }
    }

    fn pcm_xfer_nb(&self, stream_id: u32, frames: &[u8]) -> constants::AlienResult<u16> {
        match self.sound.lock().pcm_xfer_nb(stream_id, frames) {
            Ok(token) => Ok(token),
            Err(err) => Err(AlienError::EIO),
        }
    }

    fn pcm_xfer_ok(&self, token: u16) -> bool {
        match self.sound.lock().pcm_xfer_ok(token) {
            Ok(_) => true,
            Err(_) => false,
        }
    }

    fn output_streams(&self) -> Vec<u32> {
        self.sound.lock().output_streams()
    }

    fn input_streams(&self) -> Vec<u32> {
        self.sound.lock().input_streams()
    }

    fn rates_supported(&self, stream_id: u32) -> constants::AlienResult<u64> {
        match self.sound.lock().rates_supported(stream_id) {
            Ok(rate) => Ok(rate.bits()),
            Err(_) => Err(AlienError::EIO),
        }
    }
}
