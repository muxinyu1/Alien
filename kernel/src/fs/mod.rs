use alloc::string::{String, ToString};
use alloc::vec;
use core::cmp::min;

use rvfs::dentry::{LookUpFlags, vfs_rename, vfs_truncate, vfs_truncate_by_file};
use rvfs::file::{ 
    FileMode, OpenFlags, SeekFrom, vfs_close_file, vfs_llseek, vfs_mkdir,
    vfs_open_file, vfs_read_file, vfs_readdir, vfs_write_file,
};
use rvfs::inode::InodeMode;
use rvfs::link::{LinkFlags, vfs_link, vfs_readlink, vfs_symlink, vfs_unlink};
use rvfs::mount::MountFlags;
use rvfs::path::{ParsePathType, vfs_lookup_path};
use rvfs::stat::{
    KStat, StatFlags, vfs_getattr, vfs_getattr_by_file, vfs_getxattr,
    vfs_getxattr_by_file, vfs_listxattr, vfs_listxattr_by_file, vfs_removexattr,
    vfs_removexattr_by_file, vfs_setxattr, vfs_setxattr_by_file, vfs_statfs, vfs_statfs_by_file,
};
use rvfs::superblock::StatFs;

pub use stdio::*;
use syscall_table::syscall_func;

use crate::fs::vfs::VfsProvider;
use crate::task::current_task;

use rvfs::file::File;
use alloc::sync::Arc;
use crate::net::socket::Socket;

mod stdio;

pub mod vfs;

/// file + socket
#[derive(Debug)]
pub enum FileLike {
    NormalFile(Arc<File>),
    Socket(Arc<Socket>),
}

pub enum FileType {
    NormalFile,
    Socket,
}

impl FileLike {
    pub fn get_type(&self) -> FileType {
        match self {
            FileLike::NormalFile(_) => FileType::NormalFile,
            FileLike::Socket(_) => FileType::Socket,
        }
    }

    pub fn get_nf(&self) -> Option<Arc<File>> {
        match self {
            FileLike::NormalFile(nf) => Some(nf.clone()),
            FileLike::Socket(_) => panic!("get a socket file"),
        }
    }

    pub fn get_socket(&self) -> Option<Arc<Socket>> {
        match self {
            FileLike::NormalFile(_) => panic!("get a normal file when want a socket"),
            FileLike::Socket(s) => Some(s.clone()),
        }
    }
}

const AT_FDCWD: isize = -100isize;

#[syscall_func(40)]
pub fn sys_mount(
    special: *const u8,
    dir: *const u8,
    fs_type: *const u8,
    flags: usize,
    data: *const u8,
) -> isize {
    let process = current_task().unwrap();
    let special = process.transfer_str(special);
    let dir = process.transfer_str(dir);
    let fs_type = process.transfer_str(fs_type);
    let data = process.transfer_str(data);
    assert!(data.is_empty());
    let special = user_path_at(AT_FDCWD, &special, LookUpFlags::empty()).map_err(|_| -1);
    if special.is_err() {
        return -1;
    }
    let special = special.unwrap();
    let dir = user_path_at(AT_FDCWD, &dir, LookUpFlags::empty()).map_err(|_| -1);
    if dir.is_err() {
        return -1;
    }
    let dir = dir.unwrap();

    let flags = MountFlags::from_bits(flags as u32).unwrap();
    warn!(
        "mount special:{:?},dir:{:?},fs_type:{:?},flags:{:?},data:{:?}",
        special, dir, fs_type, flags, data
    );

    // now we return 0 directly
    // todo! rvfs need implement the devfs

    // let ret = do_mount::<VfsProvider>(&special, &dir, &fs_type, flags, None);
    // if ret.is_err() {
    //     return -1;
    // }
    0
}

#[syscall_func(39)]
pub fn sys_umount(dir: *const u8) -> isize {
    let process = current_task().unwrap();
    let dir = process.transfer_str(dir);
    let dir = user_path_at(AT_FDCWD, &dir, LookUpFlags::empty()).map_err(|_| -1);
    if dir.is_err() {
        return -1;
    }
    let dir = dir.unwrap();
    warn!("umount dir:{:?}", dir);
    // todo! rvfs need implement
    0
}

