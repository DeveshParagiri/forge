import { RegistryContext } from "@effect/atom-react";
import type { EnvironmentConnectionPhase } from "@t3tools/client-runtime/connection";
import type { EnvironmentThreadShell } from "@t3tools/client-runtime/state/shell";
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
} from "@react-navigation/native";
import {
  createNativeStackNavigator,
  type NativeStackScreenProps,
} from "@react-navigation/native-stack";
import { CameraView, useCameraPermissions, type BarcodeScanningResult } from "expo-camera";
import * as Clipboard from "expo-clipboard";
import { Image } from "expo-image";
import * as Linking from "expo-linking";
import * as SplashScreen from "expo-splash-screen";
import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { Alert, Keyboard, Pressable, StatusBar, useColorScheme, View } from "react-native";
import { GestureHandlerRootView } from "react-native-gesture-handler";
import { KeyboardProvider } from "react-native-keyboard-controller";
import { SafeAreaProvider, useSafeAreaInsets } from "react-native-safe-area-context";

import { AppText as Text } from "../components/AppText";
import { HomeScreen } from "../features/home/HomeScreen";
import {
  AppearancePreferencesProvider,
  useAppearancePreferences,
} from "../features/settings/appearance/AppearancePreferencesProvider";
import { ThreadDetailScreen } from "../features/threads/ThreadDetailScreen";
import { connectionTone } from "../features/connection/connectionTone";
import {
  buildPendingUserInputAnswers,
  setPendingUserInputCustomAnswer,
  togglePendingUserInputOptionSelection,
  type PendingUserInputDraftAnswer,
} from "../lib/threadActivity";
import { useThemeColor } from "../lib/useThemeColor";
import { appAtomRegistry } from "../state/atom-registry";
import type { WorkspaceState } from "../state/workspaceModel";
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
import { forgeConnectionStatusDot } from "./connectionStatusDot";
import { forgeHeaderPairTextColor } from "./chromePresentation";
import { openForgeHomeThread } from "./homeThreadNavigation";

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
  return (
    <View
      accessible={false}
      style={{ backgroundColor: presentation.color, borderRadius: 4, height: 8, width: 8 }}
    />
  );
}

function ForgeWordmark(props: {
  readonly phase: "connecting" | "connected" | "reconnecting" | "error";
}) {
  return (
    <View className="flex-row items-center gap-2">
      <Image source={require("../../assets/forge/icon.png")} style={{ height: 24, width: 24 }} />
      <Text className="font-t3-bold text-[17px] tracking-tight">Forge</Text>
      <ForgeConnectionDot phase={props.phase} />
    </View>
  );
}

function ForgeSessionHeaderTitle(props: {
  readonly phase: "connecting" | "connected" | "reconnecting" | "error";
  readonly title: string;
}) {
  return (
    <View className="max-w-[230px] flex-row items-center gap-2">
      <Text className="shrink font-t3-bold text-[17px]" numberOfLines={1}>
        {props.title}
      </Text>
      <ForgeConnectionDot phase={props.phase} />
    </View>
  );
}

function HeaderPairButton(props: { readonly onPress: () => void }) {
  const colorScheme = useColorScheme();
  return (
    <Pressable
      accessibilityLabel="Pair Forge session"
      accessibilityRole="button"
      className="rounded-full px-2.5 py-1.5 active:opacity-60"
      onPress={props.onPress}
    >
      <Text
        className="font-t3-bold text-sm"
        style={{ color: forgeHeaderPairTextColor(colorScheme) }}
      >
        Pair
      </Text>
    </Pressable>
  );
}

function connectionPhase(
  phase: "connecting" | "connected" | "reconnecting" | "error",
): EnvironmentConnectionPhase {
  return phase;
}

