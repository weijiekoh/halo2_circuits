pub mod mux;
use halo2::arithmetic::FieldExt;
use crate::gadget::mux::{MuxChip};

impl<F: FieldExt> super::Config<F> {
    pub(super) fn construct_mux_chip(&self) -> MuxChip<F> {
        MuxChip::construct(self.mux_config.clone())
    }
}