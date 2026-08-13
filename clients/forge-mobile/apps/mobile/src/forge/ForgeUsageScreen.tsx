import { useState } from "react";
import { ActivityIndicator, Pressable, RefreshControl, ScrollView, View } from "react-native";
import { useSafeAreaInsets } from "react-native-safe-area-context";

import { SymbolView } from "../components/AppSymbol";
import { AppText as Text } from "../components/AppText";
import { SettingsSection } from "../features/settings/components/SettingsSection";
import type {
  RemoteUsageAccount,
  RemoteUsageContext,
  RemoteUsageSession,
  RemoteUsageSnapshot,
} from "./protocol/protocol";
import {
  formatUsageCost,
  formatUsageDuration,
  formatUsagePercent,
  formatUsageReset,
  formatUsageTokens,
  formatUsageUpdatedAt,
  usageStateNotice,
} from "./usagePresentation";

type UsageTab = "context" | "session" | "account";

const TABS: ReadonlyArray<{ readonly id: UsageTab; readonly label: string }> = [
  { id: "context", label: "Context" },
  { id: "session", label: "This session" },
  { id: "account", label: "Account limits" },
];

function UsageNotice(props: { readonly message: string; readonly error?: boolean }) {
  return (
    <View
      accessibilityRole="alert"
      className={
        props.error
          ? "flex-row items-center gap-2 rounded-[16px] border border-danger-border bg-danger px-4 py-3"
          : "flex-row items-center gap-2 rounded-[16px] border-continuous bg-card px-4 py-3"
      }
    >
      <SymbolView
        name={props.error ? "exclamationmark.triangle" : "info.circle"}
        size={15}
        tintColor={props.error ? "#dc2626" : "#8e8e93"}
        type="monochrome"
      />
      <Text
        className={
          props.error
            ? "flex-1 text-sm text-danger-foreground"
            : "flex-1 text-sm text-foreground-muted"
        }
      >
        {props.message}
      </Text>
    </View>
  );
}

function ProgressBar(props: { readonly value: number; readonly label: string }) {
  const value = Math.min(100, Math.max(0, props.value));
  return (
    <View
      accessible
      accessibilityLabel={props.label}
      accessibilityRole="progressbar"
      accessibilityValue={{ min: 0, max: 100, now: value }}
      className="h-1.5 overflow-hidden rounded-full bg-subtle"
    >
      <View className="h-full rounded-full bg-accent" style={{ width: `${value}%` }} />
    </View>
  );
}

function MetricCell(props: {
  readonly label: string;
  readonly value: string;
  readonly detail?: string;
}) {
  return (
    <View className="w-1/2 gap-0.5 p-4">
      <Text className="text-sm text-foreground-muted">{props.label}</Text>
      <Text className="text-xl font-t3-medium tabular-nums text-foreground">{props.value}</Text>
      {props.detail ? (
        <Text className="text-xs text-foreground-tertiary">{props.detail}</Text>
      ) : null}
    </View>
  );
}

function ContextPanel(props: { readonly context?: RemoteUsageContext; readonly error?: string }) {
  if (!props.context) {
    return <UsageNotice error message={props.error ?? "Context usage is not available yet."} />;
  }
  return (
    <View className="gap-4">
      {props.error ? <UsageNotice error message={props.error} /> : null}
      <View className="gap-3 rounded-[24px] border-continuous bg-card p-4">
        <Text className="text-sm text-foreground-muted">Context used</Text>
        <Text className="text-4xl font-t3-bold tabular-nums text-foreground">
          {formatUsagePercent(props.context.usedPercent)}
        </Text>
        <ProgressBar value={props.context.usedPercent} label="Context used" />
        <Text className="text-sm text-foreground-muted">
          Automatic compaction begins near {formatUsagePercent(props.context.autoCompactPercent)}.
        </Text>
      </View>
      <SettingsSection title="Context" card>
        <View className="flex-row flex-wrap">
          <MetricCell label="Used" value={formatUsageTokens(props.context.usedTokens)} />
          <MetricCell label="Available" value={formatUsageTokens(props.context.freeTokens)} />
          <MetricCell label="Window" value={formatUsageTokens(props.context.totalTokens)} />
        </View>
      </SettingsSection>
    </View>
  );
}

