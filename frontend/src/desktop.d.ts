interface Window {
  freeRouterDesktop?: {
    platform: string;
    chooseDirectory: (cwd: string) => Promise<{ cancelled: boolean; cwd: string | null }>;
  };
}
