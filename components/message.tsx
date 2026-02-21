"use client";
import type { UseChatHelpers } from "@ai-sdk/react";
import { useState } from "react";
import type { Vote } from "@/lib/db/schema";
import type { ChatMessage } from "@/lib/types";
import { cn, sanitizeText } from "@/lib/utils";
import { useDataStream } from "./data-stream-provider";
import { DocumentToolResult } from "./document";
import { DocumentPreview } from "./document-preview";
import { MessageContent } from "./elements/message";
import { Response } from "./elements/response";
import {
  Tool,
  ToolContent,
  ToolHeader,
  ToolInput,
  ToolOutput,
} from "./elements/tool";
import { SparklesIcon } from "./icons";
import { MessageActions } from "./message-actions";
import { MessageEditor } from "./message-editor";
import { MessageReasoning } from "./message-reasoning";
import { PreviewAttachment } from "./preview-attachment";
import { Weather } from "./weather";

function extractLastJsonCodeBlock(text: string): string | null {
  const matches = [...text.matchAll(/```json\s*([\s\S]*?)```/gi)];
  if (matches.length === 0) {
    return null;
  }
  const last = matches.at(-1)?.[1]?.trim();
  return last || null;
}

/* ── Streaming phase detection ── */

type StreamPhase = "progress" | "tool-selected" | "content" | "done";

/**
 * Determine the current streaming phase of an assistant message.
 *
 * - `progress`      – progress bars are showing, no tool line yet
 * - `tool-selected` – "Using tool:" line arrived but real content hasn't
 * - `content`       – actual response content is streaming in
 * - `done`          – stream finished
 */
