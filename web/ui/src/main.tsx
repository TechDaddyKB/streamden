import { StrictMode } from "react";
import { createRoot } from "react-dom/client";
import "@streamden/kit/tokens.css";
import "./shell.css";

// Scaffold shell styled only with kit tokens. The UI talks to the engine over the public API only (D-006).
function App() {
  return (
    <div className="shell">
      <nav className="shell__nav" aria-label="Main">
        <strong className="shell__brand">StreamDen</strong>
        <a href="#actions" aria-current="page">Actions</a>
        <a href="#chat">Chat</a>
        <a href="#overlays">Overlays</a>
        <a href="#settings">Settings</a>
      </nav>
      <main className="shell__main">
        <h1>Actions</h1>
        <p className="muted">Nothing here yet. Triggers and sub-actions will appear once the engine is connected.</p>
        <section className="card">
          <h2>Engine</h2>
          <p><code>127.0.0.1:7777</code> · <span className="status">not connected</span></p>
          <div className="row">
            <button className="btn btn--primary">Connect</button>
            <button className="btn">Settings</button>
          </div>
        </section>
      </main>
    </div>
  );
}

createRoot(document.getElementById("root")!).render(
  <StrictMode>
    <App />
  </StrictMode>,
);