/// Reference: https://man7.org/linux/man-pages/man2/openat.2.html
#[syscall_func(56)]
pub fn sys_openat(dirfd: isize, path: usize, flag: usize, mode: usize) -> isize {
    // we don't support mode yet
    let file_mode = FileMode::from_bits_truncate(mode as u32);
    let flag = OpenFlags::from_bits(flag as u32).unwrap();
    let process = current_task().unwrap();
    let path = process.transfer_str(path as *const u8);
    let path = user_path_at(dirfd, &path, LookUpFlags::empty()).map_err(|_| -1);
    if path.is_err() {
        return -1;
    }
    let path = path.unwrap();
    warn!(
        "open file: {:?},flag:{:?}, mode:{:?}",
        path, flag, file_mode
    );
    let file = vfs_open_file::<VfsProvider>(&path, flag, file_mode);
    if file.is_err() {
        return -1;
    }
    let fd = process.add_file(Arc::new(FileLike::NormalFile(file.unwrap())));
    if fd.is_err() {
        -1
    } else {
        fd.unwrap() as isize
    }
}

#[syscall_func(57)]
pub fn sys_close(fd: usize) -> isize {
    let process = current_task().unwrap();
    let file = process.remove_file(fd);
    if file.is_err() {
        return -1;
    }
    let file = file.unwrap();
    let _ = vfs_close_file::<VfsProvider>(file);
    0
}

#[syscall_func(61)]
pub fn sys_getdents(fd: usize, buf: *mut u8, len: usize) -> isize {
    let process = current_task().unwrap();
    let file = process.get_file(fd);
    if file.is_none() {
        return -1;
    }
    let file = file.unwrap();
    let user_bufs = process.transfer_raw_buffer(buf, len);
    let mut buf = vec![0u8; len];
    let res = vfs_readdir(file, buf.as_mut_slice());
    if res.is_err() {
        return -1;
    }
    let mut offset = 0;
    // copy dirent_buf to user space
    for user_buf in user_bufs {
        let copy_len = user_buf.len(); // user_bufs len is equal to buf len
        user_buf.copy_from_slice(&buf[offset..offset + copy_len]);
        offset += copy_len;
    }
    res.unwrap() as isize
}

/// Reference: https://man7.org/linux/man-pages/man2/truncate64.2.html
#[syscall_func(45)]
pub fn sys_truncate(path: usize, len: usize) -> isize {
    let process = current_task().unwrap();
    let path = process.transfer_str(path as *const u8);
    let res = vfs_truncate::<VfsProvider>(&path, len);
    if res.is_err() {
        return -1;
    }
    0
}

#[syscall_func(46)]
pub fn sys_ftruncate(fd: usize, len: usize) -> isize {
    let process = current_task().unwrap();
    let file = process.get_file(fd);
    if file.is_none() {
        return -1;
    }
    let file = file.unwrap();
    let res = vfs_truncate_by_file(file, len);
    if res.is_err() {
        return -1;
    }
    0
}

#[syscall_func(63)]
pub fn sys_read(fd: usize, buf: *mut u8, len: usize) -> isize {
    let process = current_task().unwrap();
    let file = process.get_file(fd);
    if file.is_none() {
        return -1;
    }
    let file = file.unwrap();
    let mut buf = process.transfer_raw_buffer(buf, len);
    let mut count = 0;
    let mut offset = file.access_inner().f_pos;
    buf.iter_mut().for_each(|b| {
        let r = vfs_read_file::<VfsProvider>(file.clone(), b, offset as u64).unwrap();
        count += r;
        offset += r;
    });
    count as isize
}

#[syscall_func(64)]
pub fn sys_write(fd: usize, buf: *const u8, len: usize) -> isize {
    // warn!("sys_write is not implemented yet");
    let process = current_task().unwrap();
    let file = process.get_file(fd);
    if file.is_none() {
        return -1;
    }
    let file = file.unwrap();
    let mut buf = process.transfer_raw_buffer(buf, len);
    let mut count = 0;
    let mut offset = file.access_inner().f_pos;
    buf.iter_mut().for_each(|b| {
        // warn!("write file: {:?}, offset:{:?}, len:{:?}", fd, offset, b.len());
        let r = vfs_write_file::<VfsProvider>(file.clone(), b, offset as u64).unwrap();
        count += r;
        offset += r;
    });
    count as isize
}

