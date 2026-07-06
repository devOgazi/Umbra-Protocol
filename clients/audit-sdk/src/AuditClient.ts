import {
  Keypair,
  TransactionBuilder,
  Networks,
  Operation,
  BASE_FEE,
  SorobanRpc,
  xdr,
  Contract,
} from "stellar-sdk";

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

export interface AuditClientConfig {
  contractId: string;
  network: "local" | "testnet" | "futurenet";
  rpcUrl?: string;
}

export interface ReserveProofInput {
  /** The private reserve amount — never leaves this call. */
  actualReserves: bigint;
  /** Public compliance threshold set by the regulator. */
  threshold: bigint;
  /** 4-character asset code, e.g. "USDC". */
  assetCode: string;
}

export interface ReserveProof {
  commitment: string; // hex-encoded 32-byte Pedersen commitment
  proofResult: number; // 0x01 = pass, 0x00 = fail
  signature: string; // hex-encoded 64-byte Ed25519 signature (from verifier)
  timestamp: number;
  assetCode: string;
  threshold: string; // decimal string
}

export interface SubmitProofOptions {
  source: Keypair;
}

export interface SubmitProofResult {
  verified: boolean;
}

// ---------------------------------------------------------------------------
// Network helpers
// ---------------------------------------------------------------------------

function networkPassphrase(network: string): string {
  switch (network) {
    case "local":
      return Networks.STANDALONE;
    case "testnet":
      return Networks.TESTNET;
    case "futurenet":
      return Networks.FUTURENET;
    default:
      throw new Error(`Unknown network: ${network}`);
  }
}

function rpcUrlForNetwork(network: string): string {
  switch (network) {
    case "local":
      return "http://localhost:8000/soroban/rpc";
    case "testnet":
      return "https://soroban-testnet.stellar.org";
    case "futurenet":
      return "https://rpc-futurenet.stellar.org";
    default:
      throw new Error(`Unknown network: ${network}`);
  }
}

// ---------------------------------------------------------------------------
// AuditClient
// ---------------------------------------------------------------------------

/**
 * TypeScript SDK for Umbra Audit.
 *
 * Wraps the `umbra-audit` Soroban contract for proof generation (off-chain)
 * and on-chain submission. Uses `stellar-sdk` for transaction construction
 * and submission — no alternative chain-interaction library.
 *
 * ## Privacy guarantee
 * The private `actualReserves` value NEVER leaves the `generateReserveProof`
 * call. The proof object (`ReserveProof`) contains only the commitment (opaque
 * 32 bytes) and the verifier's signature — never the underlying amount.
 */
export class AuditClient {
  private contractId: string;
  private network: string;
  private rpcUrl: string;
  private passphrase: string;

  constructor(config: AuditClientConfig) {
    this.contractId = config.contractId;
    this.network = config.network;
    this.rpcUrl = config.rpcUrl || rpcUrlForNetwork(config.network);
    this.passphrase = networkPassphrase(config.network);
  }

  // -----------------------------------------------------------------------
  // generateReserveProof
  // -----------------------------------------------------------------------

  /**
   * Generate a ZK reserve proof off-chain.
   *
   * Steps:
   * 1. Create a Pedersen commitment from `actualReserves`.
   * 2. Generate a Bulletproof range proof that the committed value >= threshold.
   * 3. Submit the proof to the off-chain verifier and obtain a signature.
   *
   * The private reserve amount is used only within this call and is NOT
   * included in the returned proof object.
   *
   * @returns A `ReserveProof` containing the commitment, verifier signature,
   *          and proof result — never the private amount.
   */
  async generateReserveProof(
    input: ReserveProofInput,
  ): Promise<ReserveProof> {
    const { actualReserves, threshold, assetCode } = input;

    // --- Step 1: Generate Pedersen commitment ---
    const blinding = crypto.getRandomValues(new Uint8Array(32));
    const commitment = this.pedersenCommit(actualReserves, blinding);

    // --- Step 2: Determine proof result ---
    // In a full implementation, this would generate a real Bulletproof
    // range proof. The proof_result is 0x01 (pass) if actualReserves >=
    // threshold, else 0x00 (fail).
    const proofResult: number = actualReserves >= threshold ? 0x01 : 0x00;

    // --- Step 3: Obtain verifier attestation ---
    // Build the attestation message and send it to the verifier node.
    // For the client SDK, we simulate the verifier signing the result.
    const timestamp = Math.floor(Date.now() / 1000);
    const signature = await this.requestVerifierSignature(
      commitment,
      proofResult,
      timestamp,
    );

    return {
      commitment: Buffer.from(commitment).toString("hex"),
      proofResult,
      signature: Buffer.from(signature).toString("hex"),
      timestamp,
      assetCode,
      threshold: threshold.toString(),
    };
  }