function getStreamingPhase(text: string, isStreaming: boolean): StreamPhase {
  if (!isStreaming) {
    return "done";
  }

  const hasProgress =
    text.includes("Processing request") || /\[#{1,10}[.#]*\]/.test(text);

  if (!hasProgress) {
    // No progress bars → either content is streaming directly (AI model)
    // or it's an empty start — treat as content
    return "content";
  }

  const hasToolLine = text.includes("Using tool:");

  // Strip progress indicators + tool line to see if real content exists
  const stripped = text
    .replace(/Processing request\.\.\.\n*/g, "")
    .replace(/```text[\s\S]*?```\n*/g, "")
    .replace(/Using tool:.*\n*/g, "")
    .trim();

  if (stripped.length > 50) {
    return "content";
  }
  if (hasToolLine) {
    return "tool-selected";
  }
  return "progress";
}

/**
 * Extract the tool name from a "Using tool: `toolName`" line.
 */
function extractToolName(text: string): string | null {
  const m = text.match(/Using tool:\s*`([^`]+)`/);
  return m?.[1] ?? null;
}

/* ── Skeleton Components ── */

/** Skeleton placeholder for the "Using tool:" line before it arrives */
function ToolSelectionSkeleton() {
  return (
    <div className="mt-3 flex items-center gap-2 animate-in fade-in duration-150">
      <span className="text-sm text-muted-foreground">Selecting tool</span>
      <div className="h-5 w-32 rounded-md skeleton-shimmer" />
    </div>
  );
}

/** Generic skeleton placeholder for response content */
function ResponseSkeleton() {
  return (
    <div className="mt-3 space-y-3 animate-in fade-in duration-150">
      <div className="space-y-2.5 rounded-xl border border-border/40 bg-muted/10 p-4">
        <div className="h-3.5 w-3/4 rounded-md skeleton-shimmer" />
        <div
          className="h-3.5 w-full rounded-md skeleton-shimmer"
          style={{ animationDelay: "100ms" }}
        />
        <div
          className="h-3.5 w-5/6 rounded-md skeleton-shimmer"
          style={{ animationDelay: "200ms" }}
        />
        <div
          className="h-3.5 w-2/3 rounded-md skeleton-shimmer"
          style={{ animationDelay: "300ms" }}
        />
      </div>
    </div>
  );
}

/** Skeleton placeholder that mimics a backtest result table */
function BacktestSkeleton() {
  return (
    <div className="my-3 space-y-3 animate-in fade-in duration-150">
      {/* Summary bar skeleton */}
      <div className="flex items-center gap-3 rounded-lg border bg-muted/30 px-4 py-2.5">
        <div className="h-4 w-32 rounded skeleton-shimmer" />
        <div
          className="h-4 w-24 rounded skeleton-shimmer"
          style={{ animationDelay: "80ms" }}
        />
        <div
          className="h-4 w-16 rounded skeleton-shimmer"
          style={{ animationDelay: "160ms" }}
        />
      </div>
      {/* Table skeleton */}
      <div className="overflow-hidden rounded-lg border">
        <div className="flex gap-6 border-b bg-muted/40 px-4 py-2.5">
          <div className="h-4 w-24 rounded skeleton-shimmer" />
          <div
            className="h-4 w-16 rounded skeleton-shimmer"
            style={{ animationDelay: "60ms" }}
          />
          <div
            className="h-4 w-20 rounded skeleton-shimmer"
            style={{ animationDelay: "120ms" }}
          />
          <div
            className="h-4 w-14 rounded skeleton-shimmer"
            style={{ animationDelay: "180ms" }}
          />
          <div
            className="h-4 w-16 rounded skeleton-shimmer"
            style={{ animationDelay: "240ms" }}
          />
        </div>
        {[0, 1, 2].map((i) => (
          <div
            className="flex gap-6 border-b px-4 py-2.5 last:border-b-0"
            key={`skel-row-${String(i)}`}
          >
            <div
              className="h-4 w-28 rounded skeleton-shimmer"
              style={{ animationDelay: `${300 + i * 80}ms` }}
            />
            <div
              className="h-4 w-14 rounded skeleton-shimmer"
              style={{ animationDelay: `${340 + i * 80}ms` }}
            />
            <div
              className="h-4 w-16 rounded skeleton-shimmer"
              style={{ animationDelay: `${380 + i * 80}ms` }}
            />
            <div
              className="h-4 w-10 rounded skeleton-shimmer"
              style={{ animationDelay: `${420 + i * 80}ms` }}
            />
            <div
              className="h-4 w-12 rounded skeleton-shimmer"
              style={{ animationDelay: `${460 + i * 80}ms` }}
            />
          </div>
        ))}
      </div>
    </div>
  );
}

/* ── Backtest result extraction & types ── */

type BacktestMetrics = {
  return_pct?: number;
  total_return?: number;
  total_trades?: number;
  win_rate?: number;
};

type BacktestTrade = {
  entry_date?: string;
  exit_date?: string;
  pnl?: number;
  pnl_pct?: number;
};

type BacktestStockResult = {
  stock: string;
  success: boolean;
  metrics?: BacktestMetrics;
  trades?: BacktestTrade[];
};

type BacktestData = {
  summary?: {
    total_stocks?: number;
    successful?: number;
    failed?: number;
    execution_time?: number;
  };
  results?: Record<string, BacktestStockResult>;
};

/**
 * Extract backtest result data from a ```backtest-results code block
 * embedded by the Rust backend.
 */
function extractBacktestData(text: string): BacktestData | null {
  const match = text.match(/```backtest-results\s*([\s\S]*?)```/);
  if (!match?.[1]) {
    return null;
  }
  try {
    return JSON.parse(match[1].trim()) as BacktestData;
  } catch {
    return null;
  }
}

/**
 * Remove the ```backtest-results block from the text so it doesn't
 * render as a raw code block in the markdown view.
 */
function stripBacktestDataBlock(text: string): string {
    return text
      .replace(/###\s*Backtest Results[\s\S]*?```backtest-results\s*[\s\S]*?```/g, "")
      .replace(/```backtest-results\s*[\s\S]*?```/g, "")
      .trim();
}

/* ── Backtest Result Table Component ── */

function formatCurrency(value: number): string {
  const abs = Math.abs(value);
  if (abs >= 10_000_000) {
    return `₹${(value / 10_000_000).toFixed(2)}Cr`;
  }
  if (abs >= 100_000) {
    return `₹${(value / 100_000).toFixed(2)}L`;
  }
  return `₹${value.toLocaleString("en-IN", { maximumFractionDigits: 0 })}`;
}

function PnlCell({ value, suffix }: { value?: number; suffix?: string }) {
  if (value === undefined || value === null) {
    return <span className="text-muted-foreground">—</span>;
  }
  const isPositive = value > 0;
  const isNeg = value < 0;
  return (
    <span
      className={cn(
        "font-medium",
        isPositive && "text-emerald-600 dark:text-emerald-400",
        isNeg && "text-red-500 dark:text-red-400"
      )}
    >
      {isPositive ? "+" : ""}
      {suffix === "%" ? `${value.toFixed(2)}%` : formatCurrency(value)}
    </span>
  );
}

function BacktestResultTable({ data }: { data: BacktestData }) {
  const { summary, results } = data;
  const stocks = results ? Object.entries(results) : [];

  return (
    <div className="my-3 space-y-4 text-sm">
      {/* Summary bar */}
      {summary && (
        <div className="flex flex-wrap items-center gap-3 rounded-lg border bg-muted/30 px-4 py-2.5">
          <span className="font-semibold">Backtest Summary</span>
          <span className="text-muted-foreground">•</span>
          <span>
            <span className="font-medium text-emerald-600 dark:text-emerald-400">
              {summary.successful ?? 0}
            </span>
            /{summary.total_stocks ?? 0} stocks successful
          </span>
          {(summary.failed ?? 0) > 0 && (
            <>
              <span className="text-muted-foreground">•</span>
              <span className="text-red-500">{summary.failed} failed</span>
            </>
          )}
          <span className="text-muted-foreground">•</span>
          <span className="text-muted-foreground">
            {(summary.execution_time ?? 0).toFixed(2)}s
          </span>
        </div>
      )}

      {/* Per-stock metrics table */}
      {stocks.length > 0 && (
        <div className="overflow-x-auto rounded-lg border">
          <table className="w-full text-left">
            <thead>
              <tr className="border-b bg-muted/40">
                <th className="px-4 py-2.5 font-semibold">Stock</th>
                <th className="px-4 py-2.5 text-right font-semibold">
                  Return %
                </th>
                <th className="px-4 py-2.5 text-right font-semibold">
                  Total Return
                </th>
                <th className="px-4 py-2.5 text-right font-semibold">Trades</th>
                <th className="px-4 py-2.5 text-right font-semibold">
                  Win Rate
                </th>
              </tr>
            </thead>
            <tbody>
              {stocks.map(([symbol, result]) => {
                const m = result.metrics;
                return (
                  <tr
                    className="border-b last:border-b-0 transition-colors hover:bg-muted/20"
                    key={symbol}
                  >
                    <td className="px-4 py-2.5 font-medium">
                      {symbol}
                      {!result.success && (
                        <span className="ml-1 text-red-500" title="Failed">
                          ⚠️
                        </span>
                      )}
                    </td>
                    <td className="px-4 py-2.5 text-right">
                      <PnlCell suffix="%" value={m?.return_pct} />
                    </td>
                    <td className="px-4 py-2.5 text-right">
                      <PnlCell value={m?.total_return} />
                    </td>
                    <td className="px-4 py-2.5 text-right font-medium">
                      {m?.total_trades ?? "—"}
                    </td>
                    <td className="px-4 py-2.5 text-right">
                      {m?.win_rate !== undefined ? (
                        <span className="font-medium">
                          {m.win_rate.toFixed(1)}%
                        </span>
                      ) : (
                        <span className="text-muted-foreground">—</span>
                      )}
                    </td>
                  </tr>
                );
              })}
            </tbody>
          </table>
        </div>
      )}

      {/* Per-stock trades */}
      {stocks.map(([symbol, result]) => {
        const trades = result.trades;
        if (!trades || trades.length === 0) {
          return null;
        }
        return (
          <div key={`trades-${symbol}`}>
            <h4 className="mb-1.5 font-semibold text-xs uppercase tracking-wide text-muted-foreground">
              Trades — {symbol}
            </h4>
            <div className="overflow-x-auto rounded-lg border">
              <table className="w-full text-left">
                <thead>
                  <tr className="border-b bg-muted/40">
                    <th className="px-3 py-2 font-semibold">#</th>
                    <th className="px-3 py-2 font-semibold">Entry</th>
                    <th className="px-3 py-2 font-semibold">Exit</th>
                    <th className="px-3 py-2 text-right font-semibold">PnL</th>
                    <th className="px-3 py-2 text-right font-semibold">
                      PnL %
                    </th>
                  </tr>
                </thead>
                <tbody>
                  {trades.map((trade, i) => (
                    <tr
                      className="border-b last:border-b-0 transition-colors hover:bg-muted/20"
                      key={`${symbol}-${trade.entry_date ?? "na"}-${trade.exit_date ?? "na"}`}
                    >
                      <td className="px-3 py-2 text-muted-foreground">
                        {i + 1}
                      </td>
                      <td className="px-3 py-2">{trade.entry_date ?? "—"}</td>
                      <td className="px-3 py-2">{trade.exit_date ?? "—"}</td>
                      <td className="px-3 py-2 text-right">
                        <PnlCell value={trade.pnl} />
                      </td>
                      <td className="px-3 py-2 text-right">
                        <PnlCell suffix="%" value={trade.pnl_pct} />
                      </td>
                    </tr>
                  ))}
                </tbody>
              </table>
            </div>
          </div>
        );
      })}
    </div>
  );
}

/* ── Defaults from the /api/v1/backtest/run-config spec ── */
const BACKTEST_DEFAULTS = {
  initial_capital: 100_000,
  position_size: 15,
  stop_loss: 5,
  take_profit: 15,
  date_config: { start_date: "2024-01-01", end_date: "2025-01-01" } as Record<
    string,
    unknown
  >,
} as const;

type StrategyBacktestPayload = {
  /** Backtest-ready JSON string */
  configJson: string;
  /** Human-readable list of defaults that were filled in */
  appliedDefaults: string[];
};

/**
 * Clean an AST condition to only keep fields the backtest API expects:
 * left, operator, right.
 */
function cleanCondition(
  cond: Record<string, unknown>
): Record<string, unknown> {
  return {
    left: cond.left,
    operator: cond.operator,
    right: cond.right,
  };
}

/**
 * Clean the AST to the backtest API format: only entry[] and exit[]
 * with each condition having only left/operator/right.
 */
function cleanAst(ast: Record<string, unknown>): Record<string, unknown> {
  const cleaned: Record<string, unknown> = {};
  if (Array.isArray(ast.entry)) {
    cleaned.entry = (ast.entry as Record<string, unknown>[]).map(
      cleanCondition
    );
  }
  if (Array.isArray(ast.exit)) {
    cleaned.exit = (ast.exit as Record<string, unknown>[]).map(cleanCondition);
  }
  return cleaned;
}

/**
 * Map date_conditions from the parse/strategy_builder response format
 * to the run-config format the backtest API expects.
 *
 * Strategy builder returns (various shapes):
 *   { "type": "exclude_month", "months": ["march"] }
 *   { "type": "skip_month", "month": "march" }
 *
 * Backtest run-config expects:
 *   { "exclude": true, "months": ["march"], "type": "month" }
 *   { "type": "skip_holiday" }  (passed through as-is)
 */
function mapDateCondition(
  cond: Record<string, unknown>
): Record<string, unknown> {
  const srcType = typeof cond.type === "string" ? cond.type : "";

  // exclude_month / skip_month / month_event → { exclude: true, months: [...], type: "month" }
  if (
    srcType === "exclude_month" ||
    srcType === "skip_month" ||
    srcType === "month_event"
  ) {
    // months may be an array already or a singular "month" string
    let months: string[] = [];
    if (Array.isArray(cond.months)) {
      months = cond.months as string[];
    } else if (typeof cond.month === "string") {
      months = [cond.month];
    }
    return { exclude: true, months, type: "month" };
  }

  // skip_holiday and others → pass through as-is
  return { ...cond };
}

/**
 * Check whether date_config has meaningful explicit start/end dates.
 * Only explicit date strings count — `is_relative` / `relative_value`
 * are NOT forwarded to the backtest API.
 */
function hasRealDateConfig(srcDate: Record<string, unknown>): boolean {
  const startDate = srcDate.start_date;
  const endDate = srcDate.end_date;

  return (
    (typeof startDate === "string" && startDate.length > 0) ||
    (typeof endDate === "string" && endDate.length > 0)
  );
}

/**
 * Build a backtest-ready config from the strategy parse response.
 *
 * Structure & field order strictly matches `/api/v1/backtest/run-config`:
 *   ast → risk_params → date_config → date_conditions → stocks
 *
 * Null / missing fields are filled with defaults from the API spec.
 * `appliedDefaults` lists every default that was used so the user
 * can be prompted about them.
 */
function buildBacktestConfig(
  parsed: Record<string, unknown>
): StrategyBacktestPayload | null {
  const ast = parsed.ast as Record<string, unknown> | undefined;
  if (
    !ast ||
    typeof ast !== "object" ||
    (!Array.isArray(ast.entry) && !Array.isArray(ast.exit))
  ) {
    return null;
  }

  const appliedDefaults: string[] = [];

  // ── 1. ast ──
  const config: Record<string, unknown> = {
    ast: cleanAst(ast),
  };

  // ── 2. risk_params ──
  const srcRisk =
    parsed.risk_params && typeof parsed.risk_params === "object"
      ? (parsed.risk_params as Record<string, unknown>)
      : {};

  const initialCapital =
    typeof srcRisk.initial_capital === "number"
      ? srcRisk.initial_capital
      : null;
  const positionSize =
    typeof srcRisk.position_size === "number" ? srcRisk.position_size : null;
  const stopLoss =
    typeof srcRisk.stop_loss === "number" ? srcRisk.stop_loss : null;
  const takeProfit =
    typeof srcRisk.take_profit === "number" ? srcRisk.take_profit : null;

  const riskParams: Record<string, number> = {
    initial_capital:
      initialCapital ??
      (() => {
        appliedDefaults.push(
          `initial_capital = ${String(BACKTEST_DEFAULTS.initial_capital)}`
        );
        return BACKTEST_DEFAULTS.initial_capital;
      })(),
    position_size:
      positionSize ??
      (() => {
        appliedDefaults.push(
          `position_size = ${String(BACKTEST_DEFAULTS.position_size)}`
        );
        return BACKTEST_DEFAULTS.position_size;
      })(),
    stop_loss:
      stopLoss ??
      (() => {
        appliedDefaults.push(
          `stop_loss = ${String(BACKTEST_DEFAULTS.stop_loss)}`
        );
        return BACKTEST_DEFAULTS.stop_loss;
      })(),
    take_profit:
      takeProfit ??
      (() => {
        appliedDefaults.push(
          `take_profit = ${String(BACKTEST_DEFAULTS.take_profit)}`
        );
        return BACKTEST_DEFAULTS.take_profit;
      })(),
  };
  config.risk_params = riskParams;

  // ── 3. date_config ──
  const srcDate =
    parsed.date_config && typeof parsed.date_config === "object"
      ? (parsed.date_config as Record<string, unknown>)
      : {};

  if (hasRealDateConfig(srcDate)) {
    // User provided explicit start/end dates — only forward those
    const dateConfig: Record<string, string> = {};
    if (
      typeof srcDate.start_date === "string" &&
      srcDate.start_date.length > 0
    ) {
      dateConfig.start_date = srcDate.start_date;
    }
    if (typeof srcDate.end_date === "string" && srcDate.end_date.length > 0) {
      dateConfig.end_date = srcDate.end_date;
    }
    config.date_config = dateConfig;
  } else {
    // No real date info — use default (never include is_relative / relative_value)
    config.date_config = {
      start_date: "2024-01-01",
      end_date: "2025-01-01",
    };
    appliedDefaults.push("date_config = 2024-01-01 to 2025-01-01");
  }

  // ── 4. date_conditions ──
  if (
    Array.isArray(parsed.date_conditions) &&
    parsed.date_conditions.length > 0
  ) {
    config.date_conditions = (
      parsed.date_conditions as Record<string, unknown>[]
    ).map(mapDateCondition);
  }

  // ── 5. stocks ──
  if (Array.isArray(parsed.stocks) && parsed.stocks.length > 0) {
    config.stocks = parsed.stocks;
  }

  return {
    configJson: JSON.stringify(config, null, 2),
    appliedDefaults,
  };
}

function extractStrategyBacktestPayload(
  message: ChatMessage
): StrategyBacktestPayload | null {
  if (message.role !== "assistant") {
    return null;
  }

  const text = message.parts
    .filter((part) => part.type === "text")
    .map((part) => part.text)
    .join("\n");

  if (!text.includes("strategy_builder")) {
    return null;
  }

  const candidate = extractLastJsonCodeBlock(text);
  if (!candidate) {
    return null;
  }

  try {
    const parsed = JSON.parse(candidate) as Record<string, unknown>;

    // Full parse response: has ast.entry / ast.exit nested inside
    const payload = buildBacktestConfig(parsed);
    if (payload) {
      return payload;
    }

    // Legacy: top-level entry/exit (just the AST itself)
    const hasAstShape =
      (parsed.entry && Array.isArray(parsed.entry)) ||
      (parsed.exit && Array.isArray(parsed.exit));
    if (!hasAstShape) {
      return null;
    }
    return {
      configJson: JSON.stringify(parsed, null, 2),
      appliedDefaults: [],
    };
  } catch {
    return null;
  }
}

const PurePreviewMessage = ({
  addToolApprovalResponse,
  chatId,
  message,
  vote,
  isLoading,
  sendMessage,
  setMessages,
  regenerate,
  isReadonly,
  requiresScrollPadding: _requiresScrollPadding,
}: {
  addToolApprovalResponse: UseChatHelpers<ChatMessage>["addToolApprovalResponse"];
  chatId: string;
  message: ChatMessage;
  vote: Vote | undefined;
  isLoading: boolean;
  sendMessage: UseChatHelpers<ChatMessage>["sendMessage"];
  setMessages: UseChatHelpers<ChatMessage>["setMessages"];
  regenerate: UseChatHelpers<ChatMessage>["regenerate"];
  isReadonly: boolean;
  requiresScrollPadding: boolean;
}) => {
  const [mode, setMode] = useState<"view" | "edit">("view");
  const strategyPayload = extractStrategyBacktestPayload(message);

  const attachmentsFromMessage = message.parts.filter(
    (part) => part.type === "file"
  );

  useDataStream();

  return (
    <div
      className="group/message fade-in w-full animate-in duration-200"
      data-role={message.role}
      data-testid={`message-${message.role}`}
    >
      <div
        className={cn("flex w-full items-start gap-2 md:gap-3", {
          "justify-end": message.role === "user" && mode !== "edit",
          "justify-start": message.role === "assistant",
        })}
      >
        {message.role === "assistant" && (
          <div className="-mt-1 flex size-8 shrink-0 items-center justify-center rounded-full bg-background ring-1 ring-border">
            <SparklesIcon size={14} />
          </div>
        )}

        <div
          className={cn("flex flex-col", {
            "gap-2 md:gap-4": message.parts?.some(
              (p) => p.type === "text" && p.text?.trim()
            ),
            "w-full":
              (message.role === "assistant" &&
                (message.parts?.some(
                  (p) => p.type === "text" && p.text?.trim()
                ) ||
                  message.parts?.some((p) => p.type.startsWith("tool-")))) ||
              mode === "edit",
            "max-w-[calc(100%-2.5rem)] sm:max-w-[min(fit-content,80%)]":
              message.role === "user" && mode !== "edit",
          })}
        >
          {attachmentsFromMessage.length > 0 && (
            <div
              className="flex flex-row justify-end gap-2"
              data-testid={"message-attachments"}
            >
              {attachmentsFromMessage.map((attachment) => (
                <PreviewAttachment
                  attachment={{
                    name: attachment.filename ?? "file",
                    contentType: attachment.mediaType,
                    url: attachment.url,
                  }}
                  key={attachment.url}
                />
              ))}
            </div>
          )}

          {message.parts?.map((part, index) => {
            const { type } = part;
            const key = `message-${message.id}-part-${index}`;

            if (type === "reasoning") {
              const hasContent = part.text?.trim().length > 0;
              const isStreaming = "state" in part && part.state === "streaming";
              if (hasContent || isStreaming) {
                return (
                  <MessageReasoning
                    isLoading={isLoading || isStreaming}
                    key={key}
                    reasoning={part.text || ""}
                  />
                );
              }
            }

            if (type === "text") {
              if (mode === "view") {
                const isAssistant = message.role === "assistant";
                const backtestData = isAssistant
                  ? extractBacktestData(part.text)
                  : null;
                const displayText = backtestData
                  ? stripBacktestDataBlock(part.text)
                  : part.text;

                // Streaming phase detection
                const phase = isAssistant
                  ? getStreamingPhase(part.text, isLoading)
                  : ("done" as StreamPhase);

                const showToolSkeleton = phase === "progress";
                const showResponseSkeleton =
                  phase === "progress" || phase === "tool-selected";

                // Determine if a backtest skeleton should be shown
                // (tool-selected phase with backtester tool, or already
                // streaming backtest content but table data not yet complete)
                const toolName =
                  phase === "tool-selected" ? extractToolName(part.text) : null;
                const isBacktestIncoming = toolName === "backtester";
                const isBacktestStreaming =
                  isAssistant &&
                  isLoading &&
                  part.text.includes("### Backtest Results") &&
                  !backtestData;

                return (
                  <div key={key}>
                    <MessageContent
                      className={cn({
                        "wrap-break-word w-fit rounded-2xl bg-muted px-3 py-2 text-right text-foreground":
                          !isAssistant,
                        "bg-transparent px-0 py-0 text-left": isAssistant,
                      })}
                      data-testid="message-content"
                    >
                      <Response>{sanitizeText(displayText)}</Response>
                    </MessageContent>

                    {/* Tool selection skeleton */}
                    {showToolSkeleton && <ToolSelectionSkeleton />}

                    {/* Response / backtest skeleton while waiting */}
                    {showResponseSkeleton &&
                      (isBacktestIncoming ? (
                        <BacktestSkeleton />
                      ) : (
                        <ResponseSkeleton />
                      ))}

                    {/* Backtest table streaming skeleton */}
                    {isBacktestStreaming && <BacktestSkeleton />}

                    {/* Final backtest table with stagger animation */}
                    {backtestData && (
                      <div className="backtest-stagger">
                        <BacktestResultTable data={backtestData} />
                      </div>
                    )}
                  </div>
                );
              }

              if (mode === "edit") {
                return (
                  <div
                    className="flex w-full flex-row items-start gap-3"
                    key={key}
                  >
                    <div className="size-8" />
                    <div className="min-w-0 flex-1">
                      <MessageEditor
                        key={message.id}
                        message={message}
                        regenerate={regenerate}
                        setMessages={setMessages}
                        setMode={setMode}
                      />
                    </div>
                  </div>
                );
              }
            }

            if (type === "tool-getWeather") {
              const { toolCallId, state } = part;
              const approvalId = (part as { approval?: { id: string } })
                .approval?.id;
              const isDenied =
                state === "output-denied" ||
                (state === "approval-responded" &&
                  (part as { approval?: { approved?: boolean } }).approval
                    ?.approved === false);
              const widthClass = "w-[min(100%,450px)]";

              if (state === "output-available") {
                return (
                  <div className={widthClass} key={toolCallId}>
                    <Weather weatherAtLocation={part.output} />
                  </div>
                );
              }

              if (isDenied) {
                return (
                  <div className={widthClass} key={toolCallId}>
                    <Tool className="w-full" defaultOpen={true}>
                      <ToolHeader
                        state="output-denied"
                        type="tool-getWeather"
                      />
                      <ToolContent>
                        <div className="px-4 py-3 text-muted-foreground text-sm">
                          Weather lookup was denied.
                        </div>
                      </ToolContent>
                    </Tool>
                  </div>
                );
              }

              if (state === "approval-responded") {
                return (
                  <div className={widthClass} key={toolCallId}>
                    <Tool className="w-full" defaultOpen={true}>
                      <ToolHeader state={state} type="tool-getWeather" />
                      <ToolContent>
                        <ToolInput input={part.input} />
                      </ToolContent>
                    </Tool>
                  </div>
                );
              }

              return (
                <div className={widthClass} key={toolCallId}>
                  <Tool className="w-full" defaultOpen={true}>
                    <ToolHeader state={state} type="tool-getWeather" />
                    <ToolContent>
                      {(state === "input-available" ||
                        state === "approval-requested") && (
                        <ToolInput input={part.input} />
                      )}
                      {state === "approval-requested" && approvalId && (
                        <div className="flex items-center justify-end gap-2 border-t px-4 py-3">
                          <button
                            className="rounded-md px-3 py-1.5 text-muted-foreground text-sm transition-colors hover:bg-muted hover:text-foreground"
                            onClick={() => {
                              addToolApprovalResponse({
                                id: approvalId,
                                approved: false,
                                reason: "User denied weather lookup",
                              });
                            }}
                            type="button"
                          >
                            Deny
                          </button>
                          <button
                            className="rounded-md bg-primary px-3 py-1.5 text-primary-foreground text-sm transition-colors hover:bg-primary/90"
                            onClick={() => {
                              addToolApprovalResponse({
                                id: approvalId,
                                approved: true,
                              });
                            }}
                            type="button"
                          >
                            Allow
                          </button>
                        </div>
                      )}
                    </ToolContent>
                  </Tool>
                </div>
              );
            }

            if (type === "tool-createDocument") {
              const { toolCallId } = part;

              if (part.output && "error" in part.output) {
                return (
                  <div
                    className="rounded-lg border border-red-200 bg-red-50 p-4 text-red-500 dark:bg-red-950/50"
                    key={toolCallId}
                  >
                    Error creating document: {String(part.output.error)}
                  </div>
                );
              }

              return (
                <DocumentPreview
                  isReadonly={isReadonly}
                  key={toolCallId}
                  result={part.output}
                />
              );
            }

            if (type === "tool-updateDocument") {
              const { toolCallId } = part;

              if (part.output && "error" in part.output) {
                return (
                  <div
                    className="rounded-lg border border-red-200 bg-red-50 p-4 text-red-500 dark:bg-red-950/50"
                    key={toolCallId}
                  >
                    Error updating document: {String(part.output.error)}
                  </div>
                );
              }

              return (
                <div className="relative" key={toolCallId}>
                  <DocumentPreview
                    args={{ ...part.output, isUpdate: true }}
                    isReadonly={isReadonly}
                    result={part.output}
                  />
                </div>
              );
            }

            if (type === "tool-requestSuggestions") {
              const { toolCallId, state } = part;

              return (
                <Tool defaultOpen={true} key={toolCallId}>
                  <ToolHeader state={state} type="tool-requestSuggestions" />
                  <ToolContent>
                    {state === "input-available" && (
                      <ToolInput input={part.input} />
                    )}
                    {state === "output-available" && (
                      <ToolOutput
                        errorText={undefined}
                        output={
                          "error" in part.output ? (
                            <div className="rounded border p-2 text-red-500">
                              Error: {String(part.output.error)}
                            </div>
                          ) : (
                            <DocumentToolResult
                              isReadonly={isReadonly}
                              result={part.output}
                              type="request-suggestions"
                            />
                          )
                        }
                      />
                    )}
                  </ToolContent>
                </Tool>
              );
            }

            return null;
          })}

          {/* Fallback skeleton when assistant is loading but no text parts yet */}
          {isLoading &&
            message.role === "assistant" &&
            !message.parts?.some(
              (p) => p.type === "text" && p.text?.trim()
            ) && <ResponseSkeleton />}

          {!isReadonly && message.role === "assistant" && strategyPayload && (
            <div className="pt-1">
              {strategyPayload.appliedDefaults.length > 0 && (
                <p className="mb-1.5 text-muted-foreground text-xs">
                  Defaults applied: {strategyPayload.appliedDefaults.join(", ")}
                </p>
              )}
              <button
                className="inline-flex items-center rounded-md border px-3 py-1.5 text-sm transition-colors hover:bg-muted disabled:cursor-not-allowed disabled:opacity-50"
                disabled={isLoading}
                onClick={() => {
                  const defaultsNote =
                    strategyPayload.appliedDefaults.length > 0
                      ? `\n\n**Defaults used:** ${strategyPayload.appliedDefaults.join(", ")}`
                      : "";
                  sendMessage({
                    role: "user",
                    parts: [
                      {
                        type: "text",
                        text:
                          `Run backtest with the following config.${defaultsNote}\n\n` +
                          "```json\n" +
                          strategyPayload.configJson +
                          "\n```",
                      },
                    ],
                  });
                }}
                type="button"
              >
                Backtest Strategy
              </button>
            </div>
          )}

          {!isReadonly && (
            <MessageActions
              chatId={chatId}
              isLoading={isLoading}
              key={`action-${message.id}`}
              message={message}
              setMode={setMode}
              vote={vote}
            />
          )}
        </div>
      </div>
    </div>
  );
};

export const PreviewMessage = PurePreviewMessage;

export const ThinkingMessage = () => {
  return (
    <div
      className="group/message fade-in w-full animate-in duration-150"
      data-role="assistant"
      data-testid="message-assistant-loading"
    >
      <div className="flex items-start justify-start gap-3">
        <div className="-mt-1 flex size-8 shrink-0 items-center justify-center rounded-full bg-background ring-1 ring-border">
          <div className="animate-pulse">
            <SparklesIcon size={14} />
          </div>
        </div>

        <div className="flex w-full max-w-2xl flex-col gap-2">
          <div className="text-muted-foreground text-sm">
            Preparing response...
          </div>
          <div className="space-y-2.5 rounded-xl border border-border/40 bg-muted/10 p-4">
            <div className="h-3.5 w-3/4 rounded-md skeleton-shimmer" />
            <div
              className="h-3.5 w-full rounded-md skeleton-shimmer"
              style={{ animationDelay: "100ms" }}
            />
            <div
              className="h-3.5 w-5/6 rounded-md skeleton-shimmer"
              style={{ animationDelay: "200ms" }}
            />
            <div
              className="h-3.5 w-2/3 rounded-md skeleton-shimmer"
              style={{ animationDelay: "300ms" }}
            />
          </div>
        </div>
      </div>
    </div>
  );
};
