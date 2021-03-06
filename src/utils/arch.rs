//! # Machine specific
//!
//! Defines functions which are defined majorly in assembly. Thus, might change for one board to another.

// Platform specific Exports
// pub use cortex_m::interrupt::free as critical_section;
pub use cortex_m::interrupt::{Mutex, CriticalSection, disable, enable};
pub use cortex_m::peripheral::syst::SystClkSource;
pub use cortex_m::peripheral::Peripherals;
use crate::system::scheduler::*;
use core::cell::RefCell;

use cortex_m::register::control;
use cortex_m_rt::exception;

use crate::kernel::tasks::{schedule, TaskManager, TaskManager_C1};
use crate::system::spinlock::{spinlock, spinlock_try, spinunlock, TASKMANAGER_LOCK};
use crate::system::scheduler::TaskControlBlock;
use cortex_m_semihosting::hprintln;

#[cfg(any(feature = "events_32", feature = "events_16", feature = "events_64"))]
use crate::kernel::events::sweep_event_table;

#[cfg(feature = "task_monitor")]
use crate::kernel::task_monitor::sweep_deadlines;

#[cfg(feature = "timer")]
use crate::kernel::timer::update_time;

#[inline(never)]
unsafe fn enable_noinline(){
    enable();
}

/// this is replication of critical_section from cortex_m crate
#[inline]
pub fn critical_section<F, R>(f: F) -> R
where
    F: FnOnce(&CriticalSection) -> R,
{
    let primask = cortex_m::register::primask::read();

    // disable interrupts
    disable();

    let r = f(unsafe { &CriticalSection::new() });

    // If the interrupts were active before our `disable` call, then re-enable
    // them. Otherwise, keep them disabled
    if primask.is_active() {
        unsafe { enable_noinline() }
    }

    r
}

/// Returns the MSB of `val`. It is written using CLZ instruction.
pub fn get_msb(val: u32) -> Option<usize> {
    let mut res: usize;
    unsafe {
        asm!(
            "clz {1}, {0}",
            in(reg) val,
            out(reg) res,
        );
    }
    res = 32 - res;
    if res == 0 {
        return None;
    } else {
        res -= 1;
    }
    return Some(res);
}

/// Creates an SVC Interrupt
pub fn svc_call() {
    unsafe {
        asm!("svc 1");
    }
}

#[inline(always)]
pub unsafe fn return_to_psp() {
    asm!(
        "
        ldr r0, =0xFFFFFFFD
        bx	r0
        "
    );
}

#[inline(always)]
pub fn save_context(task_stack: &TaskControlBlock) {
    unsafe {
        asm!(
            "mrs r0, psp",
            "subs r0, #16",
            "stmia r0!,{{r4-r7}}",
            // "mov	r4, r8",
            // "mov	r5, r9",
            // "mov	r6, r10",
            // "mov	r7, r11",
            "subs	r0, #32",
            "stmia	r0!,{{r8-r11}}",
            "subs	r0, #16",
            "mov	r1, {0}",
            "@ldr	r1, [r2]",
            "str	r0, [r1]",
            in(reg) task_stack,
            out("r0") _,
            out("r1") _,
            // out("r3") _,
            // out("r4") _,
            // out("r5") _,
        )
    };
}

#[inline(always)]
pub fn load_context(task_stack: &TaskControlBlock) {
    unsafe {
        asm!(
            "cpsid	i",
            "mov	r1, {0}",
            "@ldr	r1, [r2]",
            "@ldr	r1, [r1]",
            "ldr	r0, [r1]",
            "ldmia	r0!,{{r4-r7}}",
            "mov	r8, r4",
            "mov	r9, r5",
            "mov	r10, r6",
            "mov	r11, r7",
            "ldmia	r0!,{{r4-r7}}",
            "msr	psp, r0",
            in(reg) task_stack,
            out("r0") _,
            out("r1") _,
        )
    };
}

/// ### SysTick Interrupt handler
/// Its the Crux of the Kernel’s time management module and Task scheduling.
/// This interrupt handler updates the time and also dispatches the appropriate event handlers.
/// The interrupt handler also calls `schedule()` in here so as to dispatch any higher priority
/// task if there are any.

