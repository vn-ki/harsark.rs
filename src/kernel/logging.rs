use core::cell::RefCell;

use crate::kernel::timer::get_time;
use crate::priv_execute;
use crate::system::scheduler::*;
use crate::system::system_logger::*;
use crate::utils::arch::is_privileged;
use crate::utils::arch::{critical_section, svc_call, Mutex};
use crate::KernelError;

static Logger: Mutex<RefCell<SystemLogger>> = Mutex::new(RefCell::new(SystemLogger::new()));

pub fn report(event_type: LogEventType) {
    critical_section(|cs_token| {
        Logger
            .borrow(cs_token)
            .borrow_mut()
            .push(LogEvent::new(event_type, get_time()));
    })
}

pub fn process<F>(handler: F)
where
    F: Fn(LogEvent),
{
    critical_section(|cs_token| {
        while let Some(event) = Logger.borrow(cs_token).borrow_mut().pop() {
            handler(event);
        }
    })
}

pub fn set_all(val: bool) {
    critical_section(|cs_token| {
        Logger.borrow(cs_token).borrow_mut().release_log = val;
        Logger.borrow(cs_token).borrow_mut().block_tasks_log = val;
        Logger.borrow(cs_token).borrow_mut().unblock_tasks_log = val;
        Logger.borrow(cs_token).borrow_mut().task_exit_log = val;
        Logger.borrow(cs_token).borrow_mut().resource_lock_log = val;
        Logger.borrow(cs_token).borrow_mut().resource_unlock_log = val;
        Logger.borrow(cs_token).borrow_mut().message_broadcast_log = val;
        Logger.borrow(cs_token).borrow_mut().message_recieve_log = val;
        Logger.borrow(cs_token).borrow_mut().semaphore_signal_log = val;
        Logger.borrow(cs_token).borrow_mut().semaphore_reset_log = val;
        Logger.borrow(cs_token).borrow_mut().timer_event_log = val;
    })
}

pub fn set_release(val: bool) {
    critical_section(|cs_token| {
        Logger.borrow(cs_token).borrow_mut().release_log = val;
    })
}

pub fn set_block_tasks(val: bool) {
    critical_section(|cs_token| {
        Logger.borrow(cs_token).borrow_mut().block_tasks_log = val;
    })
}

pub fn set_unblock_tasks(val: bool) {
    critical_section(|cs_token| {
        Logger.borrow(cs_token).borrow_mut().unblock_tasks_log = val;
    })
}

pub fn set_task_exit(val: bool) {
    critical_section(|cs_token| {
        Logger.borrow(cs_token).borrow_mut().task_exit_log = val;
    })
}

pub fn set_resource_lock(val: bool) {
    critical_section(|cs_token| {
        Logger.borrow(cs_token).borrow_mut().resource_lock_log = val;
    })
}

pub fn set_resource_unlock(val: bool) {
    critical_section(|cs_token| {
        Logger.borrow(cs_token).borrow_mut().resource_unlock_log = val;
    })
}

pub fn set_message_broadcast(val: bool) {
    critical_section(|cs_token| {
        Logger.borrow(cs_token).borrow_mut().message_broadcast_log = val;
    })
}

pub fn set_message_recieve(val: bool) {
    critical_section(|cs_token| {
        Logger.borrow(cs_token).borrow_mut().message_recieve_log = val;
    })
}

pub fn set_semaphore_signal(val: bool) {
    critical_section(|cs_token| {
        Logger.borrow(cs_token).borrow_mut().semaphore_signal_log = val;
    })
}

pub fn set_semaphore_reset(val: bool) {
    critical_section(|cs_token| {
        Logger.borrow(cs_token).borrow_mut().semaphore_reset_log = val;
    })
}

pub fn set_timer_event(val: bool) {
    critical_section(|cs_token| {
        Logger.borrow(cs_token).borrow_mut().timer_event_log = val;
    })
}

pub fn get_release() -> bool {
    critical_section(|cs_token| Logger.borrow(cs_token).borrow_mut().release_log)
}

pub fn get_block_tasks() -> bool {
    critical_section(|cs_token| Logger.borrow(cs_token).borrow_mut().block_tasks_log)
}

pub fn get_unblock_tasks() -> bool {
    critical_section(|cs_token| Logger.borrow(cs_token).borrow_mut().unblock_tasks_log)
}

pub fn get_task_exit() -> bool {
    critical_section(|cs_token| Logger.borrow(cs_token).borrow_mut().task_exit_log)
}

pub fn get_resource_lock() -> bool {
    critical_section(|cs_token| Logger.borrow(cs_token).borrow_mut().resource_lock_log)
}

pub fn get_resource_unlock() -> bool {
    critical_section(|cs_token| Logger.borrow(cs_token).borrow_mut().resource_unlock_log)
}

pub fn get_message_broadcast() -> bool {
    critical_section(|cs_token| Logger.borrow(cs_token).borrow_mut().message_broadcast_log)
}

pub fn get_message_recieve() -> bool {
    critical_section(|cs_token| Logger.borrow(cs_token).borrow_mut().message_recieve_log)
}

pub fn get_semaphore_signal() -> bool {
    critical_section(|cs_token| Logger.borrow(cs_token).borrow_mut().semaphore_signal_log)
}

pub fn get_semaphore_reset() -> bool {
    critical_section(|cs_token| Logger.borrow(cs_token).borrow_mut().semaphore_reset_log)
}

pub fn get_timer_event() -> bool {
    critical_section(|cs_token| Logger.borrow(cs_token).borrow_mut().timer_event_log)
}
