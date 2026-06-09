/**
 * @license
 * SPDX-License-Identifier: Apache-2.0
 */

import { Account, SyncDevice, SyncLog } from '../types';

// Generate some vintage dates for history
const dateNow = new Date();
const hoursAgo = (h: number) => new Date(dateNow.getTime() - h * 60 * 60 * 1000).toISOString();
const daysAgo = (d: number) => new Date(dateNow.getTime() - d * 24 * 60 * 60 * 1000).toISOString();

export const DEFAULT_ACCOUNTS: Account[] = [
  {
    id: 'gg-main',
    alias: 'gg-main',
    appName: 'accounts.google.com',
    username: 'hathanhtu711@gmail.com',
    passwordDecrypted: 'aB7!kL9#XQ@pR',
    alternativePasswords: ['backupGPass_Xyz9', 'googleLegacyBackup2025'],
    otpSecret: 'JBSWY3DPEHPK3PXP', // For mock OTP generation
    notes: 'Tài khoản Google chính dùng cho YouTube và Gmail cá nhân.',
    category: 'Email',
    isFavorite: true,
    isDeleted: false,
    history: [
      { id: 'h1', passwordChangedAt: '2024-05-10T14:30:00Z', value: 'oldGoogleP@ss1' },
      { id: 'h2', passwordChangedAt: '2024-03-01T09:15:00Z', value: 'anotherOldP@ss2' },
      { id: 'h3', passwordChangedAt: '2023-12-12T18:22:00Z', value: 'verySecretGoogle99' },
    ],
    createdAt: '2023-12-12T18:22:00Z',
    updatedAt: hoursAgo(2),
  },
  {
    id: 'gg-work',
    alias: 'gg-work',
    appName: 'accounts.google.com',
    username: 'hathanhtu.work@gmail.com',
    passwordDecrypted: 'WorkStrongPass#2026',
    otpSecret: 'KREWY3DPEHPK3PXP',
    notes: 'Tài khoản Google dùng làm việc chính tại cơ quan.',
    category: 'Công việc',
    isFavorite: false,
    isDeleted: false,
    history: [
      { id: 'hw1', passwordChangedAt: '2025-01-15T08:00:00Z', value: 'workLegacyOldPass' }
    ],
    createdAt: '2024-06-20T09:00:00Z',
    updatedAt: daysAgo(1),
  },
  {
    id: 'fb-main',
    alias: 'fb-main',
    appName: 'facebook.com',
    username: 'hathanhtu',
    passwordDecrypted: 'fB_secure_711_Pass',
    alternativePasswords: ['fbOldPass_2024', 'fbWorkplaceAlternative'],
    otpSecret: 'MFRGGZDFMZTWQ2RE',
    notes: 'Tài khoản Facebook chính chủ kết nối trang cá nhân.',
    category: 'Mạng xã hội',
    isFavorite: true,
    isDeleted: false,
    history: [],
    createdAt: '2024-01-10T11:20:00Z',
    updatedAt: hoursAgo(3),
  },
  {
    id: 'zalo',
    alias: 'zalo',
    appName: 'id.zalo.me',
    username: '0987654321',
    passwordDecrypted: 'zalo_chat_private_99X',
    otpSecret: '',
    notes: 'Tài khoản Zalo chính để liên lạc gia đình và bạn bè.',
    category: 'Mạng xã hội',
    isFavorite: false,
    isDeleted: false,
    history: [],
    createdAt: '2024-02-15T10:00:00Z',
    updatedAt: hoursAgo(5),
  },
  {
    id: 'ig',
    alias: 'ig',
    appName: 'instagram.com',
    username: 'hathanhtu.711',
    passwordDecrypted: 'instaGram_photo_lover',
    otpSecret: 'OBXXK433NFSW4',
    notes: 'Instagram đăng tải ảnh thường ngày.',
    category: 'Mạng xã hội',
    isFavorite: false,
    isDeleted: false,
    history: [],
    createdAt: '2024-03-12T14:50:00Z',
    updatedAt: daysAgo(1),
  },
  {
    id: 'gmail-backup',
    alias: 'gmail-backup',
    appName: 'mail.google.com',
    username: 'backup711@gmail.com',
    passwordDecrypted: 'backupPassphrase711!!',
    otpSecret: '',
    notes: 'Email khôi phục thiết lập riêng bảo mật 3 lớp.',
    category: 'Email',
    isFavorite: false,
    isDeleted: false,
    history: [],
    createdAt: '2024-04-10T08:12:00Z',
    updatedAt: daysAgo(2),
  },
  {
    id: 'steam',
    alias: 'steam',
    appName: 'steamcommunity.com',
    username: 'hathanhtu711',
    passwordDecrypted: 'gAmEr_StEaM_hAtHaNhTu',
    otpSecret: 'UFTG6ZDFMZTWQ2RE',
    notes: 'Tài khoản chứa bộ sưu tập game bản quyền.',
    category: 'Game',
    isFavorite: false,
    isDeleted: false,
    history: [],
    createdAt: '2024-05-20T19:30:00Z',
    updatedAt: daysAgo(3),
  },
  {
    id: 'vcb',
    alias: 'vcb',
    appName: 'ib.vietcombank.com.vn',
    username: 'hathanhtu711',
    passwordDecrypted: 'vCb_banking_strongPass!9',
    otpSecret: 'VCB2FA433NFSW4PP',
    notes: 'Tài khoản ngân hàng Vietcombank sử dụng chuyển khoản thẻ chính.',
    category: 'Ngân hàng',
    isFavorite: true,
    isDeleted: false,
    history: [],
    createdAt: '2023-11-20T07:15:00Z',
    updatedAt: daysAgo(4),
  },
];

