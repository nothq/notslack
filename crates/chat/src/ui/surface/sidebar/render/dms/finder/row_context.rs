#[derive(Clone, Copy)]
pub(super) struct SlackDmFinderRowContext {
    pub(super) index: usize,
    pub(super) selected: bool,
    pub(super) count: usize,
}