function ForgeHomeScreen(props: NativeStackScreenProps<ForgeStackParams, "Home">) {
  const { ready, reconnect, sessions } = useForgeSessions();
  const presented = useMemo(() => sessions.map(presentRemoteSession), [sessions]);
  const pairingIdByThread = useMemo(
    () => new Map(presented.map((entry, index) => [entry.thread.id, sessions[index]?.pairing.id])),
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
  const headerPhase =
    sessions.length > 0 && sessions.every((session) => session.connectionPhase === "connected")
      ? "connected"
      : "reconnecting";

  useEffect(() => {
    props.navigation.setOptions({
      headerTitle: () => <ForgeWordmark phase={headerPhase} />,
      headerRight: () => <HeaderPairButton onPress={() => props.navigation.navigate("Pair")} />,
    });
  }, [headerPhase, props.navigation]);

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
        onSearchQueryChange={noOp}
        onEnvironmentChange={noOp}
        onProjectChange={noOp}
        onProjectSortOrderChange={noOp}
        onThreadSortOrderChange={noOp}
        onAddConnection={() => props.navigation.navigate("Pair")}
        onOpenSettings={noOp}
        onStartNewTask={() => props.navigation.navigate("Pair")}
        onSelectThread={(thread: EnvironmentThreadShell) => {
          const pairingId = pairingIdByThread.get(thread.id);
          if (pairingId) {
            openForgeHomeThread(pairingId, reconnect, (selectedPairingId) =>
              props.navigation.navigate("Thread", { pairingId: selectedPairingId }),
            );
          }
        }}
        onArchiveThread={noOp}
        onDeleteThread={noOp}
        onSettleThread={noOpAsync}
        onSnoozeThread={noOpAsync}
        onUnsnoozeThread={noOpAsync}
        onUnsettleThread={noOp}
        onPinThread={noOpAsync}
        onUnpinThread={noOpAsync}
        onMovePinnedThread={noOpAsync}
        onRegenerateThreadTitle={noOpAsync}
        onSelectPendingTask={noOp}
        onDeletePendingTask={noOp}
        onNewThreadInProject={() => props.navigation.navigate("Pair")}
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
  const [userInputDrafts, setUserInputDrafts] = useState<
    Record<string, PendingUserInputDraftAnswer>
  >({});
  const presentation = session ? presentRemoteSession(session) : null;
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

  useEffect(() => {
    const title = session?.snapshot?.title ?? session?.pairing.metadata.title ?? "Forge session";
    props.navigation.setOptions({
      headerTitle: () => (
        <ForgeSessionHeaderTitle phase={session?.connectionPhase ?? "reconnecting"} title={title} />
      ),
    });
  }, [
    props.navigation,
    session?.connectionPhase,
    session?.pairing.metadata.title,
    session?.snapshot?.title,
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
    Keyboard.dismiss();
    if (session.snapshot?.capabilities.usage === true) {
      void api.refreshUsage(session.pairing.id);
    }
    props.navigation.navigate("Usage", { pairingId: session.pairing.id });
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
        draftAttachments={[]}
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
        onPickDraftImages={async () => undefined}
        onNativePasteImages={async () => undefined}
        onRemoveDraftImage={() => undefined}
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
          const commandId =
            command.type === "btw"
              ? api.askBtw(session.pairing.id, command.question)
              : api.sendPrompt(session.pairing.id, command.text);
          if (!commandId) return null;
          setDraft("");
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
          session.modelCommandPending || session.snapshot?.modelSwitchPending === true
        }
        remoteModelSelectionEnabled={session.snapshot?.capabilities.setModel === true}
        remoteUsageAvailable={session.snapshot?.capabilities.usage === true}
        onOpenUsage={openUsage}
      />
    </View>
  );
}

function ForgeUsageRouteScreen(props: NativeStackScreenProps<ForgeStackParams, "Usage">) {
  const api = useForgeSessions();
  const session = api.sessions.find((entry) => entry.pairing.id === props.route.params.pairingId);
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
          props.navigation.replace("Thread", { pairingId });
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
            options={{ headerTitle: () => <ForgeWordmark phase="reconnecting" /> }}
          />
          <Stack.Screen
            name="Thread"
            component={ForgeThreadScreen}
            options={{ title: "Forge session" }}
          />
          <Stack.Screen
            name="Usage"
            component={ForgeUsageRouteScreen}
            options={{ title: "Usage" }}
          />
          <Stack.Screen
            name="Pair"
            component={PairScreen}
            options={{ title: "Pair session", presentation: "modal" }}
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
