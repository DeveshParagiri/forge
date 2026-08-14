import { RegistryContext } from "@effect/atom-react";
import type { EnvironmentConnectionPhase } from "@t3tools/client-runtime/connection";
import type {
  EnvironmentProject,
  EnvironmentThreadShell,
} from "@t3tools/client-runtime/state/shell";
import type { MenuAction } from "@react-native-menu/menu";
import {
  ApprovalRequestId,
  MessageId,
  type EnvironmentId,
  type ModelSelection,
  type UserInputQuestion,
} from "@t3tools/contracts";
import {
  DarkTheme,
  DefaultTheme,
  NavigationContainer,
  createNavigationContainerRef,
  useFocusEffect,
} from "@react-navigation/native";
import {
  createNativeStackNavigator,
  type NativeStackHeaderItem,
  type NativeStackScreenProps,
} from "@react-navigation/native-stack";
import { CameraView, useCameraPermissions, type BarcodeScanningResult } from "expo-camera";
import * as Clipboard from "expo-clipboard";
import { Image } from "expo-image";
import * as Linking from "expo-linking";
import * as SplashScreen from "expo-splash-screen";
import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import {
  Alert,
  Keyboard,
  Platform,
  Pressable,
  StatusBar,
  useColorScheme,
  View,
} from "react-native";
import { GestureHandlerRootView } from "react-native-gesture-handler";
import { KeyboardProvider } from "react-native-keyboard-controller";
import { SafeAreaProvider, useSafeAreaInsets } from "react-native-safe-area-context";

import { AppText as Text } from "../components/AppText";
import { SymbolView } from "../components/AppSymbol";
import { ControlPillMenu } from "../components/ControlPill";
import { HomeScreen } from "../features/home/HomeScreen";
import {
  AppearancePreferencesProvider,
  useAppearancePreferences,
} from "../features/settings/appearance/AppearancePreferencesProvider";
import { ThreadDetailScreen } from "../features/threads/ThreadDetailScreen";
import {
  remoteNewSessionHeaderPresentation,
  remoteNewSessionNavigationTarget,
  remoteSessionCreatedRegistrationInput,
  type RemoteNewSessionHeaderPresentation,
} from "../features/threads/remoteNewSessionPresentation";
import {
  presentRemoteQueuedMessages,
  type RemoteQueuedMessagePresentation,
} from "../features/threads/remoteQueuePresentation";
import { connectionTone } from "../features/connection/connectionTone";
import {
  buildPendingUserInputAnswers,
  setPendingUserInputCustomAnswer,
  togglePendingUserInputOptionSelection,
  type PendingUserInputDraftAnswer,
} from "../lib/threadActivity";
import { useThemeColor } from "../lib/useThemeColor";
import { scopedProjectKey } from "../lib/scopedEntities";
import { appAtomRegistry } from "../state/atom-registry";
import type { WorkspaceState } from "../state/workspaceModel";
import { withNativeGlassHeaderItem } from "../features/layout/native-glass-header-items";
import {
  convertPastedImagesToAttachments,
  pickComposerImageFiles,
  pickComposerImages,
  type DraftComposerImageAttachment,
} from "../lib/composerImages";
import { compactUsageLimitLabel } from "./usagePresentation";
import { parseForgeComposerCommand } from "./protocol/composerCommand";
import { drainQueuedPairing, openOrQueuePairing } from "./protocol/pendingNavigation";
import { createPairingScanGate } from "./protocol/pairingScan";
import type { RemoteQuestion } from "./protocol/protocol";
import {
  presentInteraction,
  presentRemoteSession,
  reasoningFromSelection,
  remoteApprovalResponse,
} from "./presentationAdapter";
import { ForgeSessionsProvider, useForgeSessions } from "./state/ForgeSessionsProvider";
import { ForgeUsageScreen } from "./ForgeUsageScreen";
import {
  FORGE_SESSION_HEADER_STATUS_GEOMETRY,
  forgeConnectionStatusDot,
} from "./connectionStatusDot";
import { forgeHomeHeaderPresentation } from "./chromePresentation";
import { openForgeHomeThread } from "./homeThreadNavigation";
import { remoteHomeProjectGroupKey } from "../features/home/remoteHomePresentation";

import "../../global.css";

void SplashScreen.preventAutoHideAsync().catch(() => undefined);

type ForgeStackParams = {
  Home: undefined;
  Thread: { pairingId: string };
  Usage: { pairingId: string };
  Pair: undefined;
};

const Stack = createNativeStackNavigator<ForgeStackParams>();
const navigationRef = createNavigationContainerRef<ForgeStackParams>();

function ForgeConnectionDot(props: {
  readonly phase: "connecting" | "connected" | "reconnecting" | "error";
}) {
  const presentation = forgeConnectionStatusDot(props.phase);
  const geometry = FORGE_SESSION_HEADER_STATUS_GEOMETRY;
  return (
    <View
      accessible={false}
      style={{
        backgroundColor: presentation.color,
        borderRadius: geometry.radius,
        height: geometry.diameter,
        width: geometry.diameter,
      }}
    />
  );
}

const FORGE_HOME_HEADER = forgeHomeHeaderPresentation();

function ForgeWordmark() {
  return (
    <View className="flex-row items-center gap-2">
      <Image source={require("../../assets/forge/mark.png")} style={{ height: 24, width: 24 }} />
      <Text className="font-t3-bold text-[17px] tracking-tight text-white">Forge</Text>
    </View>
  );
}

function ForgeSessionHeaderTitle(props: {
  readonly phase: "connecting" | "connected" | "reconnecting" | "error";
  readonly title: string;
}) {
  const geometry = FORGE_SESSION_HEADER_STATUS_GEOMETRY;
  return (
    <View className="max-w-[210px] flex-row items-center" style={{ columnGap: geometry.titleGap }}>
      <Text className="shrink font-t3-bold text-[16px] text-white" numberOfLines={1}>
        {props.title}
      </Text>
      <ForgeConnectionDot phase={props.phase} />
    </View>
  );
}

