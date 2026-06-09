/**
 * @license
 * SPDX-License-Identifier: Apache-2.0
 */

export interface PasswordHistory {
  id: string;
  passwordChangedAt: string; // ISO string
  value: string;
}

export interface Account {
  id: string;
  alias: string;
  appName: string;
  username: string;
  passwordDecrypted: string;
  alternativePasswords?: string[];
  otpSecret?: string; // used for generating sham 6-digit 2FA OTP codes
  notes?: string;
  category: string; // 'Email' | 'Mạng xã hội' | 'Ngân hàng' | 'Công việc' | 'Giải trí' | 'Game' | 'Khác'
  isFavorite: boolean;
  isDeleted: boolean;
  history: PasswordHistory[];
  createdAt: string;
  updatedAt: string;
}

export interface SyncDevice {
  id: string;
  name: string; // e.g. "Windows 11 PC - Chrome"
  browser: 'Chrome' | 'Edge' | 'Firefox';
  lastSyncedAt: string;
  status: 'active' | 'inactive';
}

export interface SyncLog {
  id: string;
  timestamp: string;
  deviceName: string;
  type: 'ADDED' | 'UPDATED' | 'SCAN_COMPLETED';
  message: string;
}

export interface TelegramMessage {
  id: string;
  sender: 'user' | 'bot';
  text: string;
  timestamp: string;
  buttons?: { text: string; payload: string }[];
  isMaskedPassword?: boolean;
  maskedPasswordId?: string;
  secondsLeft?: number; // for self-destruction
}

export interface TelegramSession {
  authorizedUntil: string | null; // ISO string or null
  lastVerifiedPinAt: string | null;
  currentStep: 'idle' | 'waiting_for_account_selection' | 'waiting_for_pin' | 'showing_password';
  selectedAccountId?: string;
}
