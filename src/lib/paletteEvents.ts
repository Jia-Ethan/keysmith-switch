/** The command palette talks to the workspace on screen through window events, not props. */
export const QUICK_DEPLOY_EVENT = "keysmith:quick-deploy";
export const DEPLOY_PROMPT_EVENT = "keysmith:deploy-prompt";

export function requestQuickDeploy(): void {
  window.dispatchEvent(new CustomEvent(QUICK_DEPLOY_EVENT));
}

export function requestDeployPrompt(promptId: string): void {
  window.dispatchEvent(new CustomEvent<string>(DEPLOY_PROMPT_EVENT, { detail: promptId }));
}
