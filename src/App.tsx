/**
 * @license
 * SPDX-License-Identifier: Apache-2.0
 */

import React from 'react';
import { 
  Plus, 
  X, 
  Check, 
  RotateCw, 
  Sparkles, 
  Mail, 
  Share2, 
  Briefcase, 
  Heart, 
  Gamepad2, 
  MoreHorizontal, 
  Bot, 
  Laptop, 
  MessageSquareOff, 
  ShieldAlert, 
  Folder,
  Globe,
  Coins
} from 'lucide-react';
import Sidebar from './components/Sidebar';
import Header from './components/Header';
import AccountTable from './components/AccountTable';
import AccountDetail from './components/AccountDetail';
import TelegramSimulator from './components/TelegramSimulator';
import SyncAgentManager from './components/SyncAgentManager';
import PasswordGenerator from './components/PasswordGenerator';
import SecurityCheckup from './components/SecurityCheckup';
import TwoFactorAuthenticator from './components/TwoFactorAuthenticator';
import SmartImporter from './components/SmartImporter';

import { Account, SyncDevice, SyncLog } from './types';
import { generateBackupAccounts, INITIAL_DEVICES, INITIAL_SYNC_LOGS } from './data/defaultAccounts';

// List of available categories
const CATEGORIES = [
  { name: 'Email', icon: <Mail size={15} />, color: 'text-indigo-500' },
  { name: 'Mạng xã hội', icon: <Share2 size={15} />, color: 'text-blue-500' },
  { name: 'Ngân hàng', icon: <Coins size={15} />, color: 'text-emerald-500' },
  { name: 'Công việc', icon: <Briefcase size={15} />, color: 'text-amber-500' },
  { name: 'Giải trí', icon: <Heart size={15} />, color: 'text-rose-500' },
  { name: 'Game', icon: <Gamepad2 size={15} />, color: 'text-violet-500' },
  { name: 'Khác', icon: <MoreHorizontal size={15} />, color: 'text-slate-500' },
];

