import { StrictMode } from "react";
import { createRoot } from "react-dom/client";
import { tokens } from "@streamden/kit";

// Scaffold: an empty window. The UI talks to the engine over the public API only (D-006).
function App() {
  return <main style={{ background: tokens.color.background, minHeight: "100vh" }} />;
}

createRoot(document.getElementById("root")!).render(
  <StrictMode>
    <App />
  </StrictMode>,
);