#[cfg(feature = "timer")]
#[exception]
fn SysTick() {
    #[cfg(any(feature = "events_32", feature = "events_16", feature = "events_64"))]
    sweep_event_table();

    #[cfg(feature = "timer")]
    update_time();

    #[cfg(feature = "task_monitor")]
    sweep_deadlines();

    // hprintln!("hello");
    // schedule();
}
/// ### SVC Interrupt handler,
/// calls `tasks::schedule()`
// #[exception]
// fn SVCall() {
//     schedule();
// }

#[export_name = "SVCall_0"]
pub extern "C" fn SVCall_0() {
    schedule(&TaskManager);
}

#[export_name = "SVCall_1"]
pub extern "C" fn SVCall_1() {
    schedule(&TaskManager_C1);
}

/// ### PendSV Interrupt handler,
/// PendSV interrupt handler does the actual context switch in the Kernel.
// #[exception]
// fn PendSV() {
//     critical_section(|cs_token| {
//         let handler = &mut TaskManager.borrow(cs_token).borrow_mut();
//         let curr_tid: usize = handler.curr_tid;
//         let next_tid: usize = handler.get_next_tid() as usize;
//         if curr_tid != next_tid || (!handler.started) {
//             if handler.started {
//                 let curr_task = handler.task_control_blocks[curr_tid].as_ref().unwrap();
//                 curr_task.save_context();
//             } else {
//                 handler.started = true;
//             }
//             let next_task = handler.task_control_blocks[next_tid].as_ref().unwrap();
//             next_task.load_context();
//
//             handler.curr_tid = next_tid;
//         }
//     });
//     unsafe { return_to_psp() }
// }

#[inline(never)]
fn get_next_tcb(t1: &'static Mutex<RefCell<Scheduler>>, t2: &'static Mutex<RefCell<Scheduler>>, cs_token: &CriticalSection) -> Option<TaskControlBlock>{
    let handler = &mut t1.borrow(cs_token).borrow_mut();
    let curr_tid: usize = handler.curr_tid;
    if handler.migrated_tid > 0 {
        let mut oc_handler = &mut t2.borrow(cs_token).borrow_mut();
        if handler.running_migrated {
            // the migration has already been done and schedule was called during resource
            // unlock
            hprintln!("unmigrating task");
            let migrate_task = oc_handler.task_control_blocks[handler.migrated_tid].as_ref().unwrap();
            migrate_task.save_context();
            oc_handler.migrated_tasks = oc_handler.migrated_tasks & !(1 << handler.migrated_tid as u32);
            handler.migrated_tid = 0;
            handler.running_migrated = false;
            let curr_task = handler.task_control_blocks[curr_tid];
            return curr_task;
        } else {
            hprintln!("migrating task");
            // the tid to be migrated but migration has not occured
            let curr_task = handler.task_control_blocks[curr_tid].as_ref().unwrap();
            curr_task.save_context();
            handler.running_migrated = true;
            let migrate_task = oc_handler.task_control_blocks[handler.migrated_tid];
            return migrate_task;
        }
    } else {
        let next_tid: usize = handler.get_next_tid() as usize;
        if curr_tid != next_tid || (!handler.started) {
            if handler.started {
                let curr_task = handler.task_control_blocks[curr_tid].as_ref().unwrap();
                curr_task.save_context();
            } else {
                handler.started = true;
            }
            handler.curr_tid = next_tid;
            let next_tcb = handler.task_control_blocks[next_tid];
            return next_tcb;
        }
    }
    None
}