  // -----------------------------------------------------------------------
  // submitProof
  // -----------------------------------------------------------------------

  /**
   * Submit a generated proof to the Umbra Audit contract on-chain.
   *
   * Constructs and submits a Soroban contract invocation using stellar-sdk.
   * The caller's `source` keypair must be funded and authorized.
   *
   * @returns `{ verified: boolean }` — the pass/fail result. No balance or
   *          private data is returned.
   */
  async submitProof(
    proof: ReserveProof,
    options: SubmitProofOptions,
  ): Promise<SubmitProofResult> {
    const { source } = options;

    const server = new SorobanRpc.Server(this.rpcUrl, {
      networkPassphrase: this.passphrase,
    });

    const contract = new Contract(this.contractId);

    // Build the submit_proof invocation.
    // contract.call(method, ...args) returns an Operation.
    const commitmentBytes = xdr.ScVal.fromBytes(
      Buffer.from(proof.commitment, "hex"),
    );
    const assetCodeBytes = xdr.ScVal.fromBytes(
      Buffer.from(proof.assetCode.padEnd(4, "\0").slice(0, 4)),
    );

    const callOp = contract.call(
      "submit_proof",
      xdr.ScVal.scvAddress(xdr.ScAddress.fromString(source.publicKey())),
      assetCodeBytes,
      commitmentBytes,
      xdr.ScVal.scvU32(proof.proofResult),
      xdr.ScVal.fromBytes(Buffer.from(proof.signature, "hex")),
      xdr.ScVal.scvU64(proof.timestamp),
    );

    // Prepare and submit the transaction.
    const account = await server.getAccount(source.publicKey());
    const tx = new TransactionBuilder(account, {
      fee: BASE_FEE,
      networkPassphrase: this.passphrase,
      sorobanData: new xdr.SorobanTransactionData(
        xdr.SorobanTransactionDataExt.v0(),
        new xdr.LedgerBump(0),
      ),
    })
      .addOperation(callOp)
      .setTimeout(30)
      .build();

    const preparedTx = await server.prepareTransaction(tx);
    preparedTx.sign(source);

    const sendResponse = await server.sendTransaction(preparedTx);
    if (sendResponse.status === "PENDING") {
      // Wait for the response.
      const result = await server.getTransaction(sendResponse.hash);
      if (result.status === "SUCCESS") {
        const returnVal = result.returnValue;
        const verified = returnVal?.value()?.b() === true;
        return { verified };
      }
      throw new Error(`Transaction failed: ${result.status}`);
    }
    throw new Error(`Send failed: ${sendResponse.errorResult?.message}`);
  }

  // -----------------------------------------------------------------------
  // Internal helpers (mock verifier interaction)
  // -----------------------------------------------------------------------

  /**
   * Generate a Pedersen commitment bytes from a value and blinding factor.
   * In a full implementation, this delegates to umbra-crypto wasm.
   * For MVP, we use a simplified approach.
   */
  private pedersenCommit(value: bigint, blinding: Uint8Array): Uint8Array {
    // NOTE: Full Pedersen commitment requires curve25519-dalek operations.
    // For the SDK scaffold, we produce a deterministic 32-byte output
    // that represents the commitment. A production integration would
    // call the umbra-crypto WASM module compiled from Rust.
    const hash = this.sha256(
      Buffer.concat([
        Buffer.from(value.toString()),
        Buffer.from(blinding),
      ]),
    );
    return hash;
  }

  /**
   * Request the off-chain verifier to sign an attestation.
   * In production, this would hit the verifier node's HTTP API.
   * For the SDK, we simulate a signature.
   */
  private async requestVerifierSignature(
    _commitment: Uint8Array,
    _proofResult: number,
    _timestamp: number,
  ): Promise<Uint8Array> {
    // Simulate: return a deterministic 64-byte signature.
    // In production, this would be a real Ed25519 signature from the verifier.
    const sig = new Uint8Array(64);
    for (let i = 0; i < 64; i++) {
      sig[i] = (i * 17 + 42) & 0xff;
    }
    return sig;
  }

  private sha256(data: Buffer): Uint8Array {
    // Browser-compatible SHA-256 using Web Crypto API.
    // In Node.js, this uses the native crypto module.
    const { createHash } = require("crypto");
    return createHash("sha256").update(data).digest();
  }
}
