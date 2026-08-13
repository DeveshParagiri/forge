import type { ApprovalRequestId, ProviderApprovalDecision } from "@t3tools/contracts";
import { useEffect, useState } from "react";
import { Pressable, View } from "react-native";

import { AppText as Text, AppTextInput as TextInput } from "../../components/AppText";
import { cn } from "../../lib/cn";
import type { PendingApproval } from "../../lib/threadActivity";

export interface PendingApprovalCardProps {
  readonly approval: PendingApproval;
  readonly respondingApprovalId: ApprovalRequestId | null;
  readonly onRespond: (
    requestId: ApprovalRequestId,
    decision: ProviderApprovalDecision,
  ) => Promise<unknown>;
  readonly onRemoteRespond?: (
    requestId: ApprovalRequestId,
    actionId: string,
    feedback?: string,
  ) => Promise<unknown>;
}

export function PendingApprovalCard(props: PendingApprovalCardProps) {
  const [remoteFeedback, setRemoteFeedback] = useState("");
  const responding = props.respondingApprovalId === props.approval.requestId;
  useEffect(() => setRemoteFeedback(""), [props.approval.requestId]);
  // Opaque for the same reason as PendingUserInputCard: nothing blurs the feed
  // behind this card, so a translucent surface bleeds messages through it.
  return (
    <View className="gap-2.5 rounded-[20px] border border-neutral-200 bg-neutral-100 p-4 dark:border-white/6 dark:bg-neutral-900">
      <Text className="font-t3-bold text-2xs uppercase tracking-[1.1px] text-sky-700 dark:text-sky-300">
        Approval needed
      </Text>
      <Text className="font-t3-bold text-lg text-neutral-950 dark:text-neutral-50">
        {props.approval.displayLabel ?? props.approval.requestKind}
      </Text>
      {props.approval.detail ? (
        <Text className="font-sans text-sm leading-normal text-neutral-600 dark:text-neutral-400">
          {props.approval.detail}
        </Text>
      ) : null}
      {props.approval.remoteFeedback ? (
        <TextInput
          value={remoteFeedback}
          editable={!responding}
          onChangeText={setRemoteFeedback}
          placeholder={props.approval.remoteFeedback.placeholder}
          multiline
          className="min-h-[68px]"
        />
      ) : null}
      <View className="flex-row flex-wrap gap-2.5">
        {props.approval.remoteActions && props.onRemoteRespond
          ? props.approval.remoteActions.map((action) => (
              <Pressable
                key={action.id}
                className={cn(
                  "min-w-[112px] flex-1 items-center justify-center rounded-[14px] px-3.5 py-3",
                  action.tone === "primary"
                    ? "bg-blue-500"
                    : action.tone === "danger"
                      ? "bg-rose-100 dark:bg-rose-500/18"
                      : "bg-neutral-200 dark:bg-neutral-800",
                )}
                disabled={responding}
                onPress={() => void props.onRemoteRespond?.(props.approval.requestId, action.id)}
              >
                <Text
                  className={cn(
                    "text-center text-sm font-t3-bold",
                    action.tone === "primary"
                      ? "text-white"
                      : action.tone === "danger"
                        ? "text-rose-700 dark:text-rose-300"
                        : "text-neutral-950 dark:text-neutral-50",
                  )}
                >
                  {action.label}
                </Text>
                {action.description ? (
                  <Text className="pt-1 text-center text-xs text-neutral-500 dark:text-neutral-400">
                    {action.description}
                  </Text>
                ) : null}
              </Pressable>
            ))
          : null}
        {props.approval.remoteFeedback && props.onRemoteRespond ? (
          <Pressable
            className={cn(
              "min-w-[112px] flex-1 items-center justify-center rounded-[14px] px-3.5 py-3",
              remoteFeedback.trim() ? "bg-blue-500" : "bg-neutral-200 dark:bg-neutral-700/60",
            )}
            disabled={responding || !remoteFeedback.trim()}
            onPress={() =>
              void props.onRemoteRespond?.(
                props.approval.requestId,
                props.approval.remoteFeedback!.actionId,
                remoteFeedback.trim(),
              )
            }
          >
            <Text className="text-center text-sm font-t3-bold text-white">
              {props.approval.remoteFeedback.actionLabel}
            </Text>
          </Pressable>
        ) : null}
        {!props.approval.remoteActions ? (
          <>
        <Pressable
          className="items-center justify-center rounded-[14px] bg-blue-500 px-3.5 py-3"
          disabled={responding}
          onPress={() => void props.onRespond(props.approval.requestId, "accept")}
        >
          <Text className="font-t3-extrabold text-sm text-white">
            {props.approval.acceptLabel ?? "Allow once"}
          </Text>
        </Pressable>
        {props.approval.allowSession !== false ? (
          <Pressable
            className="items-center justify-center rounded-[14px] bg-neutral-200 px-3.5 py-3 dark:bg-neutral-800"
            disabled={responding}
            onPress={() => void props.onRespond(props.approval.requestId, "acceptForSession")}
          >
            <Text className="font-t3-bold text-sm text-neutral-950 dark:text-neutral-50">
              Allow session
            </Text>
          </Pressable>
        ) : null}
        <Pressable
          className="items-center justify-center rounded-[14px] bg-rose-100 px-3.5 py-3 dark:bg-rose-500/18"
          disabled={responding}
          onPress={() => void props.onRespond(props.approval.requestId, "decline")}
        >
          <Text className="font-t3-bold text-sm text-rose-700 dark:text-rose-300">
            {props.approval.declineLabel ?? "Decline"}
          </Text>
        </Pressable>
          </>
        ) : null}
      </View>
    </View>
  );
}
