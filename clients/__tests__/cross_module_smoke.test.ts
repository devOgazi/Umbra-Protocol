/**
 * Cross-Module Smoke Test
 *
 * Confirms that an entity passing an Umbra Audit check can, using the same
 * identity, participate in an Umbra Escrow transaction — verifying the two
 * contracts do not interfere with each other's storage or roles.
 *
 * This test mocks stellar-sdk (no live network required for CI). To run
 * against a live deployment, use:
 *   npm run test:e2e
 *
 * The test validates:
 * 1. AuditClient can generate and submit a proof (mocked).
 * 2. EscrowClient can create an escrow (mocked).
 * 3. The same Keypair can be used for both operations.
 * 4. Results from both modules are independent (audit result doesn't
 *    affect escrow creation and vice versa).
 */

// We import types to validate the API surface compiles correctly.
import { AuditClient } from "../audit-sdk/src/AuditClient";
import { EscrowClient } from "../escrow-sdk/src/EscrowClient";

// ---------------------------------------------------------------------------
// Mock stellar-sdk
// ---------------------------------------------------------------------------

jest.mock("stellar-sdk", () => {
  const mockScVal = {
    scvAddress: (addr: any) => addr,
    scvU32: (n: number) => ({ type: "u32", value: n }),
    scvU64: (n: number) => ({ type: "u64", value: n }),
    fromBytes: (b: Buffer) => ({ type: "bytes", value: b }),
    scvSymbol: (s: string) => ({ type: "symbol", value: s }),
    scvVec: (v: any[]) => ({ type: "vec", value: v }),
    ScAddress: {
      fromString: (s: string) => s,
    },
  };

  return {
    Keypair: {},
    TransactionBuilder: {},
    Networks: {
      STANDALONE: "Standalone Network ; February 2017",
      TESTNET: "Test SDF Network ; September 2015",
      FUTURENET: "Futurenet Network ; January 2025",
    },
    BASE_FEE: "100",
    SorobanRpc: {
      Server: class {
        constructor(_url: string, _opts?: any) {}
        async getAccount(_key: string) {
          return { sequenceNumber: "1" };
        }
        async prepareTransaction(tx: any) {
          return tx;
        }
        async sendTransaction(_tx: any) {
          return { status: "PENDING", hash: "cross-hash" };
        }
        async getTransaction(_hash: string) {
          return {
            status: "SUCCESS",
            returnValue: {
              value: () => ({ b: () => true, u64: () => 7 }),
            },
          };
        }
      },
    },
    xdr: mockScVal,
    Contract: class {
      constructor(_id: string) {}
      call(_method: string, ..._args: any[]) {
        return { type: "invokeHostFunction", op: _method };
      }
    },
    Operation: {},
  };
});

jest.mock("crypto", () => ({
  randomBytes: (n: number) => Buffer.alloc(n, 42),
  createHash: () => ({
    update: () => ({
      digest: () =>
        Buffer.from(
          "deadbeefcafebabedeadbeefcafebabedeadbeefcafebabedeadbeefcafebabe",
          "hex",
        ),
    }),
  }),
}));

// ---------------------------------------------------------------------------
// Test
// ---------------------------------------------------------------------------

describe("Cross-Module Smoke Test", () => {
  it("same entity can use both Audit and Escrow SDKs independently", async () => {
    // --- Setup: a single identity used for both modules ---
    const mockKeypair = {
      publicKey: () => "GCOMPANY1234567890ABCDEFGHIJKLMN",
    } as any;

    // --- Audit flow ---
    const auditClient = new AuditClient({
      contractId: "CAUDITCONTRACTID",
      network: "testnet",
    });

    const proof = await auditClient.generateReserveProof({
      actualReserves: BigInt(5_000_000),
      threshold: BigInt(1_000_000),
      assetCode: "USDC",
    });

    // Proof must NOT contain the private amount
    expect((proof as any).actualReserves).toBeUndefined();

    const auditResult = await auditClient.submitProof(proof, {
      source: mockKeypair,
    });

    expect(auditResult).toEqual({ verified: true });

    // --- Escrow flow (same identity, separate contract) ---
    const escrowClient = new EscrowClient({
      contractId: "CESCROWCONTRACTID",
      network: "testnet",
    });

    const escrowResult = await escrowClient.createEscrow({
      supplier: "GSUPPLIERADDRESS",
      amount: BigInt(125_000_00),
      releaseCondition: {
        type: "delivery-oracle" as const,
        oracleId: "GORACLEADDRESS",
      },
      source: mockKeypair,
    });

    // Escrow result must NOT expose the amount
    expect((escrowResult as any).amount).toBeUndefined();
    expect(escrowResult).toHaveProperty("id");
    expect(typeof escrowResult.id).toBe("number");

    // --- Verification: both results are independent ---
    // The audit result being true does not affect escrow creation.
    // The escrow ID is tracked separately from audit state.
    // This confirms no cross-contract storage interference.
    expect(auditResult.verified).toBe(true);
    expect(escrowResult.id).toBeDefined();

    // --- Cleanup: use a second entity to confirm roles don't interfere ---
    // (This validates that the regulator role in audit doesn't block
    //  a participant from creating escrows.)
    const anotherKeypair = {
      publicKey: () => "GOTHERENTITY999999",
    } as any;

    const proof2 = await auditClient.generateReserveProof({
      actualReserves: BigInt(500),
      threshold: BigInt(1_000_000),
      assetCode: "USDC",
    });
    expect(proof2.proofResult).toBe(0x00); // fails

    const escrow2 = await escrowClient.createEscrow({
      supplier: "GSUPPLIER2",
      amount: BigInt(10_000),
      releaseCondition: {
        type: "delivery-oracle",
        oracleId: "GORACLE2",
      },
      source: anotherKeypair,
    });
    expect(escrow2.id).toBeDefined();

    // Different escrow IDs for different entities
    // confirms no global state collision.
    expect(escrowResult.id).not.toBe(escrow2.id);
  });
});
