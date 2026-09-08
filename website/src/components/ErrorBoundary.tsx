import { Component, type ErrorInfo, type ReactNode } from 'react';

export class ErrorBoundary extends Component<{ children: ReactNode }, { failed: boolean }> {
  state = { failed: false };
  static getDerivedStateFromError() {
    return { failed: true };
  }
  componentDidCatch(_error: Error, _info: ErrorInfo) {
    /* Connect error reporting here when a service is configured. */
  }
  render() {
    if (this.state.failed)
      return (
        <main className="error-boundary">
          <h1>Something went wrong.</h1>
          <p>Please reload the page to return to the catalog.</p>
          <button className="primary-button" onClick={() => window.location.reload()}>
            Reload page
          </button>
        </main>
      );
    return this.props.children;
  }
}
