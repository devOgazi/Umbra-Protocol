# API Reference

> **Status:** Complete for Week 1. Covers both Soroban contracts and their
> TypeScript SDKs.

---

## Table of Contents

1. [Umbra Audit — Soroban Contract](#1-umbra-audit--soroban-contract)
2. [Umbra Escrow — Soroban Contract](#2-umbra-escrow--soroban-contract)
3. [@umbra/audit-sdk — TypeScript SDK](#3-umbraaudit-sdk--typescript-sdk)
4. [@umbra/escrow-sdk — TypeScript SDK](#4-umbraescrow-sdk--typescript-sdk)

---

## 1. Umbra Audit — Soroban Contract

### Contract ID

Deployed via `soroban contract deploy`. The ID is a Stellar contract hash
(e.g., `CA...`).

### Types

#### `DataKey`

```rust
pub enum DataKey {
    Initialized,
    Regulator,
    VerifierKey,
    Threshold(BytesN<4>), // keyed by 4-byte asset code
}
```

#### `ProofVerifiedEvent`

```rust
pub struct ProofVerifiedEvent {
    pub entity: Address,
    pub asset_code: BytesN<4>,
    pub threshold: u64,
    pub timestamp: u64,
    pub commitment: BytesN<32>, // Pedersen commitment (opaque)
    pub passed: bool,
}
```

### Functions

#### `init(regulator: Address, verifier_key: BytesN<32>)`

- **Auth:** None (called once during deployment)
- **Panics:** If already initialized
- **Emits:** `("init", regulator)`

#### `set_threshold(asset_code: BytesN<4>, threshold: u64)`

- **Auth:** `regulator.require_auth()`
- **Panics:** If contract not initialized; if caller is not regulator
- **Emits:** `("set_thr", asset_code, threshold)`

#### `set_verifier_key(verifier_key: BytesN<32>)`

- **Auth:** `regulator.require_auth()`
- **Emits:** `("set_vk", verifier_key)`

#### `get_threshold(asset_code: BytesN<4>) -> Option<u64>`

- **Returns:** `Some(threshold)` if set, `None` otherwise

#### `submit_proof(entity: Address, asset_code: BytesN<4>, commitment: BytesN<32>, proof_result: u32, signature: BytesN<64>, timestamp: u64) -> bool`

- **Auth:** `entity.require_auth()`
- **Panics:** If no threshold set for asset; if Ed25519 signature is invalid
- **Emits:** `("pv", ProofVerifiedEvent)`
- **Returns:** `true` if attested proof passed, `false` if attested failed

#### `submit_proofs_multi(entity: Address, proofs_blob: Bytes, timestamp: u64) -> bool`

- **Auth:** `entity.require_auth()`
- **Panics:** If blob encoding is invalid; if any asset lacks a threshold
- **Emits:** One `("pv", ProofVerifiedEvent)` per asset
- **Returns:** `true` if ALL attested proofs passed, `false` otherwise

#### `get_regulator() -> Address`

#### `get_verifier_key() -> BytesN<32>`

#### `version() -> u32`

### Events

| Topic | Payload | Description |
|---|---|---|
| `"init"` | `regulator: Address` | Contract initialized |
| `"set_thr"` | `asset_code, threshold` | Threshold updated |
| `"set_vk"` | `verifier_key` | Verifier key rotated |
| `"pv"` | `ProofVerifiedEvent` | Proof submitted and verified |

---

## 2. Umbra Escrow — Soroban Contract

### Types

#### `ReleaseCondition`

```rust
pub enum ReleaseCondition {
    DeliveryOracle(Address),
    MultiSig(MultiSigParams),
}
```

#### `MultiSigParams`

```rust
pub struct MultiSigParams {
    pub required: u32,
    pub signers: Vec<Address>,
}
```

#### `EscrowStatus`

```rust
pub enum EscrowStatus {
    Active,
    Released,
    Cancelled,
}
```

#### `EscrowRecord`

```rust
pub struct EscrowRecord {
    pub escrow_id: u64,
    pub buyer: Address,
    pub supplier: Address,
    pub commitment: BytesN<32>, // Pedersen commitment (opaque)
    pub condition: ReleaseCondition,
    pub status: EscrowStatus,
    pub created_at: u64,
}
```

#### `EscrowCreatedEvent`

```rust
pub struct EscrowCreatedEvent {
    pub escrow_id: u64,
    pub buyer: Address,
    pub supplier: Address,
    pub commitment: BytesN<32>,
    pub condition_type: Symbol,
}
```

#### `EscrowReleasedEvent`

```rust
pub struct EscrowReleasedEvent {
    pub escrow_id: u64,
    pub supplier: Address,
    pub commitment: BytesN<32>,
    pub release_path: Symbol,
}
```

#### `EscrowCancelledEvent`

```rust
pub struct EscrowCancelledEvent {
    pub escrow_id: u64,
    pub buyer: Address,
}
```

#### `DisputeDisclosureEvent`

```rust
pub struct DisputeDisclosureEvent {
    pub escrow_id: u64,
    pub arbitrator: Address,
    pub value: u64,
    pub commitment: BytesN<32>,
}
```

### Functions

#### `init(admin: Address, arbitrator: Address, verifier_key: BytesN<32>)`

- **Auth:** None (called once during deployment)
- **Emits:** `("init", (admin, arbitrator))`

#### `create_escrow(buyer: Address, supplier: Address, commitment: BytesN<32>, condition: ReleaseCondition) -> u64`

- **Auth:** `buyer.require_auth()`
- **Emits:** `("escrow_created", EscrowCreatedEvent)`
- **Returns:** The newly assigned escrow ID (monotonic counter)

#### `release_oracle(escrow_id: u64, signature: BytesN<64>)`

- **Auth:** Oracle address in the escrow's condition
- **Panics:** Escrow not active; condition is not DeliveryOracle; bad signature
- **Emits:** `("escrow_released", EscrowReleasedEvent)`

#### `approve_multisig(escrow_id: u64, approver: Address, signature: BytesN<64>) -> bool`

- **Auth:** `approver.require_auth()`
- **Returns:** `true` if threshold reached (escrow released), `false` otherwise
- **Emits:** `("escrow_released", ...)` when threshold reached

#### `cancel_escrow(escrow_id: u64)`

- **Auth:** `buyer.require_auth()`
- **Emits:** `("escrow_cancelled", EscrowCancelledEvent)`

#### `request_disclosure(escrow_id: u64, value: u64, blinding_bytes: BytesN<32>) -> EscrowRecord`

- **Auth:** `arbitrator.require_auth()`
- **Panics:** If value+blinding do not open the stored commitment
- **Emits:** `("dispute_disclosed", DisputeDisclosureEvent)`

#### `get_escrow(escrow_id: u64) -> EscrowRecord`

#### `next_escrow_id() -> u64`

#### `get_admin() -> Address`

#### `get_arbitrator() -> Address`

#### `get_verifier_key() -> BytesN<32>`

#### `set_verifier_key(verifier_key: BytesN<32>)`

- **Auth:** `admin.require_auth()`
- **Emits:** `("set_vk", verifier_key)`

#### `version() -> u32`

### Events

| Topic | Payload | Description |
|---|---|---|
| `"init"` | `(admin, arbitrator)` | Contract initialized |
| `"escrow_created"` | `EscrowCreatedEvent` | New escrow created |
| `"escrow_released"` | `EscrowReleasedEvent` | Escrow released |
| `"escrow_cancelled"` | `EscrowCancelledEvent` | Escrow cancelled |
| `"dispute_disclosed"` | `DisputeDisclosureEvent` | Arbitrator disclosed value |
| `"set_vk"` | `verifier_key` | Verifier key rotated |

---

## 3. @umbra/audit-sdk — TypeScript SDK

### Installation

```bash
npm install @umbra/audit-sdk
```

### `AuditClient`

#### Constructor

```typescript
const client = new AuditClient(config: AuditClientConfig);
```

| Parameter | Type | Description |
|---|---|---|
| `config.contractId` | `string` | Deployed Umbra Audit contract ID (e.g., `CA...`) |
| `config.network` | `"local" \| "testnet" \| "futurenet"` | Target Stellar network |
| `config.rpcUrl?` | `string` | Optional override for Soroban RPC URL |

#### `generateReserveProof(input: ReserveProofInput): Promise<ReserveProof>`

```typescript
const proof = await client.generateReserveProof({
  actualReserves: privateReserveAmount, // bigint, never leaves this call
  threshold: publicThreshold,           // bigint
  assetCode: "USDC",                    // 4-char asset code
});
```

**Parameters (`ReserveProofInput`):**

| Field | Type | Description |
|---|---|---|
| `actualReserves` | `bigint` | **Private.** The company's actual reserve balance. Used only within this call to generate the commitment and proof result. Never included in the returned proof object. |
| `threshold` | `bigint` | **Public.** The regulator-defined compliance threshold for the asset. |
| `assetCode` | `string` | 4-character asset code (e.g., `"USDC"`, `"XLM_"`, `"BTC"` padded). |

**Returns (`ReserveProof`):**

| Field | Type | Description |
|---|---|---|
| `commitment` | `string` | Hex-encoded 32-byte Pedersen commitment. Opaque — does not reveal the reserve amount. |
| `proofResult` | `number` | `0x01` if reserves >= threshold, `0x00` otherwise. |
| `signature` | `string` | Hex-encoded 64-byte Ed25519 signature from the off-chain verifier. |
| `timestamp` | `number` | Unix timestamp when the proof was generated. |
| `assetCode` | `string` | The asset code (echoed from input). |
| `threshold` | `string` | The threshold value as a decimal string (echoed from input). |

**Privacy:** The `actualReserves` value is used only within this function. It
is not stored in the returned `ReserveProof` object.

#### `submitProof(proof: ReserveProof, options: SubmitProofOptions): Promise<SubmitProofResult>`

```typescript
const result = await client.submitProof(proof, {
  source: companyKeypair,
});
```

**Parameters:**

| Parameter | Type | Description |
|---|---|---|
| `proof` | `ReserveProof` | The proof object returned by `generateReserveProof`. |
| `options.source` | `Keypair` | Stellar keypair of the submitting entity. Must be funded and authorized. |

**Returns (`SubmitProofResult`):**

| Field | Type | Description |
|---|---|---|
| `verified` | `boolean` | `true` if the attested proof passed, `false` if it failed. No balance or private data is returned. |

---

## 4. @umbra/escrow-sdk — TypeScript SDK

### Installation

```bash
npm install @umbra/escrow-sdk
```

### `EscrowClient`

#### Constructor

```typescript
const client = new EscrowClient(config: EscrowClientConfig);
```

| Parameter | Type | Description |
|---|---|---|
| `config.contractId` | `string` | Deployed Umbra Escrow contract ID (e.g., `CA...`) |
| `config.network` | `"local" \| "testnet" \| "futurenet"` | Target Stellar network |
| `config.rpcUrl?` | `string` | Optional override for Soroban RPC URL |

#### `createEscrow(input: CreateEscrowInput): Promise<CreateEscrowResult>`

```typescript
const escrow = await client.createEscrow({
  supplier: supplierAddress,
  amount: 125_000_00,
  releaseCondition: { type: "delivery-oracle", oracleId: "..." },
  source: buyerKeypair,
});
```

**Parameters (`CreateEscrowInput`):**

| Field | Type | Description |
|---|---|---|
| `supplier` | `string` | Stellar public key (G...) of the escrow recipient. |
| `amount` | `bigint` | **Private.** The escrow amount. Used only to generate a Pedersen commitment; NEVER sent to the contract in plaintext. |
| `releaseCondition` | `ReleaseConditionType` | Either `{ type: "delivery-oracle", oracleId: string }` or `{ type: "multisig", required: number, signers: string[] }`. |
| `source` | `Keypair` | Stellar keypair of the buyer. Must sign the transaction. |

**Returns (`CreateEscrowResult`):**

| Field | Type | Description |
|---|---|---|
| `id` | `number` | The newly created escrow's unique identifier. The amount is NOT echoed. |

**Privacy:** The `amount` is never logged, stored, or included in the return
value. Only a 32-byte Pedersen commitment is submitted on-chain.
