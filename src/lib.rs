// Copyright 2018 Parity Technologies (UK) Ltd.
// This file is part of finality-afg.

// finality-afg is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.

// finality-afg is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.

// You should have received a copy of the GNU General Public License
// along with finality-afg. If not, see <http://www.gnu.org/licenses/>.

//! Finality gadget for blockchains.
//!
//! https://hackmd.io/iA4XazxWRJ21LqMxwPSEZg?view

extern crate parking_lot;

mod bitfield;
mod round;
mod vote_graph;

#[cfg(test)]
mod testing;

use std::fmt;
/// A prevote for a block and its ancestors.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Prevote<H> {
	target_hash: H,
	target_number: u32,
}

impl<H> Prevote<H> {
	pub fn new(target_hash: H, target_number: u32) -> Self {
		Prevote { target_hash, target_number }
	}
}

/// A precommit for a block and its ancestors.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Precommit<H> {
	target_hash: H,
	target_number: u32,
}

impl<H> Precommit<H> {
	pub fn new(target_hash: H, target_number: u32) -> Self {
		Precommit { target_hash, target_number }
	}
}

#[derive(Clone, PartialEq, Debug)]
pub enum Error {
	BlockNotInSubtree,
}

impl fmt::Display for Error {
	fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
		match *self {
			Error::BlockNotInSubtree => write!(f, "Block not in subtree of base"),
		}
	}
}

impl ::std::error::Error for Error {
	fn description(&self) -> &str {
		match *self {
			Error::BlockNotInSubtree => "Block not in subtree of base",
		}
	}
}

/// Chain context necessary for implementation of the finality gadget.
pub trait Chain<H> {
	/// Get the ancestry of a block up to but not including the base hash.
	/// Should be in reverse order from `block`'s parent.
	///
	/// If the block is not a descendent of `base`, returns an error.
	fn ancestry(&self, base: H, block: H) -> Result<Vec<H>, Error>;
}

/// An equivocation (double-vote) in a given round.
#[derive(Debug, Clone, PartialEq)]
pub struct Equivocation<Id, V, S> {
	/// The round number equivocated in.
	pub round_number: u64,
	/// The identity of the equivocator.
	pub identity: Id,
	/// The first vote in the equivocation.
	pub	first: (V, S),
	/// The second vote in the equivocation.
	pub second: (V, S),
}