function HeaderPairButton(props: { readonly onPress: () => void }) {
  return (
    <Pressable
      accessibilityHint="Opens the scanner to add another private Forge session."
      accessibilityLabel={FORGE_HOME_HEADER.addSessionAccessibilityLabel}
      accessibilityRole="button"
      className="h-10 w-10 items-center justify-center rounded-full border border-white/15 active:opacity-60"
      onPress={props.onPress}
    >
      <Text className="-mt-0.5 text-[28px] font-t3-medium leading-[30px] text-white">
        {FORGE_HOME_HEADER.addSessionLabel}
      </Text>
    </Pressable>
  );
}

function promptForSessionRename(
  currentTitle: string,
  rename: (title: string | null) => void,
): void {
  if (Platform.OS !== "ios") return;
  Alert.prompt(
    "Rename session",
    "Leave the name blank to use the title from the desktop session.",
    [
      { text: "Cancel", style: "cancel" },
      {
        text: "Save",
        onPress: (value?: string) => rename(value?.trim() || null),
      },
    ],
    "plain-text",
    currentTitle,
  );
}

function ForgeThreadHeaderActions(props: {
  readonly pinned: boolean;
  readonly title: string;
  readonly onArchive: () => void;
  readonly newSessionAction: RemoteNewSessionHeaderPresentation | null;
  readonly onCreateNewSession: () => void;
  readonly onPinChange: () => void;
  readonly onRename: () => void;
}) {
  const iconColor = "#FFFFFF";
  const dividerColor = "rgba(255,255,255,0.14)";
  const actions = useMemo<MenuAction[]>(
    () => [
      {
        id: props.pinned ? "unpin" : "pin",
        title: props.pinned ? "Unpin" : "Pin",
        image: props.pinned ? "pin.slash" : "pin",
      },
      ...(Platform.OS === "ios"
        ? [{ id: "rename", title: "Rename", image: "square.and.pencil" } satisfies MenuAction]
        : []),
      {
        id: "archive",
        title: "Archive",
        image: "archivebox",
        attributes: { destructive: true },
      },
    ],
    [props.pinned],
  );
  return (
    <View
      className="h-11 flex-row items-center overflow-hidden rounded-full"
      style={{ backgroundColor: "rgba(255,255,255,0.1)" }}
    >
      {props.newSessionAction ? (
        <>
          <Pressable
            accessibilityLabel={props.newSessionAction.accessibilityLabel}
            accessibilityRole="button"
            className="h-11 w-12 items-center justify-center active:opacity-60"
            disabled={props.newSessionAction.disabled}
            onPress={props.onCreateNewSession}
            style={{ opacity: props.newSessionAction.disabled ? 0.45 : 1 }}
          >
            <SymbolView
              name={props.newSessionAction.systemImage}
              size={20}
              tintColor={iconColor}
              type="monochrome"
            />
          </Pressable>
          <View className="h-6 w-px" style={{ backgroundColor: dividerColor }} />
        </>
      ) : null}
      <ControlPillMenu
        actions={actions}
        onPressAction={({ nativeEvent }) => {
          if (nativeEvent.event === "pin" || nativeEvent.event === "unpin") props.onPinChange();
          if (nativeEvent.event === "rename") props.onRename();
          if (nativeEvent.event === "archive") props.onArchive();
        }}
        title={props.title}
      >
        <View
          accessible
          accessibilityLabel={`More actions for ${props.title}`}
          accessibilityRole="button"
          className="h-11 w-12 items-center justify-center"
        >
          <SymbolView name="ellipsis" size={20} tintColor={iconColor} type="monochrome" />
        </View>
      </ControlPillMenu>
    </View>
  );
}

function forgeHomeNativeHeaderItems(onAddSession: () => void): NativeStackHeaderItem[] {
  return [
    withNativeGlassHeaderItem({
      accessibilityLabel: FORGE_HOME_HEADER.addSessionAccessibilityLabel,
      icon: { name: "plus", type: "sfSymbol" as const },
      identifier: "forge-home-add-session",
      label: "",
      onPress: onAddSession,
      type: "button" as const,
    } satisfies Extract<NativeStackHeaderItem, { type: "button" }>),
  ];
}

function forgeThreadNativeHeaderItems(props: {
  readonly pinned: boolean;
  readonly title: string;
  readonly newSessionAction: RemoteNewSessionHeaderPresentation | null;
  readonly onCreateNewSession: () => void;
  readonly onArchive: () => void;
  readonly onPinChange: () => void;
  readonly onRename: () => void;
}): NativeStackHeaderItem[] {
  const newSessionItem = props.newSessionAction
    ? withNativeGlassHeaderItem({
        accessibilityLabel: props.newSessionAction.accessibilityLabel,
        disabled: props.newSessionAction.disabled,
        icon: { name: props.newSessionAction.systemImage, type: "sfSymbol" as const },
        identifier: "forge-thread-new-session",
        label: "",
        onPress: props.onCreateNewSession,
        type: "button" as const,
      } satisfies Extract<NativeStackHeaderItem, { type: "button" }>)
    : null;
  const pinIcon: "pin" | "pin.slash" = props.pinned ? "pin.slash" : "pin";
  const moreItem = withNativeGlassHeaderItem({
    accessibilityLabel: `More actions for ${props.title}`,
    icon: { name: "ellipsis", type: "sfSymbol" as const },
    identifier: "forge-thread-more",
    label: "",
    menu: {
      title: props.title,
      items: [
        {
          type: "action" as const,
          label: props.pinned ? "Unpin" : "Pin",
          icon: { name: pinIcon, type: "sfSymbol" as const },
          onPress: props.onPinChange,
        },
        {
          type: "action" as const,
          label: "Rename",
          icon: { name: "square.and.pencil", type: "sfSymbol" as const },
          onPress: props.onRename,
        },
        {
          type: "action" as const,
          destructive: true,
          label: "Archive",
          icon: { name: "archivebox", type: "sfSymbol" as const },
          onPress: props.onArchive,
        },
      ],
    },
    type: "menu" as const,
  } satisfies Extract<NativeStackHeaderItem, { type: "menu" }>);
  return [...(newSessionItem ? [newSessionItem] : []), moreItem];
}

function connectionPhase(
  phase: "connecting" | "connected" | "reconnecting" | "error",
): EnvironmentConnectionPhase {
  return phase;
}

