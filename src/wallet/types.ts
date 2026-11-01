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

export type BtcBalance = {
  address: string;
  derivationPath: string;
  isFirstReceiveAddress: boolean;
  confirmedSatoshis: number;
  unconfirmedSatoshis: number;
  totalSatoshis: number;
  formattedBtc: string;
};

export type TokenBalance = {
  symbol: string;
  name: string;
  contractAddress?: string;
  rawHex: string;
  formatted: string;
  decimals: number;
  available: boolean;
  note?: string;
};

export type BscBalance = {
  address: string;
  derivationPath: string;
  chainId: number;
  networkName: string;
  bnbRawWei: string;
  formattedBnb: string;
  usdt: TokenBalance;
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