// Add supplementary items to match "49 accounts" in total
const APP_TEMPLATES = [
  { name: 'github.com', prefix: 'git', category: 'Công việc' },
  { name: 'netflix.com', prefix: 'netflix', category: 'Giải trí' },
  { name: 'spotify.com', prefix: 'spotify', category: 'Giải trí' },
  { name: 'shopee.vn', prefix: 'shopee', category: 'Khác' },
  { name: 'lazada.vn', prefix: 'lazada', category: 'Khác' },
  { name: 'tiktok.com', prefix: 'tiktok', category: 'Mạng xã hội' },
  { name: 'gitlab.com', prefix: 'gitlab', category: 'Công việc' },
  { name: 'binance.com', prefix: 'binance', category: 'Ngân hàng' },
  { name: 'chatgpt.com', prefix: 'openai', category: 'Công việc' },
  { name: 'trello.com', prefix: 'trello', category: 'Công việc' },
];

export const generateBackupAccounts = (): Account[] => {
  const result = [...DEFAULT_ACCOUNTS];
  let idCounter = 1;
  while (result.length < 49) {
    const template = APP_TEMPLATES[idCounter % APP_TEMPLATES.length];
    const sequence = Math.floor(idCounter / APP_TEMPLATES.length) + 1;
    const alias = `${template.prefix}-${sequence > 1 ? sequence : 'main'}`;
    const username = `hathanhtu.${template.prefix}${sequence}@gmail.com`;
    
    result.push({
      id: `generated-${idCounter}`,
      alias,
      appName: template.name,
      username,
      passwordDecrypted: `genPassword_${template.prefix}_${sequence * 123}`,
      otpSecret: idCounter % 3 === 0 ? 'MFRGGZDFMZTWQ2RE' : undefined,
      notes: `Đồng bộ tự động từ trình duyệt. Tài khoản thuộc danh mục ${template.category}.`,
      category: template.category,
      isFavorite: false,
      isDeleted: false,
      history: [],
      createdAt: daysAgo(10 + idCounter),
      updatedAt: daysAgo(1 + (idCounter % 5)),
    });
    idCounter++;
  }
  return result;
};

export const INITIAL_DEVICES: SyncDevice[] = [
  {
    id: 'dev-1',
    name: 'Windows 11 PC - Chrome',
    browser: 'Chrome',
    lastSyncedAt: hoursAgo(2),
    status: 'active',
  },
  {
    id: 'dev-2',
    name: 'Office Desktop - Edge',
    browser: 'Edge',
    lastSyncedAt: daysAgo(1),
    status: 'active',
  },
  {
    id: 'dev-3',
    name: 'Macbook Air - Firefox',
    browser: 'Firefox',
    lastSyncedAt: daysAgo(3),
    status: 'inactive',
  },
];

export const INITIAL_SYNC_LOGS: SyncLog[] = [
  {
    id: 'log-1',
    timestamp: hoursAgo(2),
    deviceName: 'Windows 11 PC - Chrome',
    type: 'SCAN_COMPLETED',
    message: 'Quét trình duyệt hoàn tất. Phát hiện 0 thay đổi.',
  },
  {
    id: 'log-2',
    timestamp: hoursAgo(4),
    deviceName: 'Windows 11 PC - Chrome',
    type: 'UPDATED',
    message: 'Tài khoản gg-main được phát hiện thay đổi mật khẩu và cập nhật tự động.',
  },
  {
    id: 'log-3',
    timestamp: daysAgo(1),
    deviceName: 'Office Desktop - Edge',
    type: 'ADDED',
    message: 'Thêm tài khoản mới: trello từ đồng bộ Edge.',
  },
];