#[syscall_func(17)]
pub fn sys_getcwd(buf: *mut u8, len: usize) -> isize {
    assert!(!buf.is_null());
    let process = current_task().unwrap();
    let cwd = process.access_inner().cwd();

    let path = vfs_lookup_path(
        cwd.cwd.clone(),
        cwd.cmnt.clone(),
        ParsePathType::Relative("".to_string()),
        LookUpFlags::empty(),
    )
        .unwrap();

    let mut buf = process.transfer_raw_buffer(buf, len);
    let mut count = 0;
    let mut cwd = path.as_bytes();
    buf.iter_mut().for_each(|buf| {
        // fill buf
        if !cwd.is_empty() {
            let min = min(cwd.len(), buf.len());
            buf[..min].copy_from_slice(&cwd[..min]);
            count += min;
            cwd = &cwd[min..];
        }
    });
    buf[0].as_ptr() as isize
}

#[syscall_func(49)]
pub fn sys_chdir(path: *const u8) -> isize {
    let process = current_task().unwrap();
    let path = process.transfer_str(path);
    let file = vfs_open_file::<VfsProvider>(
        path.as_str(),
        OpenFlags::O_RDWR | OpenFlags::O_DIRECTORY,
        FileMode::FMODE_READ,
    );
    if file.is_err() {
        return -1;
    }
    let file = file.unwrap();

    if file.f_dentry.access_inner().d_inode.mode != InodeMode::S_DIR {
        return -1;
    }
    process.access_inner().fs_info.cwd = file.f_dentry.clone();
    process.access_inner().fs_info.cmnt = file.f_mnt.clone();
    0
}

#[syscall_func(83)]
pub fn sys_mkdir(path: *const u8) -> isize {
    info!("sys_mkdir");
    let process = current_task().unwrap();
    let path = process.transfer_str(path);
    let file = vfs_mkdir::<VfsProvider>(&path, FileMode::FMODE_WRITE);
    if file.is_err() {
        return -1;
    }
    0
}

#[syscall_func(62)]
pub fn sys_lseek(fd: usize, offset: isize, whence: usize) -> isize {
    let process = current_task().unwrap();
    let file = process.get_file(fd);
    if file.is_none() {
        return -1;
    }
    let file = file.unwrap();
    let seek = SeekFrom::from((whence, offset as usize));
    let res = vfs_llseek(file, seek);
    if res.is_err() {
        return -1;
    }
    0
}

#[syscall_func(80)]
pub fn sys_fstat(fd: usize, stat: *mut u8) -> isize {
    assert!(!stat.is_null());
    let process = current_task().unwrap();
    let file = process.get_file(fd);
    if file.is_none() {
        return -1;
    }
    let file = file.unwrap();
    let stat = stat as *mut KStat;
    let stat = process.transfer_raw_ptr(stat);
    let attr = vfs_getattr_by_file(file);
    if attr.is_err() {
        return -1;
    }
    let attr = attr.unwrap();
    *stat = attr;
    0
}

/// If the pathname given in oldpath is relative, then it is interpreted relative to
/// the directory referred to by the file descriptor olddirfd (rather than relative
/// to the current working directory of the calling process, as is done by link(2) for a relative pathname).
/// If oldpath is relative and olddirfd is the special value AT_FDCWD, then oldpath
/// is interpreted relative to the current working directory of the calling process (like link(2)).
/// If oldpath is absolute, then olddirfd is ignored.
///
/// The interpretation of newpath is as for oldpath, except that a relative pathname is interpreted relative to the directory referred to by the file descriptor newdirfd.
#[syscall_func(37)]
pub fn sys_linkat(
    old_fd: isize,
    old_name: *const u8,
    new_fd: isize,
    new_name: *const u8,
    flag: usize,
) -> isize {
    let flag = LinkFlags::from_bits(flag as u32);
    if flag.is_none() {
        return -1;
    }
    let flag = flag.unwrap();
    let flag = flag - LinkFlags::AT_SYMLINK_FOLLOW - LinkFlags::AT_EMPTY_PATH;
    if !flag.is_empty() {
        warn!("sys_linkat: flag is not empty");
        return -1;
    }
    // we try to find the old path according to the old_fd and old_name and flag
    let mut lookup_flag = LookUpFlags::empty();
    if flag.contains(LinkFlags::AT_SYMLINK_FOLLOW) {
        lookup_flag |= LookUpFlags::READ_LINK;
    }
    if flag.contains(LinkFlags::AT_EMPTY_PATH) {
        lookup_flag |= LookUpFlags::EMPTY;
    }
    let process = current_task().unwrap();
    let old_name = process.transfer_str(old_name);
    let old_path = user_path_at(old_fd, &old_name, lookup_flag).map_err(|_| -1);
    if old_path.is_err() {
        return -1;
    }
    let new_name = process.transfer_str(new_name);
    let new_path = user_path_at(new_fd, &new_name, lookup_flag).map_err(|_| -1);
    if new_path.is_err() {
        return -1;
    }
    let old_path = old_path.unwrap();
    let new_path = new_path.unwrap();
    warn!("old_path: {},new_path: {}", old_path, new_path);
    let res = vfs_link::<VfsProvider>(old_path.as_str(), new_path.as_str());
    if res.is_err() {
        return -1;
    }
    0
}