function ForgeHomeScreen(props: NativeStackScreenProps<ForgeStackParams, "Home">) {
  const {
    archiveSession,
    completeNewSession,
    newSession,
    pinSession,
    ready,
    reconnect,
    registerPairing,
    releaseActiveSession,
    renameSession,
    sessions,
    unpinSession,
  } = useForgeSessions();
  const presented = useMemo(() => sessions.map(presentRemoteSession), [sessions]);
  const sessionByPairingId = useMemo(
    () => new Map(sessions.map((session) => [session.pairing.id, session])),
    [sessions],
  );
  const pairingIdByThread = useMemo(
    () => new Map(presented.map((entry, index) => [entry.thread.id, sessions[index]?.pairing.id])),
    [presented, sessions],
  );
  const pairingIdByProjectRef = useMemo(
    () =>
      new Map(
        presented.flatMap((entry, index) => {
          const pairingId = sessions[index]?.pairing.id;
          return pairingId
            ? [
                [
                  scopedProjectKey(entry.project.environmentId, entry.project.id),
                  pairingId,
                ] as const,
              ]
            : [];
        }),
      ),
    [presented, sessions],
  );
  const [newSessionSourcePairingId, setNewSessionSourcePairingId] = useState<string | null>(null);
  const newSessionFlowPairingIdRef = useRef<string | null>(null);
  const projectGroupKeyByProjectRef = useMemo(
    () =>
      new Map(
        presented.map((entry, index) => [
          scopedProjectKey(entry.project.environmentId, entry.project.id),
          remoteHomeProjectGroupKey(
            sessions[index]?.pairing.host ?? "private-forge",
            entry.project.workspaceRoot,
          ),
        ]),
      ),
    [presented, sessions],
  );
  const environments = useMemo(
    () =>
      presented.map((entry, index) => ({
        environmentId: entry.environmentId,
        label: sessions[index]?.pairing.host ?? "Private Forge",
        connectionState: connectionPhase(sessions[index]?.connectionPhase ?? "connecting"),
      })),
    [presented, sessions],
  );
  const catalogState = useMemo<WorkspaceState>(() => {
    const hasSnapshot = sessions.some((session) => session.snapshot !== null);
    const hasConnected = sessions.some((session) => session.connectionPhase === "connected");
    const hasConnecting = sessions.some(
      (session) =>
        session.connectionPhase === "connecting" || session.connectionPhase === "reconnecting",
    );
    return {
      isLoadingConnections: !ready,
      hasConnections: sessions.length > 0,
      hasLoadedShellSnapshot: hasSnapshot,
      hasPendingShellSnapshot: hasConnecting,
      hasReadyEnvironment: hasConnected,
      hasConnectingEnvironment: hasConnecting,
      connectingEnvironments: [],
      connectionState: hasConnected ? "connected" : hasConnecting ? "connecting" : "available",
      connectionError: null,
      shellSnapshotError: null,
      latestCachedSnapshotReceivedAt: null,
      networkStatus: "online",
    };
  }, [ready, sessions]);
  const noOp = useCallback(() => undefined, []);
  const noOpAsync = useCallback(async () => false, []);
  useFocusEffect(
    useCallback(() => {
      releaseActiveSession();
    }, [releaseActiveSession]),
  );
  useEffect(() => {
    const addSession = () => props.navigation.navigate("Pair");
    props.navigation.setOptions({
      headerBlurEffect: undefined,
      headerShadowVisible: false,
      headerStyle: { backgroundColor: FORGE_HOME_HEADER.backgroundColor },
      headerTintColor: "#FFFFFF",
      headerTitle: () => <ForgeWordmark />,
      headerTransparent: false,
      headerRight:
        Platform.OS === "ios" ? undefined : () => <HeaderPairButton onPress={addSession} />,
      statusBarStyle: "light",
      unstable_headerRightItems:
        Platform.OS === "ios" ? () => forgeHomeNativeHeaderItems(addSession) : undefined,
    });
  }, [props.navigation]);

  const pairingIdForThread = useCallback(
    (thread: EnvironmentThreadShell) => pairingIdByThread.get(thread.id),
    [pairingIdByThread],
  );
  const pairingIdForProject = useCallback(
    (project: EnvironmentProject) =>
      pairingIdByProjectRef.get(scopedProjectKey(project.environmentId, project.id)),
    [pairingIdByProjectRef],
  );
  const isNewThreadInProjectSupported = useCallback(
    (project: EnvironmentProject) => {
      const pairingId = pairingIdForProject(project);
      return (
        pairingId !== undefined &&
        sessionByPairingId.get(pairingId)?.snapshot?.capabilities.newSession === true
      );
    },
    [pairingIdForProject, sessionByPairingId],
  );
  const isNewThreadInProjectPending = useCallback(
    (project: EnvironmentProject) => {
      const pairingId = pairingIdForProject(project);
      return (
        pairingId !== undefined &&
        (newSessionSourcePairingId === pairingId ||
          sessionByPairingId.get(pairingId)?.newSessionCommandPending === true)
      );
    },
    [newSessionSourcePairingId, pairingIdForProject, sessionByPairingId],
  );
  const requestNewSessionInProject = useCallback(
    (project: EnvironmentProject) => {
      if (newSessionSourcePairingId !== null) return;
      const pairingId = pairingIdForProject(project);
      if (!pairingId) return;
      const source = sessionByPairingId.get(pairingId);
      if (source?.snapshot?.capabilities.newSession !== true || source.newSessionCommandPending) {
        return;
      }
      setNewSessionSourcePairingId(pairingId);
      reconnect(pairingId);
    },
    [newSessionSourcePairingId, pairingIdForProject, reconnect, sessionByPairingId],
  );
  useEffect(() => {
    const sourcePairingId = newSessionSourcePairingId;
    if (!sourcePairingId || newSessionFlowPairingIdRef.current !== null) return;
    const source = sessionByPairingId.get(sourcePairingId);
    if (!source || source.connectionPhase === "error") {
      setNewSessionSourcePairingId(null);
      releaseActiveSession(sourcePairingId);
      return;
    }
    if (source.connectionPhase !== "connected" || source.newSessionCommandPending) return;
    if (source.snapshot?.capabilities.newSession !== true) {
      setNewSessionSourcePairingId(null);
      releaseActiveSession(sourcePairingId);
      return;
    }

    newSessionFlowPairingIdRef.current = sourcePairingId;
    const created = newSession(sourcePairingId);
    if (!created) {
      newSessionFlowPairingIdRef.current = null;
      setNewSessionSourcePairingId(null);
      releaseActiveSession(sourcePairingId);
      return;
    }
    void created
      .then(async (outcome) => {
        const registeredPairingId = await registerPairing(
          remoteSessionCreatedRegistrationInput(outcome),
          outcome.sessionId,
        );
        const accepted = await completeNewSession(
          sourcePairingId,
          registeredPairingId,
          outcome.sessionId,
        );
        if (!accepted) return;
        const target = remoteNewSessionNavigationTarget(outcome, registeredPairingId);
        props.navigation.reset({
          index: 1,
          routes: [{ name: "Home" }, { name: "Thread", params: { pairingId: target.pairingId } }],
        });
      })
      .catch(() => undefined)
      .finally(() => {
        if (newSessionFlowPairingIdRef.current === sourcePairingId) {
          newSessionFlowPairingIdRef.current = null;
        }
        setNewSessionSourcePairingId((current) => (current === sourcePairingId ? null : current));
        releaseActiveSession(sourcePairingId);
      });
  }, [
    completeNewSession,
    newSession,
    newSessionSourcePairingId,
    props.navigation,
    registerPairing,
    releaseActiveSession,
    sessionByPairingId,
  ]);
  const renameThread = useCallback(
    (thread: EnvironmentThreadShell) => {
      const pairingId = pairingIdForThread(thread);
      if (!pairingId) return;
      promptForSessionRename(thread.title, (title) => {
        void renameSession(pairingId, title);
      });
    },
    [pairingIdForThread, renameSession],
  );

  return (
    <View className="flex-1 bg-screen">
      <HomeScreen
        projects={presented.map((entry) => entry.project)}
        threads={presented.map((entry) => entry.thread)}
        pendingTasks={[]}
        catalogState={catalogState}
        savedConnectionsById={{}}
        environments={environments}
        searchQuery=""
        selectedEnvironmentId={null}
        selectedProjectKey={null}
        projectSortOrder="updated_at"
        threadSortOrder="updated_at"
        projectGroupingMode="repository"
        projectGroupKeyByProjectRef={projectGroupKeyByProjectRef}
        onSearchQueryChange={noOp}
        onEnvironmentChange={noOp}
        onProjectChange={noOp}
        onProjectSortOrderChange={noOp}
        onThreadSortOrderChange={noOp}
        onAddConnection={() => props.navigation.navigate("Pair")}
        onOpenSettings={noOp}
        onStartNewTask={() => props.navigation.navigate("Pair")}
        onSelectThread={(thread: EnvironmentThreadShell) => {
          const pairingId = pairingIdForThread(thread);
          if (pairingId) {
            openForgeHomeThread(pairingId, reconnect, (selectedPairingId) =>
              props.navigation.navigate("Thread", { pairingId: selectedPairingId }),
            );
          }
        }}
        onArchiveThread={(thread) => {
          const pairingId = pairingIdForThread(thread);
          if (pairingId) void archiveSession(pairingId);
        }}
        onDeleteThread={noOp}
        onSettleThread={noOpAsync}
        onSnoozeThread={noOpAsync}
        onUnsnoozeThread={noOpAsync}
        onUnsettleThread={noOp}
        onPinThread={(thread) => {
          const pairingId = pairingIdForThread(thread);
          return pairingId ? pinSession(pairingId) : Promise.resolve(false);
        }}
        onUnpinThread={(thread) => {
          const pairingId = pairingIdForThread(thread);
          return pairingId ? unpinSession(pairingId) : Promise.resolve(false);
        }}
        onMovePinnedThread={noOpAsync}
        onRegenerateThreadTitle={noOpAsync}
        {...(Platform.OS === "ios" ? { onRenameThread: renameThread } : {})}
        onSelectPendingTask={noOp}
        onDeletePendingTask={noOp}
        onNewThreadInProject={requestNewSessionInProject}
        isNewThreadInProjectPending={isNewThreadInProjectPending}
        isNewThreadInProjectSupported={isNewThreadInProjectSupported}
        remoteOnly
      />
    </View>
  );
}

