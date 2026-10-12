import { Component, type ErrorInfo, type ReactNode } from 'react'
import { t } from '../domain/localization'
import { errorDetails, errorStack } from '../platform/diagnostics/error-details'
import { logDiagnostic } from '../platform/diagnostics/log'

/** Lives outside the shell so its failure cannot remove the error display. */
export class CrashBoundary extends Component<{ children: ReactNode }, { details: Record<string, unknown> | null }> {
  state: { details: Record<string, unknown> | null } = { details: null }
  static getDerivedStateFromError(error: unknown) { return { details: errorDetails(error) } }
  componentDidCatch(error: unknown, info: ErrorInfo) {
    const details = { ...errorDetails(error), componentStack: errorStack(info.componentStack) }
    this.setState({ details })
    void logDiagnostic('application', { ...details, diagnostics: details })
  }
  render() {
    if (!this.state.details) return this.props.children
    return <main className="not-tauri" role="alert">
      <p>{String(this.state.details.message)}</p>
      <pre className="error-raw">{JSON.stringify(this.state.details, null, 2)}</pre>
      <button className="btn" onClick={() => window.location.reload()}>{t('english', 'Reload app')}</button>
    </main>
  }
}
