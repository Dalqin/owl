import { Component, StrictMode } from "react";
import { createRoot } from "react-dom/client";

import App from "./App.jsx";
import "./styles.css";

// If anything throws while rendering (for example the core sent a value the UI
// does not know), show it instead of a blank window.
class ErrorBoundary extends Component {
  state = { error: null };
  static getDerivedStateFromError(error) {
    return { error };
  }
  render() {
    if (this.state.error) {
      return (
        <main>
          <h1>Something went wrong</h1>
          <p className="note error" role="alert">{String(this.state.error.message ?? this.state.error)}</p>
        </main>
      );
    }
    return this.props.children;
  }
}

createRoot(document.getElementById("root")).render(
  <StrictMode>
    <ErrorBoundary>
      <App />
    </ErrorBoundary>
  </StrictMode>,
);