function questionResponse(
  questions: ReadonlyArray<RemoteQuestion>,
  answers: Record<string, string | ReadonlyArray<string>>,
) {
  return questions.map((question, questionIndex) => {
    const answer = answers[`question-${questionIndex}`];
    const selected = Array.isArray(answer) ? answer : [answer];
    const optionIndices = selected.flatMap((label) => {
      const index = question.options.findIndex((option) => option.label === label);
      return index >= 0 ? [index] : [];
    });
    const freeform = typeof answer === "string" && optionIndices.length === 0 ? answer : undefined;
    return { questionIndex, optionIndices, ...(freeform ? { freeform } : {}) };
  });
}

function ForgeThreadScreen(props: NativeStackScreenProps<ForgeStackParams, "Thread">) {
  const api = useForgeSessions();
  const session = api.sessions.find((entry) => entry.pairing.id === props.route.params.pairingId);
  const [draft, setDraft] = useState("");
  const [draftAttachments, setDraftAttachments] = useState<
    ReadonlyArray<DraftComposerImageAttachment>
  >([]);
  const [newSessionFinalizing, setNewSessionFinalizing] = useState(false);
  const [userInputDrafts, setUserInputDrafts] = useState<
    Record<string, PendingUserInputDraftAnswer>
  >({});
  const presentation = session ? presentRemoteSession(session) : null;
  const pairingId = props.route.params.pairingId;
  const reconnect = api.reconnect;
  const releaseActiveSession = api.releaseActiveSession;
  useFocusEffect(
    useCallback(() => {
      reconnect(pairingId);
      return () => releaseActiveSession(pairingId);
    }, [pairingId, reconnect, releaseActiveSession]),
  );
  const pendingInteractions = (session?.snapshot?.activeInteractions ?? []).filter(
    (entry) => entry.status === undefined || entry.status === "pending",
  );
  const interactionPresentations = pendingInteractions
    .map(presentInteraction)
    .filter((entry) => entry !== null);
  const approvalPresentations = interactionPresentations.filter((entry) => entry.approval !== null);
  const userInputPresentation = interactionPresentations.find((entry) => entry.userInput !== null);
  const userInputInteraction = userInputPresentation?.interaction;
  const answers = userInputPresentation?.userInput
    ? buildPendingUserInputAnswers(userInputPresentation.userInput.questions, userInputDrafts)
    : null;
  const remoteQueuedMessages = useMemo(
    () =>
      presentRemoteQueuedMessages(
        session?.snapshot?.queue ?? [],
        new Set(session?.pendingQueueItemIds ?? []),
      ),
    [session?.pendingQueueItemIds, session?.snapshot?.queue],
  );
  const effectiveTitle = presentation?.thread.title ?? "Forge session";
  const renameCurrentSession = useCallback(() => {
    if (!session) return;
    promptForSessionRename(effectiveTitle, (title) => {
      void api.renameSession(session.pairing.id, title);
    });
  }, [api, effectiveTitle, session]);
  const toggleCurrentSessionPin = useCallback(() => {
    if (!session) return;
    const mutation =
      session.pairing.metadata.pinnedAt === undefined
        ? api.pinSession(session.pairing.id)
        : api.unpinSession(session.pairing.id);
    void mutation;
  }, [api, session]);
  const archiveCurrentSession = useCallback(() => {
    if (!session) return;
    void api.archiveSession(session.pairing.id).then((archived) => {
      if (archived) props.navigation.popTo("Home");
    });
  }, [api, props.navigation, session]);
  const newSessionAction = useMemo(
    () =>
      remoteNewSessionHeaderPresentation({
        supported: session?.snapshot?.capabilities.newSession === true,
        pending: session?.newSessionCommandPending === true || newSessionFinalizing,
        hasExecutableHandler: session !== undefined,
      }),
    [
      newSessionFinalizing,
      session,
      session?.newSessionCommandPending,
      session?.snapshot?.capabilities.newSession,
    ],
  );
  const createNewSession = useCallback(() => {
    if (!session || newSessionFinalizing) return;
    setNewSessionFinalizing(true);
    const created = api.newSession(session.pairing.id);
    if (!created) {
      setNewSessionFinalizing(false);
      return;
    }
    void created
      .then(async (outcome) => {
        const registeredPairingId = await api.registerPairing(
          remoteSessionCreatedRegistrationInput(outcome),
          outcome.sessionId,
        );
        const accepted = await api.completeNewSession(
          session.pairing.id,
          registeredPairingId,
          outcome.sessionId,
        );
        if (!accepted) return;
        const target = remoteNewSessionNavigationTarget(outcome, registeredPairingId);
        props.navigation.reset({
          index: 1,
          routes: [{ name: "Home" }, { name: "Thread", params: { pairingId: target.pairingId } }],
        });
      })
      .catch(() => undefined)
      .finally(() => setNewSessionFinalizing(false));
  }, [api, newSessionFinalizing, props.navigation, session]);
  const editQueuedMessage = useCallback(
    (message: RemoteQueuedMessagePresentation) => {
      if (Platform.OS !== "ios" || !session || message.expectedVersion === null) return;
      const expectedVersion = message.expectedVersion;
      Alert.prompt(
        "Edit message",
        undefined,
        [
          { text: "Cancel", style: "cancel" },
          {
            text: "Save",
            onPress: (value?: string) => {
              const text = value?.trim() ?? "";
              if (!text || text === message.text.trim()) return;
              void api.editQueuedPrompt(
                session.pairing.id,
                message.queueItemId,
                expectedVersion,
                text,
              );
            },
          },
        ],
        "plain-text",
        message.text,
      );
    },
    [api, session],
  );
  const steerQueuedMessage = useCallback(
    (message: RemoteQueuedMessagePresentation) => {
      if (!session || message.expectedVersion === null) return;
      void api.steerQueuedPrompt(session.pairing.id, message.queueItemId, message.expectedVersion);
    },
    [api, session],
  );
  const cancelQueuedMessage = useCallback(
    (message: RemoteQueuedMessagePresentation) => {
      if (!session || message.expectedVersion === null) return;
      void api.cancelQueuedPrompt(session.pairing.id, message.queueItemId, message.expectedVersion);
    },
    [api, session],
  );

  useEffect(() => {
    const headerActions = {
      pinned: session?.pairing.metadata.pinnedAt !== undefined,
      title: effectiveTitle,
      newSessionAction,
      onCreateNewSession: createNewSession,
      onArchive: archiveCurrentSession,
      onPinChange: toggleCurrentSessionPin,
      onRename: renameCurrentSession,
    };
    props.navigation.setOptions({
      headerBlurEffect: undefined,
      headerRight:
        session && Platform.OS !== "ios"
          ? () => <ForgeThreadHeaderActions {...headerActions} />
          : undefined,
      headerShadowVisible: false,
      headerStyle: { backgroundColor: FORGE_HOME_HEADER.backgroundColor },
      headerTitle: () => (
        <ForgeSessionHeaderTitle
          phase={session?.connectionPhase ?? "reconnecting"}
          title={effectiveTitle}
        />
      ),
      headerTintColor: "#FFFFFF",
      headerTransparent: false,
      statusBarStyle: "light",
      unstable_headerRightItems:
        session && Platform.OS === "ios"
          ? () => forgeThreadNativeHeaderItems(headerActions)
          : undefined,
    });
  }, [
    archiveCurrentSession,
    createNewSession,
    effectiveTitle,
    newSessionAction,
    props.navigation,
    renameCurrentSession,
    session?.connectionPhase,
    session?.pairing.metadata.pinnedAt,
    session?.newSessionCommandPending,
    session?.snapshot?.capabilities.newSession,
    session?.snapshot?.title,
    toggleCurrentSessionPin,
  ]);

  if (!session || !presentation) {
    return (
      <View className="flex-1 items-center justify-center bg-screen px-6">
        <Text className="text-center text-base text-muted-foreground">
          This pairing is no longer available.
        </Text>
      </View>
    );
  }

  const submitUserInput = async () => {
    if (userInputInteraction?.kind !== "question" || !answers) return null;
    return api.resolveInteraction(session.pairing.id, userInputInteraction.interactionId, {
      kind: "question",
      answers: questionResponse(userInputInteraction.questions, answers),
    });
  };
  const respondToRemoteApproval = async (
    requestId: ApprovalRequestId,
    actionId: string,
    feedback?: string,
  ) => {
    const interaction = pendingInteractions.find(
      (candidate) => candidate.interactionId === String(requestId),
    );
    if (!interaction) return null;
    const response = remoteApprovalResponse(interaction, actionId, feedback);
    return response
      ? api.resolveInteraction(session.pairing.id, interaction.interactionId, response)
      : null;
  };
  const respondingApprovalIds = approvalPresentations.flatMap((entry) =>
    session.pendingInteractionIds.includes(entry.interaction.interactionId)
      ? [ApprovalRequestId.make(entry.interaction.interactionId)]
      : [],
  );
  const respondingUserInputId =
    userInputInteraction &&
    session.pendingInteractionIds.includes(userInputInteraction.interactionId)
      ? ApprovalRequestId.make(userInputInteraction.interactionId)
      : null;

  const connectionState = connectionPhase(session.connectionPhase);
  const openUsage = () => {
    if (session.snapshot?.capabilities.usage === true) {
      void api.refreshUsage(session.pairing.id);
    }
  };
  const pickDraftImages = async () => {
    const result = await pickComposerImages({ existingCount: draftAttachments.length });
    if (result.error) Alert.alert("Attachments", result.error);
    if (result.images.length > 0) {
      setDraftAttachments((current) => [...current, ...result.images]);
    }
  };
  const pickDraftFiles = async () => {
    const result = await pickComposerImageFiles({ existingCount: draftAttachments.length });
    if (result.error) {
      Alert.alert("Attachments", result.error);
    }
    if (result.images.length > 0) {
      setDraftAttachments((current) => [...current, ...result.images]);
    }
  };
  return (
    <View className="flex-1 bg-screen">
      <ThreadDetailScreen
        selectedThread={presentation.thread}
        contentPresentation={session.snapshot ? { kind: "ready" } : { kind: "loading" }}
        screenTone={connectionTone(connectionState)}
        connectionError={null}
        environmentLabel={session.pairing.host}
        selectedThreadFeed={presentation.feed}
        remoteAssistantResponseMessageIds={presentation.assistantResponseMessageIds}
        remoteWorkDisclosures={presentation.workDisclosures}
        remoteQueuedMessages={remoteQueuedMessages}
        {...(session.snapshot?.capabilities.queueControl === true
          ? {
              ...(Platform.OS === "ios" ? { onEditRemoteQueuedMessage: editQueuedMessage } : {}),
              onSteerRemoteQueuedMessage: steerQueuedMessage,
              onCancelRemoteQueuedMessage: cancelQueuedMessage,
            }
          : {})}
        activeWorkStartedAt={presentation.activeWorkStartedAt}
        activePendingApproval={approvalPresentations[0]?.approval ?? null}
        remotePendingApprovals={approvalPresentations.flatMap((entry) =>
          entry.approval ? [entry.approval] : [],
        )}
        respondingApprovalId={null}
        remoteRespondingApprovalIds={respondingApprovalIds}
        activePendingUserInput={userInputPresentation?.userInput ?? null}
        activePendingUserInputDrafts={userInputDrafts}
        activePendingUserInputAnswers={answers}
        respondingUserInputId={respondingUserInputId}
        draftMessage={draft}
        draftAttachments={draftAttachments}
        connectionStateLabel={connectionState}
        activeThreadBusy={session.snapshot?.status === "running"}
        environmentId={presentation.environmentId}
        projectWorkspaceRoot={session.snapshot?.cwd ?? null}
        threadCwd={session.snapshot?.cwd ?? null}
        selectedThreadQueueCount={session.snapshot?.queue?.length ?? 0}
        serverConfig={presentation.serverConfig}
        usesAutomaticContentInsets
        onOpenConnectionEditor={() => props.navigation.navigate("Pair")}
        onChangeDraftMessage={setDraft}
        onPickDraftImages={pickDraftImages}
        onPickDraftFiles={pickDraftFiles}
        onNativePasteImages={async (uris) => {
          const converted = await convertPastedImagesToAttachments({
            uris,
            existingCount: draftAttachments.length,
          });
          if (converted.length > 0) {
            setDraftAttachments((current) => [...current, ...converted]);
          }
        }}
        onRemoveDraftImage={(id) =>
          setDraftAttachments((current) => current.filter((attachment) => attachment.id !== id))
        }
        onStopThread={() => void api.cancel(session.pairing.id)}
        onSendMessage={async () => {
          const command = parseForgeComposerCommand(draft);
          if (!command || command.type === "invalidBtw") return null;
          if (command.type === "usage") {
            setDraft("");
            openUsage();
            return null;
          }
          if (command.type === "btw" && session.snapshot?.capabilities.btw !== true) return null;
          if (command.type === "prompt" && session.snapshot?.capabilities.prompt !== true)
            return null;
          const images = draftAttachments.flatMap((attachment) => {
            const data = attachment.dataUrl.includes(",")
              ? (attachment.dataUrl.split(",")[1] ?? "")
              : attachment.dataUrl;
            return data
              ? [
                  {
                    name: attachment.name,
                    mimeType: attachment.mimeType,
                    data,
                  },
                ]
              : [];
          });
          const commandId =
            command.type === "btw"
              ? api.askBtw(session.pairing.id, command.question)
              : api.sendPrompt(session.pairing.id, command.text, images);
          if (!commandId) return null;
          setDraft("");
          setDraftAttachments([]);
          return MessageId.make(commandId);
        }}
        onReconnectEnvironment={() => api.reconnect(session.pairing.id)}
        onUpdateThreadModelSelection={(selection: ModelSelection) => {
          if (
            session.snapshot?.capabilities.setModel !== true ||
            session.snapshot.modelSwitchPending === true ||
            session.modelCommandPending
          ) {
            return;
          }
          void api.setModel(session.pairing.id, selection.model, reasoningFromSelection(selection));
        }}
        onSetRemoteFastMode={(enabled) => {
          if (
            session.snapshot?.capabilities.fastMode !== true ||
            session.snapshot.fastMode?.supported !== true ||
            session.snapshot.fastMode.pending === true ||
            session.fastModeCommandPending ||
            session.snapshot.modelSwitchPending === true ||
            session.modelCommandPending
          ) {
            return;
          }
          void api.setFastMode(session.pairing.id, enabled);
        }}
        onUpdateThreadRuntimeMode={() => undefined}
        onUpdateThreadInteractionMode={() => undefined}
        onRespondToApproval={async () => null}
        onRespondToRemoteApproval={respondToRemoteApproval}
        onSelectUserInputOption={(
          _requestId: ApprovalRequestId,
          question: UserInputQuestion,
          label: string,
        ) =>
          setUserInputDrafts((current) => ({
            ...current,
            [question.id]: togglePendingUserInputOptionSelection(
              question,
              current[question.id],
              label,
            ),
          }))
        }
        onChangeUserInputCustomAnswer={(_requestId, questionId, value) =>
          setUserInputDrafts((current) => ({
            ...current,
            [questionId]: setPendingUserInputCustomAnswer(current[questionId], value),
          }))
        }
        onSubmitUserInput={submitUserInput}
        remoteOnly
        remoteCancellable={session.snapshot?.capabilities.cancel === true}
        remoteModelCommandPending={
          session.modelCommandPending ||
          session.fastModeCommandPending ||
          session.snapshot?.modelSwitchPending === true ||
          session.snapshot?.fastMode?.pending === true
        }
        remoteModelSelectionEnabled={session.snapshot?.capabilities.setModel === true}
        remoteFastMode={{
          supported:
            session.snapshot?.capabilities.fastMode === true &&
            session.snapshot.fastMode?.supported === true,
          enabled: session.snapshot?.fastMode?.enabled === true,
          pending:
            session.fastModeCommandPending ||
            session.modelCommandPending ||
            session.snapshot?.fastMode?.pending === true ||
            session.snapshot?.modelSwitchPending === true,
        }}
        remoteUsageAvailable={session.snapshot?.capabilities.usage === true}
        remoteUsageLabel={compactUsageLimitLabel(session.snapshot?.usage)}
        onOpenUsage={openUsage}
      />
    </View>
  );
}

