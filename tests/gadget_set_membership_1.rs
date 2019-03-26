extern crate bulletproofs;
extern crate curve25519_dalek;
extern crate merlin;

use bulletproofs::r1cs::{ConstraintSystem, R1CSError, R1CSProof, Variable, Prover, Verifier};
use curve25519_dalek::scalar::Scalar;
use bulletproofs::{BulletproofGens, PedersenGens};
use curve25519_dalek::ristretto::CompressedRistretto;
use bulletproofs::r1cs::LinearCombination;

mod utils;
use utils::AllocatedScalar;


pub fn set_membership_1_gadget<CS: ConstraintSystem>(
    cs: &mut CS,
    v: AllocatedScalar,
    diff_vars: Vec<AllocatedScalar>,
    set: &[u64]
) -> Result<(), R1CSError> {
    let set_length = set.len();
    // Accumulates product of elements in `diff_vars`
    let mut product: LinearCombination = Variable::One().into();

    for i in 0..set_length {
        // Take difference of value and each set element, `v - set[i]`
        let elem_lc: LinearCombination = vec![(Variable::One(), Scalar::from(set[i]))].iter().collect();
        let v_minus_elem = v.variable - elem_lc;

        // Since `diff_vars[i]` is `set[i] - v`, `v - set[i]` + `diff_vars[i]` should be 0
        cs.constrain(diff_vars[i].variable + v_minus_elem);

        let (_, _, o) = cs.multiply(product.clone(), diff_vars[i].variable.into());
        product = o.into();
    }

    // Ensure product of elements if `diff_vars` is 0
    cs.constrain(product);

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use merlin::Transcript;

    #[test]
    fn set_membership_1_check_gadget() {
        let set: Vec<u64> = vec![2, 3, 5, 6, 8, 20, 25];
        let value = 20u64;

        assert!(set_membership_1_check_helper(value, set).is_ok());
    }

    // Prove that difference between 1 set element and value is zero, hence value does not equal any set element.
    // For this create a vector of differences and prove that product of elements of such vector is 0
    fn set_membership_1_check_helper(value: u64, set: Vec<u64>) -> Result<(), R1CSError> {
        let pc_gens = PedersenGens::default();
        let bp_gens = BulletproofGens::new(128, 1);

        let set_length = set.len();

        let (proof, commitments) = {
            let mut comms: Vec<CompressedRistretto> = vec![];
            let mut diff_vars: Vec<AllocatedScalar> = vec![];

            let mut prover_transcript = Transcript::new(b"SetMemebership1Test");
            let mut rng = rand::thread_rng();

            let mut prover = Prover::new(&bp_gens, &pc_gens, &mut prover_transcript);
            let value = Scalar::from(value);
            let (com_value, var_value) = prover.commit(value.clone(), Scalar::random(&mut rng));
            let alloc_scal = AllocatedScalar {
                variable: var_value,
                assignment: Some(value),
            };
            comms.push(com_value);

            for i in 0..set_length {
                let elem = Scalar::from(set[i]);
                let diff = elem - value;

                // Take difference of set element and value, `set[i] - value`
                let (com_diff, var_diff) = prover.commit(diff.clone(), Scalar::random(&mut rng));
                let alloc_scal_diff = AllocatedScalar {
                    variable: var_diff,
                    assignment: Some(diff),
                };
                diff_vars.push(alloc_scal_diff);
                comms.push(com_diff);
            }

            assert!(set_membership_1_gadget(&mut prover, alloc_scal, diff_vars, &set).is_ok());

            println!("For set size {}, no of constraints is {}", &set_length, &prover.num_constraints());

            let proof = prover.prove()?;

            (proof, comms)
        };

        let mut verifier_transcript = Transcript::new(b"SetMemebership1Test");
        let mut verifier = Verifier::new(&bp_gens, &pc_gens, &mut verifier_transcript);
        let mut diff_vars: Vec<AllocatedScalar> = vec![];

        let var_val = verifier.commit(commitments[0]);
        let alloc_scal = AllocatedScalar {
            variable: var_val,
            assignment: None,
        };

        for i in 1..set_length+1 {
            let var_diff = verifier.commit(commitments[i]);
            let alloc_scal_diff = AllocatedScalar {
                variable: var_diff,
                assignment: None,
            };
            diff_vars.push(alloc_scal_diff);
        }

        assert!(set_membership_1_gadget(&mut verifier, alloc_scal, diff_vars, &set).is_ok());

        Ok(verifier.verify(&proof)?)
    }
}