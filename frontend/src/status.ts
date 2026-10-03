export type Status = {
  exa_configured: boolean;
  auth_required: boolean;
  management_auth_required: boolean;
  default_provider: string;
  requests: number;
  fallbacks: number;
  providers: {
    id: string;
    model: string;
    configured: boolean;
    key_count: number;
    keys: { id: string; label: string; retry_after_seconds?: number | null }[];
  }[];
};
