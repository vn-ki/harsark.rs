//! # Resource Manager
//! The Definition of Data-structures required for resource management.
//! The Resource manager handles the details of which processes have access to the which resource
//! and implements the locking and unlocking mechanism.

use crate::config::MAX_RESOURCES;
use crate::system::scheduler::TaskId;
use crate::KernelError;

const PI: i32 = -1;

pub struct PiStack {
    /// Points the top of the `pi_stack`.
    top: usize,
    /// This stack is used for locking and unlocking of resources.
    // TODO: Why is this i32 and not u32??
    pi_stack: [i32; MAX_RESOURCES],
    /// Hold the ceiling of the resource with the highest ceiling amongst the currently locked resources.
    pub system_ceiling: i32,
}

impl PiStack {
    pub const fn new() -> Self {
        Self {
            top: 0,
            pi_stack: [PI; MAX_RESOURCES],
            system_ceiling: PI,
        }
    }

    /// Pops the stack top and assigns the `system_ceiling` to the new stack top.
    pub fn pop_stack(&mut self) -> Result<(), KernelError> {
        if self.top == 0 {
            return Err(KernelError::Empty);
        }
        self.top -= 1;
        self.system_ceiling = self.pi_stack[self.top];
        Ok(())
    }

    pub fn top(&self) -> u32 {
        // XXX does this check bounds
        // do we need a bounds check?
        self.pi_stack[self.top] as u32
    }

    /// Pushes the passed ceiling onto the pi_stack.
    pub fn push_stack(&mut self, ceiling: TaskId) -> Result<(), KernelError> {
        self.top += 1;
        if self.top >= MAX_RESOURCES {
            return Err(KernelError::LimitExceeded);
        }
        self.pi_stack[self.top] = ceiling as i32;
        self.system_ceiling = ceiling as i32;
        Ok(())
    }
}
