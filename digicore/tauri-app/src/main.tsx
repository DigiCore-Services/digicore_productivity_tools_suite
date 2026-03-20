import React, { useState, useEffect } from "react";
import ReactDOM from "react-dom/client";
import { getCurrentWindow } from "@tauri-apps/api/window";
import App from "./App";
import KmsApp from "./KmsApp";
import VariableInputWindow from "./VariableInputWindow";
import "./index.css";

const Entry = () => {
  const [label, setLabel] = useState<string | null>(null);

  useEffect(() => {
    setLabel(getCurrentWindow().label);
  }, []);

  if (!label) {
    return (
      <div className="h-screen bg-[var(--dc-bg)]" />
    );
  }

  if (label === "kms") {
    return <KmsApp />;
  }

  if (label === "variable-input") {
    return <VariableInputWindow />;
  }

  return <App />;
};

ReactDOM.createRoot(document.getElementById("root")!).render(
  <React.StrictMode>
    <Entry />
  </React.StrictMode>
);
