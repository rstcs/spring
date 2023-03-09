//! dispatcher module is used to distribute tasks according to different models

pub trait Dispatcher {
    /// query current task process, returning 0 to 1
    fn get_process(&self) -> f64;

    /// worker apply a job from dispatcher, return true continue to handle,
    /// return false worker will exit.
    fn try_apply_job(&mut self) -> bool;

    /// when worker complete job, it will notify the dispatcher
    fn notify_complete_job(&mut self);

    /// when the program receives an external termination signal, notify the
    /// Dispatcher to process it
    fn cancel(&mut self);
}

pub struct CountDispatcher {}

impl CountDispatcher {
    pub fn new() -> Self {
        Self {}
    }
}

impl Dispatcher for CountDispatcher {
    fn get_process(&self) -> f64 {
        todo!()
    }

    fn try_apply_job(&mut self) -> bool {
        todo!()
    }

    fn notify_complete_job(&mut self) {
        todo!()
    }

    fn cancel(&mut self) {
        todo!()
    }
}
