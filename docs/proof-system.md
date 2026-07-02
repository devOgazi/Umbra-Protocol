# Proof System

> **Status:** Placeholder — to be completed in Day 5.

This document will specify:
- The ZK proof scheme used by Umbra Audit and Umbra Escrow
- Circuit design (range proofs, equality proofs, ratio proofs)
- Trusted setup process (if applicable)
- Pluggable backend architecture

Current implementation uses **Bulletproofs** as the primary range-proof scheme (see `contracts/umbra-crypto/src/range_proof.rs`), with Pedersen commitments for hiding values (see `contracts/umbra-crypto/src/commitment.rs`).
