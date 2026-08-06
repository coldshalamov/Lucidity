import type {
  ConversationId,
  ConversationPresentation,
  HostPresentationSnapshot,
  RuntimeState,
  UsageMeasure,
  UsageSnapshot
} from "./protocol";

export type ConnectionPhase = "offline" | "connecting" | "online" | "reconnecting" | "error";
export type ConversationSection = "currentWorkspace" | "otherProjects" | "settled";

export interface ConnectionViewState {
  phase: ConnectionPhase;
  message: string;
  attempt: number;
}

export interface RuntimePresentation {
  token: "IDLE" | "START" | "WORK" | "NEEDS" | "APPR" | "DONE" | "FAIL" | "EXT?";
  label: string;
  icon: string;
  themeColor: string;
  cap: string;
}

export interface ConversationTreeRow {
  id: ConversationId;
  label: string;
  description: string;
  tooltip: string;
  accessibilityLabel: string;
  contextValue: "lucidity.active" | "lucidity.settled";
  icon: string;
  themeColor: string;
  attached: boolean;
}

export const RUNTIME_PRESENTATION: Readonly<Record<RuntimeState, RuntimePresentation>> = {
  not_running: {
    token: "IDLE",
    label: "not running",
    icon: "circle-outline",
    themeColor: "descriptionForeground",
    cap: "centered hollow cap"
  },
  starting: {
    token: "START",
    label: "starting",
    icon: "run",
    themeColor: "charts.blue",
    cap: "solid top third"
  },
  working: {
    token: "WORK",
    label: "working",
    icon: "pulse",
    themeColor: "charts.blue",
    cap: "full solid with event tick"
  },
  waiting_for_input: {
    token: "NEEDS",
    label: "waiting for input",
    icon: "question",
    themeColor: "charts.yellow",
    cap: "full with one notch"
  },
  awaiting_approval: {
    token: "APPR",
    label: "awaiting approval",
    icon: "shield",
    themeColor: "charts.orange",
    cap: "full with two notches"
  },
  completed_idle: {
    token: "DONE",
    label: "completed and idle",
    icon: "pass-filled",
    themeColor: "testing.iconPassed",
    cap: "centered solid tick"
  },
  failed: {
    token: "FAIL",
    label: "failed",
    icon: "error",
    themeColor: "testing.iconFailed",
    cap: "full with cross"
  },
  unknown_external: {
    token: "EXT?",
    label: "unknown external state",
    icon: "circle-large-outline",
    themeColor: "descriptionForeground",
    cap: "dashed cap"
  }
};

export function emptyPresentation(): HostPresentationSnapshot {
  return {
    conversations: [],
    usage: {
      conversationId: null,
      quota: null,
      context: null,
      observedAt: "",
      source: "unavailable",
      stale: false,
      error: null
    }
  };
}

export function conversationsForSection(
  snapshot: HostPresentationSnapshot,
  section: ConversationSection,
  currentWorkspacePath: string | undefined
): ConversationPresentation[] {
  const workspace = normalizePath(currentWorkspacePath);
  return snapshot.conversations.filter((conversation) => {
    if (section === "settled") {
      return conversation.organizationState === "settled";
    }
    if (conversation.organizationState !== "active") {
      return false;
    }
    const isCurrent = workspace !== undefined && normalizePath(conversation.projectPath) === workspace;
    return section === "currentWorkspace" ? isCurrent : !isCurrent;
  });
}

export function toTreeRow(conversation: ConversationPresentation): ConversationTreeRow {
  const runtime = RUNTIME_PRESENTATION[conversation.runtimeState];
  const project = conversation.projectPath ? lastPathComponent(conversation.projectPath) : "No project";
  const attachment = conversation.attached ? "Attached. " : "";
  const pending = conversation.pendingIdentity ? "Pending identity confirmation. " : "";
  const stale = conversation.evidenceStale ? "Evidence is stale or uncertain. " : "";
  const rowError = conversation.rowError ? `Row error: ${conversation.rowError}. ` : "";
  const eventCount = `${conversation.eventTally} structured event${conversation.eventTally === 1 ? "" : "s"}`;

  return {
    id: conversation.id,
    label: conversation.title,
    description: `${runtime.token} · ${project} · ${eventCount}${conversation.attached ? " · WELD" : ""}`,
    tooltip: `${conversation.adapterDisplayName} · ${conversation.native.nativeSessionId}\n${runtime.token}: ${runtime.label}\n${conversation.projectPath ?? "No project"}`,
    accessibilityLabel: `${conversation.title}. ${attachment}${runtime.token}, ${runtime.label}, ${runtime.cap}. ${conversation.organizationState}. Adapter ${conversation.adapterDisplayName}. Project ${project}. ${eventCount}. ${pending}${stale}${rowError}`.trim(),
    contextValue:
      conversation.organizationState === "settled" ? "lucidity.settled" : "lucidity.active",
    icon: runtime.icon,
    themeColor: runtime.themeColor,
    attached: conversation.attached
  };
}

export function usageStatusText(usage: UsageSnapshot, phase: ConnectionPhase): string {
  if (phase !== "online") {
    return "$(plug) Lucidity: Offline";
  }
  const quota = formatMeasure(usage.quota);
  const context = formatMeasure(usage.context);
  const stale = usage.stale ? " · Stale" : "";
  return `$(pulse) Quota ${quota} · Context ${context}${stale}`;
}

export function usageTooltip(usage: UsageSnapshot, phase: ConnectionPhase): string {
  if (phase !== "online") {
    return "Lucidity host is offline. Start Lucidity Agent Terminal and run Reconnect.";
  }
  const error = usage.error ? `\nError: ${usage.error}` : "";
  return `Account quota: ${formatMeasure(usage.quota)}\nConversation context: ${formatMeasure(usage.context)}\nSource: ${usage.source}${usage.stale ? " (stale)" : ""}${error}`;
}

function formatMeasure(measure: UsageMeasure | null): string {
  if (measure === null || measure.limit <= 0) {
    return "—";
  }
  return `${Math.round((measure.value / measure.limit) * 100)}%`;
}

function normalizePath(value: string | null | undefined): string | undefined {
  if (value === null || value === undefined || value.length === 0) {
    return undefined;
  }
  return value.replaceAll("/", "\\").replace(/\\+$/, "").toLocaleLowerCase("en-US");
}

function lastPathComponent(value: string): string {
  const parts = value.split(/[\\/]/).filter((part) => part.length > 0);
  return parts.at(-1) ?? value;
}
