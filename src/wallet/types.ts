export type WalletStatus = {
  exists: boolean;
  unlocked: boolean;
  signingEnabled: boolean;
  network: string;
  failedAttempts: number;
  lockoutSeconds: number;
};

export type PublicAddresses = {
  btcMainnet: string;
  btcSignet: string;
  bscAddress: string;
  usdtBep20Contract: string;
};

export type Action = 'create' | 'restore' | 'unlock';

export const offlineStatus: WalletStatus = {
  exists: false,
  unlocked: false,
  signingEnabled: false,
  network: 'offline-only',
  failedAttempts: 0,
  lockoutSeconds: 0,
};

export function getErrorMessage(error: unknown): string {
  if (typeof error === 'string') return error;
  if (error instanceof Error) return error.message;
  return 'An unexpected error occurred';
}
