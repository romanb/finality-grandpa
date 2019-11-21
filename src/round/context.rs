
//! The context of a GRANDPA round tracks the set of voters
//! and equivocations for the purpose of computing vote weights.

use crate::bitfield::{Bitfield, Bit1};
use crate::voter_set::{VoterSet, VoterInfo};
use crate::std::hash::Hash;
use crate::std::ops::AddAssign;
use crate::weights::VoteWeight;

use super::Phase;

/// The context of a `Round` in which vote weights are calculated.
#[cfg_attr(feature = "std", derive(Debug))]
#[cfg_attr(test, derive(Clone))]
pub struct Context<T: Eq + Hash> {
	voters: VoterSet<T>,
	equivocations: Bitfield,
}

impl<T: Eq + Hash> Context<T> {
	/// Create a new context for a round with the given set of voters.
	pub fn new(voters: VoterSet<T>) -> Self {
		Context {
			voters,
			equivocations: Bitfield::new(),
		}
	}

	/// Get the set of voters.
	pub fn voters(&self) -> &VoterSet<T> {
		&self.voters
	}

	/// Get the weight of observed equivocations in phase `p`.
	pub fn equivocation_weight(&self, p: Phase) -> VoteWeight {
		match p {
			Phase::Prevote => weight(self.equivocations.iter1s_even(), &self.voters),
			Phase::Precommit => weight(self.equivocations.iter1s_odd(), &self.voters),
		}
	}

	/// Record voter `v` as an equivocator in phase `p`.
	pub fn equivocated(&mut self, v: &VoterInfo, p: Phase) {
		self.equivocations.set_bit(Vote::new(v, p).bit.position);
	}

	/// Compute the vote weight on node `n` in phase `p`, taking into account
	/// equivocations.
	pub fn weight(&self, n: &VoteNode, p: Phase) -> VoteWeight {
		if self.equivocations.is_blank() {
			match p {
				Phase::Prevote => weight(n.bits.iter1s_even(), &self.voters),
				Phase::Precommit => weight(n.bits.iter1s_odd(), &self.voters),
			}
		} else {
			match p {
				Phase::Prevote => {
					let bits = n.bits.iter1s_merged_even(&self.equivocations);
					weight(bits, &self.voters)
				}
				Phase::Precommit => {
					let bits = n.bits.iter1s_merged_odd(&self.equivocations);
					weight(bits, &self.voters)
				}
			}
		}
	}
}

/// A single vote that can be incorporated into a `VoteNode`.
pub struct Vote { bit: Bit1 }

impl Vote {
	/// Create a new vote cast by voter `v` in phase `p`.
	pub fn new(v: &VoterInfo, p: Phase) -> Vote {
		Vote {
			bit: Bit1 {
				position: match p {
					Phase::Prevote => v.position() * 2,
					Phase::Precommit => v.position() * 2 + 1
				}
			}
		}
	}

	/// Get the voter who cast the vote from the given voter set,
	/// if it is contained in that set.
	fn voter<'a, Id>(&'a self, vs: &'a VoterSet<Id>) -> Option<(&'a Id, &'a VoterInfo)>
	where
		Id: Eq + Hash
	{
		vs.nth(self.bit.position / 2)
	}
}

/// A node on which `Vote`s can be accumulated, for use in a `VoteGraph`.
///
/// The weight of any `VoteNode` is always computed in a `Context`,
/// taking into account equivocations. See [`Context::weight`].
#[derive(Clone)]
#[cfg_attr(feature = "std", derive(Debug))]
pub struct VoteNode {
	bits: Bitfield,
}

impl Default for VoteNode {
	fn default() -> Self {
		Self {
			bits: Bitfield::new(),
		}
	}
}

impl AddAssign<&VoteNode> for VoteNode {
	fn add_assign(&mut self, rhs: &VoteNode) {
		self.bits.merge(&rhs.bits);
	}
}

impl AddAssign<&Vote> for VoteNode {
	fn add_assign(&mut self, rhs: &Vote) {
		self.bits.set_bit(rhs.bit.position);
	}
}

/// Compute the prevote and precommit weights of a sequence
/// of vote-bits in the context of the given set of voters.
fn weight<V, I>(bits: I, voters: &VoterSet<V>) -> VoteWeight
where
	V: Eq + Hash,
	I: Iterator<Item = Bit1>
{
	let mut total = VoteWeight(0);

	for bit in bits {
		let vote = Vote { bit };
		if let Some((_id, v)) = vote.voter(voters) {
			total = total + v.weight()
		}
	}

	total
}

#[cfg(test)]
mod tests {
	use super::*;
	use quickcheck::*;
	use rand::Rng;
	use rand::seq::SliceRandom;

	impl Arbitrary for Phase {
		fn arbitrary<G: Gen>(g: &mut G) -> Self {
			*[Phase::Prevote, Phase::Precommit].choose(g).unwrap()
		}
	}

	impl Arbitrary for Context<usize> {
		fn arbitrary<G: Gen>(g: &mut G) -> Self {
			let mut ctx = Context::new(VoterSet::arbitrary(g));
			let n = g.gen_range(0, ctx.voters.len().get());
			let equivocators = (0 ..= n)
				.map(|_| ctx.voters.nth_mod(g.gen()).1.clone())
				.collect::<Vec<_>>();
			for v in equivocators {
				ctx.equivocated(&v, Phase::arbitrary(g))
			}
			ctx
		}
	}

	#[test]
	fn vote_voter() {
		fn prop(vs: VoterSet<usize>, phase: Phase) {
			for (id, v) in vs.iter() {
				assert_eq!(Vote::new(v, phase).voter(&vs), Some((id, v)))
			}
		}

		quickcheck(prop as fn(_,_))
	}

	#[test]
	fn weights() {
		fn prop(ctx: Context<usize>, phase: Phase, voters: Vec<usize>) {
			let e = ctx.equivocation_weight(phase);
			let t = ctx.voters.total_weight();

			// The equivocation weight must never be larger than the total
			// voter weight.
			assert!(e <= t);

			// Let a random subset of voters cast a vote, whether already
			// an equivocator or not.
			let mut n = VoteNode::default();
			let mut expected = e;
			for v in voters {
				let (_id, v) = ctx.voters.nth_mod(v);
				let vote = Vote::new(v, phase);

				// We only expect the weight to increase if the voter did not
				// start out as an equivocator and did not yet vote.
				if !ctx.equivocations.test_bit(vote.bit.position) &&
					!n.bits.test_bit(vote.bit.position)
				{
					expected = expected + v.weight();
				}

				n += &vote;
			}

			// Let the context compute the weight.
			let w = ctx.weight(&n, phase);

			// A vote-node weight must never be greater than the total voter weight.
			assert!(w <= t);

			assert_eq!(w, expected);
		}

		quickcheck(prop as fn(_,_,_))
	}
}