#[syscall_func(35)]
pub fn sys_unlinkat(fd: isize, path: *const u8, flag: usize) -> isize {
    assert_eq!(flag, 0);
    let process = current_task().unwrap();
    let path = process.transfer_str(path);
    let path = user_path_at(fd, &path, LookUpFlags::empty()).map_err(|_| -1);
    if path.is_err() {
        return -1;
    }
    // TODO we need make sure the file of the path is not being used
    let path = path.unwrap();
    let res = vfs_unlink::<VfsProvider>(path.as_str());
    if res.is_err() {
        return -1;
    }
    0
}

#[syscall_func(36)]
pub fn sys_symlinkat(old_name: *const u8, new_fd: isize, new_name: *const u8) -> isize {
    let process = current_task().unwrap();
    let old_name = process.transfer_str(old_name);
    let new_name = process.transfer_str(new_name);
    let new_path = user_path_at(new_fd, &new_name, LookUpFlags::empty()).map_err(|_| -1);
    if new_path.is_err() {
        return -1;
    }
    let new_path = new_path.unwrap();
    let res = vfs_symlink::<VfsProvider>(old_name.as_str(), new_path.as_str());
    if res.is_err() {
        return -1;
    }
    0
}

#[syscall_func(78)]
pub fn sys_readlinkat(fd: isize, path: *const u8, buf: *mut u8, size: usize) -> isize {
    let process = current_task().unwrap();
    let path = process.transfer_str(path);
    let path = user_path_at(fd, &path, LookUpFlags::empty()).map_err(|_| -1);
    if path.is_err() {
        return -1;
    }
    let path = path.unwrap();
    let mut buf = process.transfer_raw_buffer(buf, size);

    println!("readlink path: {}", path);
    let res = vfs_readlink::<VfsProvider>(path.as_str(), buf[0]);
    if res.is_err() {
        return -1;
    }
    let res = res.unwrap();
    res as isize
}

/// Reference: https://man7.org/linux/man-pages/man2/newfstatat.2.html
#[syscall_func(79)]
pub fn sys_fstateat(dir_fd: isize, path: *const u8, stat: *mut u8, flag: usize) -> isize {
    let process = current_task().unwrap();
    let path = process.transfer_str(path);
    let path = user_path_at(dir_fd, &path, LookUpFlags::empty()).map_err(|_| -1);
    if path.is_err() {
        return -1;
    }
    let path = path.unwrap();
    let stat = stat as *mut KStat;
    let stat = process.transfer_raw_ptr(stat);
    let flag = StatFlags::from_bits(flag as u32);
    if flag.is_none() {
        return -1;
    }
    let flag = flag.unwrap();
    let res = vfs_getattr::<VfsProvider>(path.as_str(), flag);
    if res.is_err() {
        return -1;
    }
    let res = res.unwrap();
    *stat = res;
    0
}

/// Reference: https://man7.org/linux/man-pages/man2/fstatfs64.2.html
#[syscall_func(44)]
pub fn sys_fstatfs(fd: isize, buf: *mut u8) -> isize {
    let process = current_task().unwrap();
    let buf = buf as *mut StatFs;
    let buf = process.transfer_raw_ptr(buf);
    let file = process.get_file(fd as usize);
    if file.is_none() {
        return -1;
    }
    let file = file.unwrap();
    let res = vfs_statfs_by_file(file);
    if res.is_err() {
        return -1;
    }
    let res = res.unwrap();
    *buf = res;
    0
}

