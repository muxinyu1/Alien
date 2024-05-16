use alloc::sync::Arc;
use constants::DeviceId;
use device_interface::SoundDevice;
use spin::Once;
use vfscore::{file::VfsFile, inode::VfsInode, utils::VfsNodeType};
use vfscore::inode::InodeAttr;
use vfscore::VfsResult;
use vfscore::utils::VfsFileStat;

pub static SOUND_DEVICE: Once<Arc<dyn SoundDevice>> = Once::new();

pub fn init_sound(sound: Arc<dyn SoundDevice>) {
    SOUND_DEVICE.call_once(|| sound);
}

pub struct SOUNDDevice {
    device_id: DeviceId,
    device: Arc<dyn SoundDevice>
}

impl SOUNDDevice {
    pub fn new(device_id: DeviceId, device: Arc<dyn SoundDevice>) -> Self {
        Self { device_id, device }
    }
    pub fn device_id(&self) -> DeviceId {
        self.device_id
    }
}

impl VfsFile for SOUNDDevice {

}

impl VfsInode for SOUNDDevice {
    fn inode_type(&self) -> vfscore::utils::VfsNodeType {
        VfsNodeType::CharDevice
    }
    fn set_attr(&self, _attr: InodeAttr) -> VfsResult<()> {
        Ok(())
    }
    fn get_attr(&self) -> VfsResult<VfsFileStat> {
        Ok(VfsFileStat {
            st_rdev: self.device_id.id(),
            ..Default::default()
        })
    }
}