// SPDX-License-Identifier: MIT
// Portions adapted from CC Switch (c) 2025 Jason Young
// https://github.com/farion1231/cc-switch

import React from "react";
import { logFrontendError } from "../api";
import { Button } from "./ui";

interface State {
  hasError: boolean;
}

export class ErrorBoundary extends React.Component<React.PropsWithChildren, State> {
  state: State = { hasError: false };

  static getDerivedStateFromError(): State {
    return { hasError: true };
  }

  componentDidCatch(error: Error, info: React.ErrorInfo): void {
    void logFrontendError(error.message, info.componentStack ?? undefined).catch(() => undefined);
  }

  render(): React.ReactNode {
    if (!this.state.hasError) return this.props.children;
    return (
      <main className="flex min-h-full items-center justify-center bg-background p-6 text-foreground">
        <section role="alert" className="w-full max-w-md space-y-4 rounded-lg border border-border bg-card p-6 shadow-sm">
          <h1 className="text-base font-semibold">界面遇到了问题</h1>
          <p className="text-sm leading-6 text-muted-foreground">
            错误已写入应用日志。重新加载界面后可以继续；如果反复出现，请附上日志。
          </p>
          <Button className="w-full" variant="primary" onClick={() => window.location.reload()}>
            重新加载界面
          </Button>
        </section>
      </main>
    );
  }
}
