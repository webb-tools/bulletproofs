extern crate bulletproofs;
extern crate curve25519_dalek;
extern crate merlin;

use bulletproofs::r1cs::{ConstraintSystem, R1CSError, R1CSProof, Variable, Prover, Verifier};
use curve25519_dalek::scalar::Scalar;
use bulletproofs::{BulletproofGens, PedersenGens};
use curve25519_dalek::ristretto::CompressedRistretto;
use bulletproofs::r1cs::LinearCombination;

mod utils;
use utils::{AllocatedScalar, is_nonzero_gadget};


pub fn set_non_membership_gadget<CS: ConstraintSystem>(
    cs: &mut CS,
    v: AllocatedScalar,
    diff_vars: Vec<AllocatedScalar>,
    diff_inv_vars: Vec<AllocatedScalar>,
    set: &[u64]
) -> Result<(), R1CSError> {
    let set_length = set.len();

    for i in 0..set_length {
        // Take difference of value and each set element, `v - set[i]`
        let elem_lc: LinearCombination = vec![(Variable::One(), Scalar::from(set[i]))].iter().collect();
        let v_minus_elem = v.variable - elem_lc;

        // Since `diff_vars[i]` is `set[i] - v`, `v - set[i]` + `diff_vars[i]` should be 0
        cs.constrain(diff_vars[i].variable + v_minus_elem);

        // Ensure `set[i] - v` is non-zero
        is_nonzero_gadget(cs, diff_vars[i], diff_inv_vars[i])?;
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use merlin::Transcript;
    use std::time::{Duration, Instant};

    #[test]
    fn set_non_membership_check_gadget() {
        let set: Vec<u64> = vec![5, 9, 32, 1, 85, 2, 7, 11, 14, 26];
        let value = 19u64;

        assert!(set_non_membership_check_helper(value, set).is_ok());
    }

    // Prove that difference between each set element and value is non-zero, hence value does not equal any set element.
    fn set_non_membership_check_helper(value: u64, set: Vec<u64>) -> Result<(), R1CSError> {
        let pc_gens = PedersenGens::default();
        let bp_gens = BulletproofGens::new(128, 1);

        let set_length = set.len();

        let start = Instant::now();
        let (proof, commitments) = {
            let mut comms: Vec<CompressedRistretto> = vec![];
            let mut diff_vars: Vec<AllocatedScalar> = vec![];
            let mut diff_inv_vars: Vec<AllocatedScalar> = vec![];

            let mut prover_transcript = Transcript::new(b"SetNonMemebershipTest");
            let mut rng = rand::thread_rng();

            let mut prover = Prover::new(&pc_gens, &mut prover_transcript);
            let value= Scalar::from(value);
            let (com_value, var_value) = prover.commit(value.clone(), Scalar::random(&mut rng));
            let alloc_scal = AllocatedScalar {
                variable: var_value,
                assignment: Some(value),
            };
            comms.push(com_value);

            for i in 0..set_length {
                let elem = Scalar::from(set[i]);
                let diff = elem - value;
                let diff_inv = diff.invert();

                // Take difference of set element and value, `set[i] - value`
                let (com_diff, var_diff) = prover.commit(diff.clone(), Scalar::random(&mut rng));
                let alloc_scal_diff = AllocatedScalar {
                    variable: var_diff,
                    assignment: Some(diff),
                };
                diff_vars.push(alloc_scal_diff);
                comms.push(com_diff);

                // Inverse needed to prove that difference `set[i] - value` is non-zero
                let (com_diff_inv, var_diff_inv) = prover.commit(diff_inv.clone(), Scalar::random(&mut rng));
                let alloc_scal_diff_inv = AllocatedScalar {
                    variable: var_diff_inv,
                    assignment: Some(diff_inv),
                };
                diff_inv_vars.push(alloc_scal_diff_inv);
                comms.push(com_diff_inv);
            }

            assert!(set_non_membership_gadget(&mut prover, alloc_scal, diff_vars, diff_inv_vars, &set).is_ok());

            println!("For set size {}, no of constraints is {}", &set_length, &prover.num_constraints());

            let proof = prover.prove(&bp_gens)?;

            (proof, comms)
        };
        println!("Proving time for set nonmembership for set length {} is {:?}", &set.len(), start.elapsed());
        println!("Proof size is {}", proof.to_bytes().len());

        let start = Instant::now();
        let mut verifier_transcript = Transcript::new(b"SetNonMemebershipTest");
        let mut verifier = Verifier::new(&mut verifier_transcript);
        let mut diff_vars: Vec<AllocatedScalar> = vec![];
        let mut diff_inv_vars: Vec<AllocatedScalar> = vec![];

        let var_val = verifier.commit(commitments[0]);
        let alloc_scal = AllocatedScalar {
            variable: var_val,
            assignment: None,
        };

        for i in 1..set_length+1 {
            let var_diff = verifier.commit(commitments[2*i-1]);
            let alloc_scal_diff = AllocatedScalar {
                variable: var_diff,
                assignment: None,
            };
            diff_vars.push(alloc_scal_diff);

            let var_diff_inv = verifier.commit(commitments[2*i]);
            let alloc_scal_diff_inv = AllocatedScalar {
                variable: var_diff_inv,
                assignment: None,
            };
            diff_inv_vars.push(alloc_scal_diff_inv);
        }

        assert!(set_non_membership_gadget(&mut verifier, alloc_scal, diff_vars, diff_inv_vars, &set).is_ok());

        let r = Ok(verifier.verify(&proof, &pc_gens, &bp_gens)?);
        println!("Verification time for set membership for set length {} is {:?}", &set.len(), start.elapsed());
        r
    }
}