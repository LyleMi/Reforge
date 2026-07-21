import React from "react";
import { createRoot } from "react-dom/client";
import { App } from "./reportApp";
import "./styles.css";

createRoot(document.getElementById("reforge-report-root")!).render(<App />);
