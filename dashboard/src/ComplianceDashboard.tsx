import { useState, useEffect, useCallback } from "react";

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

interface ProofVerifiedEvent {
  entity: string;
  asset_code: string;
  threshold: string;
  timestamp: string;
  commitment: string;
  passed: boolean;
}

type ConnectionStatus = "connecting" | "connected" | "error";

// ---------------------------------------------------------------------------
// Configuration
// ---------------------------------------------------------------------------

// In production, these would come from environment variables.
// For development, the user sets them via the NetworkConfig form (see below).
const DEFAULT_RPC_URL = "http://localhost:8000/soroban/rpc";
const DEFAULT_CONTRACT_ID = "";

// ---------------------------------------------------------------------------
// Compliance Dashboard Component
// ---------------------------------------------------------------------------

export default function ComplianceDashboard() {
  const [events, setEvents] = useState<ProofVerifiedEvent[]>([]);
  const [status, setStatus] = useState<ConnectionStatus>("connecting");
  const [error, setError] = useState<string | null>(null);
  const [loading, setLoading] = useState(false);
  const [rpcUrl, setRpcUrl] = useState(DEFAULT_RPC_URL);
  const [contractId, setContractId] = useState(DEFAULT_CONTRACT_ID);
  const [configOpen, setConfigOpen] = useState(false);

  const fetchEvents = useCallback(async () => {
    if (!contractId) {
      setStatus("error");
      setError("No contract ID configured. Set the Umbra Audit contract ID.");
      return;
    }
    setLoading(true);
    setError(null);
    try {
      // We use the Stellar RPC method to query contract events.
      // This mirrors what stellar-sdk does under the hood.
      const response = await fetch(rpcUrl, {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({
          jsonrpc: "2.0",
          id: 1,
          method: "getEvents",
          params: {
            startLedger: 0,
            filters: [
              {
                type: "contract",
                contractIds: [contractId],
                topics: [["AAAADwAAAAJwdg=="]], // base64 of symbol_short!("pv")
              },
            ],
            pagination: { limit: 50 },
          },
        }),
      });

      if (!response.ok) {
        throw new Error(`RPC error: ${response.status}`);
      }

      const data = await response.json();
      if (data.error) {
        throw new Error(data.error.message || "RPC error response");
      }

      const parsed: ProofVerifiedEvent[] = (data.result?.events || []).map(
        (evt: any) => {
          // The event value is a base64-encoded XDR struct.
          // For this scaffold we attempt to decode topic values for display.
          const topicValues = (evt.topic || []).map((t: string) => {
            try {
              return atob(t);
            } catch {
              return t;
            }
          });
          const valueStr = evt.value
            ? decodeEventValue(evt.value)
            : "{}";
          let parsedValue: any = {};
          try {
            parsedValue = JSON.parse(valueStr);
          } catch {
            // fallback: show raw
          }
          return {
            entity:
              parsedValue.entity?.toString() ||
              topicValues[1] ||
              "unknown",
            asset_code:
              parsedValue.asset_code?.toString() ||
              topicValues[2] ||
              "???",
            threshold: parsedValue.threshold?.toString() || "0",
            timestamp: parsedValue.timestamp?.toString() || "0",
            commitment:
              parsedValue.commitment?.toString() || "",
            passed: parsedValue.passed === true,
          };
        },
      );

      setEvents(parsed);
      setStatus("connected");
    } catch (err: any) {
      setStatus("error");
      setError(err.message || "Failed to fetch events");
    } finally {
      setLoading(false);
    }
  }, [rpcUrl, contractId]);

  useEffect(() => {
    if (contractId) {
      fetchEvents();
    } else {
      setStatus("error");
      setError("Enter the Umbra Audit contract ID to begin.");
    }
  }, [contractId, fetchEvents]);

  const passingCount = events.filter((e) => e.passed).length;
  const failingCount = events.filter((e) => !e.passed).length;

  return (
    <div className="dashboard">
      <div className="dashboard-header">
        <h2>Compliance Status</h2>
        <div style={{ display: "flex", gap: "0.5rem" }}>
          <button
            className="refresh-btn"
            onClick={() => setConfigOpen(!configOpen)}
          >
            {configOpen ? "Close Config" : "Configure"}
          </button>
          <button
            className="refresh-btn"
            onClick={fetchEvents}
            disabled={loading || !contractId}
          >
            {loading ? "Loading..." : "Refresh"}
          </button>
        </div>
      </div>

      {configOpen && (
        <div
          style={{
            padding: "1rem 1.25rem",
            background: "#161b22",
            borderBottom: "1px solid #30363d",
          }}
        >
          <label style={{ display: "block", marginBottom: "0.75rem" }}>
            <span style={{ fontSize: "0.8rem", color: "#8b949e" }}>
              RPC URL
            </span>
            <input
              type="text"
              value={rpcUrl}
              onChange={(e) => setRpcUrl(e.target.value)}
              style={{
                display: "block",
                width: "100%",
                marginTop: "0.25rem",
                padding: "0.4rem 0.6rem",
                background: "#0d1117",
                border: "1px solid #30363d",
                borderRadius: "4px",
                color: "#c9d1d9",
                fontSize: "0.85rem",
              }}
            />
          </label>
          <label style={{ display: "block" }}>
            <span style={{ fontSize: "0.8rem", color: "#8b949e" }}>
              Umbra Audit Contract ID
            </span>
            <input
              type="text"
              value={contractId}
              onChange={(e) => setContractId(e.target.value)}
              placeholder="CA...AUDIT_CONTRACT_ID"
              style={{
                display: "block",
                width: "100%",
                marginTop: "0.25rem",
                padding: "0.4rem 0.6rem",
                background: "#0d1117",
                border: "1px solid #30363d",
                borderRadius: "4px",
                color: "#c9d1d9",
                fontSize: "0.85rem",
              }}
            />
          </label>
        </div>
      )}

      {status === "connected" && events.length > 0 && (
        <>
          <div className="status-bar">
            <div className="status-item">
              <span className="status-label">Total Events</span>
              <span className="status-value">{events.length}</span>
            </div>
            <div className="status-item">
              <span className="status-label">Passing</span>
              <span className="status-value passing">{passingCount}</span>
            </div>
            <div className="status-item">
              <span className="status-label">Failing</span>
              <span className="status-value failing">{failingCount}</span>
            </div>
          </div>

          <table className="event-table">
            <thead>
              <tr>
                <th>Status</th>
                <th>Entity</th>
                <th>Asset</th>
                <th>Threshold</th>
                <th>Timestamp</th>
              </tr>
            </thead>
            <tbody>
              {events.map((evt, i) => (
                <tr key={i}>
                  <td>
                    <span className={`badge ${evt.passed ? "pass" : "fail"}`}>
                      {evt.passed ? "PASS" : "FAIL"}
                    </span>
                  </td>
                  <td style={{ fontFamily: "monospace", fontSize: "0.8rem" }}>
                    {evt.entity.slice(0, 16)}...
                  </td>
                  <td>{evt.asset_code}</td>
                  <td>{Number(evt.threshold).toLocaleString()}</td>
                  <td>
                    {evt.timestamp !== "0"
                      ? new Date(Number(evt.timestamp) * 1000).toISOString()
                      : "—"}
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        </>
      )}

      {status === "connected" && events.length === 0 && (
        <div className="empty">
          No ProofVerified events found. Submit a proof to the contract to see
          it here.
        </div>
      )}

      {status === "error" && (
        <div className="error">
          {error || "Could not connect to the Soroban RPC endpoint."}
          <br />
          <button
            className="refresh-btn"
            onClick={fetchEvents}
            style={{ marginTop: "0.75rem" }}
          >
            Retry
          </button>
        </div>
      )}

      <div className="network-info">
        Connected to: <code>{rpcUrl}</code>
        {contractId && (
          <>
            {" | Contract: "}
            <code>{contractId}</code>
          </>
        )}
      </div>
    </div>
  );
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/**
 * Attempt to decode a base64-encoded XDR event value into a readable string.
 * This is a best-effort parse; the actual XDR decoding would require the
 * soroban-client or stellar-sdk's XDR parser.
 */
function decodeEventValue(base64: string): string {
  try {
    const raw = atob(base64);
    // Try to extract meaningful fields from the XDR bytes.
    // The ProofVerifiedEvent XDR contains: entity(addr), asset_code(4B),
    // threshold(u64), timestamp(u64), commitment(32B), passed(bool).
    // For the scaffold, we return the raw bytes as hex for inspection.
    const hex = Array.from(raw)
      .map((c) => c.charCodeAt(0).toString(16).padStart(2, "0"))
      .join(" ");
    return JSON.stringify({ raw_hex: hex });
  } catch {
    return base64;
  }
}