#[syscall_func(43)]
pub fn sys_statfs(path: *const u8, statfs: *const u8) -> isize {
    let process = current_task().unwrap();
    let buf = statfs as *mut StatFs;
    let buf = process.transfer_raw_ptr(buf);
    let path = process.transfer_str(path);

    warn!("statfs path: {}", path);

    let res = vfs_statfs::<VfsProvider>(path.as_str());
    if res.is_err() {
        return -1;
    }
    let res = res.unwrap();
    *buf = res;
    0
}

/// Reference: https://man7.org/linux/man-pages/man2/renameat.2.html
#[syscall_func(38)]
pub fn sys_renameat(
    old_dirfd: isize,
    old_path: *const u8,
    new_dirfd: isize,
    new_path: *const u8,
) -> isize {
    let process = current_task().unwrap();
    let old_path = process.transfer_str(old_path);
    let new_path = process.transfer_str(new_path);
    let old_path = user_path_at(old_dirfd, &old_path, LookUpFlags::empty()).map_err(|_| -1);
    if old_path.is_err() {
        return -1;
    }
    let old_path = old_path.unwrap();
    let new_path = user_path_at(new_dirfd, &new_path, LookUpFlags::empty()).map_err(|_| -1);
    if new_path.is_err() {
        return -1;
    }
    let new_path = new_path.unwrap();
    let res = vfs_rename::<VfsProvider>(old_path.as_str(), new_path.as_str());
    if res.is_err() {
        return -1;
    }
    0
}

/// Reference: https://man7.org/linux/man-pages/man2/mkdirat.2.html
#[syscall_func(34)]
pub fn sys_mkdirat(dirfd: isize, path: *const u8, flag: usize) -> isize {
    let process = current_task().unwrap();
    let path = process.transfer_str(path);
    let path = user_path_at(dirfd, &path, LookUpFlags::empty()).map_err(|_| -1);
    if path.is_err() {
        return -1;
    }
    let path = path.unwrap();
    let flag = OpenFlags::from_bits_truncate(flag as u32);
    warn!("mkdirat path: {}, flag: {:?}", path, flag);
    let mut mode = FileMode::FMODE_READ;
    if flag.contains(OpenFlags::O_WRONLY) {
        mode |= FileMode::FMODE_WRITE;
    }
    let res = vfs_mkdir::<VfsProvider>(path.as_str(), mode);
    if res.is_err() {
        return -1;
    }
    0
}

/// Reference: https://man7.org/linux/man-pages/man2/setxattr.2.html
#[syscall_func(5)]
pub fn sys_setxattr(
    path: *const u8,
    name: *const u8,
    value: *const u8,
    size: usize,
    flag: usize,
) -> isize {
    // we ignore flag
    assert_eq!(flag, 0);
    let process = current_task().unwrap();
    let path = process.transfer_str(path);
    let name = process.transfer_str(name);
    let value = process.transfer_raw_buffer(value, size);
    let res = vfs_setxattr::<VfsProvider>(path.as_str(), name.as_str(), value[0]);
    if res.is_err() {
        return -1;
    }
    0
}

/// The difference between lsetxattr and setxattr is that lsetxattr will not follow the symbolic link
#[syscall_func(6)]
pub fn sys_lsetxattr(
    path: *const u8,
    name: *const u8,
    value: *const u8,
    size: usize,
    flag: usize,
) -> isize {
    sys_setxattr(path, name, value, size, flag)
}

#[syscall_func(7)]
pub fn sys_fsetxattr(
    fd: usize,
    name: *const u8,
    value: *const u8,
    size: usize,
    flag: usize,
) -> isize {
    // we ignore flag
    assert_eq!(flag, 0);
    let process = current_task().unwrap();
    let name = process.transfer_str(name);
    let value = process.transfer_raw_buffer(value, size);
    let file = process.get_file(fd);
    if file.is_none() {
        return -1;
    }
    let file = file.unwrap();
    let res = vfs_setxattr_by_file(file, name.as_str(), value[0]);
    if res.is_err() {
        return -1;
    }
    0
}

/// Reference: https://man7.org/linux/man-pages/man2/getxattr.2.html
#[syscall_func(8)]
pub fn sys_getxattr(path: *const u8, name: *const u8, value: *const u8, size: usize) -> isize {
    let process = current_task().unwrap();
    let path = process.transfer_str(path);
    let name = process.transfer_str(name);
    let mut value = process.transfer_raw_buffer(value, size);
    // assert_eq!(value.len(),1);
    if value.is_empty() {
        value.push(&mut [0u8; 0])
    }
    let res = vfs_getxattr::<VfsProvider>(path.as_str(), name.as_str(), value[0]);
    if res.is_err() {
        return -1;
    }
    let res = res.unwrap();
    res as isize
}

