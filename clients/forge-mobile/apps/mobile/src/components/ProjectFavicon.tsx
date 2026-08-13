import type { EnvironmentId } from "@t3tools/contracts";
import { View } from "react-native";

import { useThemeColor } from "../lib/useThemeColor";
import { SymbolView } from "./AppSymbol";

/**
 * Retained T3 row boundary with remote asset loading deliberately disabled.
 * Forge pairing snapshots do not expose favicon files or an asset endpoint.
 */
export function ProjectFavicon(props: {
  readonly environmentId: EnvironmentId;
  readonly open?: boolean;
  readonly size?: number;
  readonly projectTitle: string;
  readonly workspaceRoot?: string | null;
  readonly faviconPath?: string | null;
}) {
  const size = props.size ?? 42;
  const iconMuted = useThemeColor("--color-icon-subtle");
  return (
    <View style={{ width: size, height: size, alignItems: "center", justifyContent: "center" }}>
      <SymbolView
        name={{ ios: "folder.fill", android: props.open ? "folder_open" : "folder" }}
        size={size * 0.78}
        tintColor={iconMuted}
        type="monochrome"
      />
    </View>
  );
}
