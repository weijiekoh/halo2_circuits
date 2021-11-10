use halo2::{
    arithmetic::FieldExt,
    circuit::{Chip, Layouter},
    plonk::{Error}
};

mod chip;
pub use chip::{MuxConfig, MuxChip};


pub trait MuxInstructions<F: FieldExt> 
: Chip<F> 
{
    type Cell;

    fn mux(
        &self,
        layouter: impl Layouter<F>,
        a: Self::Cell,
        b: Self::Cell,
        selector: Self::Cell,
    ) -> Result<Self::Cell, Error>;

}