#[export_name = "PendSV_0"]
pub extern "C" fn PendSV_0() {
    critical_section(|cs_token| {
        spinlock(&TASKMANAGER_LOCK);
        // if false {
        //     let handler = &mut TaskManager.borrow(cs_token).borrow_mut();
        //     let curr_tid: usize = handler.curr_tid;
        //     if handler.migrated_tid > 0 {
        //         let mut oc_handler = &mut TaskManager_C1.borrow(cs_token).borrow_mut();
        //         if handler.running_migrated {
        //             // the migration has already been done and schedule was called during resource
        //             // unlock
        //             hprintln!("unmigrating task");
        //             let migrate_task = oc_handler.task_control_blocks[handler.migrated_tid].as_ref().unwrap();
        //             migrate_task.save_context();
        //             oc_handler.migrated_tasks = oc_handler.migrated_tasks & !(1 << handler.migrated_tid as u32);
        //             let curr_task = handler.task_control_blocks[curr_tid].as_ref().unwrap();
        //             curr_task.load_context();
        //             handler.migrated_tid = 0;
        //             handler.running_migrated = false;
        //         } else {
        //             hprintln!("migrating task");
        //             // the tid to be migrated but migration has not occured
        //             let curr_task = handler.task_control_blocks[curr_tid].as_ref().unwrap();
        //             curr_task.save_context();
        //             let migrate_task = oc_handler.task_control_blocks[handler.migrated_tid].as_ref().unwrap();
        //             migrate_task.load_context();
        //             handler.running_migrated = true;
        //         }
        //     } else {
        //         let next_tid: usize = handler.get_next_tid() as usize;
        //         if curr_tid != next_tid || (!handler.started) {
        //             if handler.started {
        //                 let curr_task = handler.task_control_blocks[curr_tid].as_ref().unwrap();
        //                 curr_task.save_context();
        //             } else {
        //                 handler.started = true;
        //             }
        //             handler.curr_tid = next_tid;
        //             let next_task = handler.task_control_blocks[next_tid].as_ref().unwrap();
        //             // next_task.load_context();
        //
        //         }
        //     }
        // }
        // let next_task = {
        //     let handler = TaskManager.borrow(cs_token).borrow();
        //     handler.task_control_blocks[handler.curr_tid]
        // };
        if let Some(ref next_task) = get_next_tcb(&TaskManager, &TaskManager_C1, cs_token) {
            next_task.load_context();
        }
        spinunlock(&TASKMANAGER_LOCK);
    });
    unsafe { return_to_psp() }
}

#[export_name = "PendSV_1"]
pub extern "C" fn PendSV_1() {
    critical_section(|cs_token| {
        spinlock(&TASKMANAGER_LOCK);
        // let handler = &mut TaskManager_C1.borrow(cs_token).borrow_mut();
        // let curr_tid: usize = handler.curr_tid;
        // if handler.migrated_tid > 0 {
        //     let mut oc_handler = &mut TaskManager.borrow(cs_token).borrow_mut();
        //     if handler.running_migrated {
        //         // the migration has already been done and schedule was called during resource
        //         // unlock
        //         let migrate_task = oc_handler.task_control_blocks[handler.migrated_tid].as_ref().unwrap();
        //         migrate_task.save_context();
        //         oc_handler.migrated_tasks = oc_handler.migrated_tasks & !(1 << handler.migrated_tid as u32);
        //         handler.migrated_tid = 0;
        //         handler.running_migrated = false;
        //         let curr_task = handler.task_control_blocks[curr_tid].as_ref().unwrap();
        //         curr_task.load_context();
        //     } else {
        //         // the tid to be migrated but migration has not occured
        //         let curr_task = handler.task_control_blocks[curr_tid].as_ref().unwrap();
        //         curr_task.save_context();
        //         handler.running_migrated = true;
        //         let migrate_task = oc_handler.task_control_blocks[handler.migrated_tid].as_ref().unwrap();
        //         migrate_task.load_context();
        //     }
        // } else {
        //     let next_tid: usize = handler.get_next_tid() as usize;
        //     if curr_tid != next_tid || (!handler.started) {
        //         if handler.started {
        //             let curr_task = handler.task_control_blocks[curr_tid].as_ref().unwrap();
        //             curr_task.save_context();
        //         } else {
        //             handler.started = true;
        //         }
        //         handler.curr_tid = next_tid;
        //         let next_task = handler.task_control_blocks[next_tid].as_ref().unwrap();
        //         next_task.load_context();
        //     }
        // }

        if let Some(ref next_task) = get_next_tcb(&TaskManager_C1, &TaskManager, cs_token) {
            next_task.load_context();
        }
        spinunlock(&TASKMANAGER_LOCK);
    });
    unsafe { return_to_psp() }
}

pub fn set_pendsv() {
    // XXX TODO
    // this enable is required because something something is disabling the interrupts, needs
    // investigation to find out who is disabling it
    unsafe {enable()};
    cortex_m::peripheral::SCB::set_pendsv();
    // unsafe {asm!("isb")};
}

pub fn wait_for_interrupt() {
    cortex_m::asm::wfi();
}

/// Returns true if Currently the Kernel is operating in Privileged mode.
pub fn is_privileged() -> bool {
    return control::read().npriv() == control::Npriv::Privileged;
}
