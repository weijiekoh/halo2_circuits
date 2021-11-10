use std::marker::PhantomData;

use halo2::{
    arithmetic::FieldExt,
    circuit::{Chip, Layouter, Region},
    plonk::{Advice, Column, ConstraintSystem, Error, Selector, Expression},
    poly::Rotation,
};

use super::MuxInstructions;
use super::super::super::CellValue;

#[derive(Clone, Debug)]
pub struct MuxConfig {
    pub advice: [Column<Advice>; 3],
    pub s_mux: Selector,
    pub s_bool: Selector
}

#[derive(Debug)]
pub struct MuxChip<F: FieldExt> {
    pub config: MuxConfig,
    pub _marker: PhantomData<F>,
}

impl<F: FieldExt> MuxChip<F> {
    pub fn configure(
        meta: &mut ConstraintSystem<F>,
        advice: [Column<Advice>; 3],
    ) -> <Self as Chip<F>>::Config {

        for column in &advice {
            meta.enable_equality((*column).into());
        }

        let s_bool = meta.selector();

        meta.create_gate("bool", |meta| {
            let selector = meta.query_advice(advice[2], Rotation::cur());
            let s_bool = meta.query_selector(s_bool);

            vec![s_bool * selector.clone() * (Expression::Constant(F::one()) - selector)]
        });

        let s_mux = meta.selector();

        meta.create_gate("mux", |meta| {
            let a = meta.query_advice(advice[0], Rotation::cur());
            let b = meta.query_advice(advice[1], Rotation::cur());
            let selector = meta.query_advice(advice[2], Rotation::cur());
            let out = meta.query_advice(advice[0], Rotation::next());
            let s_mux = meta.query_selector(s_mux);

            vec![s_mux * (out - ((b - a.clone()) * selector + a))]
        });

        MuxConfig {
            advice,
            s_mux,
            s_bool

        }
    }

    pub fn construct(config: <Self as Chip<F>>::Config) -> Self {
        Self {
            config,
            _marker: PhantomData,
        }
    }
}
// ANCHOR_END: chip-config

impl<F: FieldExt> Chip<F> for MuxChip<F> {
    type Config = MuxConfig;
    type Loaded = ();

    fn config(&self) -> &Self::Config {
        &self.config
    }

    fn loaded(&self) -> &Self::Loaded {
        &()
    }
}

impl<F: FieldExt> MuxInstructions<F> for MuxChip<F> {
    type Cell = CellValue<F>;

    fn mux(
        &self,
        mut layouter: impl Layouter<F>,
        a: Self::Cell,
        b: Self::Cell,
        selector: Self::Cell,
    ) -> Result<Self::Cell, Error> {
        let config = self.config();

        let mut out = None;
        layouter.assign_region(
            || "mux",
            |mut region: Region<'_, F>| {
                let mut row_offset = 0;

                let a_cell = region.assign_advice(
                    || "a",
                    config.advice[0],
                    row_offset,
                    || a.value.ok_or(Error::SynthesisError),
                )?;
                let b_cell = region.assign_advice(
                    || "b",
                    config.advice[1],
                    row_offset,
                    || b.value.ok_or(Error::SynthesisError),
                )?;
                let selector_cell = region.assign_advice(
                    || "selector",
                    config.advice[2],
                    row_offset,
                    || selector.value.ok_or(Error::SynthesisError),
                )?;
                region.constrain_equal(a.cell, a_cell)?;
                region.constrain_equal(b.cell, b_cell)?;
                region.constrain_equal(selector.cell, selector_cell)?;

                config.s_bool.enable(&mut region, row_offset)?;
                config.s_mux.enable(&mut region, row_offset)?;

                row_offset += 1;

                let mux_value: F = if selector.value == Some(F::zero()) {
                    a.value.ok_or(Error::SynthesisError)?
                } else {
                    b.value.ok_or(Error::SynthesisError)?
                };

                let mux_cell = region.assign_advice(
                    || "mux result",
                    config.advice[0],
                    row_offset,
                    || Ok(mux_value),
                )?;

                out = Some(CellValue { cell: mux_cell, value: Some(mux_value) });
                Ok(())
            },
        )?;

        Ok(out.unwrap())
    }
}
// ANCHOR END: add-instructions-impl
