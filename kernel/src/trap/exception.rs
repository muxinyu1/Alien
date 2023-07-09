use riscv::register::scause::{Exception, Trap};
use rvfs::file::vfs_read_file;

use crate::arch::interrupt_enable;
use crate::error::{AlienError, AlienResult};
use crate::fs::vfs::VfsProvider;
use crate::syscall;
use crate::task::{current_task, current_trap_frame, do_exit};

pub fn syscall_exception_handler() {
    // enable interrupt
    interrupt_enable();
    // jump to next instruction anyway
    let mut cx = current_trap_frame();
    cx.update_sepc();
    // get system call return value
    let parameters = cx.parameters();
    let syscall_name = syscall_define::syscall_name(parameters[0]);

    let p_name = current_task().unwrap().get_name();
    let tid = current_task().unwrap().get_tid();
    let pid = current_task().unwrap().get_pid();
    if !p_name.contains("shell") && !p_name.contains("init") && !p_name.contains("ls") {
        // ignore shell and init
        warn!(
            "[pid:{}, tid: {}][p_name: {}] syscall: [{}] {}({:#x}, {:#x}, {:#x}, {:#x}, {:#x}, {:#x})",
            pid,
            tid,
            p_name,
            parameters[0],
            syscall_name,
            parameters[1],
            parameters[2],
            parameters[3],
            parameters[4],
            parameters[5],
            parameters[6]
        );
    }

    let result = syscall::do_syscall(parameters[0], &parameters[1..]);
    if !p_name.contains("shell") && !p_name.contains("init") && !p_name.contains("ls") {
        warn!(
            "[pid:{}, tid: {}] syscall: [{}] result: {:?}",
            pid, tid, syscall_name, result
        );
    }

    if result.is_none() {
        error!("The syscall {} is not implemented!", syscall_name);
        do_exit(-1);
    }
    // cx is changed during sys_exec, so we have to call it again
    cx = current_trap_frame();
    cx.update_res(result.unwrap() as usize);
}

/// the solution for page fault
pub fn page_exception_handler(trap: Trap, addr: usize) -> AlienResult<()> {
    trace!(
        "[pid: {}] page fault addr:{:#x} trap:{:?}",
        current_task().unwrap().get_pid(),
        addr,
        trap
    );
    match trap {
        Trap::Exception(Exception::LoadPageFault) => load_page_fault_exception_handler(addr)?,
        Trap::Exception(Exception::StorePageFault) => store_page_fault_exception_handler(addr)?,
        _ => {
            return Err(AlienError::Other);
        }
    }
    Ok(())
}

pub fn load_page_fault_exception_handler(addr: usize) -> AlienResult<()> {
    let info = {
        let process = current_task().unwrap();
        process.access_inner().do_load_page_fault(addr)
    };
    if info.is_err() {
        return Err(AlienError::Other);
    }
    let (file, buf, offset) = info.unwrap();
    if file.is_some() {
        let _r = vfs_read_file::<VfsProvider>(file.unwrap().get_file(), buf, offset);
    }

    Ok(())
}

pub fn store_page_fault_exception_handler(addr: usize) -> AlienResult<()> {
    let process = current_task().unwrap();
    trace!(
        "[pid: {}] do store page fault addr:{:#x}",
        process.get_pid(),
        addr
    );
    let res = process.access_inner().do_store_page_fault(addr)?;
    if res.is_some() {
        let (file, buf, offset) = res.unwrap();
        if file.is_some() {
            let _r = vfs_read_file::<VfsProvider>(file.unwrap().get_file(), buf, offset);
        }
    }
    Ok(())
}