#[syscall_func(9)]
pub fn sys_lgetxattr(path: *const u8, name: *const u8, value: *const u8, size: usize) -> isize {
    sys_getxattr(path, name, value, size)
}

#[syscall_func(10)]
pub fn sys_fgetxattr(fd: usize, name: *const u8, value: *const u8, size: usize) -> isize {
    let process = current_task().unwrap();
    let name = process.transfer_str(name);
    let mut value = process.transfer_raw_buffer(value, size);
    // assert_eq!(value.len(),1);
    if value.is_empty() {
        value.push(&mut [0u8; 0])
    }
    let file = process.get_file(fd);
    if file.is_none() {
        return -1;
    }
    let file = file.unwrap();
    let res = vfs_getxattr_by_file(file, name.as_str(), value[0]);
    if res.is_err() {
        return -1;
    }
    let res = res.unwrap();
    res as isize
}

/// Reference: https://man7.org/linux/man-pages/man2/listxattr.2.html
#[syscall_func(11)]
pub fn sys_listxattr(path: *const u8, list: *const u8, size: usize) -> isize {
    let process = current_task().unwrap();
    let path = process.transfer_str(path);
    let mut list = process.transfer_raw_buffer(list, size);
    if list.is_empty() {
        list.push(&mut [0u8; 0])
    }
    let res = vfs_listxattr::<VfsProvider>(path.as_str(), list[0]);
    if res.is_err() {
        return -1;
    }
    let res = res.unwrap();
    res as isize
}

#[syscall_func(12)]
pub fn sys_llistxattr(path: *const u8, list: *const u8, size: usize) -> isize {
    sys_listxattr(path, list, size)
}

#[syscall_func(13)]
pub fn sys_flistxattr(fd: usize, list: *const u8, size: usize) -> isize {
    let process = current_task().unwrap();
    let mut list = process.transfer_raw_buffer(list, size);
    if list.is_empty() {
        list.push(&mut [0u8; 0])
    }
    let file = process.get_file(fd);
    if file.is_none() {
        return -1;
    }
    let file = file.unwrap();
    let res = vfs_listxattr_by_file(file, list[0]);
    if res.is_err() {
        return -1;
    }
    let res = res.unwrap();
    res as isize
}

/// Reference: https://man7.org/linux/man-pages/man2/removexattr.2.html
#[syscall_func(14)]
pub fn sys_removexattr(path: *const u8, name: *const u8) -> isize {
    let process = current_task().unwrap();
    let path = process.transfer_str(path);
    let name = process.transfer_str(name);
    let res = vfs_removexattr::<VfsProvider>(path.as_str(), name.as_str());
    if res.is_err() {
        return -1;
    }
    0
}

#[syscall_func(15)]
pub fn sys_lremovexattr(path: *const u8, name: *const u8) -> isize {
    sys_removexattr(path, name)
}

#[syscall_func(16)]
pub fn sys_fremovexattr(fd: usize, name: *const u8) -> isize {
    let process = current_task().unwrap();
    let name = process.transfer_str(name);
    let file = process.get_file(fd);
    if file.is_none() {
        return -1;
    }
    let file = file.unwrap();
    let res = vfs_removexattr_by_file(file, name.as_str());
    if res.is_err() {
        return -1;
    }
    0
}

fn user_path_at(fd: isize, path: &str, flag: LookUpFlags) -> Result<String, ()> {
    let process = current_task().unwrap();
    let path = ParsePathType::from(path);
    let res = if path.is_relative() {
        if fd == AT_FDCWD {
            let fs_context = process.access_inner().fs_info.clone();
            vfs_lookup_path(fs_context.cwd, fs_context.cmnt, path, flag).map_err(|_| ())
        } else {
            let fd = fd as usize;
            let file = process.get_file(fd);
            if file.is_none() {
                return Err(());
            }
            let file = file.unwrap();
            vfs_lookup_path(file.f_dentry.clone(), file.f_mnt.clone(), path, flag).map_err(|_| ())
        }
    } else {
        Ok(path.path())
    };
    res
}
