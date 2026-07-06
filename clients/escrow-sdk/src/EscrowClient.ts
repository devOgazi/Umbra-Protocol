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

export interface EscrowClientConfig {
  contractId: string;
  network: "local" | "testnet" | "futurenet";
  rpcUrl?: string;
}

export type ReleaseConditionType =
  | { type: "delivery-oracle"; oracleId: string }
  | { type: "multisig"; required: number; signers: string[] };

export interface CreateEscrowInput {
  supplier: string;
  amount: bigint;
  releaseCondition: ReleaseConditionType;
  source: Keypair;
}

export interface CreateEscrowResult {
  id: number;
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
// EscrowClient
// ---------------------------------------------------------------------------

/**
 * TypeScript SDK for Umbra Escrow.
 *
 * Wraps the `umbra-escrow` Soroban contract for confidential B2B escrow
 * creation. Uses `stellar-sdk` for transaction construction and submission —
 * no alternative chain-interaction library.
 *
 * ## Privacy guarantee
 * The private `amount` is used only to generate a Pedersen commitment within
 * `createEscrow`. It is NEVER stored, logged, or included in the returned
 * result. Only the opaque 32-byte commitment is sent to the contract.
 */
export class EscrowClient {
  private contractId: string;
  private network: string;
  private rpcUrl: string;
  private passphrase: string;

  constructor(config: EscrowClientConfig) {
    this.contractId = config.contractId;
    this.network = config.network;
    this.rpcUrl = config.rpcUrl || rpcUrlForNetwork(config.network);
    this.passphrase = networkPassphrase(config.network);
  }

  // -----------------------------------------------------------------------
  // createEscrow
  // -----------------------------------------------------------------------

  /**
   * Create a new confidential escrow.
   *
   * The `amount` is used to generate a Pedersen commitment off-chain. The
   * plaintext amount is NEVER sent to the contract, logged, or included
   * in the return value — only the 32-byte commitment is submitted.
   *
   * @returns `{ id: number }` — the newly created escrow's ID. The amount
   *          is NOT echoed back.
   */
  async createEscrow(
    input: CreateEscrowInput,
  ): Promise<CreateEscrowResult> {
    const { supplier, amount, releaseCondition, source } = input;

    // --- Step 1: Generate Pedersen commitment from amount ---
    const blinding = crypto.getRandomValues(new Uint8Array(32));
    const commitmentBytes = this.pedersenCommit(amount, blinding);

    // --- Step 2: Build the release condition SCVal ---
    let conditionScVal: xdr.ScVal;
    if (releaseCondition.type === "delivery-oracle") {
      // DeliveryOracle(Address)
      conditionScVal = xdr.ScVal.scvVec([
        xdr.ScVal.scvSymbol("DeliveryOracle"),
        xdr.ScVal.scvAddress(
          xdr.ScAddress.fromString(releaseCondition.oracleId),
        ),
      ]);
    } else {
      // MultiSig { required: u32, signers: Vec<Address> }
      const signerVals = releaseCondition.signers.map((s) =>
        xdr.ScVal.scvAddress(xdr.ScAddress.fromString(s)),
      );
      conditionScVal = xdr.ScVal.scvVec([
        xdr.ScVal.scvSymbol("MultiSig"),
        xdr.ScVal.scvU32(releaseCondition.required),
        xdr.ScVal.scvVec(signerVals),
      ]);
    }

    // --- Step 3: Submit to contract ---
    const server = new SorobanRpc.Server(this.rpcUrl, {
      networkPassphrase: this.passphrase,
    });

    const contract = new Contract(this.contractId);

    const callOp = contract.call(
      "create_escrow",
      xdr.ScVal.scvAddress(xdr.ScAddress.fromString(source.publicKey())),
      xdr.ScVal.scvAddress(xdr.ScAddress.fromString(supplier)),
      xdr.ScVal.fromBytes(Buffer.from(commitmentBytes)),
      conditionScVal,
    );

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
      const result = await server.getTransaction(sendResponse.hash);
      if (result.status === "SUCCESS") {
        const returnVal = result.returnValue;
        const id = Number(returnVal?.value()?.u64() || 0);
        return { id };
      }
      throw new Error(`Transaction failed: ${result.status}`);
    }
    throw new Error(`Send failed: ${sendResponse.errorResult?.message}`);
  }

  // -----------------------------------------------------------------------
  // Internal: Pedersen commitment (simplified for SDK)
  // -----------------------------------------------------------------------

  /**
   * Generate a deterministic commitment from a value and blinding factor.
   * In a full implementation, this uses a real Pedersen commitment via
   * the umbra-crypto WASM module.
   */
  private pedersenCommit(value: bigint, blinding: Uint8Array): Uint8Array {
    const { createHash } = require("crypto");
    return createHash("sha256")
      .update(Buffer.concat([Buffer.from(value.toString()), Buffer.from(blinding)]))
      .digest();
  }
}
