import React from "react";
import { createRoot } from "react-dom/client";

function BootPlaceholder() {
  return <div>Polymarket Signal Cockpit</div>;
}

createRoot(document.getElementById("root")!).render(
  <React.StrictMode>
    <BootPlaceholder />
  </React.StrictMode>,
);
