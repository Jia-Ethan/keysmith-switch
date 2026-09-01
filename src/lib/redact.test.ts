import { describe, expect, it } from "vitest";
import { redactSecrets, toastSafeMessage } from "./redact";

describe("redactSecrets", () => {
  it("redacts api keys, tokens, cookies, and credentials", () => {
    const raw = [
      'api_key=sk-live-secret-value',
      'Authorization: Bearer eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9.aaa.bbb',
      "cookie: session=abc123",
      "password: hunter2",
      "https://user:supersecret@example.com/path",
    ].join("\n");

    const redacted = redactSecrets(raw);
    expect(redacted).not.toMatch(/sk-live-secret-value/);
    expect(redacted).not.toMatch(/eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9/);
    expect(redacted).not.toMatch(/session=abc123/);
    expect(redacted).not.toMatch(/hunter2/);
    expect(redacted).not.toMatch(/supersecret/);
    expect(redacted).toMatch(/\[REDACTED\]/);
  });
});

describe("toastSafeMessage", () => {
  it("does not dump a full prompt body into a toast", () => {
    const body = `SYSTEM PROMPT\n${"you are a helpful assistant. ".repeat(80)}`;
    const toasted = toastSafeMessage(body);
    expect(toasted.length).toBeLessThan(body.length);
    expect(toasted.length).toBeLessThanOrEqual(241);
    expect(toasted.endsWith("…")).toBe(true);
  });
});
