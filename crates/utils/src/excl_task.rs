// Exclusive task, make sure only one long-running operation is being executed

#[derive(Debug)]
pub struct ExclTask<Output, Cause = String> {
    requested: Option<Cause>,
    in_process: bool,
    last_result: Option<Output>,
}

impl<Output, Cause> Default for ExclTask<Output, Cause> {
    fn default() -> Self {
        Self { requested: None, in_process: false, last_result: None }
    }
}

impl<Output, Cause> ExclTask<Output, Cause> {
    pub fn request(&mut self, reason: Cause) {
        self.requested = Some(reason);
    }

    pub fn should_start(&mut self) -> Option<Cause> {
        if self.in_process {
            return None;
        }
        self.in_process = self.requested.is_some();
        self.requested.take()
    }

    pub fn complete(&mut self, result: Option<Output>) {
        assert!(self.in_process);
        self.in_process = false;
        self.last_result = result;
    }

    pub fn last_op_result(&self) -> &Option<Output> {
        &self.last_result
    }

    pub fn in_process(&self) -> bool {
        self.in_process
    }

    pub fn has_op_requested(&self) -> bool {
        self.requested.is_some()
    }
}
