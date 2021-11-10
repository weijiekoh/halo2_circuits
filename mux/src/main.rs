use std::marker::PhantomData;
use halo2::{
    arithmetic::FieldExt,
    circuit::{Layouter, SimpleFloorPlanner},
    plonk::{Advice, Instance, Circuit, Column, ConstraintSystem, Error},
    pasta::Fp
};

mod utils;
mod gadget;

use gadget:: {
    mux::{MuxChip, MuxConfig, MuxInstructions}
};

use crate:: {
    utils::{UtilitiesInstructions, CellValue}
};

// Absolute offsets for public inputs.
const MUX_OUTPUT: usize = 0;

#[derive(Clone, Debug)]
pub struct Config<F> {
    advice: [Column<Advice>; 3],
    instance: Column<Instance>,
    mux_config: MuxConfig,
    _marker: PhantomData<F>,
}


#[derive(Debug, Default)]
pub struct MuxCircuit<F> {
    a: Option<F>,
    b: Option<F>,
    selector: Option<F>
}

impl<F: FieldExt> UtilitiesInstructions<F> for MuxCircuit<F> {
    type Var = CellValue<F>;
}

impl<F: FieldExt> Circuit<F> for MuxCircuit<F> {
    type Config = Config<F>;
    type FloorPlanner = SimpleFloorPlanner;

    fn without_witnesses(&self) -> Self {
        Self::default()
    }

    fn configure(meta: &mut ConstraintSystem<F>) -> Self::Config {

        let advice = [
            meta.advice_column(),
            meta.advice_column(),
            meta.advice_column(),
        ];

        let instance = meta.instance_column();
        meta.enable_equality(instance.into());

        for advice in advice.iter() {
            meta.enable_equality((*advice).into());
        }

        let mux_config = MuxChip::configure(meta, advice);

        Config {
            advice, 
            instance,
            mux_config,
            _marker: PhantomData
        }
    }

    fn synthesize(
        &self,
        config: Self::Config,
        mut layouter: impl Layouter<F>,
    ) -> Result<(), Error> {

        let mux_chip = config.construct_mux_chip();
        
        let a = self.load_private(
            layouter.namespace(|| "witness a"),
            config.advice[0],
            self.a,
        )?;

        let b = self.load_private(
            layouter.namespace(|| "witness a"),
            config.advice[0],
            self.b,
        )?;

        let selector = self.load_private(
            layouter.namespace(|| "witness a"),
            config.advice[0],
            self.selector,
        )?;

        let mux_value = mux_chip.mux(layouter.namespace(|| "calculate mux"), a, b, selector)?;

        self.constrain_public(layouter.namespace(|| "constrain mux_value"), config.instance, mux_value, MUX_OUTPUT)?;
        Ok({})
    }
}


fn main() {
    use halo2::{dev::MockProver};

    let k = 4;

    let a = Fp::from(3);
    let b = Fp::from(2);
    let selector = Fp::from(0);

    let circuit = MuxCircuit {
        a: Some(a),
        b: Some(b),
        selector: Some(selector)
    };

    let mut public_inputs = vec![a];

    let prover = MockProver::run(k, &circuit, vec![public_inputs.clone()]).unwrap();
    assert_eq!(prover.verify(), Ok(()));

    public_inputs[0] = b;
    let prover = MockProver::run(k, &circuit, vec![public_inputs]).unwrap();
    assert!(prover.verify().is_err());
}