function SessionPanel(props: { readonly session?: RemoteUsageSession; readonly error?: string }) {
  const session = props.session;
  if (!session) {
    return <UsageNotice error message={props.error ?? "Session usage is not available yet."} />;
  }
  const partial = session.incomplete || session.costState === "partial";
  const costDetail =
    session.costState === "partial"
      ? "Some calls lack trustworthy cost."
      : session.costState === "unavailable"
        ? "Provider cost unavailable."
        : "Reported session cost.";
  return (
    <View className="gap-4">
      {partial ? (
        <UsageNotice
          error
          message={props.error ?? "Usage is partial and may under-count this session."}
        />
      ) : props.error ? (
        <UsageNotice error message={props.error} />
      ) : null}
      <SettingsSection title="This session" card>
        <View className="flex-row flex-wrap">
          <MetricCell
            label="Processed tokens"
            value={formatUsageTokens(session.totalTokens)}
            detail={`${session.modelCalls} model call${session.modelCalls === 1 ? "" : "s"}`}
          />
          <MetricCell label="Cost" value={formatUsageCost(session)} detail={costDetail} />
          <MetricCell
            label="Input"
            value={formatUsageTokens(session.inputTokens)}
            detail={`${formatUsageTokens(session.cachedReadTokens)} cached`}
          />
          <MetricCell
            label="Output"
            value={formatUsageTokens(session.outputTokens)}
            detail={`${formatUsageTokens(session.reasoningTokens)} reasoning`}
          />
          <MetricCell label="Cache writes" value={formatUsageTokens(session.cacheCreationTokens)} />
          <MetricCell label="API time" value={formatUsageDuration(session.apiDurationMs)} />
        </View>
      </SettingsSection>
      {session.models?.length ? (
        <SettingsSection title="By model" card>
          {session.models.map((model, index) => (
            <View
              key={model.modelId}
              className={
                index === 0
                  ? "flex-row items-center gap-3 p-4"
                  : "flex-row items-center gap-3 border-t border-border-subtle p-4"
              }
            >
              <View className="min-w-0 flex-1 gap-0.5">
                <Text className="text-base font-t3-medium text-foreground" numberOfLines={1}>
                  {model.modelId}
                </Text>
                <Text className="text-sm text-foreground-muted">
                  {formatUsageTokens(model.totalTokens)} tokens · {model.modelCalls} calls
                </Text>
              </View>
              <Text className="text-base tabular-nums text-foreground">
                {formatUsageCost(model)}
              </Text>
            </View>
          ))}
        </SettingsSection>
      ) : null}
    </View>
  );
}

function AccountPanel(props: { readonly account?: RemoteUsageAccount; readonly error?: string }) {
  const account = props.account;
  if (!account || account.status !== "ready") {
    return (
      <UsageNotice
        error
        message={props.error ?? account?.message ?? "Account limits are not available yet."}
      />
    );
  }
  return (
    <View className="gap-4">
      {props.error ? <UsageNotice error message={props.error} /> : null}
      <SettingsSection title="Account limits" card>
        <View className="flex-row flex-wrap">
          <MetricCell label="Provider" value={account.provider} />
          {account.plan ? <MetricCell label="Plan" value={account.plan} /> : null}
          {account.credits ? (
            <MetricCell
              label="Credits"
              value={account.credits.unlimited ? "Unlimited" : account.credits.balance}
            />
          ) : null}
          {account.allowed === false ? <MetricCell label="Status" value="Limit reached" /> : null}
        </View>
      </SettingsSection>
      {account.windows.length ? (
        <SettingsSection title="Limits" card>
          {account.windows.map((window, index) => (
            <View
              key={`${window.label}:${index}`}
              className={index === 0 ? "gap-2 p-4" : "gap-2 border-t border-border-subtle p-4"}
            >
              <View className="flex-row items-baseline justify-between gap-3">
                <Text className="text-base font-t3-medium text-foreground">{window.label}</Text>
                <Text className="text-sm tabular-nums text-foreground-muted">
                  {formatUsagePercent(window.usedPercent)} used
                </Text>
              </View>
              <ProgressBar value={window.usedPercent} label={`${window.label} used`} />
              {formatUsageReset(window.resetAt, window.resetLabel) ? (
                <Text className="text-xs text-foreground-tertiary">
                  Resets {formatUsageReset(window.resetAt, window.resetLabel)}
                </Text>
              ) : null}
            </View>
          ))}
        </SettingsSection>
      ) : (
        <UsageNotice
          error
          message={account.message ?? "This provider did not report quota windows."}
        />
      )}
    </View>
  );
}

