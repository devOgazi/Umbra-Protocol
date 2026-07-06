import ComplianceDashboard from "./ComplianceDashboard";
import "./App.css";

export default function App() {
  return (
    <main className="app">
      <header className="app-header">
        <h1>Umbra Protocol</h1>
        <p className="subtitle">Compliance Dashboard</p>
      </header>
      <ComplianceDashboard />
    </main>
  );
}
