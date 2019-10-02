use core::ptr;

use crate::config::{MAX_STACK_SIZE, MAX_TASKS, SYSTICK_INTERRUPT_INTERVAL};
use crate::errors::KernelError;
use crate::interrupt_handlers::svc_call;
use crate::kernel::helper::get_msb;
use cortex_m::interrupt::free as execute_critical;
use cortex_m::peripheral::syst::SystClkSource;
use cortex_m::register::control::Npriv;

use crate::kernel::types::TaskId;

use crate::kernel::scheduler::*;
use core::borrow::BorrowMut;

static empty_task: TaskControlBlock = TaskControlBlock { sp: 0 };

// GLOBALS:
static mut all_tasks: TaskManager = TaskManager::new();
#[no_mangle]
static mut os_curr_task: &TaskControlBlock = &empty_task;
#[no_mangle]
static mut os_next_task: &TaskControlBlock = &empty_task;
// end GLOBALS

/// Initialize the switcher system
pub fn init(is_preemptive: bool) {
    execute_critical(|_| unsafe { all_tasks.init(is_preemptive) })
}

// The below section just sets up the timer and starts it.
pub fn start_kernel() {
    let cp = cortex_m::Peripherals::take().unwrap();
    let mut syst = cp.SYST;
    syst.set_clock_source(SystClkSource::Core);
    syst.set_reload(SYSTICK_INTERRUPT_INTERVAL);
    syst.enable_counter();
    syst.enable_interrupt();

    execute_critical(|_| unsafe { all_tasks.start_kernel() });
    preempt();
}

pub fn create_task<T: Sized>(
    priority: usize,
    handler_fn: fn(&T) -> !,
    param: &T,
) -> Result<(), KernelError> {
    execute_critical(|_| unsafe { all_tasks.create_task(priority, handler_fn, param) })
}

pub fn schedule() {
    let ctrl_reg = cortex_m::register::control::read();
    if ctrl_reg.npriv() == Npriv::Privileged {
        preempt();
    } else {
        svc_call();
    }
}

fn preempt() -> Result<(), KernelError> {
    execute_critical(|_| {
        let handler = unsafe { &mut all_tasks };
        let HT = handler.get_HT();
        if handler.is_running {
            if handler.curr_pid != HT {
                context_switch(handler.curr_pid, HT);
            }
        }
        return Ok(());
    })
}

fn context_switch(curr: usize, next: usize) {
    let handler = unsafe { &mut all_tasks };
    let task_curr = &handler.threads[curr];
    if handler.started {
        unsafe {
            os_curr_task = task_curr.as_ref().unwrap();
        }
    } else {
        handler.started = true;
    }
    handler.curr_pid = next;
    let task_next = &handler.threads[next];
    unsafe {
        os_next_task = task_next.as_ref().unwrap();
        cortex_m::peripheral::SCB::set_pendsv();
    }
}

pub fn is_preemptive() -> bool {
    execute_critical(|_| {
        unsafe {
            all_tasks.is_preemptive
        }
    })
}

pub fn get_pid() -> usize {
    execute_critical(|_| {
        let handler = unsafe { &mut all_tasks };
        return handler.curr_pid;
    })
}

pub fn block_tasks(tasks_mask: u32) {
    execute_critical(|_| {
        unsafe{all_tasks.BTV |= tasks_mask;}
    })
}

pub fn unblock_tasks(tasks_mask: u32) {
    execute_critical(|_| {
        unsafe{all_tasks.BTV &= !tasks_mask;}
    })
}

pub fn task_exit() {
    execute_critical(|_| {
        let rt = get_pid();
        unsafe { all_tasks.ATV &= !(1 << rt as u32) };
    });
    schedule()
}

pub fn release(tasks_mask: &u32) {
    execute_critical(|_| {
        unsafe{all_tasks.release(&tasks_mask)};
    });
    schedule();
}

pub fn enable_preemption() {
    execute_critical(|_| {
        unsafe { all_tasks.is_preemptive = true };
    })
}

pub fn disable_preemption() {
    execute_critical(|_| {
        unsafe {all_tasks.is_preemptive = false;}
    })
}

#[macro_export]
macro_rules! spawn {
    ($task_name: ident, $priority: expr, $var: ident, $param: expr, $handler_fn: block) => {
        create_task($priority,|$var| loop {
            $handler_fn
            task_exit();
        },&$param).unwrap();
        static $task_name: TaskId = $priority;
    };
    ($task_name: ident, $priority: expr, $handler_fn: block) => {
        create_task($priority,|_| loop {
            $handler_fn
            task_exit();
        },0).unwrap();
        static $task_name: TaskId = $priority;
    };
}