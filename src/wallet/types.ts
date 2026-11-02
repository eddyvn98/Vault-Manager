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

export type UtxoInfo = {
  txid: string;
  vout: number;
  valueSatoshis: number;
  address: string;
};

export type BtcTxPreview = {
  draftId: string;
  network: string;
  recipientAddress: string;
  amountSatoshis: number;
  formattedAmountBtc: string;
  feeSatoshis: number;
  formattedFeeBtc: string;
  feeRateSatPerVb: number;
  selectedUtxos: UtxoInfo[];
  changeAddress: string;
  changeSatoshis: number;
  unsignedPsbtBase64: string;
  signingEnabled: boolean;
  note: string;
};

export type BscTxPreview = {
  draftId: string;
  chainId: number;
  networkName: string;
  isTokenTransfer: boolean;
  tokenSymbol: string;
  tokenContract?: string;
  fromAddress: string;
  recipientAddress: string;
  amountRaw: string;
  formattedAmount: string;
  nonce: number;
  gasLimit: number;
  gasPriceGwei: string;
  estimatedFeeBnb: string;
  calldataHex: string;
  unsignedTxSummary: string;
  signingEnabled: boolean;
  note: string;
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
