import { EscrowClient } from "../EscrowClient";

// ---------------------------------------------------------------------------
// Mocks
// ---------------------------------------------------------------------------

jest.mock("stellar-sdk", () => {
  return {
    Keypair: {
      fromSecret: (secret: string) => ({
        publicKey: () => "GABCDEF1234567890",
        secret: () => secret,
      }),
    },
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
          return { status: "PENDING", hash: "test-hash-escrow" };
        }
        async getTransaction(_hash: string) {
          return {
            status: "SUCCESS",
            returnValue: { value: () => ({ u64: () => 42 }) },
          };
        }
      },
    },
    xdr: {
      scvAddress: (addr: any) => addr,
      scvU32: (n: number) => ({ type: "u32", value: n }),
      scvU64: (n: number) => ({ type: "u64", value: n }),
      fromBytes: (b: Buffer) => ({ type: "bytes", value: b }),
      scvSymbol: (s: string) => ({ type: "symbol", value: s }),
      scvVec: (v: any[]) => ({ type: "vec", value: v }),
      ScAddress: {
        fromString: (s: string) => s,
      },
    },
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
  randomBytes: (n: number) => Buffer.alloc(n, 99),
  createHash: () => ({
    update: () => ({
      digest: () =>
        Buffer.from(
          "aaabbbcccdddeeefff000111222333444555666777888999000aaabbbcccddd",
          "hex",
        ),
    }),
  }),
}));

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

describe("EscrowClient", () => {
  let client: EscrowClient;

  beforeEach(() => {
    client = new EscrowClient({
      contractId: "CAESCDUMMY",
      network: "testnet",
    });
  });

  describe("createEscrow", () => {
    it("returns an escrow id without echoing the amount", async () => {
      const mockKeypair = {
        publicKey: () => "GBUYER1234567890",
      } as any;

      const result = await client.createEscrow({
        supplier: "GSUPPLIER123456789",
        amount: BigInt(125_000_00),
        releaseCondition: {
          type: "delivery-oracle" as const,
          oracleId: "GORACLE1234567890",
        },
        source: mockKeypair,
      });

      // Must NOT expose the amount
      expect((result as any).amount).toBeUndefined();
      expect((result as any).value).toBeUndefined();

      // Must return the escrow id
      expect(result).toHaveProperty("id");
      expect(typeof result.id).toBe("number");
    });

    it("accepts a multisig release condition", async () => {
      const mockKeypair = {
        publicKey: () => "GBUYER1234567890",
      } as any;

      const result = await client.createEscrow({
        supplier: "GSUPPLIER123456789",
        amount: BigInt(50_000),
        releaseCondition: {
          type: "multisig",
          required: 2,
          signers: ["GSIGNER1", "GSIGNER2", "GSIGNER3"],
        },
        source: mockKeypair,
      });

      expect(result).toHaveProperty("id");
    });
  });
});