export function ForgeUsageScreen(props: {
  readonly usage?: RemoteUsageSnapshot;
  readonly refreshing: boolean;
  readonly canRefresh: boolean;
  readonly connectionError?: string | null;
  readonly onRefresh: () => void;
}) {
  const insets = useSafeAreaInsets();
  const [tab, setTab] = useState<UsageTab>("context");
  const notice = usageStateNotice(props.usage, props.refreshing);
  const isRefreshing = props.refreshing || props.usage?.status === "loading";

  return (
    <View className="flex-1 bg-sheet">
      <ScrollView
        className="flex-1"
        contentInsetAdjustmentBehavior="automatic"
        showsVerticalScrollIndicator={false}
        contentContainerClassName="gap-4 px-5 pt-4"
        contentContainerStyle={{ paddingBottom: Math.max(insets.bottom, 18) + 18 }}
        refreshControl={
          <RefreshControl
            refreshing={isRefreshing}
            enabled={props.canRefresh}
            onRefresh={props.onRefresh}
          />
        }
      >
        <View className="flex-row items-center gap-3 rounded-[24px] border-continuous bg-card p-4">
          <View className="size-10 items-center justify-center rounded-full bg-subtle">
            <SymbolView name="chart.bar.xaxis" size={18} tintColor="#8e8e93" type="monochrome" />
          </View>
          <View className="min-w-0 flex-1 gap-0.5">
            <Text className="text-lg font-t3-bold text-foreground">Usage</Text>
            <Text className="text-sm text-foreground-muted">
              {formatUsageUpdatedAt(props.usage?.refreshedAt)}
            </Text>
          </View>
          <Pressable
            accessibilityLabel="Refresh usage"
            accessibilityRole="button"
            accessibilityState={{ disabled: !props.canRefresh || isRefreshing }}
            disabled={!props.canRefresh || isRefreshing}
            onPress={props.onRefresh}
            className="size-10 items-center justify-center rounded-full bg-subtle active:opacity-70 disabled:opacity-40"
          >
            {isRefreshing ? (
              <ActivityIndicator size="small" color="#8e8e93" />
            ) : (
              <SymbolView name="arrow.clockwise" size={16} tintColor="#8e8e93" type="monochrome" />
            )}
          </Pressable>
        </View>

        <View className="flex-row overflow-hidden rounded-full border-continuous bg-card">
          {TABS.map((item) => {
            const selected = tab === item.id;
            return (
              <Pressable
                key={item.id}
                accessibilityRole="tab"
                accessibilityState={{ selected }}
                onPress={() => setTab(item.id)}
                className={
                  selected
                    ? "flex-1 items-center rounded-full bg-subtle-strong px-2 py-2.5"
                    : "flex-1 items-center px-2 py-2.5"
                }
              >
                <Text
                  className={
                    selected
                      ? "text-xs font-t3-medium text-foreground"
                      : "text-xs text-foreground-muted"
                  }
                  numberOfLines={1}
                >
                  {item.label}
                </Text>
              </Pressable>
            );
          })}
        </View>

        {props.connectionError ? <UsageNotice error message={props.connectionError} /> : null}
        {notice ? <UsageNotice error={notice.tone === "error"} message={notice.text} /> : null}

        {tab === "context" ? (
          <ContextPanel context={props.usage?.context} error={props.usage?.errors?.context} />
        ) : null}
        {tab === "session" ? (
          <SessionPanel session={props.usage?.session} error={props.usage?.errors?.session} />
        ) : null}
        {tab === "account" ? (
          <AccountPanel account={props.usage?.account} error={props.usage?.errors?.account} />
        ) : null}
      </ScrollView>
    </View>
  );
}
