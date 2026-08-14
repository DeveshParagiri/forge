declare module "lucide-react/dist/esm/icons/*.js" {
  import type { LucideIcon } from "lucide-react";

  const icon: LucideIcon;
  export default icon;
}

declare module "*.png" {
  const url: string;
  export default url;
}
