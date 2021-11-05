use std::marker::PhantomData;

use halo2::{
    arithmetic::FieldExt,
    circuit::{Cell, Chip, Layouter, Region, SimpleFloorPlanner},
    plonk::{Advice, Circuit, Column, ConstraintSystem, Error, Instance, Selector},
    poly::Rotation,
};

// ANCHOR: field-instructions
/// A variable representing a number.
#[derive(Clone)]
struct Number<F: FieldExt> {
    cell: Cell,
    value: Option<F>,
}

trait FieldInstructions<F: FieldExt>: MuxInstructions<F> {
    /// Variable representing a number.
    type Num;

    /// Loads a number into the circuit as a private input.
    fn load_private(
        &self,
        layouter: impl Layouter<F>,
        a: Option<F>,
    ) -> Result<<Self as FieldInstructions<F>>::Num, Error>;

    /// Returns `d = (b - a) * c + a`.
    fn mux(
        &self,
        layouter: &mut impl Layouter<F>,
        a: <Self as FieldInstructions<F>>::Num,
        b: <Self as FieldInstructions<F>>::Num,
        c: <Self as FieldInstructions<F>>::Num,
    ) -> Result<<Self as FieldInstructions<F>>::Num, Error>;

    /// Exposes a number as a public input to the circuit.
    fn expose_public(
        &self,
        layouter: impl Layouter<F>,
        num: <Self as FieldInstructions<F>>::Num,
        row: usize,
    ) -> Result<(), Error>;
}
// ANCHOR_END: field-instructions

// ANCHOR: mux-instructions
trait MuxInstructions<F: FieldExt>: Chip<F> {
    /// Variable representing a number.
    type Num;

    /// Returns `c = a + b`.
    fn do_mux(
        &self,
        layouter: impl Layouter<F>,
        a: Self::Num,
        b: Self::Num,
        c: Self::Num,
    ) -> Result<Self::Num, Error>;
}
// ANCHOR_END: mux-instructions

// ANCHOR: field-config
// The top-level config that provides all necessary columns and permutations
// for the other configs.
#[derive(Clone, Debug)]
struct FieldConfig {
    /// For this chip, we will use three advice columns to implement our instructions.
    /// These are also the columns through which we communicate with other parts of
    /// the circuit.
    advice: [Column<Advice>; 3],

    /// Public inputs
    instance: Column<Instance>,

    mux_config: MuxConfig,
}
// ANCHOR END: field-config

// ANCHOR: mux-config
#[derive(Clone, Debug)]
struct MuxConfig {
    advice: [Column<Advice>; 3],
    s_mux: Selector,
}
// ANCHOR_END: mux-config

// ANCHOR: field-chip
/// The top-level chip that will implement the `FieldInstructions`.
struct FieldChip<F: FieldExt> {
    config: FieldConfig,
    _marker: PhantomData<F>,
}
// ANCHOR_END: field-chip

// ANCHOR: mux-chip
struct MuxChip<F: FieldExt> {
    config: MuxConfig,
    _marker: PhantomData<F>,
}
// ANCHOR END: mux-chip

// ANCHOR: mux-chip-trait-impl
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
// ANCHOR END: mux-chip-trait-impl

// ANCHOR: mux-chip-impl
impl<F: FieldExt> MuxChip<F> {
    fn construct(config: <Self as Chip<F>>::Config, _loaded: <Self as Chip<F>>::Loaded) -> Self {
        Self {
            config,
            _marker: PhantomData,
        }
    }

    fn configure(
        meta: &mut ConstraintSystem<F>,
        advice: [Column<Advice>; 3],
    ) -> <Self as Chip<F>>::Config {
        let s_mux = meta.selector();

        // Define our mux gate!
        meta.create_gate("mux", |meta| {
            let lhs = meta.query_advice(advice[0], Rotation::cur());
            let rhs = meta.query_advice(advice[1], Rotation::cur());
            let xhs = meta.query_advice(advice[2], Rotation::cur());
            let out = meta.query_advice(advice[0], Rotation::next());
            let s_mux = meta.query_selector(s_mux);

            //d = (b - a) * c;// + a;
            vec![s_mux * ((rhs - lhs.clone()) * xhs.clone() - out)]
        });

        MuxConfig { advice, s_mux }
    }
}
// ANCHOR END: mux-chip-impl

