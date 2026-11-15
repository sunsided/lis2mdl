//! An `embedded-hal` delay that advances a [`Lis2mdlModel`].

use crate::Lis2mdlModel;
use core::cell::RefCell;

/// A delay provider that advances the time of a [`Lis2mdlModel`] instead of waiting.
#[derive(Debug, Clone, Copy)]
pub struct FakeDelay<'a> {
    model: &'a RefCell<Lis2mdlModel>,
}

impl<'a> FakeDelay<'a> {
    /// Creates a delay provider for the specified model.
    pub fn new(model: &'a RefCell<Lis2mdlModel>) -> Self {
        Self { model }
    }
}

impl embedded_hal::delay::DelayNs for FakeDelay<'_> {
    fn delay_ns(&mut self, ns: u32) {
        self.model.borrow_mut().tick(ns / 1_000);
    }
}

impl embedded_hal_async::delay::DelayNs for FakeDelay<'_> {
    async fn delay_ns(&mut self, ns: u32) {
        self.model.borrow_mut().tick(ns / 1_000);
    }
}
