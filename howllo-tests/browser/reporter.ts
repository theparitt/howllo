import { randomUUID } from "node:crypto";
import { mkdirSync, writeFileSync } from "node:fs";
import { relative, resolve } from "node:path";
import type { FullResult, Reporter, TestCase, TestResult } from "@playwright/test/reporter";

type CaseRecord = {
  id: string;
  name: string;
  outcome: "PASS" | "FAIL" | "MISSING_CAPABILITY" | "SKIPPED";
  reason?: string;
  severity?: "P0" | "P2";
  source: string;
  trace?: string;
  duration_ms: number;
};

class CapabilityReporter implements Reporter {
  private runId = randomUUID();
  private cases: CaseRecord[] = [];

  onTestEnd(test: TestCase, result: TestResult): void {
    const missing = result.annotations.find((entry) => entry.type === "MISSING_CAPABILITY");
    const outcome = result.status === "passed" ? "PASS"
      : result.status === "skipped" ? (missing ? "MISSING_CAPABILITY" : "SKIPPED")
      : "FAIL";
    this.cases.push({
      id: test.title.match(/^[A-Z]+-\d+/)?.[0] || test.id,
      name: test.title,
      outcome,
      ...(missing?.description ? { reason: missing.description } : {}),
      ...(outcome === "FAIL" ? { severity: test.title.startsWith("ISOLATION-") ? "P0" as const : "P2" as const } : {}),
      source: relative(__dirname, test.location.file),
      ...(result.attachments.find((attachment) => attachment.name === "trace")?.path
        ? { trace: relative(__dirname, result.attachments.find((attachment) => attachment.name === "trace")!.path!) }
        : {}),
      duration_ms: result.duration,
    });
  }

  async onEnd(result: FullResult): Promise<{ status: FullResult["status"] } | void> {
    if (!this.cases.length) return;
    const counts = { PASS: 0, FAIL: 0, MISSING_CAPABILITY: 0, SKIPPED: 0 };
    for (const item of this.cases) counts[item.outcome]++;
    const incomplete = counts.MISSING_CAPABILITY > 0 || counts.SKIPPED > 0;
    const finalStatus = incomplete && result.status === "passed" ? "failed" : result.status;
    const report = {
      kind: "howllo_browser_smoke",
      run_id: this.runId,
      certification: "NOT_CERTIFIED",
      browser_coverage: incomplete ? "INCOMPLETE" : "COMPLETE",
      test_status: finalStatus,
      counts,
      cases: this.cases,
    };
    const folder = resolve(__dirname, "reports");
    mkdirSync(folder, { recursive: true });
    writeFileSync(resolve(folder, "summary.json"), `${JSON.stringify(report, null, 2)}\n`, { mode: 0o600 });
    console.log(`Browser report: ${resolve(folder, "summary.json")} (${counts.PASS} pass, ${counts.FAIL} fail, ${counts.MISSING_CAPABILITY} missing)`);
    return { status: finalStatus };
  }
}

export default CapabilityReporter;