// ANCHOR: mux-instructions-impl
impl<F: FieldExt> MuxInstructions<F> for FieldChip<F> {
    type Num = Number<F>;
    fn do_mux(
        &self,
        layouter: impl Layouter<F>,
        a: Self::Num,
        b: Self::Num,
        c: Self::Num,
    ) -> Result<Self::Num, Error> {
        let config = self.config().mux_config.clone();

        let mux_chip = MuxChip::<F>::construct(config, ());
        mux_chip.do_mux(layouter, a, b, c)
    }
}

impl<F: FieldExt> MuxInstructions<F> for MuxChip<F> {
    type Num = Number<F>;

    fn do_mux(
        &self,
        mut layouter: impl Layouter<F>,
        a: Self::Num,
        b: Self::Num,
        c: Self::Num,
    ) -> Result<Self::Num, Error> {
        let config = self.config();

        let mut out = None;
        layouter.assign_region(
            || "mux",
            |mut region: Region<'_, F>| {
                // We only want to use a single mux gate in this region,
                // so we enable it at region offset 0; this means it will constrain
                // cells at offsets 0 and 1.
                config.s_mux.enable(&mut region, 0)?;

                // The inputs we've been given could be located anywhere in the circuit,
                // but we can only rely on relative offsets inside this region. So we
                // assign new cells inside the region and constrain them to have the
                // same values as the inputs.
                let lhs = region.assign_advice(
                    || "lhs",
                    config.advice[0],
                    0,
                    || a.value.ok_or(Error::SynthesisError),
                )?;
                let rhs = region.assign_advice(
                    || "rhs",
                    config.advice[1],
                    0,
                    || b.value.ok_or(Error::SynthesisError),
                )?;
                let xhs = region.assign_advice(
                    || "xhs",
                    config.advice[2],
                    0,
                    || b.value.ok_or(Error::SynthesisError),
                )?;
                region.constrain_equal(a.cell, lhs)?;
                region.constrain_equal(b.cell, rhs)?;
                region.constrain_equal(c.cell, xhs)?;

                // Now we can assign the mux result into the output position.
                let value = a.value.and_then(|a| b.value.map(|b| (b - a)));

                let cell = region.assign_advice(
                    || "lhs * rhs",
                    config.advice[0],
                    1,
                    || value.ok_or(Error::SynthesisError),
                )?;

                // Finally, we return a variable representing the output,
                // to be used in another part of the circuit.
                out = Some(Number { cell, value });
                Ok(())
            },
        )?;

        Ok(out.unwrap())
    }
}
// ANCHOR END: mux-instructions-impl

// ANCHOR: field-chip-trait-impl
impl<F: FieldExt> Chip<F> for FieldChip<F> {
    type Config = FieldConfig;
    type Loaded = ();

    fn config(&self) -> &Self::Config {
        &self.config
    }

    fn loaded(&self) -> &Self::Loaded {
        &()
    }
}
// ANCHOR_END: field-chip-trait-impl

// ANCHOR: field-chip-impl
impl<F: FieldExt> FieldChip<F> {
    fn construct(config: <Self as Chip<F>>::Config, _loaded: <Self as Chip<F>>::Loaded) -> Self {
        Self {
            config,
            _marker: PhantomData,
        }
    }

    fn configure(
        meta: &mut ConstraintSystem<F>,
        advice: [Column<Advice>; 3],
        instance: Column<Instance>,
    ) -> <Self as Chip<F>>::Config {
        let mux_config = MuxChip::configure(meta, advice);

        meta.enable_equality(instance.into());
        for column in &advice {
            meta.enable_equality((*column).into());
        }

        FieldConfig {
            advice,
            instance,
            mux_config,
        }
    }
}
// ANCHOR_END: field-chip-impl

// ANCHOR: field-instructions-impl
impl<F: FieldExt> FieldInstructions<F> for FieldChip<F> {
    type Num = Number<F>;

    fn load_private(
        &self,
        mut layouter: impl Layouter<F>,
        value: Option<F>,
    ) -> Result<<Self as FieldInstructions<F>>::Num, Error> {
        let config = self.config();

        let mut num = None;
        layouter.assign_region(
            || "load private",
            |mut region| {
                let cell = region.assign_advice(
                    || "private input",
                    config.advice[0],
                    0,
                    || value.ok_or(Error::SynthesisError),
                )?;
                num = Some(Number { cell, value });
                Ok(())
            },
        )?;
        Ok(num.unwrap())
    }

