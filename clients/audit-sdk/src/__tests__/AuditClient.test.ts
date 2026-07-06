import { AuditClient } from "../AuditClient";

// ---------------------------------------------------------------------------
// Mocks
// ---------------------------------------------------------------------------

// Mock stellar-sdk entirely to avoid real network calls.
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
    Keypair: {
      fromSecret: (secret: string) => ({
        publicKey: () => "GABCDEF1234567890",
        secret: () => secret,
      }),
    },
    TransactionBuilder: {
      // mock
    },
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
          return { status: "PENDING", hash: "test-hash" };
        }
        async getTransaction(_hash: string) {
          return {
            status: "SUCCESS",
            returnValue: { value: () => ({ b: () => true }) },
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

// Mock crypto.randomValues and node crypto
jest.mock("crypto", () => ({
  randomBytes: (n: number) => Buffer.alloc(n, 42),
  createHash: () => ({
    update: () => ({
      digest: () => Buffer.from("abcdef1234567890abcdef1234567890abcdef1234567890abcdef1234567890", "hex"),
    }),
  }),
}));

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

describe("AuditClient", () => {
  let client: AuditClient;

  beforeEach(() => {
    client = new AuditClient({
      contractId: "CAABCDEFG",
      network: "testnet",
    });
  });

  describe("generateReserveProof", () => {
    it("returns a proof object without the private amount", async () => {
      const proof = await client.generateReserveProof({
        actualReserves: BigInt(2_000_000),
        threshold: BigInt(1_000_000),
        assetCode: "USDC",
      });

      // Must NOT contain the private amount
      expect((proof as any).actualReserves).toBeUndefined();
      expect((proof as any).privateReserveAmount).toBeUndefined();

      // Must contain public fields
      expect(proof.commitment).toBeDefined();
      expect(typeof proof.commitment).toBe("string");
      expect(proof.commitment.length).toBe(64); // hex-encoded 32 bytes

      expect(proof.proofResult).toBe(0x01); // pass
      expect(proof.signature).toBeDefined();
      expect(typeof proof.signature).toBe("string");
      expect(proof.assetCode).toBe("USDC");
    });

    it("sets proofResult 0x01 when reserves >= threshold", async () => {
      const proof = await client.generateReserveProof({
        actualReserves: BigInt(1_000_000),
        threshold: BigInt(1_000_000),
        assetCode: "USDC",
      });
      expect(proof.proofResult).toBe(0x01);
    });

    it("sets proofResult 0x00 when reserves < threshold", async () => {
      const proof = await client.generateReserveProof({
        actualReserves: BigInt(500_000),
        threshold: BigInt(1_000_000),
        assetCode: "USDC",
      });
      expect(proof.proofResult).toBe(0x00);
    });
  });

  describe("submitProof", () => {
    it("returns verified: true for a valid proof", async () => {
      const proof = await client.generateReserveProof({
        actualReserves: BigInt(2_000_000),
        threshold: BigInt(1_000_000),
        assetCode: "USDC",
      });

      const mockKeypair = {
        publicKey: () => "GABCDEF1234567890",
      } as any;

      const result = await client.submitProof(proof, {
        source: mockKeypair,
      });

      expect(result).toEqual({ verified: true });
    });
  });
});
