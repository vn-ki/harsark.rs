use core::ptr;

use crate::config::{MAX_STACK_SIZE, MAX_TASKS, SYSTICK_INTERRUPT_INTERVAL};
use crate::errors::KernelError;
use crate::interrupt_handlers::svc_call;
use crate::kernel::helper::get_msb;
use cortex_m::interrupt::free as execute_critical;
use cortex_m::peripheral::syst::SystClkSource;
use cortex_m::register::control::Npriv;

pub type TaskId = u32;

#[repr(C)]
struct TaskManager {
    RT: usize,
    is_running: bool,
    threads: [Option<TaskControlBlock>; MAX_TASKS],
    BTV: u32,
    ATV: u32,
    is_preemptive: bool,
    started: bool,
}

/// A single thread's state
#[repr(C)]
#[derive(Clone, Copy)]
struct TaskControlBlock {
    // fields used in assembly, do not reorder them
    sp: usize, // current stack pointer of this thread
}

static empty_task: TaskControlBlock = TaskControlBlock { sp: 0 };

// GLOBALS:
static mut all_tasks: TaskManager = TaskManager {
    RT: 0,
    is_running: false,
    threads: [None; MAX_TASKS],
    ATV: 1,
    BTV: 0,
    is_preemptive: false,
    started: false,
};
#[no_mangle]
static mut TASK_STACKS: [[u32; MAX_STACK_SIZE]; MAX_TASKS] = [[0; MAX_STACK_SIZE]; MAX_TASKS];
#[no_mangle]
static mut os_curr_task: &TaskControlBlock = &empty_task;
#[no_mangle]
static mut os_next_task: &TaskControlBlock = &empty_task;
// end GLOBALS

/// Initialize the switcher system
pub fn init(is_preemptive: bool) {
    execute_critical(|_| {
        unsafe {
            let ptr: usize = core::intrinsics::transmute(&all_tasks);
            all_tasks.is_preemptive = is_preemptive;
        }
        /*
            This is the default task, that just puts the board for a power-save mode
            until any event (interrupt/exception) occurs.
        */
        create_task(0, || loop {
            cortex_m::asm::wfe();
        })
        .unwrap();
    });
}

// The below section just sets up the timer and starts it.
pub fn start_kernel() -> Result<(), KernelError> {
    execute_critical(|_| {
        let cp = cortex_m::Peripherals::take().unwrap();
        let mut syst = cp.SYST;
        syst.set_clock_source(SystClkSource::Core);
        syst.set_reload(SYSTICK_INTERRUPT_INTERVAL);
        syst.enable_counter();
        syst.enable_interrupt();
        unsafe {
            all_tasks.is_running = true;
        }
        preempt();
        return Ok(());
    })
}

pub fn release(tasks_mask: &u32) {
    execute_critical(|_| {
        let handler = unsafe { &mut all_tasks };
        handler.ATV |= *tasks_mask;
        preempt();
    });
}

pub fn create_task(priority: usize, handler_fn: fn() -> !) -> Result<(), KernelError> {
    execute_critical(|_| {
        let mut stack = unsafe { &mut TASK_STACKS[priority] };
        match create_tcb(stack, handler_fn) {
            Ok(tcb) => {
                insert_tcb(priority, tcb)?;
                return Ok(());
            }
            Err(e) => return Err(e),
        }
    })
}

pub fn preempt() {
    let ctrl_reg = cortex_m::register::control::read();
    if ctrl_reg.npriv() == Npriv::Privileged {
        preempt_call();
    } else {
        svc_call();
    }
}

pub fn preempt_call() -> Result<(), KernelError> {
    execute_critical(|_| {
        let handler = unsafe { &mut all_tasks };
        if handler.is_running {
            let HT = get_HT();
            // schedule a thread to be run
            if handler.RT != HT {
                let task_rt = &handler.threads[handler.RT];
                if handler.started {
                    if let Some(task_rt) = task_rt {
                        unsafe {
                            os_curr_task = &task_rt;
                        }
                    }
                } else {
                    handler.started = true;
                }
                handler.RT = HT;
                let task = &handler.threads[handler.RT];
                if let Some(task) = task {
                    unsafe {
                        os_next_task = &task;
                        cortex_m::peripheral::SCB::set_pendsv();
                    }
                } else {
                    return Err(KernelError::DoesNotExist);
                }
            }
        }
        return Ok(());
    })
}

fn get_HT() -> usize {
    execute_critical(|_| {
        let handler = unsafe { &mut all_tasks };
        let mask = handler.ATV & !handler.BTV;
        return get_msb(&mask);
    })
}

fn create_tcb(stack: &mut [u32], handler: fn() -> !) -> Result<TaskControlBlock, KernelError> {
    execute_critical(|_| {
        if stack.len() < 32 {
            return Err(KernelError::StackTooSmall);
        }

        let idx = stack.len() - 1;
        stack[idx] = 1 << 24; // xPSR
        let pc: usize = handler as usize;
        stack[idx - 1] = pc as u32; // PC

        let sp: usize = unsafe { core::intrinsics::transmute(&stack[stack.len() - 16]) };
        let tcb = TaskControlBlock { sp: sp as usize };
        Ok(tcb)
    })
}

fn insert_tcb(idx: usize, tcb: TaskControlBlock) -> Result<(), KernelError> {
    execute_critical(|_| {
        let handler = unsafe { &mut all_tasks };
        if idx >= MAX_TASKS {
            return Err(KernelError::DoesNotExist);
        }
        handler.threads[idx] = Some(tcb);
        return Ok(());
    })
}

pub fn is_preemptive() -> bool {
    execute_critical(|_| {
        let handler = unsafe { &mut all_tasks };
        handler.is_preemptive
    })
}

pub fn get_RT() -> usize {
    execute_critical(|_| {
        let handler = unsafe { &mut all_tasks };
        return handler.RT;
    })
}

pub fn block_tasks(tasks_mask: u32) {
    execute_critical(|_| {
        let handler = unsafe { &mut all_tasks };
        handler.BTV |= tasks_mask;
    })
}

pub fn unblock_tasks(tasks_mask: u32) {
    execute_critical(|_| {
        let handler = unsafe { &mut all_tasks };
        handler.BTV &= !tasks_mask;
    })
}

pub fn task_exit() {
    execute_critical(|_| {
        let rt = get_RT();
        let handler = unsafe { &mut all_tasks };
        handler.ATV &= !(1 << rt as u32);
        preempt();
    })
}

pub fn release_tasks(tasks: &[TaskId]) {
    execute_critical(|_| {
        let mut mask = 0;
        for tid in tasks {
            mask |= 1 << *tid;
        }
        release(&mask);
    })
}

pub fn enable_preemption() {
    execute_critical(|_| {
        let handler = unsafe { &mut all_tasks };
        handler.is_preemptive = true;
    })
}

pub fn disable_preemption() {
    execute_critical(|_| {
        let handler = unsafe { &mut all_tasks };
        handler.is_preemptive = false;
    })
}

#[macro_export]
macro_rules! spawn {
    ($task_name: ident, $priority: expr, $handler_fn: block) => {
        create_task($priority,|| loop {
            $handler_fn
            task_exit();
        }).unwrap();
        static $task_name: TaskId = $priority;
    }
}