    /// Returns `d = (b - a) * c + a`
    fn mux(
        &self,
        layouter: &mut impl Layouter<F>,
        a: <Self as FieldInstructions<F>>::Num,
        b: <Self as FieldInstructions<F>>::Num,
        c: <Self as FieldInstructions<F>>::Num,
    ) -> Result<<Self as FieldInstructions<F>>::Num, Error> {
        self.do_mux(layouter.namespace(|| "(b - a) * c + a"), a, b, c)
    }

    fn expose_public(
        &self,
        mut layouter: impl Layouter<F>,
        num: <Self as FieldInstructions<F>>::Num,
        row: usize,
    ) -> Result<(), Error> {
        let config = self.config();

        layouter.constrain_instance(num.cell, config.instance, row)
    }
}
// ANCHOR_END: field-instructions-impl

// ANCHOR: circuit
/// The full circuit implementation.
///
/// In this struct we store the private input variables. We use `Option<F>` because
/// they won't have any value during key generation. During proving, if any of these
/// were `None` we would get an error.
#[derive(Default)]
struct MyCircuit<F: FieldExt> {
    a: Option<F>,
    b: Option<F>,
    c: Option<F>,
}

impl<F: FieldExt> Circuit<F> for MyCircuit<F> {
    // Since we are using a single chip for everything, we can just reuse its config.
    type Config = FieldConfig;
    type FloorPlanner = SimpleFloorPlanner;

    fn without_witnesses(&self) -> Self {
        Self::default()
    }

    fn configure(meta: &mut ConstraintSystem<F>) -> Self::Config {
        // We create the two advice columns that FieldChip uses for I/O.
        let advice = [meta.advice_column(), meta.advice_column(), meta.advice_column()];

        // We also need an instance column to store public inputs.
        let instance = meta.instance_column();

        FieldChip::configure(meta, advice, instance)
    }

    fn synthesize(
        &self,
        config: Self::Config,
        mut layouter: impl Layouter<F>,
    ) -> Result<(), Error> {
        let field_chip = FieldChip::<F>::construct(config, ());

        // Load our private values into the circuit.
        let a = field_chip.load_private(layouter.namespace(|| "load a"), self.a)?;
        let b = field_chip.load_private(layouter.namespace(|| "load b"), self.b)?;
        let c = field_chip.load_private(layouter.namespace(|| "load c"), self.c)?;

        // Use `mux` to get `d = (b - a) * c + a
        let d = field_chip.mux(&mut layouter, a, b, c)?;

        // Expose the result as a public input to the circuit.
        field_chip.expose_public(layouter.namespace(|| "expose d"), d, 0)
    }
}
// ANCHOR_END: circuit

#[allow(clippy::many_single_char_names)]
fn main() {
    use halo2::{dev::MockProver, pasta::Fp};

    // ANCHOR: test-circuit
    // The number of rows in our circuit cannot exceed 2^k. Since our example
    // circuit is very small, we can pick a very small value here.
    let k = 4;

    /*
     * (b - a) * c + a
     * (cb - ca) + a
     */

    // Prepare the private and public inputs to the circuit!
    let a = Fp::rand();
    let b = Fp::rand();
    let c = Fp::rand();
    let d = (b - a) * c;// + a;

    // Instantiate the circuit with the private inputs.
    let circuit = MyCircuit {
        a: Some(a),
        b: Some(b),
        c: Some(b),
    };

    // Arrange the public input. We expose the mux result in row 0
    // of the instance column, so we position it there in our public inputs.
    let mut public_inputs = vec![d];

    // Given the correct public input, our circuit will verify.
    let prover = MockProver::run(k, &circuit, vec![public_inputs.clone()]).unwrap();
    assert_eq!(prover.verify(), Ok(()));

    // If we try some other public input, the proof will fail!
    public_inputs[0] += Fp::one();
    let prover = MockProver::run(k, &circuit, vec![public_inputs]).unwrap();
    assert!(prover.verify().is_err());
    // ANCHOR_END: test-circuit
}

