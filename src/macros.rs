//! Macro Definitions

/// The tasks must be looping infinitely and call `task_exit` whenever a particular task is done.
/// This makes it complicated to create tasks and also might introduce undefined behavior if task_exit is not called.
/// The `spawn` macro makes it easier to define tasks. It also defines a static variable of type TaskId,
/// which corresponds to the task created.
///
/// ## Examples
///
/// ```rust
/// shared = 10;
/// spawn!(task2, 2, stack1, shared, params, {
///     hprintln!("{}", params);
/// });
/// spawn!(task3, 3, stack2, {
///     hprintln!("Hello!");
/// });
/// ```
#[macro_export]
macro_rules! spawn {
    ($tm: expr, $priority: expr, $stack: expr, $handler_fn: block) => {
        create_task(
            $tm,
            $priority,
            unsafe{ &mut $stack },
            || loop {
                $handler_fn
                task_exit($tm);
        }).unwrap();
    };
    ($tm: expr, $priority: expr, $deadline: expr, $stack: expr, $handler_fn: block) => {
        create_task(
            $tm,
            $priority,
            $deadline,
            unsafe{ &mut $stack },
            || loop {
                $handler_fn
                task_exit($tm);
        }).unwrap();
    };
}

/// `priv_execute!` executes the code block only if the current context is in privileged mode.
/// ## Example
/// ```rust
/// priv_execute!({
///     hprintln!("Privileged!");
/// });
/// ```
#[macro_export]
macro_rules! priv_execute {
    ($handler: block) => {
        match is_privileged() {
            false => Err(KernelError::AccessDenied),
            true => $handler,
        }
    };
}