export default function App() {
  // Persistence Loading states
  const [accounts, setAccounts] = React.useState<Account[]>(() => {
    const saved = localStorage.getItem('vault_accounts');
    if (saved) {
      try {
        return JSON.parse(saved);
      } catch (e) {
        // Fallback
      }
    }
    return generateBackupAccounts();
  });

  const [devices, setDevices] = React.useState<SyncDevice[]>(() => {
    const saved = localStorage.getItem('vault_sync_devices');
    if (saved) {
      try {
        return JSON.parse(saved);
      } catch (e) {}
    }
    return INITIAL_DEVICES;
  });

  const [logs, setLogs] = React.useState<SyncLog[]>(() => {
    const saved = localStorage.getItem('vault_sync_logs');
    if (saved) {
      try {
        return JSON.parse(saved);
      } catch (e) {}
    }
    return INITIAL_SYNC_LOGS;
  });

  const [selectedAccountId, setSelectedAccountId] = React.useState<string | null>(null);

  const [searchQuery, setSearchQuery] = React.useState('');
  const [selectedCategory, setSelectedCategory] = React.useState<string | null>(null);
  const [currentTab, setCurrentTab] = React.useState('accounts'); // 'accounts' | 'favorites' | 'trash' | 'generator' | 'security' | 'sync' | 'telegram' | etc
  const [telegramPin, setTelegramPin] = React.useState(() => {
    return localStorage.getItem('vault_telegram_pin') || '2580';
  });
  const [telegramId, setTelegramId] = React.useState(() => {
    return localStorage.getItem('vault_telegram_id') || '610580967';
  });
  const [telegramBotToken, setTelegramBotToken] = React.useState(() => {
    return localStorage.getItem('vault_telegram_bot_token') || '';
  });
  const [isRealBotEnabled, setIsRealBotEnabled] = React.useState(() => {
    return localStorage.getItem('vault_is_real_bot_enabled') === 'true';
  });
  const [botConnectionStatus, setBotConnectionStatus] = React.useState<'disconnected' | 'connecting' | 'connected' | 'error'>('disconnected');
  const [telegramMessages, setTelegramMessages] = React.useState<any[]>([]);

  // Access count tracking for favorite lookup suggestions (smart suggestions)
  const [accessCounts, setAccessCounts] = React.useState<Record<string, number>>(() => {
    try {
      const saved = localStorage.getItem('vault_account_access_counts');
      return saved ? JSON.parse(saved) : {};
    } catch {
      return {};
    }
  });

  React.useEffect(() => {
    localStorage.setItem('vault_account_access_counts', JSON.stringify(accessCounts));
  }, [accessCounts]);

  const incrementAccessCount = React.useCallback((accountId: string) => {
    setAccessCounts(prev => ({
      ...prev,
      [accountId]: (prev[accountId] || 0) + 1
    }));
  }, []);

  const frequentAccounts = React.useMemo(() => {
    return accounts
      .filter(acc => !acc.isDeleted)
      .sort((a, b) => {
        const countA = accessCounts[a.id] || 0;
        const countB = accessCounts[b.id] || 0;
        if (countA !== countB) {
          return countB - countA;
        }
        return a.alias.localeCompare(b.alias);
      });
  }, [accounts, accessCounts]);

  const selectAndTrackAccount = (accId: string) => {
    setSelectedAccountId(accId);
    incrementAccessCount(accId);
  };

  // Telegram Float Widget state
  const [showTelegramFloat, setShowTelegramFloat] = React.useState(false);

  // Mobile navigation collapsible sidebar state
  const [isSidebarOpen, setIsSidebarOpen] = React.useState(false);

  // Desktop sidebar minified/collapsible state
  const [isSidebarMinified, setIsSidebarMinified] = React.useState(() => {
    return localStorage.getItem('vault_sidebar_minified') === 'true';
  });

  React.useEffect(() => {
    localStorage.setItem('vault_sidebar_minified', isSidebarMinified ? 'true' : 'false');
  }, [isSidebarMinified]);

  // Modals management
  const [showAddModal, setShowAddModal] = React.useState(false);
  const [showEditModal, setShowEditModal] = React.useState(false);
  const [modalAccountData, setModalAccountData] = React.useState<Partial<Account>>({});

  const isTgWebApp = React.useMemo(() => {
    return window.location.search.includes('tg_webapp=true') || window.location.hash.includes('tg_webapp=true');
  }, []);

  // Persistence triggers
  React.useEffect(() => {
    localStorage.setItem('vault_accounts', JSON.stringify(accounts));
  }, [accounts]);

  React.useEffect(() => {
    localStorage.setItem('vault_sync_devices', JSON.stringify(devices));
  }, [devices]);

  React.useEffect(() => {
    localStorage.setItem('vault_sync_logs', JSON.stringify(logs));
  }, [logs]);

  React.useEffect(() => {
    localStorage.setItem('vault_telegram_pin', telegramPin);
  }, [telegramPin]);

  React.useEffect(() => {
    localStorage.setItem('vault_telegram_id', telegramId);
  }, [telegramId]);

  React.useEffect(() => {
    localStorage.setItem('vault_telegram_bot_token', telegramBotToken);
  }, [telegramBotToken]);

  React.useEffect(() => {
    localStorage.setItem('vault_is_real_bot_enabled', String(isRealBotEnabled));
  }, [isRealBotEnabled]);

  // Light mode explicit enforcement
  React.useEffect(() => {
    document.documentElement.classList.remove('dark');
    document.body.classList.remove('dark');
  }, []);

  const activeAccount = React.useMemo(() => {
    if (!selectedAccountId) return null;
    return accounts.find(a => a.id === selectedAccountId) || null;
  }, [accounts, selectedAccountId]);

  // Actions trigger: Favorite Toggle
  const handleToggleFavorite = (id: string, e: React.MouseEvent) => {
    e.stopPropagation();
    setAccounts(prev => 
      prev.map(acc => {
        if (acc.id === id) {
          return { ...acc, isFavorite: !acc.isFavorite, updatedAt: new Date().toISOString() };
        }
        return acc;
      })
    );
  };

  // Delete Account (Soft delete to trash first)
  const handleDeleteAccount = (id: string, e: React.MouseEvent) => {
    e.stopPropagation();
    const target = accounts.find(a => a.id === id);
    if (!target) return;

    if (target.isDeleted) {
      // Permanent Delete
      if (confirm(`Bạn có chắc chắn muốn xóa vĩnh viễn tài khoản "${target.alias}" khỏi Database? Thao tác này không thể thu hồi.`)) {
        setAccounts(prev => prev.filter(acc => acc.id !== id));
        if (selectedAccountId === id) setSelectedAccountId(null);
      }
    } else {
      // Soft Delete
      setAccounts(prev => 
        prev.map(acc => {
          if (acc.id === id) {
            return { ...acc, isDeleted: true, updatedAt: new Date().toISOString() };
          }
          return acc;
        })
      );
      if (selectedAccountId === id) setSelectedAccountId(null);
    }
  };

  // Restore Account from Trash
  const handleRestoreAccount = (id: string) => {
    setAccounts(prev => 
      prev.map(acc => {
        if (acc.id === id) {
          return { ...acc, isDeleted: false, updatedAt: new Date().toISOString() };
        }
        return acc;
      })
    );
    setSelectedAccountId(id);
  };

  // Update account changes with Password history tracking correctly
  const handleUpdateAccount = (updatedData: Partial<Account>) => {
    if (!updatedData.id) return;
    setAccounts(prev => 
      prev.map(acc => {
        if (acc.id === updatedData.id) {
          const passChanged = updatedData.passwordDecrypted && updatedData.passwordDecrypted !== acc.passwordDecrypted;
          let newHistory = [...acc.history];
          
          if (passChanged) {
            newHistory = [
              {
                id: `hist-${Date.now()}`,
                passwordChangedAt: new Date().toISOString(),
                value: acc.passwordDecrypted
              },
              ...newHistory
            ];
          }

          return {
            ...acc,
            ...updatedData,
            history: newHistory,
            updatedAt: new Date().toISOString()
          };
        }
        return acc;
      })
    );
    setShowEditModal(false);
  };

  // Create new Account
  const handleCreateAccount = (newData: Partial<Account>) => {
    const freshAccount: Account = {
      id: `acc-${Date.now()}`,
      alias: newData.alias || 'unnamed-alias',
      appName: newData.appName || 'unknown_app',
      username: newData.username || 'username',
      passwordDecrypted: newData.passwordDecrypted || '',
      alternativePasswords: newData.alternativePasswords || [],
      otpSecret: newData.otpSecret || '',
      notes: newData.notes || '',
      category: newData.category || 'Khác',
      isFavorite: false,
      isDeleted: false,
      history: [],
      createdAt: new Date().toISOString(),
      updatedAt: new Date().toISOString()
    };

    setAccounts(prev => [freshAccount, ...prev]);
    setSelectedAccountId(freshAccount.id);
    setShowAddModal(false);
  };

  // Trigger Mock Sync Agent Scanning simulation
  const handleSyncDevice = (deviceId: string) => {
    const device = devices.find(d => d.id === deviceId);
    if (!device) return;

    // Simulate Sync scanning progress
    setTimeout(() => {
      const now = new Date();
      
      // Update Device synctime
      setDevices(prev => 
        prev.map(d => {
          if (d.id === deviceId) {
            return { ...d, lastSyncedAt: now.toISOString() };
          }
          return d;
        })
      );

      // Create a simulated brand new synchronized account found
      const syncChoices = [
        { alias: 'netflix-sync', app: 'netflix.com', user: 'hathanhtu711@gmail.com', pass: 'N@tflx_syncP@ssX9', cat: 'Giải trí', notes: 'Đồng bộ tự động phát hiện đổi mật khẩu từ Netflix.' },
        { alias: 'shopee-sync', app: 'shopee.vn', user: 'hathanhtu711', pass: 'shOp_99X_crypt', cat: 'Khác', notes: 'Mới được lưu trữ từ đồng bộ trình duyệt.' },
        { alias: 'github-work', app: 'github.com', user: 'hathanhtu.work', pass: 'git_work_token_abc123', cat: 'Công việc', notes: 'Được phát hiện từ SQLite Login Data của trình duyệt.' }
      ];

      const chosen = syncChoices[Math.floor(Math.random() * syncChoices.length)];
      const alreadyExists = accounts.some(a => a.alias === chosen.alias);

      if (!alreadyExists) {
        // Insert new synced account
        const accId = `sync-acc-${Date.now()}`;
        const fresh: Account = {
          id: accId,
          alias: chosen.alias,
          appName: chosen.app,
          username: chosen.user,
          passwordDecrypted: chosen.pass,
          category: chosen.cat,
          notes: chosen.notes,
          isFavorite: false,
          isDeleted: false,
          history: [],
          createdAt: now.toISOString(),
          updatedAt: now.toISOString()
        };

        setAccounts(prev => [fresh, ...prev]);
        
        // Add Synchronized LOGS
        const newLog: SyncLog = {
          id: `log-${Date.now()}`,
          timestamp: now.toISOString(),
          deviceName: device.name,
          type: 'ADDED',
          message: `Đồng bộ hoàn tất: Tự động phát hiện & thêm tài khoản "${chosen.alias}" vào database.`
        };
        setLogs(prev => [newLog, ...prev]);
      } else {
        // Add Completes logs only
        const newLog: SyncLog = {
          id: `log-${Date.now()}`,
          timestamp: now.toISOString(),
          deviceName: device.name,
          type: 'SCAN_COMPLETED',
          message: 'Quét trình duyệt hoàn tất. Không phát hiện thay đổi mật khẩu nào mới.'
        };
        setLogs(prev => [newLog, ...prev]);
      }

    }, 2000);
  };

  // Add mock device sync helper
  const handleAddDevice = () => {
    const browsers: ('Chrome' | 'Edge' | 'Firefox')[] = ['Chrome', 'Edge', 'Firefox'];
    const chosenBrowser = browsers[Math.floor(Math.random() * browsers.length)];
    const id = `dev-${Date.now()}`;
    const newDevice: SyncDevice = {
      id,
      name: `PC-Desktop - ${chosenBrowser} Cục bộ`,
      browser: chosenBrowser,
      lastSyncedAt: new Date().toISOString(),
      status: 'active'
    };

    setDevices(prev => [...prev, newDevice]);

    const newLog: SyncLog = {
      id: `log-${Date.now()}`,
      timestamp: new Date().toISOString(),
      deviceName: newDevice.name,
      type: 'SCAN_COMPLETED',
      message: `Thiết bị máy trạm được kết nối 24/7 thành công. Trao đổi khóa bảo mật AES hoàn thành.`
    };
    setLogs(prev => [newLog, ...prev]);
  };

  // Unlink device
  const handleDeleteDevice = (id: string) => {
    const dev = devices.find(d => d.id === id);
    if (!dev) return;

    if (confirm(`Bạn có chắc muốn hủy liên kết Sync Agent có thiết bị "${dev.name}" không?`)) {
      setDevices(prev => prev.filter(d => d.id !== id));
      
      const newLog: SyncLog = {
        id: `log-${Date.now()}`,
        timestamp: new Date().toISOString(),
        deviceName: dev.name,
        type: 'SCAN_COMPLETED',
        message: `Ngắt liên kết thiết bị thành công.`
      };
      setLogs(prev => [newLog, ...prev]);
    }
  };

  // --- REAL TELEGRAM BOT AND SIMULATOR CORE LOGIC ---
  const [realBotStep, setRealBotStep] = React.useState<'idle' | 'showing_matches' | 'confirm_view' | 'waiting_pin'>('idle');
  const [realBotTargetAccount, setRealBotTargetAccount] = React.useState<Account | null>(null);

  const [simulatorStep, setSimulatorStep] = React.useState<'none' | 'showing_matches' | 'confirm_view' | 'waiting_pin'>('none');
  const [simulatorTargetAccount, setSimulatorTargetAccount] = React.useState<any>(null);
  const [simulatorPinVerified, setSimulatorPinVerified] = React.useState(false);
  const [simulatorLastListedAccounts, setSimulatorLastListedAccounts] = React.useState<Account[]>([]);

  // Real bot last listed accounts for numbered choice on phone
  const [realBotLastListedAccounts, setRealBotLastListedAccounts] = React.useState<Account[]>([]);

  const realBotStepRef = React.useRef(realBotStep);
  const realBotTargetAccountRef = React.useRef(realBotTargetAccount);
  const accountsRef = React.useRef(accounts);
  const telegramPinRef = React.useRef(telegramPin);
  const telegramIdRef = React.useRef(telegramId);
  const telegramBotTokenRef = React.useRef(telegramBotToken);
  
  const simulatorLastListedAccountsRef = React.useRef<Account[]>([]);
  const realBotLastListedAccountsRef = React.useRef<Account[]>([]);

  React.useEffect(() => { realBotStepRef.current = realBotStep; }, [realBotStep]);
  React.useEffect(() => { realBotTargetAccountRef.current = realBotTargetAccount; }, [realBotTargetAccount]);
  React.useEffect(() => { accountsRef.current = accounts; }, [accounts]);
  React.useEffect(() => { telegramPinRef.current = telegramPin; }, [telegramPin]);
  React.useEffect(() => { telegramIdRef.current = telegramId; }, [telegramId]);
  React.useEffect(() => { telegramBotTokenRef.current = telegramBotToken; }, [telegramBotToken]);
  React.useEffect(() => { simulatorLastListedAccountsRef.current = simulatorLastListedAccounts; }, [simulatorLastListedAccounts]);
  React.useEffect(() => { realBotLastListedAccountsRef.current = realBotLastListedAccounts; }, [realBotLastListedAccounts]);

  const syncMessageToSimulator = (
    sender: 'user' | 'bot', 
    text: string, 
    buttons?: any[],
    isDestructive = false, 
    duration = 30
  ) => {
    const nowStr = new Date().toLocaleTimeString([], { hour: '2-digit', minute: '2-digit' });
    setTelegramMessages(prev => [
      ...prev,
      {
        id: `sync-${Date.now()}-${Math.random()}`,
        sender,
        text,
        time: nowStr,
        buttons,
        isDestructive,
        destructionSeconds: isDestructive ? duration : undefined
      }
    ]);
  };

  // Initialize welcoming chat messages for both simulation & real-time log monitoring
  React.useEffect(() => {
    const nowStr = new Date().toLocaleTimeString([], { hour: '2-digit', minute: '2-digit' });
    setTelegramMessages([
      {
        id: 'welcome',
        sender: 'bot',
        text: `Xin chào! Tôi là **Vault Bot 🤖** bảo mật 24/7 của bạn.\n\nTôi chỉ phản hồi duy nhất tin nhắn của chủ sở hữu Telegram ID: **${telegramId}**.\n\n*⚡️ Hãy bấm nút nhanh bên dưới hoặc nhập số:*\n1️⃣ Xem danh sách tài khoản (Nhập \`1\`)\n2️⃣ Trạng thái và kiểm tra PIN (Nhập \`2\`)\n3️⃣ Reset trạng thái PIN hôm nay (Nhập \`3\`)\n4️⃣ Hướng dẫn / Trợ giúp sử dụng (Nhập \`4\`)\n\nHoặc gõ trực tiếp từ khóa (ví dụ: \`gg\`, \`fb\`) để tìm kiếm tài sản siêu tốc!`,
        time: nowStr
      }
    ]);
  }, [telegramId]);

  const handleSimulatorSendMessage = (textToSend: string) => {
    if (!textToSend.trim()) return;
    const nowStr = new Date().toLocaleTimeString([], { hour: '2-digit', minute: '2-digit' });

    const userMsg = {
      id: `user-${Date.now()}-${Math.random()}`,
      sender: 'user' as const,
      text: textToSend,
      time: nowStr
    };

    setTelegramMessages(prev => [...prev, userMsg]);

    setTimeout(() => {
      processSimulatorBotCommand(textToSend);
    }, 600);
  };

  const processSimulatorBotCommand = (cmd: string) => {
    const cleanCmd = cmd.trim().toLowerCase();
    const nowStr = new Date().toLocaleTimeString([], { hour: '2-digit', minute: '2-digit' });
    
    const addSimulatorBotMessage = (text: string, buttons?: any[], isDestructive = false, duration = 30) => {
      setTelegramMessages(prev => [
        ...prev,
        {
          id: `bot-${Date.now()}-${Math.random()}`,
          sender: 'bot',
          text,
          time: nowStr,
          buttons,
          isDestructive,
          destructionSeconds: isDestructive ? duration : undefined
        }
      ]);
    };

    // 1. Check if we are waiting for a PIN input
    if (simulatorStep === 'waiting_pin') {
      if (cleanCmd === telegramPin) {
        setSimulatorPinVerified(true);
        setSimulatorStep('none');
        revealSimulatorPassword(simulatorTargetAccount, addSimulatorBotMessage);
      } else if (cleanCmd === 'hủy' || cleanCmd === 'huy' || cleanCmd === '/cancel' || cleanCmd === '2') {
        setSimulatorStep('none');
        setSimulatorTargetAccount(null);
        addSimulatorBotMessage("❌ Đã hủy quy trình xác thực PIN.");
      } else {
        addSimulatorBotMessage("❌ Mã PIN nhập không chính xác! Vui lòng bấm phím số bên dưới hoặc gõ lại mã PIN bảo mật:");
      }
      return;
    }

    // 2. Check if we are in confirmation step
    if (simulatorStep === 'confirm_view') {
      if (cleanCmd === '1' || cleanCmd.includes('xác nhận') || cleanCmd.includes('xem')) {
        checkSimulatorPin(simulatorTargetAccount, addSimulatorBotMessage);
      } else if (cleanCmd === '2' || cleanCmd === 'hủy' || cleanCmd === 'huy' || cleanCmd === '/cancel') {
        setSimulatorStep('none');
        setSimulatorTargetAccount(null);
        addSimulatorBotMessage("❌ Đã hủy quy trình truy cập mật khẩu.");
      } else {
        addSimulatorBotMessage("⚠️ Lựa chọn không hợp lệ. Vui lòng bấm phím `1` để Xem mật khẩu hoặc `2` để Hủy bỏ.");
      }
      return;
    }

    // 3. Check if we are in matches selection step
    if (simulatorStep === 'showing_matches') {
      const index = parseInt(cleanCmd);
      if (!isNaN(index) && index >= 1 && index <= simulatorLastListedAccounts.length) {
        const selected = simulatorLastListedAccounts[index - 1];
        handleSelectSimulatorAccount(selected, addSimulatorBotMessage);
      } else if (cleanCmd === 'hủy' || cleanCmd === 'huy' || cleanCmd === '/cancel') {
        setSimulatorStep('none');
        addSimulatorBotMessage("❌ Đã hủy quy trình hiển thị tài khoản.");
      } else {
        addSimulatorBotMessage(`⚠️ Lựa chọn không hợp lệ. Vui lòng nhập số từ 1 đến ${simulatorLastListedAccounts.length} hoặc bấm phím bên dưới để lựa chọn.`);
      }
      return;
    }

    // 4. Default Step Menu & Actions
    if (cleanCmd === '1' || cleanCmd.includes('tìm kiếm') || cleanCmd === 'tìm' || cleanCmd === 'tim') {
      addSimulatorBotMessage("🔍 **BẮT ĐẦU TÌM KIẾM CHI TIẾT**\n\nHãy nhập tên viết tắt hoặc từ khóa bất kỳ của tài khoản cần tìm (Ví dụ: `gg`, `zalo`, `fb`...)\n\n👉 *Mẹo:* Bạn có thể gõ trực tiếp từ khóa này bất kỳ lúc nào mà không cần nhấp nút Tìm kiếm!");
    } else if (cleanCmd.includes('cài đặt') || cleanCmd.includes('settings')) {
      addSimulatorBotMessage("⚙️ **CÀI ĐẶT & TRỢ GIÚP BẢO MẬT**\n\nVui lòng lựa chọn các tùy chọn bảo mật hoặc xem hướng dẫn bằng các phím ở bàn phím bên dưới:");
    } else if (cleanCmd === '🔐 kiểm tra trạng thái pin' || cleanCmd === '/pin' || cleanCmd.includes('kiểm tra pin') || cleanCmd === '2') {
      if (simulatorPinVerified) {
        addSimulatorBotMessage("🔐 *Trạng thái PIN hôm nay:* **ĐÃ XÁC THỰC ✅**\nBạn không cần nhập lại PIN để xem mật khẩu.");
      } else {
        addSimulatorBotMessage("🔐 *Trạng thái PIN hôm nay:* **CHƯA XÁC THỰC ❌**\nBạn sẽ cần nhập PIN bảo mật 1 lần để mở khóa mật khẩu.");
      }
    } else if (cleanCmd === '🔑 reset khóa pin' || cleanCmd === '/resetpin' || cleanCmd.includes('reset pin') || cleanCmd === '3') {
      setSimulatorPinVerified(false);
      addSimulatorBotMessage("🔑 Đã reset trạng thái xác minh PIN hôm nay. Lần xem mật khẩu tiếp theo bạn sẽ cần nhập lại PIN.");
    } else if (cleanCmd === '🆘 trợ giúp sử dụng' || cleanCmd === '/start' || cleanCmd === '/help' || cleanCmd === '🆘' || cleanCmd.includes('trợ giúp') || cleanCmd.includes('hướng dẫn') || cleanCmd === '4') {
      addSimulatorBotMessage(`🤖 *Secure Vault Bot* \nQuản lý mật khẩu bằng bàn phím hệ thống thông minh.\n\n*⚡️ Các phím chức năng chính:*\n🔍 **Tìm kiếm**: Nhập từ khóa truy tìm nhanh.\n⚙️ **Cài đặt & Trợ giúp**: Quản lý PIN khóa, xem trạng thái đăng nhập, và trợ giúp.\n🌐 **Web Vault**: Mở giao diện Web App đỉnh cao trực tiếp bên trong Telegram.\n\n*🔍 Tra cứu siêu tốc:*\nGõ thẳng từ khóa (ví dụ: \`gg\`, \`fb\`, \`zalo\`) để tìm tài khoản mà không cần gõ lệnh dài dòng.`);
    } else if (cleanCmd === '↩️ quay lại menu chính' || cleanCmd === 'quay lại' || cleanCmd === 'back') {
      addSimulatorBotMessage("↩️ Đã quay lại Menu chính.");
    } else if (cleanCmd === '🌐 mở web vault' || cleanCmd.includes('web vault') || cleanCmd.includes('miniapp')) {
      addSimulatorBotMessage("🌐 **Đang mở Web Vault...**\nGiao diện Web App tương tác tuyệt vời đang mở ngay bên trong Telegram của bạn.");
    } else {
      // Treat as custom text search query
      const matches = accounts.filter(
        acc => !acc.isDeleted && (
          acc.alias.toLowerCase().includes(cleanCmd) || 
          acc.appName.toLowerCase().includes(cleanCmd)
        )
      );

      if (matches.length === 0) {
        addSimulatorBotMessage(`🔍 Không tìm thấy tài khoản nào liên quan đến từ khóa "${cmd}". Hãy thử lại hoặc nhập \`1\` để xem toàn bộ danh sách.`);
      } else if (matches.length === 1) {
        setSimulatorLastListedAccounts(matches);
        handleSelectSimulatorAccount(matches[0], addSimulatorBotMessage);
      } else {
        setSimulatorLastListedAccounts(matches);
        setSimulatorStep('showing_matches');
        const countText = `🔍 Tìm thấy ${matches.length} tài khoản cho từ khóa "${cmd}":\n\n`;
        const listText = matches.map((acc, idx) => {
          return `${idx + 1}. **${acc.alias}** (${acc.username})`;
        }).join('\n\n');

        addSimulatorBotMessage(countText + listText + "\n\n👉 Vui lòng nhập số tương ứng (ví dụ: `1`, `2`...) hoặc click bàn phím bên dưới để chọn:");
      }
    }
  };

  const handleSelectSimulatorAccount = (account: any, addMsg: any) => {
    setSimulatorTargetAccount(account);
    setSimulatorStep('confirm_view');
    const questionText = `Tài khoản: **${account.alias}**\nUsername: **${account.username}**\n\nBạn có muốn xem mật khẩu không?`;
    
    addMsg(questionText);
  };

  const checkSimulatorPin = (account: any, addMsg: any) => {
    if (simulatorPinVerified) {
      revealSimulatorPassword(account, addMsg);
    } else {
      setSimulatorStep('waiting_pin');
      addMsg("🔐 Vui lòng nhập mã PIN bảo mật của bạn để tiếp tục (hôm nay bạn chưa xác thực trong 24 giờ):");
    }
  };

  const revealSimulatorPassword = (account: any, addMsg: any) => {
    const now = new Date();
    const currentSecs = now.getSeconds();
    const left = 30 - (currentSecs % 30);
    const baseNum = Math.floor(now.getTime() / 30000);
    const secretHash = account.id.charCodeAt(0) * 8219 + (account.otpSecret ? account.otpSecret.charCodeAt(0) * 1923 : 0);
    const codeNum = (baseNum * 313 + secretHash) % 900000 + 100000;
    const codeStr = codeNum.toString();
    const otpCodeString = account.otpSecret ? `${codeStr.slice(0, 3)} ${codeStr.slice(3)}` : 'Chưa thiết lập 2FA';

    const altPassSection = account.alternativePasswords && account.alternativePasswords.length > 0
      ? `\n**Mật khẩu khác chung tài khoản:**\n` + account.alternativePasswords.map((p: string, idx: number) => `• Mật khẩu ${idx + 2}: \`${p}\``).join('\n')
      : '';

    const outputText = `🔓 **XÁC MINH PIN HỢP LỆ ✅**\nBạn sẽ không cần nhập PIN trong vòng 24h tới.\n\n` +
      `**Tài khoản:** ${account.alias}\n` +
      `**Username:** \`${account.username}\`\n\n` +
      `**Mật khẩu chính:** \`${account.passwordDecrypted}\`${altPassSection}\n\n` +
      `**Mã OTP (2FA):** \`${otpCodeString}\` (còn ${left} giây)\n\n` +
      `🚨 **Lưu ý bảo mật:** Bộ đệm an toàn sẽ tự động xóa tin nhắn nhạy cảm này khỏi màn hình chat sau 30 giây!`;

    addMsg(outputText, undefined, true, 30);
  };

  // REAL-TIME TELEGRAM LONG POLLING BOT DEPLOYMENT
  React.useEffect(() => {
    let active = true;
    
    const startPolling = async () => {
      if (!isRealBotEnabled || !telegramBotToken) {
        setBotConnectionStatus('disconnected');
        return;
      }
      
      setBotConnectionStatus('connecting');
      let offset = Number(localStorage.getItem('vault_telegram_offset')) || 0;
      
      while (active && isRealBotEnabled && telegramBotToken) {
        try {
          const res = await fetch(`https://api.telegram.org/bot${telegramBotToken}/getUpdates?offset=${offset}&timeout=15`);
          if (!res.ok) {
            throw new Error("Mã kết nối lỗi");
          }
          const data = await res.json();
          if (!active) break;
          
          if (data.ok) {
            setBotConnectionStatus('connected');
            const updates = data.result;
            
            if (updates.length > 0) {
              for (const update of updates) {
                await handleTelegramUpdate(update);
                offset = update.update_id + 1;
                localStorage.setItem('vault_telegram_offset', String(offset));
              }
            }
          } else {
            setBotConnectionStatus('error');
            await new Promise(r => setTimeout(r, 6000));
          }
        } catch (err) {
          console.error("Lỗi kết nối Polling Telegram:", err);
          if (active) {
            setBotConnectionStatus('error');
            await new Promise(r => setTimeout(r, 6000));
          }
        }
      }
    };
    
    startPolling();
    
    return () => {
      active = false;
    };
  }, [isRealBotEnabled, telegramBotToken]);

  const handleTelegramUpdate = async (update: any) => {
    const token = telegramBotTokenRef.current;
    if (!token) return;

    const sendRealTelegramMessage = async (chatId: string, text: string, replyMarkup?: any) => {
      try {
        const res = await fetch(`https://api.telegram.org/bot${token}/sendMessage`, {
          method: 'POST',
          headers: { 'Content-Type': 'application/json' },
          body: JSON.stringify({
            chat_id: chatId,
            text: text,
            parse_mode: 'Markdown',
            reply_markup: replyMarkup
          })
        });
        return await res.json();
      } catch (err) {
        console.error("Lỗi gửi tin nhắn qua Bot:", err);
      }
    };

    const deleteRealTelegramMessageAfterDelay = (chatId: string, messageId: number, delayMs: number) => {
      setTimeout(async () => {
        try {
          await fetch(`https://api.telegram.org/bot${token}/deleteMessage`, {
            method: 'POST',
            headers: { 'Content-Type': 'application/json' },
            body: JSON.stringify({
              chat_id: chatId,
              message_id: messageId
            })
          });
          await sendRealTelegramMessage(chatId, `🔒 *[TIN NHẮN TỰ HỦY]* Mật khẩu truy cập tạm thời vừa rồi đã được xóa sạch khỏi lịch sử chat để đảm bảo an toàn tuyệt đối.`);
        } catch (err) {
          console.error("Lỗi xóa tin nhắn tự hủy:", err);
        }
      }, delayMs);
    };

    if (update.callback_query) {
      const callback = update.callback_query;
      const chatId = callback.message.chat.id;
      const data = callback.data;
      const senderId = String(callback.from.id);

      try {
        await fetch(`https://api.telegram.org/bot${token}/answerCallbackQuery`, {
          method: 'POST',
          headers: { 'Content-Type': 'application/json' },
          body: JSON.stringify({ callback_query_id: callback.id })
        });
      } catch (e) {}

      if (senderId !== telegramIdRef.current) {
        await sendRealTelegramMessage(chatId, `⚠️ *CẢNH BÁO BẢO MẬT*:\n\nBạn không phải là Admin đăng ký của Vault này. Bot từ chối phản hồi.`);
        return;
      }

      if (data.startsWith('select_acc:')) {
        const accId = data.split('select_acc:')[1];
        const account = accountsRef.current.find(a => a.id === accId);
        
        if (account) {
          setRealBotTargetAccount(account);
          setRealBotStep('waiting_pin');
          
          await sendRealTelegramMessage(
            chatId, 
            `🔐 Bạn đã chọn: *${account.alias}* (${account.username})\n\nVui lòng nhập mã PIN bảo mật gồm 4 chữ số để xác thực mở khóa:`
          );
          syncMessageToSimulator('bot', `🔐 Bạn đã chọn: ${account.alias} (${account.username})\nVui lòng nhập mã PIN bảo mật để mở khóa.`);
        }
      } else if (data === 'cancel_flow') {
        setRealBotStep('idle');
        setRealBotTargetAccount(null);
        await sendRealTelegramMessage(chatId, `❌ Quy trình lấy dữ liệu mật khẩu đã được hủy bỏ thành công.`);
        syncMessageToSimulator('bot', `Đã hủy quy trình truy cập mật khẩu.`);
      }
      return;
    }

    if (update.message && update.message.text) {
      const message = update.message;
      const chatId = message.chat.id;
      const senderId = String(message.from.id);
      const text = message.text.trim();
      const textLower = text.toLowerCase();

      if (senderId !== telegramIdRef.current) {
        await sendRealTelegramMessage(chatId, `⚠️ *CẢNH BÁO BẢO MẬT*:\n\nThiết bị có ID: \`${senderId}\` không khớp với Telegram ID Admin đăng ký của Vault này. Bot từ chối phản hồi.`);
        return;
      }

      syncMessageToSimulator('user', text);

      // 1. If waiting for PIN
      if (realBotStepRef.current === 'waiting_pin' && realBotTargetAccountRef.current) {
        const account = realBotTargetAccountRef.current;
        if (text === telegramPinRef.current) {
          setRealBotStep('idle');
          setRealBotTargetAccount(null);

          const now = new Date();
          const currentSecs = now.getSeconds();
          const left = 30 - (currentSecs % 30);
          const baseNum = Math.floor(now.getTime() / 30000);
          const secretHash = account.id.charCodeAt(0) * 8219 + (account.otpSecret ? account.otpSecret.charCodeAt(0) * 1923 : 0);
          const codeNum = (baseNum * 313 + secretHash) % 900000 + 100000;
          const codeStr = codeNum.toString();
          const otpCodeString = account.otpSecret ? `${codeStr.slice(0, 3)} ${codeStr.slice(3)}` : 'Chưa thiết lập 2FA';

          const altPassSection = account.alternativePasswords && account.alternativePasswords.length > 0
            ? `\n*Mật khẩu khác chung tài khoản:*\n` + account.alternativePasswords.map((p: string, idx: number) => `• Mật khẩu ${idx + 2}: \`${p}\``).join('\n')
            : '';

          const revealText = `🔓 *XÁC MINH PIN HỢP LỆ ✅*\n\n` +
            `*Tài khoản:* ${account.alias}\n` +
            `*Website/App:* \`${account.appName}\`\n` +
            `*Username:* \`${account.username}\`\n` +
            `*Mật khẩu chính:* \`${account.passwordDecrypted}\`${altPassSection}\n\n` +
            `*Mã OTP (2FA):* \`${otpCodeString}\` (còn ${left} giây)\n\n` +
            `🚨 *Lưu ý bảo mật quan trọng:* Tin nhắn chứa dữ liệu cực nhạy này sẽ tự động biến mất và *XÓA KHỎI LỊCH SỬ CHAT* trong vòng 30 giây để đảm bảo an toàn tuyệt đối!`;

          const replyResult = await sendRealTelegramMessage(chatId, revealText);
          
          if (replyResult && replyResult.ok && replyResult.result) {
            const sentMsgId = replyResult.result.message_id;
            deleteRealTelegramMessageAfterDelay(chatId, sentMsgId, 30000);
          }

          syncMessageToSimulator('bot', `🔓 XÁC MINH PIN HỢP LỆ ✅\nTài khoản: ${account.alias}\nUsername: ${account.username}\nPassword: ${account.passwordDecrypted}\nOTP (2FA): ${otpCodeString} (còn ${left}s)`, undefined, true, 30);

        } else if (textLower === '/cancel' || textLower === 'hủy' || textLower === 'huy' || textLower === '2' || textLower === 'hủy bỏ') {
          setRealBotStep('idle');
          setRealBotTargetAccount(null);
          await sendRealTelegramMessage(chatId, `❌ Quy trình nhập PIN đã được hủy bỏ.`);
          syncMessageToSimulator('bot', `Đã hủy quy trình truy cập mật khẩu.`);
        } else {
          await sendRealTelegramMessage(chatId, `❌ *Mã PIN nhập vào không chính xác!*\n\nVui lòng thử lại hoặc gõ \`/cancel\` để hủy bỏ quy trình.`);
          syncMessageToSimulator('bot', `❌ Mã PIN nhập không chính xác! Vui lòng nhập lại mã PIN bảo mật:`);
        }
        return;
      }

      // 2. If in confirmation step
      if (realBotStepRef.current === 'confirm_view' && realBotTargetAccountRef.current) {
        const account = realBotTargetAccountRef.current;
        if (text === '1' || textLower.includes('xác nhận') || textLower.includes('xem')) {
          setRealBotStep('waiting_pin');
          await sendRealTelegramMessage(chatId, `🔐 *Xác thực:* Vui lòng nhập mã PIN bảo mật gồm 4 chữ số để mở khóa tài khoản *${account.alias}*:`);
          syncMessageToSimulator('bot', `🔐 Vui lòng nhập mã PIN bảo mật của bạn để tiếp tục:`);
        } else if (text === '2' || textLower === '/cancel' || textLower === 'hủy' || textLower === 'huy' || textLower === 'hủy bỏ') {
          setRealBotStep('idle');
          setRealBotTargetAccount(null);
          await sendRealTelegramMessage(chatId, `❌ Đã hủy quy trình xem mật khẩu.`);
          syncMessageToSimulator('bot', `Đã hủy quy trình truy cập mật khẩu.`);
        } else {
          await sendRealTelegramMessage(chatId, `⚠️ Lựa chọn không hợp lệ. Vui lòng nhập \`1\` để Xem mật khẩu hoặc \`2\` để Hủy.`);
        }
        return;
      }

      // 3. If in account listing step
      if (realBotStepRef.current === 'showing_matches') {
        const index = parseInt(text);
        if (!isNaN(index) && index >= 1 && index <= realBotLastListedAccountsRef.current.length) {
          const selected = realBotLastListedAccountsRef.current[index - 1];
          setRealBotTargetAccount(selected);
          setRealBotStep('confirm_view');
          
          const questionText = `❓ Bạn đã chọn: *${selected.alias}* (${selected.username})\n\nBạn có muốn mở khóa xem mật khẩu không?\n1️⃣ Gõ \`1\` để Xem mật khẩu\n2️⃣ Gõ \`2\` để Hủy bỏ`;
          await sendRealTelegramMessage(chatId, questionText);
          syncMessageToSimulator('bot', `Tài khoản: ${selected.alias}\nUsername: ${selected.username}\n\nBạn có muốn xem mật khẩu không?`);
        } else if (textLower === '/cancel' || textLower === 'hủy' || textLower === 'huy') {
          setRealBotStep('idle');
          await sendRealTelegramMessage(chatId, `❌ Đã hủy quy trình lựa chọn.`);
          syncMessageToSimulator('bot', `Đã hủy quy trình lựa chọn.`);
        } else {
          await sendRealTelegramMessage(chatId, `⚠️ Chỉ số không hợp lệ. Vui lòng nhập số từ 1 đến ${realBotLastListedAccountsRef.current.length} hoặc gõ \`hủy\`.`);
        }
        return;
      }

      // 4. Default Step Actions
      if (textLower === '/start' || textLower === '/help' || textLower === '🆘' || textLower === 'trợ giúp' || textLower === 'hướng dẫn' || textLower.includes('trợ giúp sử dụng') || textLower === '4') {
        const welcome = `🤖 *Secure Vault Bot* \nQuản lý dữ liệu mật khẩu an toàn.\n\n` +
          `*⚡️ Các chức năng nhanh dưới bàn phím hệ thống:*\n` +
          `🔍 *Tìm kiếm*: Tìm tài khoản theo từ khóa (gõ trực tiếp tên viết tắt để tìm)\n` +
          `⚙️ *Cài đặt & Trợ giúp*: Quản lý mã PIN bảo mật, reset trạng thái, hoặc xem Hướng dẫn\n` +
          `🌐 *Web Vault*: Mở trực tiếp Web App quản trị ngay bên trong ứng dụng Telegram!`;

        const mainKeyboard = {
          keyboard: [
            [
              { text: "🔍 Tìm kiếm" },
              { text: "⚙️ Cài đặt & Trợ giúp" }
            ],
            [
              { text: "🌐 Mở Web Vault", web_app: { url: window.location.origin } }
            ]
          ],
          resize_keyboard: true,
          one_time_keyboard: false
        };

        await sendRealTelegramMessage(chatId, welcome, mainKeyboard);
        syncMessageToSimulator('bot', `Xin chào! Tôi đã sẵn sàng hoạt động cùng ID thực tế ${telegramIdRef.current}. Hãy sử dụng bàn phím chức năng hoàn toàn mới hoặc gõ để tra cứu.`);
      } else if (textLower === '🔍 tìm kiếm' || textLower === 'tìm kiếm' || textLower === 'tìm' || textLower === 'tim' || textLower === '1') {
        const tipSearchText = `🔍 *BẮT ĐẦU TÌM KIẾM CHI TIẾT*\n\nHãy nhập tên viết tắt hoặc từ khóa bất kỳ của tài khoản cần tìm (Ví dụ: \`gg\`, \`zalo\`, \`fb\`...)\n\n👉 *Mẹo:* Bạn có thể gõ trực tiếp từ khóa này bất kỳ lúc nào mà không cần nhấp nút Tìm kiếm!`;
        await sendRealTelegramMessage(chatId, tipSearchText);
        syncMessageToSimulator('bot', `🔍 Đã kích hoạt tìm kiếm. Vui lòng nhập từ khóa.`);
      } else if (textLower === '⚙️ cài đặt & trợ giúp' || textLower === 'cài đặt') {
        const settingsText = `⚙️ *CÀI ĐẶT & HƯỚNG DẪN BẢO MẬT*:\n\nVui lòng lựa chọn các tùy chọn bảo mật hoặc xem hướng dẫn bằng các phím ở bàn phím bên dưới:`;
        const settingsKeyboard = {
          keyboard: [
            [
              { text: "🔐 Kiểm tra trạng thái PIN" },
              { text: "🔑 Reset khóa PIN" }
            ],
            [
              { text: "🆘 Trợ giúp sử dụng" },
              { text: "↩️ Quay lại Menu chính" }
            ]
          ],
          resize_keyboard: true,
          one_time_keyboard: false
        };
        await sendRealTelegramMessage(chatId, settingsText, settingsKeyboard);
        syncMessageToSimulator('bot', `Đã mở menu cài đặt bảo mật.`);
      } else if (textLower === '🔐 kiểm tra trạng thái pin' || textLower === '/pin' || textLower.includes('kiểm tra pin') || textLower === '2') {
        const isVerified = simulatorPinVerified;
        if (isVerified) {
          await sendRealTelegramMessage(chatId, `🔐 *Trạng thái PIN hôm nay:* **ĐÃ XÁC THỰC ✅**\nBạn sẽ không cần nhập PIN để xem mật khẩu.`);
        } else {
          await sendRealTelegramMessage(chatId, `🔐 *Trạng thái PIN hôm nay:* **CHƯA XÁC THỰC ❌**\nBạn sẽ cần nhập PIN mật khẩu khi xem tài khoản.`);
        }
      } else if (textLower === '🔑 reset khóa pin' || textLower === '/resetpin' || textLower.includes('reset pin') || textLower === '3') {
        setSimulatorPinVerified(false);
        await sendRealTelegramMessage(chatId, `🔑 Đã đặt lại trạng thái PIN. Lần truy cập tiếp theo sẽ cần nhập lại PIN bảo mật.`);
        syncMessageToSimulator('bot', `Đã reset trạng thái xác minh PIN.`);
      } else if (textLower === '↩️ quay lại menu chính' || textLower === 'quay lại') {
        const mainKeyboard = {
          keyboard: [
            [
              { text: "🔍 Tìm kiếm" },
              { text: "⚙️ Cài đặt & Trợ giúp" }
            ],
            [
              { text: "🌐 Mở Web Vault", web_app: { url: window.location.origin } }
            ]
          ],
          resize_keyboard: true,
          one_time_keyboard: false
        };
        await sendRealTelegramMessage(chatId, `↩️ Đã quay lại Menu chính.`, mainKeyboard);
        syncMessageToSimulator('bot', `Đã quay lại Menu chính.`);
      } else if (textLower === '🌐 mở web vault' || textLower.includes('web vault')) {
        const webappText = `🌐 *MỞ WEB VAULT MINI APP*:\n\nBạn có thể nhấp trực tiếp vào nút dưới đây để mở giao diện quản trị Web Vault mượt mà, đầy đủ tính năng ngay bên trong ứng dụng Telegram của bạn!`;
        const webappKeyboard = {
          keyboard: [
            [
              { text: "🌐 Mở Web Vault", web_app: { url: window.location.origin } }
            ],
            [
              { text: "↩️ Quay lại Menu chính" }
            ]
          ],
          resize_keyboard: true,
          one_time_keyboard: false
        };
        await sendRealTelegramMessage(chatId, webappText, webappKeyboard);
        syncMessageToSimulator('bot', `Đã hướng dẫn mở Web Vault Mini App.`);
      } else {
        // Treat as direct search alias
        const aliasQuery = textLower.startsWith('/get ') ? textLower.replace('/get ', '').trim() : textLower;
        const matches = accountsRef.current.filter(
          acc => !acc.isDeleted && (
            acc.alias.toLowerCase().includes(aliasQuery) || 
            acc.appName.toLowerCase().includes(aliasQuery)
          )
        );

        if (matches.length === 0) {
          await sendRealTelegramMessage(chatId, `🔍 Không tìm thấy tài khoản nào khớp với từ khóa "${aliasQuery}". Hãy thử lại hoặc gõ \`1\` để xem danh sách.`);
          syncMessageToSimulator('bot', `🔍 Không tìm thấy tài khoản nào liên quan đến "${aliasQuery}" trong Web Vault.`);
        } else if (matches.length === 1) {
          const target = matches[0];
          setRealBotTargetAccount(target);
          setRealBotStep('confirm_view');

          const inlineKeyboard = {
            inline_keyboard: [
              [
                { text: "🔓 1. Xem mật khẩu", callback_data: `select_acc:${target.id}` },
                { text: "❌ 2. Hủy bỏ", callback_data: "cancel_flow" }
              ]
            ]
          };

          await sendRealTelegramMessage(
            chatId, 
            `🔍 Tìm thấy 1 tài khoản khớp:\n\n` +
            `• *Alias:* ${target.alias}\n` +
            `• *Username:* \`${target.username}\`\n\n` +
            `Bạn có muốn mở khóa xem chi tiết mật khẩu & mã OTP 2FA không?\n1️⃣ Gõ \`1\` để Xem mật khẩu\n2️⃣ Gõ \`2\` để Hủy`,
            inlineKeyboard
          );

          syncMessageToSimulator('bot', `Tài khoản: ${target.alias}\nUsername: ${target.username}\n\nBạn có muốn xem mật khẩu không?`);

        } else {
          setRealBotLastListedAccounts(matches);
          setRealBotStep('showing_matches');
          
          const listText = matches.map((acc, idx) => `${idx + 1}. *${acc.alias}* (${acc.username})`).join('\n');
          const inlineKeyboard = {
            inline_keyboard: matches.map(acc => [
              { text: `${acc.alias} (${acc.username})`, callback_data: `select_acc:${acc.id}` }
            ])
          };

          await sendRealTelegramMessage(
            chatId,
            `🔍 Tìm thấy *${matches.length} tài khoản* khớp với "${aliasQuery}":\n\n${listText}\n\nVui lòng nhập số tương ứng từ *1* đến *${matches.length}* hoặc click chọn bên dưới:`,
            inlineKeyboard
          );

          syncMessageToSimulator(
            'bot', 
            `Tìm thấy ${matches.length} tài khoản liên quan đến "${aliasQuery}":\n\nVui lòng chọn tài khoản trong danh sách.`
          );
        }
      }
    }
  };

  // Edit forms trigger callback helper
  const triggerEditModal = (account: Account, e: React.MouseEvent) => {
    e.stopPropagation();
    setModalAccountData(account);
    setShowEditModal(true);
  };

  // Inline Quick Random Generator
  const generateRandomModalPassword = () => {
    const pool = 'ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789!@#$%^&*';
    let output = '';
    for (let i = 0; i < 16; i++) {
      output += pool.charAt(Math.floor(Math.random() * pool.length));
    }
    setModalAccountData(prev => ({ ...prev, passwordDecrypted: output }));
  };

  if (isTgWebApp) {
    return (
      <div className="h-screen w-screen flex flex-col bg-slate-50 dark:bg-[#151518] overflow-hidden relative">
        {/* Header simple for mobile inside Telegram Webview */}
        <div className="bg-[#1c7bb6] text-white px-4 py-3 flex items-center justify-between shrink-0 shadow-sm relative z-20">
          <div className="flex items-center gap-2">
            <Globe size={16} className="text-sky-300 shrink-0" />
            <span className="font-bold text-xs uppercase tracking-wider">Kraft Vault (Web App)</span>
          </div>
          <span className="text-[9px] px-1.5 py-0.5 rounded-full bg-sky-500/20 text-sky-100 font-bold border border-sky-450/20">Mini App Mode 🔐</span>
        </div>

        {/* Display accounts inside the webview */}
        <div className="flex-1 flex flex-col min-h-0 overflow-hidden relative z-10">
          <AccountTable
            accounts={accounts}
            selectedCategory={selectedCategory}
            searchQuery={searchQuery}
            setSearchQuery={setSearchQuery}
            selectedAccountId={selectedAccountId}
            onSelectAccount={(account) => selectAndTrackAccount(account.id)}
            onToggleFavorite={handleToggleFavorite}
            onEditAccount={triggerEditModal}
            onDeleteAccount={handleDeleteAccount}
            onAddAccount={() => {
              setModalAccountData({
                alias: '',
                appName: '',
                username: '',
                passwordDecrypted: '',
                alternativePasswords: [],
                otpSecret: '',
                notes: '',
                category: selectedCategory || 'Email'
              });
              setShowAddModal(true);
            }}
            activeTab="accounts"
            isMiniApp={true}
          />

          {activeAccount && (
            <div className="fixed inset-0 z-50 bg-slate-950/40 backdrop-blur-xs flex items-end justify-center p-0">
              <div className="w-full max-h-[85vh] bg-white dark:bg-slate-900 rounded-t-3xl overflow-y-auto flex flex-col animate-[slideUp_0.2s_ease-out] shadow-[0_-8px_30px_rgba(0,0,0,0.1)]">
                <div className="p-4 border-b border-slate-100 dark:border-slate-800 flex justify-between items-center shrink-0">
                  <span className="font-bold text-sm text-slate-800 dark:text-slate-100">Chi tiết tài khoản</span>
                  <button 
                    onClick={() => setSelectedAccountId(null)}
                    className="p-1 hover:bg-slate-100 dark:hover:bg-slate-800 rounded-full text-slate-400 cursor-pointer"
                  >
                    <X size={18} />
                  </button>
                </div>
                <div className="p-0 flex-1">
                  <AccountDetail
                    account={activeAccount}
                    onToggleFavorite={handleToggleFavorite}
                    onEditAccount={triggerEditModal}
                    onDeleteAccount={handleDeleteAccount}
                    onRestoreAccount={handleRestoreAccount}
                    onClose={() => setSelectedAccountId(null)}
                    isMiniApp={true}
                  />
                </div>
              </div>
            </div>
          )}
        </div>

        {/* MODAL GIAO DIỆN: THÊM TÀI KHOẢN (ADD) */}
        {showAddModal && (
          <div id="add-modal-overlay" className="fixed inset-0 z-[60] bg-slate-950/40 backdrop-blur-xs flex items-center justify-center p-4">
            <div id="add-modal-box" className="w-full max-w-sm bg-white dark:bg-[#1C1C1E] rounded-3xl p-5 space-y-4 shadow-xl border-none">
              <div className="flex items-center justify-between border-b pb-2">
                <span className="font-bold text-sm tracking-tight">Thêm tài khoản mới</span>
                <button onClick={() => setShowAddModal(false)} className="p-1"><X size={14} /></button>
              </div>
              <div className="space-y-3 text-xs overflow-y-auto max-h-[60vh] pr-1">
                <div className="flex flex-col gap-1">
                  <label className="font-semibold text-slate-500">Alias ngắn</label>
                  <input type="text" value={modalAccountData.alias || ''} onChange={(e) => setModalAccountData(prev => ({ ...prev, alias: e.target.value }))} className="w-full p-2 bg-[#F2F2F7] dark:bg-[#2C2C2E] text-slate-800 dark:text-white border dark:border-slate-800 rounded-lg font-bold" />
                </div>
                <div className="flex flex-col gap-1">
                  <label className="font-semibold text-slate-500">App / Web</label>
                  <input type="text" value={modalAccountData.appName || ''} onChange={(e) => setModalAccountData(prev => ({ ...prev, appName: e.target.value }))} className="w-full p-2 bg-[#F2F2F7] dark:bg-[#2C2C2E] text-slate-800 dark:text-white border dark:border-slate-800 rounded-lg" />
                </div>
                <div className="flex flex-col gap-1">
                  <label className="font-semibold text-slate-500">Tên đăng ký / Username</label>
                  <input type="text" value={modalAccountData.username || ''} onChange={(e) => setModalAccountData(prev => ({ ...prev, username: e.target.value }))} className="w-full p-2 bg-[#F2F2F7] dark:bg-[#2C2C2E] text-slate-800 dark:text-white border dark:border-slate-800 rounded-lg font-mono" />
                </div>
                <div className="flex flex-col gap-1">
                  <label className="font-semibold text-slate-500">Mật khẩu</label>
                  <div className="flex gap-1">
                    <input type="text" value={modalAccountData.passwordDecrypted || ''} onChange={(e) => setModalAccountData(prev => ({ ...prev, passwordDecrypted: e.target.value }))} className="flex-1 p-2 bg-[#F2F2F7] dark:bg-[#2C2C2E] text-slate-800 dark:text-white border dark:border-slate-800 rounded-lg font-mono font-bold" />
                    <button type="button" onClick={generateRandomModalPassword} className="p-2 bg-slate-100 dark:bg-slate-800 text-[10px] font-bold">tạo</button>
                  </div>
                </div>
                <div className="flex flex-col gap-1">
                  <label className="font-semibold text-slate-500">Mã Authenticator OTP (2FA)</label>
                  <input type="text" value={modalAccountData.otpSecret || ''} onChange={(e) => setModalAccountData(prev => ({ ...prev, otpSecret: e.target.value.replace(/\s+/g, '').toUpperCase() }))} className="w-full p-2 bg-[#F2F2F7] dark:bg-[#2C2C2E] text-slate-800 dark:text-white border dark:border-slate-800 rounded-lg font-mono" />
                </div>
              </div>
              <div className="flex justify-end gap-2 pt-2 border-t dark:border-slate-800">
                <button onClick={() => setShowAddModal(false)} className="px-3 py-1.5 bg-slate-100 dark:bg-slate-800 rounded-lg text-xs font-semibold">Hủy</button>
                <button onClick={() => { handleCreateAccount(modalAccountData) }} className="px-3 py-1.5 bg-blue-500 text-white rounded-lg text-xs font-semibold">Thêm</button>
              </div>
            </div>
          </div>
        )}

        {/* MODAL GIAO DIỆN: SỬA TÀI KHOẢN (EDIT) */}
        {showEditModal && (
          <div id="edit-modal-overlay" className="fixed inset-0 z-[60] bg-slate-950/40 backdrop-blur-xs flex items-center justify-center p-4">
            <div id="edit-modal-box" className="w-full max-w-sm bg-white dark:bg-[#1C1C1E] rounded-3xl p-5 space-y-4 shadow-xl border-none">
              <div className="flex items-center justify-between border-b pb-2">
                <span className="font-bold text-sm tracking-tight">Sửa tài khoản</span>
                <button onClick={() => setShowEditModal(false)} className="p-1"><X size={14} /></button>
              </div>
              <div className="space-y-3 text-xs overflow-y-auto max-h-[60vh] pr-1">
                <div className="flex flex-col gap-1">
                  <label className="font-semibold text-slate-500">Alias ngắn</label>
                  <input type="text" value={modalAccountData.alias || ''} onChange={(e) => setModalAccountData(prev => ({ ...prev, alias: e.target.value }))} className="w-full p-2 bg-[#F2F2F7] dark:bg-[#2C2C2E] text-slate-800 dark:text-white border dark:border-slate-800 rounded-lg font-bold" />
                </div>
                <div className="flex flex-col gap-1">
                  <label className="font-semibold text-slate-500">App / Web</label>
                  <input type="text" value={modalAccountData.appName || ''} onChange={(e) => setModalAccountData(prev => ({ ...prev, appName: e.target.value }))} className="w-full p-2 bg-[#F2F2F7] dark:bg-[#2C2C2E] text-slate-800 dark:text-white border dark:border-slate-800 rounded-lg" />
                </div>
                <div className="flex flex-col gap-1">
                  <label className="font-semibold text-slate-500">Tên đăng ký / Username</label>
                  <input type="text" value={modalAccountData.username || ''} onChange={(e) => setModalAccountData(prev => ({ ...prev, username: e.target.value }))} className="w-full p-2 bg-[#F2F2F7] dark:bg-[#2C2C2E] text-slate-800 dark:text-white border dark:border-slate-800 rounded-lg font-mono" />
                </div>
                <div className="flex flex-col gap-1">
                  <label className="font-semibold text-slate-500">Mật khẩu</label>
                  <div className="flex gap-1">
                    <input type="text" value={modalAccountData.passwordDecrypted || ''} onChange={(e) => setModalAccountData(prev => ({ ...prev, passwordDecrypted: e.target.value }))} className="flex-1 p-2 bg-[#F2F2F7] dark:bg-[#2C2C2E] text-slate-800 dark:text-white border dark:border-slate-800 rounded-lg font-mono font-bold" />
                    <button type="button" onClick={generateRandomModalPassword} className="p-2 bg-slate-100 dark:bg-slate-800 text-[10px] font-bold">tạo</button>
                  </div>
                </div>
                <div className="flex flex-col gap-1">
                  <label className="font-semibold text-slate-500">Mã Authenticator OTP (2FA)</label>
                  <input type="text" value={modalAccountData.otpSecret || ''} onChange={(e) => setModalAccountData(prev => ({ ...prev, otpSecret: e.target.value.replace(/\s+/g, '').toUpperCase() }))} className="w-full p-2 bg-[#F2F2F7] dark:bg-[#2C2C2E] text-slate-800 dark:text-white border dark:border-slate-800 rounded-lg font-mono" />
                </div>
              </div>
              <div className="flex justify-end gap-2 pt-2 border-t dark:border-slate-800">
                <button onClick={() => setShowEditModal(false)} className="px-3 py-1.5 bg-slate-100 dark:bg-slate-800 rounded-lg text-xs font-semibold">Hủy</button>
                <button onClick={() => { handleUpdateAccount(modalAccountData) }} className="px-3 py-1.5 bg-blue-500 text-white rounded-lg text-xs font-semibold">Lưu</button>
              </div>
            </div>
          </div>
        )}
      </div>
    );
  }

  return (
    <div className="h-screen flex text-slate-800 font-sans overflow-hidden bg-[#F5F5F7]">
      
      {/* Sidebar Navigation */}
      <Sidebar
        currentTab={currentTab}
        setCurrentTab={(tab) => {
          setCurrentTab(tab);
          // Set selection filter to match Trash, Favorites tabs or general all accounts
        }}
        selectedCategory={selectedCategory}
        setSelectedCategory={setSelectedCategory}
        accounts={accounts}
        categories={CATEGORIES}
        onAddCategory={() => {
          const newCat = prompt("Nhập tên danh mục mới muốn tạo:");
          if (newCat && newCat.trim()) {
            CATEGORIES.push({
              name: newCat.trim(),
              icon: <Folder size={15} />,
              color: 'text-slate-500'
            });
            alert(`Đã thêm danh mục mới: ${newCat.trim()}`);
          }
        }}
        isOpen={isSidebarOpen}
        onClose={() => setIsSidebarOpen(false)}
        isMinified={isSidebarMinified}
        setIsMinified={setIsSidebarMinified}
      />

      {/* Main Panel Content Window */}
      <div id="main-application-window" className="flex-1 flex flex-col min-w-0 overflow-hidden relative">
        
        {/* Dynamic Inner views router area */}
        <div id="dynamic-content-router" className="flex-1 flex min-w-0 overflow-hidden">
          
          {/* View accounts database (Split Screen layout matching design image side-by-side) */}
          {(currentTab === 'accounts' || currentTab === 'favorites' || currentTab === 'trash') && (
            <>
              <AccountTable
                accounts={accounts}
                selectedCategory={selectedCategory}
                searchQuery={searchQuery}
                setSearchQuery={setSearchQuery}
                selectedAccountId={selectedAccountId}
                onSelectAccount={(account) => selectAndTrackAccount(account.id)}
                onToggleFavorite={handleToggleFavorite}
                onEditAccount={triggerEditModal}
                onDeleteAccount={handleDeleteAccount}
                onAddAccount={() => {
                  setModalAccountData({
                    alias: '',
                    appName: '',
                    username: '',
                    passwordDecrypted: '',
                    alternativePasswords: [],
                    otpSecret: '',
                    notes: '',
                    category: selectedCategory || 'Email'
                  });
                  setShowAddModal(true);
                }}
                activeTab={currentTab}
              />

              <AccountDetail
                account={activeAccount}
                onToggleFavorite={handleToggleFavorite}
                onEditAccount={triggerEditModal}
                onDeleteAccount={handleDeleteAccount}
                onRestoreAccount={handleRestoreAccount}
                onClose={() => setSelectedAccountId(null)}
              />
            </>
          )}

          {/* View Generator */}
          {currentTab === 'generator' && (
            <PasswordGenerator />
          )}

          {/* View Security Audit */}
          {currentTab === 'security' && (
            <SecurityCheckup
              accounts={accounts}
              onSelectAccount={(acc) => selectAndTrackAccount(acc.id)}
              setCurrentTab={setCurrentTab}
            />
          )}

          {/* View 2FA Authenticator */}
          {currentTab === 'authenticator' && (
            <TwoFactorAuthenticator
              accounts={accounts}
              onAddAccount={handleCreateAccount}
              onUpdateAccount={handleUpdateAccount}
            />
          )}

          {/* View sync settings */}
          {currentTab === 'sync' && (
            <SyncAgentManager
              devices={devices}
              logs={logs}
              onTriggerSync={handleSyncDevice}
              onAddDevice={handleAddDevice}
              onDeleteDevice={handleDeleteDevice}
            />
          )}

          {/* View standalone bot simulation */}
          {currentTab === 'telegram' && (
            <div className="flex-1 overflow-y-auto bg-[#F5F5F7] dark:bg-[#1A1A1E] p-6">
              <div className="max-w-6xl mx-auto space-y-6">
                
                {/* Header Title */}
                <div className="text-center max-w-xl mx-auto">
                  <h3 className="font-display font-bold text-2xl text-slate-800 dark:text-slate-100 flex items-center justify-center gap-2">
                    <Bot className="text-sky-500 animate-pulse" size={28} />
                    <span>Tích Hợp Telegram Bot Bảo Mật</span>
                  </h3>
                  <p className="text-xs text-slate-400 mt-2">
                    Cấu hình kết nối với Telegram Bot thật để truy xuất mật khẩu an toàn và mã 2FA từ bất kì chiếc điện thoại nào của bạn.
                  </p>
                </div>

                <div className="grid grid-cols-1 lg:grid-cols-12 gap-6 items-start">
                  
                  {/* Left Column: Cấu hình Bot Thật */}
                  <div className="lg:col-span-5 bg-white dark:bg-[#1C1C1E] p-5 rounded-3xl shadow-[0_8px_30px_rgba(0,0,0,0.03)] border-none space-y-5">
                    <div className="pb-3 border-b border-slate-100/10 dark:border-slate-800/40">
                      <h4 className="font-display font-bold text-sm text-slate-800 dark:text-slate-200 flex items-center gap-1.5">
                        <span className="w-2.5 h-2.5 rounded-full bg-sky-500"></span>
                        <span>Thiết lập Telegram Bot Thật</span>
                      </h4>
                      <p className="text-[11px] text-slate-400 mt-0.5">Nhập Token của bạn từ @BotFather để kích hoạt bot thật hoạt động.</p>
                    </div>

                    <div className="space-y-4">
                      {/* Token Input */}
                      <div className="flex flex-col gap-1.5">
                        <label className="text-xs font-semibold text-slate-600 dark:text-slate-400">Telegram Bot Token</label>
                        <input
                          type="password"
                          placeholder="Tokens (v.v. 123456:ABC-DEF...)"
                          value={telegramBotToken}
                          onChange={(e) => setTelegramBotToken(e.target.value.trim())}
                          className="w-full bg-[#F5F5F7] dark:bg-[#1C1C1E] text-xs px-3 py-2 rounded-xl border border-slate-100 dark:border-slate-700/40 focus:outline-none focus:border-blue-500 font-mono"
                        />
                      </div>

                      {/* Telegram User ID */}
                      <div className="flex flex-col gap-1.5">
                        <label className="text-xs font-semibold text-slate-600 dark:text-slate-400">ID Admin Telegram gửi lệnh</label>
                        <input
                          type="text"
                          placeholder="ID Telegram của bạn (v.v. 610580967)"
                          value={telegramId}
                          onChange={(e) => setTelegramId(e.target.value.replace(/\D/g, ''))}
                          className="w-full bg-[#F5F5F7] dark:bg-[#1C1C1E] text-xs px-3 py-2 rounded-xl border border-slate-100 dark:border-slate-700/40 focus:outline-none focus:border-blue-500 font-mono font-bold"
                        />
                        <span className="text-[10px] text-slate-400 font-medium">Dùng bot @userinfobot trên Telegram để lấy ID của bạn. Chỉ ID này mới có quyền lấy mật khẩu!</span>
                      </div>

                      {/* Mã PIN đồng bộ */}
                      <div className="flex flex-col gap-1.5">
                        <label className="text-xs font-semibold text-slate-600 dark:text-slate-400">Mã PIN xác thực (Đồng bộ)</label>
                        <input
                          type="text"
                          maxLength={6}
                          value={telegramPin}
                          onChange={(e) => setTelegramPin(e.target.value.replace(/\D/g, ''))}
                          className="w-full bg-[#F5F5F7] dark:bg-[#1C1C1E] text-xs px-3 py-2 rounded-xl border border-slate-100 dark:border-slate-700/40 focus:outline-none focus:border-blue-500 font-mono font-bold"
                        />
                      </div>

                      {/* Switch Kích hoạt Real-Bot */}
                      <div className="flex items-center justify-between p-3 rounded-2xl bg-slate-50 dark:bg-[#1C1C1E]/55 border-none">
                        <div>
                          <span className="text-xs font-bold text-slate-700 dark:text-slate-350 block">Kích hoạt Bot Thật</span>
                          <span className="text-[10px] text-slate-400">Khởi chạy Polling qua trình duyệt của bạn</span>
                        </div>
                        <button
                          onClick={() => {
                            if (!telegramBotToken && !isRealBotEnabled) {
                              alert("Vui lòng nhập Telegram Bot Token trước khi kích hoạt!");
                              return;
                            }
                            setIsRealBotEnabled(!isRealBotEnabled);
                          }}
                          className={`w-11 h-6 rounded-full transition-colors relative focus:outline-none cursor-pointer p-0.5 ${isRealBotEnabled ? 'bg-sky-500' : 'bg-slate-300 dark:bg-slate-700'}`}
                        >
                          <span className={`block w-5 h-5 rounded-full bg-white shadow-sm transform transition-transform ${isRealBotEnabled ? 'translate-x-5' : 'translate-x-0'}`}></span>
                        </button>
                      </div>

                      {/* Connection status display */}
                      <div className="flex items-center gap-2 p-3 rounded-2xl border-none text-xs justify-between font-semibold font-sans bg-[#F5F5F7] dark:bg-[#1C1C1E]/30">
                        <span className="text-slate-500">Trạng thái kết nối:</span>
                        {isRealBotEnabled ? (
                          botConnectionStatus === 'connecting' ? (
                            <span className="text-amber-500 flex items-center gap-1">
                              <span className="w-1.5 h-1.5 rounded-full bg-amber-500 animate-ping"></span>
                              Đang kết nối...
                            </span>
                          ) : botConnectionStatus === 'connected' ? (
                            <span className="text-emerald-500 flex items-center gap-1">
                              <span className="w-1.5 h-1.5 rounded-full bg-emerald-500 animate-pulse"></span>
                              Đang trực tuyến (Web Polling)
                            </span>
                          ) : (
                            <span className="text-rose-500 flex items-center gap-1">
                              <span className="w-1.5 h-1.5 rounded-full bg-rose-500 animate-bounce"></span>
                              Lỗi Token / Mất mạng
                            </span>
                          )
                        ) : (
                          <span className="text-slate-400">Chưa kích hoạt bot thật</span>
                        )}
                      </div>

                    </div>

                    {/* Hướng dẫn sử dụng */}
                    <div className="bg-sky-50/50 dark:bg-[#1a3a54]/10 border-none p-3.5 rounded-2xl space-y-2 mb-2">
                      <h5 className="text-[11px] font-bold text-sky-700 dark:text-sky-300 uppercase tracking-wide">💡 CÁCH HOẠT ĐỘNG BOT TRONG 1 PHÚT:</h5>
                      <ol className="text-[10px] text-slate-500 dark:text-slate-400 space-y-1.5 list-decimal list-inside leading-relaxed">
                        <li>Chat với robot <a href="https://t.me/BotFather" target="_blank" rel="noreferrer" className="text-sky-600 font-bold hover:underline">@BotFather</a> trên Telegram để tạo Bot mới và copy chuỗi <strong>Token</strong>.</li>
                        <li>Lấy ID Telegram của bạn từ bot <a href="https://t.me/userinfobot" target="_blank" rel="noreferrer" className="text-sky-600 font-bold hover:underline">@userinfobot</a>.</li>
                        <li>Dán thông tin vào form trên và bật <strong>Kích hoạt Bot Thật</strong>.</li>
                        <li>Bấm vào liên kết bot của bạn trên điện thoại, gõ lệnh <code>/start</code> và sau đó gõ <code>/get gg</code> để truy vấn mật khẩu bảo mật thử nghiệm!</li>
                      </ol>
                    </div>

                  </div>

                  {/* Right Column: Simulator View */}
                  <div className="lg:col-span-7 flex flex-col justify-center items-center">
                    <TelegramSimulator
                      accounts={accounts}
                      telegramPin={telegramPin}
                      telegramId={telegramId}
                      messages={telegramMessages}
                      setMessages={setTelegramMessages}
                      pinVerifiedToday={simulatorPinVerified}
                      setPinVerifiedToday={setSimulatorPinVerified}
                      onSendMessage={handleSimulatorSendMessage}
                      realBotEnabled={isRealBotEnabled}
                      currentStep={simulatorStep}
                      setCurrentStep={setSimulatorStep}
                      activeWorkflowAccount={simulatorTargetAccount}
                      setActiveWorkflowAccount={setSimulatorTargetAccount}
                      lastListedAccounts={simulatorLastListedAccounts}
                      setLastListedAccounts={setSimulatorLastListedAccounts}
                      frequentAccounts={frequentAccounts}
                      onIncrementAccess={incrementAccessCount}
                    />
                  </div>

                </div>

              </div>
            </div>
          )}

          {/* CSV Import simulation tab */}
          {currentTab === 'import' && (
            <SmartImporter 
              categories={CATEGORIES.map(c => c.name)}
              onImportSuccess={(newAccounts) => {
                newAccounts.forEach(acc => {
                  handleCreateAccount(acc);
                });
                setCurrentTab('accounts');
              }}
            />
          )}

          {/* Export simulation tab */}
          {currentTab === 'export' && (
            <div className="flex-1 overflow-y-auto bg-[#F5F5F7] dark:bg-[#1A1A1E] p-8 flex flex-col items-center justify-center select-none">
              <div className="max-w-md bg-white dark:bg-[#1C1C1E] p-6 rounded-2xl shadow-[0_8px_30px_rgba(0,0,0,0.03)] border-none space-y-4">
                <h3 className="font-display font-bold text-base text-slate-800 dark:text-slate-200">
                  Xuất dữ liệu an toàn (Mã hóa / Bản thô)
                </h3>
                <p className="text-xs text-slate-400 leading-relaxed">
                  Xuất tất cả mật khẩu và mã OTP 2FA hiện tại ra tệp nén ZIP hoặc CSV thô phục vụ mục đích sao lưu dự phòng ngoại tuyến.
                </p>

                <div className="grid grid-cols-1 md:grid-cols-2 gap-3 pb-2">
                  <div className="bg-slate-50 dark:bg-[#2C2C2E]/40 border-none p-4 rounded-xl text-center space-y-2">
                    <span className="text-xl block">📄</span>
                    <h5 className="text-xs font-bold text-slate-700 dark:text-slate-200">Xuất CSV Bản Thô</h5>
                    <p className="text-[10px] text-slate-400">Tải về danh sách mật khẩu dạng bảng thô, lý tưởng để nạp sang các phần mềm khác.</p>
                  </div>
                  <div className="bg-slate-50 dark:bg-[#2C2C2E]/40 border-none p-4 rounded-xl text-center space-y-2">
                    <span className="text-xl block">🔐</span>
                    <h5 className="text-xs font-bold text-slate-700 dark:text-slate-200">Xuất Backup Mã hóa</h5>
                    <p className="text-[10px] text-slate-400">Xuất toàn bộ cơ sở dữ liệu đã được khóa AES-256 an toàn bằng Master Key.</p>
                  </div>
                </div>

                <div className="flex justify-between items-center bg-amber-50/60 dark:bg-amber-950/20 border-none p-3 rounded-xl">
                  <span className="text-amber-600 dark:text-amber-400 block text-xs leading-relaxed font-medium">
                    ⚠️ <strong>Khuyến nghị bảo mật:</strong> Bản thô CSV không chứa mã hóa. Tránh truyền tải tệp tin này qua internet công cộng hoặc mạng không an toàn.
                  </span>
                </div>

                <div className="flex justify-end pt-2">
                  <button 
                    onClick={() => {
                      alert("Tải tệp ZIP backup thành công! Toàn bộ 49 tài khoản lưu trữ được nén an toàn.");
                    }}
                    className="px-5 py-2.5 bg-[#007AFF] hover:bg-[#0063E1] dark:bg-[#0A84FF] dark:hover:bg-[#0072E3] text-white rounded-lg text-xs font-semibold cursor-pointer shadow-sm active:scale-[0.98] transition-all"
                  >
                    Tải về tệp sao lưu ngay
                  </button>
                </div>
              </div>
            </div>
          )}

          {/* Settings tab layout */}
          {currentTab === 'settings' && (
            <div className="flex-1 overflow-y-auto bg-[#F5F5F7] dark:bg-[#1A1A1E] p-8 select-none">
              <div className="max-w-2xl bg-white dark:bg-[#1C1C1E] rounded-3xl p-6 shadow-[0_8px_30px_rgba(0,0,0,0.03)] border-none space-y-6">
                <div>
                  <h3 className="font-display font-bold text-base text-slate-800 dark:text-slate-100">Cấu hình Hệ thống & Master Key</h3>
                  <p className="text-xs text-slate-400 mt-1">Quản lý nâng cao an toàn cơ chế mã hóa giải mã Master Key AES và tích hợp API Vault Bot.</p>
                </div>

                <hr className="border-slate-150/10 dark:border-slate-800/40" />

                <div className="space-y-4">
                  <div className="flex flex-col gap-1.5">
                    <label className="text-xs font-semibold text-slate-700 dark:text-zinc-350 pl-0.5">Biến môi trường Master Key giải mã</label>
                    <input
                      type="password"
                      disabled
                      value="********************************************************"
                      className="bg-[#F2F2F7] dark:bg-[#2C2C2E]/60 p-2.5 px-3.5 rounded-lg border border-[#E5E5EA] dark:border-[#3A3A3C] font-mono text-xs select-none cursor-not-allowed opacity-65 text-slate-400 dark:text-zinc-500"
                    />
                    <span className="text-[10px] text-slate-400 dark:text-zinc-500 mt-0.5">Khóa giải mã AES được nạp tự động qua biến hệ thống Windows nền. Không cho phép hiển thị plaintext.</span>
                  </div>

                  <div className="flex flex-col gap-1.5">
                    <label className="text-xs font-semibold text-slate-700 dark:text-zinc-350 pl-0.5">API Callback Path</label>
                    <input
                      type="text"
                      disabled
                      value="https://localhost:3000/api/v1/sync/post"
                      className="bg-[#F2F2F7] dark:bg-[#2C2C2E]/60 p-2.5 px-3.5 rounded-lg border border-[#E5E5EA] dark:border-[#3A3A3C] font-mono text-xs select-none cursor-not-allowed opacity-65 text-slate-400 dark:text-zinc-500"
                    />
                    <span className="text-[10px] text-slate-400 dark:text-zinc-500 mt-0.5">Endpoint nạp dữ liệu HTTP POST dành cho trình ứng dụng Sync AgentWindows nền.</span>
                  </div>
                </div>
              </div>
            </div>
          )}

        </div>
      </div>

      {/* FLOAT WIDGET BOT TELEGRAM: cực kỳ tiện ích cho phép toggle chat nhanh ở góc dưới màn hình */}
      <div id="float-telegram-integration" className="fixed bottom-20 right-6 z-[60]">
        {!showTelegramFloat ? (
          <button
            id="btn-open-float-telegram"
            onClick={() => setShowTelegramFloat(true)}
            className="w-14 h-14 rounded-full bg-[#1c7bb6] hover:bg-blue-600 text-white flex items-center justify-center shadow-lg shadow-[#1c7bb6]/35 active:scale-95 transition-all cursor-pointer relative group p-0 overflow-hidden"
            title="Mở giả lập Telegram Bot"
          >
            <img 
              src="https://images.unsplash.com/photo-1517841905240-472988babdf9?auto=format&fit=crop&w=120&h=120&q=80"
              alt="Vault Bot Avatar"
              referrerPolicy="no-referrer"
              className="w-full h-full object-cover rounded-full"
            />
            <span className="absolute right-0.5 top-0.5 w-3.5 h-3.5 rounded-full bg-emerald-500 border-2 border-white dark:border-slate-900"></span>
            
            {/* Tooltip hint pop-up */}
            <span className="absolute right-16 py-1 px-2.5 whitespace-nowrap bg-slate-900 border border-slate-800 rounded-lg text-[10px] font-bold text-white shadow-md opacity-0 group-hover:opacity-100 transition-opacity pointer-events-none duration-150">
              Nhấp để Chat Telegram Bot 🤖
            </span>
          </button>
        ) : (
          <div className="w-[340px] sm:w-[380px] bg-white dark:bg-slate-900 rounded-3xl overflow-hidden shadow-2xl shadow-blue-500/10 border border-slate-200 dark:border-slate-800 flex flex-col h-[520px] transition-all duration-300">
            
            {/* Minimal Header with close button */}
            <div className="bg-slate-100 dark:bg-slate-900 px-3.5 py-2 border-b border-slate-150 dark:border-slate-800 flex items-center justify-between select-none shrink-0 text-xs font-semibold text-slate-500">
              <span className="flex items-center gap-1.5 font-bold">
                <img 
                  src="https://images.unsplash.com/photo-1517841905240-472988babdf9?auto=format&fit=crop&w=40&h=40&q=80"
                  alt="Tiny Vault Bot Avatar"
                  referrerPolicy="no-referrer"
                  className="w-4.5 h-4.5 rounded-full object-cover"
                />
                <span>Mô phỏng Vault Bot Telegram</span>
              </span>
              <button
                onClick={() => setShowTelegramFloat(false)}
                className="p-1 hover:bg-slate-200 dark:hover:bg-slate-800 rounded-lg text-slate-400 hover:text-slate-600 transition-colors cursor-pointer"
              >
                <X size={14} />
              </button>
            </div>

            {/* Telegram simulated core display */}
            <div className="flex-1 overflow-hidden flex flex-col pt-1 bg-slate-50 dark:bg-slate-950">
              <TelegramSimulator
                accounts={accounts}
                telegramPin={telegramPin}
                telegramId={telegramId}
                messages={telegramMessages}
                setMessages={setTelegramMessages}
                pinVerifiedToday={simulatorPinVerified}
                setPinVerifiedToday={setSimulatorPinVerified}
                onSendMessage={handleSimulatorSendMessage}
                realBotEnabled={isRealBotEnabled}
              />
            </div>
          </div>
        )}
      </div>

      {/* MODAL GIAO DIÊN: THÊM TÀI KHOẢN MỚI */}
      {showAddModal && (
        <div id="add-modal-overlay" className="fixed inset-0 z-40 bg-slate-950/40 backdrop-blur-xs flex items-center justify-center p-4">
          <div id="add-modal-box" className="w-full max-w-lg bg-white dark:bg-[#1C1C1E] rounded-3xl p-6 space-y-4 shadow-[0_24px_60px_rgba(0,0,0,0.15)] border-none animate-in zoom-in-95 duration-150">
            <div className="flex items-center justify-between border-b border-slate-100 dark:border-slate-800/80 pb-3">
              <h3 className="font-display font-bold text-base text-slate-800 dark:text-slate-100 flex items-center gap-2">
                <Plus size={18} className="text-blue-600" />
                <span>Thêm tài khoản lưu trữ mới</span>
              </h3>
              <button onClick={() => setShowAddModal(false)} className="p-1 hover:bg-slate-100 dark:hover:bg-slate-800 rounded-lg text-slate-400">
                <X size={16} />
              </button>
            </div>

            <div className="grid grid-cols-2 gap-4">
              <div className="flex flex-col gap-1.5">
                <label className="text-[11px] font-semibold text-slate-500 dark:text-zinc-400 uppercase tracking-wider pl-1">Alias ngắn gọn</label>
                <input
                  type="text"
                  placeholder="v.v gg-main, fb-work"
                  value={modalAccountData.alias}
                  onChange={(e) => setModalAccountData(prev => ({ ...prev, alias: e.target.value }))}
                  className="bg-[#F2F2F7] dark:bg-[#2C2C2E] text-xs px-3.5 py-2.5 rounded-lg border border-[#E5E5EA] dark:border-[#3A3A3C] focus:bg-white dark:focus:bg-[#1C1C1E] focus:outline-none focus:ring-2 focus:ring-[#007AFF]/35 dark:focus:ring-[#0A84FF]/35 transition-all text-slate-800 dark:text-zinc-100 font-sans placeholder-slate-400/80 dark:placeholder-zinc-500"
                />
              </div>

              <div className="flex flex-col gap-1.5">
                <label className="text-[11px] font-semibold text-slate-500 dark:text-zinc-400 uppercase tracking-wider pl-1">Website / Tên App</label>
                <input
                  type="text"
                  placeholder="v.v accounts.google.com"
                  value={modalAccountData.appName}
                  onChange={(e) => setModalAccountData(prev => ({ ...prev, appName: e.target.value }))}
                  className="bg-[#F2F2F7] dark:bg-[#2C2C2E] text-xs px-3.5 py-2.5 rounded-lg border border-[#E5E5EA] dark:border-[#3A3A3C] focus:bg-white dark:focus:bg-[#1C1C1E] focus:outline-none focus:ring-2 focus:ring-[#007AFF]/35 dark:focus:ring-[#0A84FF]/35 transition-all text-slate-800 dark:text-zinc-100 font-sans placeholder-slate-400/80 dark:placeholder-zinc-500"
                />
              </div>

              <div className="flex flex-col gap-1.5">
                <label className="text-[11px] font-semibold text-slate-500 dark:text-zinc-400 uppercase tracking-wider pl-1">Tên đăng nhập (Username)</label>
                <input
                  type="text"
                  placeholder="v.v hathanhtu711@gmail.com"
                  value={modalAccountData.username}
                  onChange={(e) => setModalAccountData(prev => ({ ...prev, username: e.target.value }))}
                  className="bg-[#F2F2F7] dark:bg-[#2C2C2E] text-xs px-3.5 py-2.5 rounded-lg border border-[#E5E5EA] dark:border-[#3A3A3C] focus:bg-white dark:focus:bg-[#1C1C1E] focus:outline-none focus:ring-2 focus:ring-[#007AFF]/35 dark:focus:ring-[#0A84FF]/35 transition-all text-slate-800 dark:text-zinc-100 font-mono placeholder-slate-400/80 dark:placeholder-zinc-500"
                />
              </div>

              <div className="flex flex-col gap-1.5">
                <label className="text-[11px] font-semibold text-slate-500 dark:text-zinc-400 uppercase tracking-wider pl-1">Danh mục</label>
                <select
                  value={modalAccountData.category}
                  onChange={(e) => setModalAccountData(prev => ({ ...prev, category: e.target.value }))}
                  className="bg-[#F2F2F7] dark:bg-[#2C2C2E] text-xs px-3.5 py-2.5 rounded-lg border border-[#E5E5EA] dark:border-[#3A3A3C] focus:bg-white dark:focus:bg-[#1C1C1E] focus:outline-none focus:ring-2 focus:ring-[#007AFF]/35 dark:focus:ring-[#0A84FF]/35 transition-all text-slate-800 dark:text-zinc-100 cursor-pointer appearance-none"
                >
                  {CATEGORIES.map(cat => (
                    <option key={cat.name} value={cat.name}>{cat.name}</option>
                  ))}
                </select>
              </div>

              <div className="col-span-2 flex flex-col gap-1.5">
                <label className="text-[11px] font-semibold text-slate-500 dark:text-zinc-400 uppercase tracking-wider pl-1">Mật khẩu lưu trữ</label>
                <div className="flex gap-2">
                  <input
                    type="text"
                    placeholder="Nhập mật khẩu của tài khoản..."
                    value={modalAccountData.passwordDecrypted}
                    onChange={(e) => setModalAccountData(prev => ({ ...prev, passwordDecrypted: e.target.value }))}
                    className="flex-1 bg-[#F2F2F7] dark:bg-[#2C2C2E] text-xs px-3.5 py-2.5 rounded-lg border border-[#E5E5EA] dark:border-[#3A3A3C] focus:bg-white dark:focus:bg-[#1C1C1E] focus:outline-none focus:ring-2 focus:ring-[#007AFF]/35 dark:focus:ring-[#0A84FF]/35 transition-all text-slate-800 dark:text-zinc-100 font-mono placeholder-slate-400/80 dark:placeholder-zinc-500"
                  />
                  <button
                    type="button"
                    onClick={generateRandomModalPassword}
                    className="px-4 bg-slate-100 dark:bg-[#2C2C2E]/80 hover:bg-slate-200/80 dark:hover:bg-[#3A3A3C] text-slate-700 dark:text-zinc-100 rounded-lg text-xs font-bold transition-all shrink-0 flex items-center gap-1.5 active:scale-[0.98]"
                    title="Tạo nhanh mật khẩu ngẫu nhiên"
                  >
                    <RotateCw size={12} className="text-[#007AFF]" />
                    <span>Tạo ngẫu nhiên</span>
                  </button>
                </div>
              </div>

              <div className="col-span-2 flex flex-col gap-1.5">
                <label className="text-[11px] font-semibold text-slate-500 dark:text-zinc-400 uppercase tracking-wider pl-1 flex items-center gap-1">
                  <span className="text-amber-600 dark:text-amber-500">Các mật khẩu phụ / dự phòng khác</span>
                  <span className="text-[10px] text-slate-400 dark:text-zinc-500 font-normal normal-case">(cách nhau bằng dấu phẩy)</span>
                </label>
                <textarea
                  placeholder="v.v. pass_backup_1, old_password_123"
                  value={modalAccountData.alternativePasswords ? modalAccountData.alternativePasswords.join('\n') : ''}
                  onChange={(e) => setModalAccountData(prev => ({ 
                    ...prev, 
                    alternativePasswords: e.target.value.split(/[\n,]+/).map(s => s.trim()).filter(Boolean) 
                  }))}
                  rows={2}
                  className="bg-[#F2F2F7] dark:bg-[#2C2C2E] text-xs px-3.5 py-2.5 rounded-lg border border-[#E5E5EA] dark:border-[#3A3A3C] focus:bg-white dark:focus:bg-[#1C1C1E] focus:outline-none focus:ring-2 focus:ring-[#007AFF]/35 dark:focus:ring-[#0A84FF]/35 transition-all text-slate-800 dark:text-zinc-100 font-mono resize-none leading-relaxed placeholder-slate-400/80 dark:placeholder-zinc-500"
                />
              </div>

              <div className="col-span-2 flex flex-col gap-1.5">
                <label className="text-[11px] font-semibold text-slate-500 dark:text-zinc-400 uppercase tracking-wider pl-1">Khóa bí mật OTP Secret-Key (Mã hóa 2FA - tùy chọn)</label>
                <input
                  type="text"
                  placeholder="v.v JBSWY3DPEHPK3PXP"
                  value={modalAccountData.otpSecret}
                  onChange={(e) => setModalAccountData(prev => ({ ...prev, otpSecret: e.target.value.replace(/\s+/g, '').toUpperCase() }))}
                  className="bg-[#F2F2F7] dark:bg-[#2C2C2E] text-xs px-3.5 py-2.5 rounded-lg border border-[#E5E5EA] dark:border-[#3A3A3C] focus:bg-white dark:focus:bg-[#1C1C1E] focus:outline-none focus:ring-2 focus:ring-[#007AFF]/35 dark:focus:ring-[#0A84FF]/35 transition-all text-slate-800 dark:text-zinc-100 font-mono placeholder-slate-400/80 dark:placeholder-zinc-500"
                />
              </div>

              <div className="col-span-2 flex flex-col gap-1.5">
                <label className="text-[11px] font-semibold text-slate-500 dark:text-zinc-400 uppercase tracking-wider pl-1">Ghi chú bổ sung</label>
                <textarea
                  placeholder="Cung cấp các lưu ý đặc biệt, thông tin phục hồi..."
                  value={modalAccountData.notes}
                  onChange={(e) => setModalAccountData(prev => ({ ...prev, notes: e.target.value }))}
                  rows={2}
                  className="bg-[#F2F2F7] dark:bg-[#2C2C2E] text-xs px-3.5 py-2.5 rounded-lg border border-[#E5E5EA] dark:border-[#3A3A3C] focus:bg-white dark:focus:bg-[#1C1C1E] focus:outline-none focus:ring-2 focus:ring-[#007AFF]/35 dark:focus:ring-[#0A84FF]/35 transition-all text-slate-800 dark:text-zinc-100 font-sans resize-none placeholder-slate-400/80 dark:placeholder-zinc-500"
                />
              </div>
            </div>

            <div className="flex justify-end gap-2.5 pt-4 border-t border-slate-100 dark:border-slate-800/40">
              <button
                onClick={() => setShowAddModal(false)}
                className="px-4 py-2 bg-slate-100 dark:bg-[#2C2C2E] hover:bg-slate-200 dark:hover:bg-[#3A3A3C] text-slate-700 dark:text-zinc-200 rounded-lg text-xs font-semibold transition-all cursor-pointer active:scale-[0.98]"
              >
                Hủy
              </button>
              <button
                onClick={() => handleCreateAccount(modalAccountData)}
                className="px-5 py-2 bg-[#007AFF] hover:bg-[#0063E1] dark:bg-[#0A84FF] dark:hover:bg-[#0072E3] text-white rounded-lg text-xs font-semibold shadow-sm cursor-pointer active:scale-[0.98] transition-all"
              >
                Tạo tài khoản mới
              </button>
            </div>
          </div>
        </div>
      )}

      {/* MODAL GIAO DIỆN: CHỈNH SỬA TÀI KHOẢN (EDIT) */}
      {showEditModal && (
        <div id="edit-modal-overlay" className="fixed inset-0 z-40 bg-slate-950/40 backdrop-blur-xs flex items-center justify-center p-4">
          <div id="edit-modal-box" className="w-full max-w-lg bg-white dark:bg-[#1C1C1E] rounded-3xl p-6 space-y-4 shadow-[0_24px_60px_rgba(0,0,0,0.15)] border-none animate-in zoom-in-95 duration-150">
            <div className="flex items-center justify-between border-b border-slate-100 dark:border-slate-800/80 pb-3">
              <h3 className="font-display font-bold text-base text-slate-800 dark:text-slate-100 flex items-center gap-2">
                <Sparkles size={18} className="text-amber-500" />
                <span>Chỉnh sửa thông tin tài khoản</span>
              </h3>
              <button onClick={() => setShowEditModal(false)} className="p-1 hover:bg-slate-100 dark:hover:bg-slate-800 rounded-lg text-slate-400">
                <X size={16} />
              </button>
            </div>

            <div className="grid grid-cols-2 gap-4">
              <div className="flex flex-col gap-1.5">
                <label className="text-[11px] font-semibold text-slate-500 dark:text-zinc-400 uppercase tracking-wider pl-1">Alias ngắn gọn</label>
                <input
                  type="text"
                  value={modalAccountData.alias}
                  onChange={(e) => setModalAccountData(prev => ({ ...prev, alias: e.target.value }))}
                  className="bg-[#F2F2F7] dark:bg-[#2C2C2E] text-xs px-3.5 py-2.5 rounded-lg border border-[#E5E5EA] dark:border-[#3A3A3C] focus:bg-white dark:focus:bg-[#1C1C1E] focus:outline-none focus:ring-2 focus:ring-[#007AFF]/35 dark:focus:ring-[#0A84FF]/35 transition-all text-slate-800 dark:text-zinc-100 font-sans"
                />
              </div>

              <div className="flex flex-col gap-1.5">
                <label className="text-[11px] font-semibold text-slate-500 dark:text-zinc-400 uppercase tracking-wider pl-1">Website / Tên App</label>
                <input
                  type="text"
                  value={modalAccountData.appName}
                  onChange={(e) => setModalAccountData(prev => ({ ...prev, appName: e.target.value }))}
                  className="bg-[#F2F2F7] dark:bg-[#2C2C2E] text-xs px-3.5 py-2.5 rounded-lg border border-[#E5E5EA] dark:border-[#3A3A3C] focus:bg-white dark:focus:bg-[#1C1C1E] focus:outline-none focus:ring-2 focus:ring-[#007AFF]/35 dark:focus:ring-[#0A84FF]/35 transition-all text-slate-800 dark:text-zinc-100 font-sans"
                />
              </div>

              <div className="flex flex-col gap-1.5">
                <label className="text-[11px] font-semibold text-slate-500 dark:text-zinc-400 uppercase tracking-wider pl-1">Tên đăng nhập (Username)</label>
                <input
                  type="text"
                  value={modalAccountData.username}
                  onChange={(e) => setModalAccountData(prev => ({ ...prev, username: e.target.value }))}
                  className="bg-[#F2F2F7] dark:bg-[#2C2C2E] text-xs px-3.5 py-2.5 rounded-lg border border-[#E5E5EA] dark:border-[#3A3A3C] focus:bg-white dark:focus:bg-[#1C1C1E] focus:outline-none focus:ring-2 focus:ring-[#007AFF]/35 dark:focus:ring-[#0A84FF]/35 transition-all text-slate-800 dark:text-zinc-100 font-mono"
                />
              </div>

              <div className="flex flex-col gap-1.5">
                <label className="text-[11px] font-semibold text-slate-500 dark:text-zinc-400 uppercase tracking-wider pl-1">Danh mục</label>
                <select
                  value={modalAccountData.category}
                  onChange={(e) => setModalAccountData(prev => ({ ...prev, category: e.target.value }))}
                  className="bg-[#F2F2F7] dark:bg-[#2C2C2E] text-xs px-3.5 py-2.5 rounded-lg border border-[#E5E5EA] dark:border-[#3A3A3C] focus:bg-white dark:focus:bg-[#1C1C1E] focus:outline-none focus:ring-2 focus:ring-[#007AFF]/35 dark:focus:ring-[#0A84FF]/35 transition-all text-slate-800 dark:text-zinc-100 cursor-pointer appearance-none"
                >
                  {CATEGORIES.map(cat => (
                    <option key={cat.name} value={cat.name}>{cat.name}</option>
                  ))}
                </select>
              </div>

              <div className="col-span-2 flex flex-col gap-1.5">
                <div className="flex justify-between items-center pl-1">
                  <label className="text-[11px] font-semibold text-slate-500 dark:text-zinc-400 uppercase tracking-wider">Mật khẩu hiện hành</label>
                  <span className="text-[10px] text-amber-600 dark:text-amber-500 font-semibold block">⚠️ Tạo một bản ghi mật khẩu cũ ở lịch sử</span>
                </div>
                <div className="flex gap-2">
                  <input
                    type="text"
                    value={modalAccountData.passwordDecrypted}
                    onChange={(e) => setModalAccountData(prev => ({ ...prev, passwordDecrypted: e.target.value }))}
                    className="flex-1 bg-[#F2F2F7] dark:bg-[#2C2C2E] text-xs px-3.5 py-2.5 rounded-lg border border-[#E5E5EA] dark:border-[#3A3A3C] focus:bg-white dark:focus:bg-[#1C1C1E] focus:outline-none focus:ring-2 focus:ring-[#007AFF]/35 dark:focus:ring-[#0A84FF]/35 transition-all text-slate-800 dark:text-zinc-100 font-mono font-bold"
                  />
                  <button
                    type="button"
                    onClick={generateRandomModalPassword}
                    className="px-4 bg-slate-100 dark:bg-[#2C2C2E]/80 hover:bg-slate-200/80 dark:hover:bg-[#3A3A3C] text-slate-700 dark:text-zinc-100 rounded-lg text-xs font-bold transition-all shrink-0 flex items-center gap-1.5 active:scale-[0.98]"
                  >
                    <RotateCw size={12} className="text-[#007AFF]" />
                    <span>Tạo mới</span>
                  </button>
                </div>
              </div>

              <div className="col-span-2 flex flex-col gap-1.5">
                <label className="text-[11px] font-semibold text-slate-500 dark:text-zinc-400 uppercase tracking-wider pl-1 flex items-center gap-1">
                  <span className="text-amber-600 dark:text-amber-550">Các mật khẩu phụ / dự phòng khác</span>
                  <span className="text-[10px] text-slate-400 dark:text-zinc-500 font-normal normal-case">(cách nhau bằng dấu phẩy)</span>
                </label>
                <textarea
                  placeholder="v.v. pass_backup_1, old_password_123"
                  value={modalAccountData.alternativePasswords ? modalAccountData.alternativePasswords.join('\n') : ''}
                  onChange={(e) => setModalAccountData(prev => ({ 
                    ...prev, 
                    alternativePasswords: e.target.value.split(/[\n,]+/).map(s => s.trim()).filter(Boolean) 
                  }))}
                  rows={2}
                  className="bg-[#F2F2F7] dark:bg-[#2C2C2E] text-xs px-3.5 py-2.5 rounded-lg border border-[#E5E5EA] dark:border-[#3A3A3C] focus:bg-white dark:focus:bg-[#1C1C1E] focus:outline-none focus:ring-2 focus:ring-[#007AFF]/35 dark:focus:ring-[#0A84FF]/35 transition-all text-slate-800 dark:text-zinc-100 font-mono resize-none leading-relaxed placeholder-slate-400/80 dark:placeholder-zinc-500"
                />
              </div>

              <div className="col-span-2 flex flex-col gap-1.5">
                <label className="text-[11px] font-semibold text-slate-500 dark:text-zinc-400 uppercase tracking-wider pl-1">Khóa bí mật OTP Secret-Key (Mã hóa 2FA - tùy chọn)</label>
                <input
                  type="text"
                  value={modalAccountData.otpSecret}
                  onChange={(e) => setModalAccountData(prev => ({ ...prev, otpSecret: e.target.value.replace(/\s+/g, '').toUpperCase() }))}
                  className="bg-[#F2F2F7] dark:bg-[#2C2C2E] text-xs px-3.5 py-2.5 rounded-lg border border-[#E5E5EA] dark:border-[#3A3A3C] focus:bg-white dark:focus:bg-[#1C1C1E] focus:outline-none focus:ring-2 focus:ring-[#007AFF]/35 dark:focus:ring-[#0A84FF]/35 transition-all text-slate-800 dark:text-zinc-100 font-mono"
                />
              </div>

              <div className="col-span-2 flex flex-col gap-1.5">
                <label className="text-[11px] font-semibold text-slate-500 dark:text-zinc-400 uppercase tracking-wider pl-1">Ghi chú bổ sung</label>
                <textarea
                  value={modalAccountData.notes}
                  onChange={(e) => setModalAccountData(prev => ({ ...prev, notes: e.target.value }))}
                  rows={2}
                  className="bg-[#F2F2F7] dark:bg-[#2C2C2E] text-xs px-3.5 py-2.5 rounded-lg border border-[#E5E5EA] dark:border-[#3A3A3C] focus:bg-white dark:focus:bg-[#1C1C1E] focus:outline-none focus:ring-2 focus:ring-[#007AFF]/35 dark:focus:ring-[#0A84FF]/35 transition-all text-slate-800 dark:text-zinc-100 font-sans resize-none"
                />
              </div>
            </div>

            <div className="flex justify-end gap-2.5 pt-4 border-t border-slate-100 dark:border-slate-800/40">
              <button
                onClick={() => setShowEditModal(false)}
                className="px-4 py-2 bg-slate-100 dark:bg-[#2C2C2E] hover:bg-slate-200 dark:hover:bg-[#3A3A3C] text-slate-700 dark:text-zinc-200 rounded-lg text-xs font-semibold transition-all cursor-pointer active:scale-[0.98]"
              >
                Hủy
              </button>
              <button
                onClick={() => handleUpdateAccount(modalAccountData)}
                className="px-5 py-2 bg-[#007AFF] hover:bg-[#0063E1] dark:bg-[#0A84FF] dark:hover:bg-[#0072E3] text-white rounded-lg text-xs font-semibold shadow-sm cursor-pointer active:scale-[0.98] transition-all"
              >
                Lưu thay đổi
              </button>
            </div>
          </div>
        </div>
      )}

    </div>
  );
}