function ForgeUsageRouteScreen(props: NativeStackScreenProps<ForgeStackParams, "Usage">) {
  const api = useForgeSessions();
  const session = api.sessions.find((entry) => entry.pairing.id === props.route.params.pairingId);
  const pairingId = props.route.params.pairingId;
  const reconnect = api.reconnect;
  const releaseActiveSession = api.releaseActiveSession;
  useFocusEffect(
    useCallback(() => {
      reconnect(pairingId);
      return () => releaseActiveSession(pairingId);
    }, [pairingId, reconnect, releaseActiveSession]),
  );
  return (
    <ForgeUsageScreen
      usage={session?.snapshot?.usage}
      refreshing={
        session?.usageCommandPending === true || session?.snapshot?.usage?.status === "loading"
      }
      canRefresh={session?.snapshot?.capabilities.usage === true}
      connectionError={session ? null : "This pairing is no longer available."}
      onRefresh={() => {
        if (session) void api.refreshUsage(session.pairing.id);
      }}
    />
  );
}

function PairScreen(props: NativeStackScreenProps<ForgeStackParams, "Pair">) {
  const { registerPairing } = useForgeSessions();
  const [permission, requestPermission] = useCameraPermissions();
  const [pairingPending, setPairingPending] = useState(false);
  const [pairingError, setPairingError] = useState<string | null>(null);
  const [scanArmed, setScanArmed] = useState(true);
  const [permissionPending, setPermissionPending] = useState(false);
  const [permissionError, setPermissionError] = useState<string | null>(null);
  const scanGate = useRef(createPairingScanGate()).current;
  const insets = useSafeAreaInsets();
  const screenColor = useThemeColor("--color-screen");
  const foregroundColor = useThemeColor("--color-foreground");
  const mutedForegroundColor = useThemeColor("--color-foreground-muted");
  const primaryColor = useThemeColor("--color-primary");
  const primaryForegroundColor = useThemeColor("--color-primary-foreground");
  const dangerForegroundColor = useThemeColor("--color-danger-foreground");

  const register = useCallback(
    (input: string) => {
      if (!scanGate.tryBegin()) return;
      setPairingPending(true);
      setPairingError(null);
      void registerPairing(input)
        .then((pairingId) => {
          props.navigation.reset({
            index: 1,
            routes: [{ name: "Home" }, { name: "Thread", params: { pairingId } }],
          });
        })
        .catch((error: unknown) => {
          const message =
            error instanceof Error ? error.message : "That QR code is not a Forge pairing.";
          scanGate.rearm();
          setPairingPending(false);
          setPairingError(message);
          Alert.alert("Could not pair", message);
        });
    },
    [props.navigation, registerPairing, scanGate],
  );
  const onBarcodeScanned = useCallback(
    (result: BarcodeScanningResult) => {
      setScanArmed(false);
      register(result.data);
    },
    [register],
  );
  const pastePairing = useCallback(() => {
    void Clipboard.getStringAsync()
      .then(register)
      .catch((error: unknown) => {
        const message =
          error instanceof Error ? error.message : "Forge could not read the clipboard.";
        setPairingError(message);
        Alert.alert("Could not paste pairing", message);
      });
  }, [register]);
  const requestOrOpenCameraSettings = useCallback(() => {
    if (permissionPending) return;
    setPermissionPending(true);
    setPermissionError(null);
    const permissionAction =
      permission?.canAskAgain === false ? Linking.openSettings() : requestPermission();
    void permissionAction
      .catch((error: unknown) => {
        setPermissionError(
          error instanceof Error ? error.message : "Forge could not open camera settings.",
        );
      })
      .finally(() => setPermissionPending(false));
  }, [permission?.canAskAgain, permissionPending, requestPermission]);

  if (!permission?.granted) {
    const cameraStatus = permission
      ? "Forge stores the private pairing only in this device's secure storage."
      : "Forge is checking camera access on this device.";
    const cameraAction = permission?.canAskAgain === false ? "Open Settings" : "Allow camera";
    return (
      <View
        className="flex-1 items-center justify-center gap-4 px-8"
        style={{ backgroundColor: screenColor }}
      >
        <Text className="text-center text-lg font-t3-bold" style={{ color: foregroundColor }}>
          Scan the QR shown by /rc
        </Text>
        <Text className="text-center text-sm" style={{ color: mutedForegroundColor }}>
          {cameraStatus}
        </Text>
        {permission ? (
          <Pressable
            accessibilityRole="button"
            className="rounded-full px-5 py-3"
            disabled={permissionPending}
            onPress={requestOrOpenCameraSettings}
            style={{ backgroundColor: primaryColor, opacity: permissionPending ? 0.6 : 1 }}
          >
            <Text className="font-t3-bold" style={{ color: primaryForegroundColor }}>
              {permissionPending ? "Opening…" : cameraAction}
            </Text>
          </Pressable>
        ) : null}
        {permissionError ? (
          <Text className="text-center text-sm" style={{ color: dangerForegroundColor }}>
            {permissionError}
          </Text>
        ) : null}
        <Pressable accessibilityRole="button" onPress={pastePairing}>
          <Text className="font-t3-bold" style={{ color: foregroundColor }}>
            Paste pairing instead
          </Text>
        </Pressable>
      </View>
    );
  }

  return (
    <View className="flex-1 bg-black">
      <CameraView
        barcodeScannerSettings={{ barcodeTypes: ["qr"] }}
        onBarcodeScanned={scanArmed && !pairingPending ? onBarcodeScanned : undefined}
        style={{ flex: 1 }}
      />
      <View
        className="absolute bottom-0 left-0 right-0 items-center gap-3 bg-black/65 px-8 pt-5"
        style={{ paddingBottom: Math.max(insets.bottom, 20) }}
      >
        <Text className="text-center text-base font-t3-bold text-white">
          {pairingPending ? "Pairing…" : "Scan the private Forge pairing"}
        </Text>
        {pairingError ? (
          <Text className="text-center text-sm text-[#fca5a5]">{pairingError}</Text>
        ) : null}
        {!scanArmed && !pairingPending ? (
          <Pressable
            accessibilityRole="button"
            onPress={() => {
              setPairingError(null);
              setScanArmed(true);
            }}
          >
            <Text className="font-t3-bold text-white">Scan another code</Text>
          </Pressable>
        ) : null}
        <Pressable accessibilityRole="button" onPress={pastePairing}>
          <Text className="font-t3-bold text-white">Paste pairing instead</Text>
        </Pressable>
      </View>
    </View>
  );
}

