import React from "react";
import ReactDOM from "react-dom/client";
import App from "./App";
import { ErrorBoundary } from "./components/ErrorBoundary";
import "./i18n";
import "./index.css";
import { applyPlatformAttributes } from "./lib/platform";

applyPlatformAttributes();

// Design preview: `npm run dev`, then open /?mock=1. Dev builds only.
if (import.meta.env.DEV && new URLSearchParams(window.location.search).has("mock")) {
  const { installMockTauri } = await import("./dev/mockTauri");
  installMockTauri();
}

ReactDOM.createRoot(document.getElementById("root")!).render(
  <React.StrictMode>
    <ErrorBoundary>
      <App />
    </ErrorBoundary>
  </React.StrictMode>,
);