function ForgeNavigation() {
  const { isReady } = useAppearancePreferences();
  const { registerPairing } = useForgeSessions();
  const colorScheme = useColorScheme();
  const statusBarBg = useThemeColor("--color-status-bar");
  const pendingNavigation = useRef<{ pairingId: string | null }>({ pairingId: null });
  const navigateToPairing = useCallback((pairingId: string) => {
    navigationRef.navigate("Thread", { pairingId });
  }, []);

  useEffect(() => {
    if (isReady) void SplashScreen.hideAsync();
  }, [isReady]);
  useEffect(() => {
    const receive = (url: string | null) => {
      if (!url || !url.startsWith("forge://pair?")) return;
      void registerPairing(url)
        .then((pairingId) => {
          openOrQueuePairing(
            pendingNavigation.current,
            pairingId,
            () => navigationRef.isReady(),
            navigateToPairing,
          );
        })
        .catch(() =>
          Alert.alert("Could not pair", "That Forge pairing link is invalid or expired."),
        );
    };
    void Linking.getInitialURL().then(receive);
    const subscription = Linking.addEventListener("url", (event) => receive(event.url));
    return () => subscription.remove();
  }, [navigateToPairing, registerPairing]);

  return (
    <>
      <StatusBar
        barStyle={colorScheme === "dark" ? "light-content" : "dark-content"}
        backgroundColor={statusBarBg}
        translucent
      />
      <NavigationContainer
        ref={navigationRef}
        theme={colorScheme === "dark" ? DarkTheme : DefaultTheme}
        onReady={() => drainQueuedPairing(pendingNavigation.current, navigateToPairing)}
      >
        <Stack.Navigator
          screenOptions={{
            headerBackButtonDisplayMode: "minimal",
            headerBlurEffect:
              colorScheme === "dark" ? "systemChromeMaterialDark" : "systemChromeMaterialLight",
            headerTransparent: true,
          }}
        >
          <Stack.Screen
            name="Home"
            component={ForgeHomeScreen}
            options={{
              headerBlurEffect: undefined,
              headerShadowVisible: false,
              headerStyle: { backgroundColor: FORGE_HOME_HEADER.backgroundColor },
              headerTintColor: "#FFFFFF",
              headerTitle: () => <ForgeWordmark />,
              headerTransparent: false,
              statusBarStyle: "light",
            }}
          />
          <Stack.Screen
            name="Thread"
            component={ForgeThreadScreen}
            options={{
              headerBlurEffect: undefined,
              headerShadowVisible: false,
              headerStyle: { backgroundColor: FORGE_HOME_HEADER.backgroundColor },
              headerTintColor: "#FFFFFF",
              headerTransparent: false,
              statusBarStyle: "light",
              title: "Forge session",
            }}
          />
          <Stack.Screen
            name="Usage"
            component={ForgeUsageRouteScreen}
            options={{ title: "Usage" }}
          />
          <Stack.Screen
            name="Pair"
            component={PairScreen}
            options={{ title: "Add session", presentation: "modal" }}
          />
        </Stack.Navigator>
      </NavigationContainer>
    </>
  );
}

export default function ForgeApp() {
  return (
    <RegistryContext.Provider value={appAtomRegistry}>
      <AppearancePreferencesProvider>
        <GestureHandlerRootView className="flex-1">
          <KeyboardProvider statusBarTranslucent>
            <SafeAreaProvider>
              <ForgeSessionsProvider>
                <ForgeNavigation />
              </ForgeSessionsProvider>
            </SafeAreaProvider>
          </KeyboardProvider>
        </GestureHandlerRootView>
      </AppearancePreferencesProvider>
    </RegistryContext.Provider>
  );
}
