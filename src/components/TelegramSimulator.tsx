/**
 * @license
 * SPDX-License-Identifier: Apache-2.0
 */

import React from 'react';
import { 
  Bot, 
  Send, 
  User, 
  Clock, 
  Check, 
  AlertTriangle, 
  Lock, 
  Trash2, 
  ArrowLeft, 
  Info,
  ShieldCheck,
  Smartphone,
  Settings,
  Globe,
  Search,
  Key,
  RefreshCw,
  X,
  ChevronRight,
  Copy,
  LockKeyhole
} from 'lucide-react';
import { Account } from '../types';

interface TelegramSimulatorProps {
  accounts: Account[];
  telegramPin: string;
  telegramId: string;
  messages?: ChatMessage[];
  setMessages?: React.Dispatch<React.SetStateAction<ChatMessage[]>>;
  pinVerifiedToday?: boolean;
  setPinVerifiedToday?: (val: boolean) => void;
  onSendMessage?: (text: string) => void;
  realBotEnabled?: boolean;
  currentStep?: 'none' | 'showing_matches' | 'confirm_view' | 'waiting_pin';
  setCurrentStep?: (val: 'none' | 'showing_matches' | 'confirm_view' | 'waiting_pin') => void;
  activeWorkflowAccount?: Account | null;
  setActiveWorkflowAccount?: (acc: Account | null) => void;
  lastListedAccounts?: Account[];
  setLastListedAccounts?: (accs: Account[]) => void;
  frequentAccounts?: Account[];
  onIncrementAccess?: (id: string) => void;
}

export interface ChatMessage {
  id: string;
  sender: 'user' | 'bot';
  text: string;
  time: string;
  buttons?: { text: string; action: () => void; variant?: 'success' | 'danger' | 'primary' }[];
  isDestructive?: boolean;
  isSelfDestroyed?: boolean;
  destructionSeconds?: number;
}

export default function TelegramSimulator({
  accounts,
  telegramPin,
  telegramId,
  messages: propMessages,
  setMessages: propSetMessages,
  pinVerifiedToday: propPinVerified,
  setPinVerifiedToday: propSetPinVerified,
  onSendMessage,
  realBotEnabled = false,
  currentStep: propCurrentStep,
  setCurrentStep: propSetCurrentStep,
  activeWorkflowAccount: propActiveWorkflowAccount,
  setActiveWorkflowAccount: propSetActiveWorkflowAccount,
  lastListedAccounts: propLastListedAccounts,
  setLastListedAccounts: propSetLastListedAccounts,
  frequentAccounts = [],
  onIncrementAccess
}: TelegramSimulatorProps) {
  const [localMessages, setLocalMessages] = React.useState<ChatMessage[]>([]);
  const messages = propMessages !== undefined ? propMessages : localMessages;
  const setMessages = propSetMessages !== undefined ? propSetMessages : setLocalMessages;

  const [localPinVerified, setLocalPinVerified] = React.useState(false);
  const pinVerifiedToday = propPinVerified !== undefined ? propPinVerified : localPinVerified;
  const setPinVerifiedToday = propSetPinVerified !== undefined ? propSetPinVerified : setLocalPinVerified;

  const [inputText, setInputText] = React.useState('');

  const [localActiveAccount, setLocalActiveAccount] = React.useState<Account | null>(null);
  const activeWorkflowAccount = propActiveWorkflowAccount !== undefined ? propActiveWorkflowAccount : localActiveAccount;
  const setActiveWorkflowAccount = propSetActiveWorkflowAccount !== undefined ? propSetActiveWorkflowAccount : setLocalActiveAccount;

  const [localStep, setLocalStep] = React.useState<'none' | 'showing_matches' | 'confirm_view' | 'waiting_pin'>('none');
  const currentStep = propCurrentStep !== undefined ? propCurrentStep : localStep;
  const setCurrentStep = propSetCurrentStep !== undefined ? propSetCurrentStep : setLocalStep;

  const [localLastListed, setLocalLastListed] = React.useState<Account[]>([]);
  const lastListedAccounts = propLastListedAccounts !== undefined ? propLastListedAccounts : localLastListed;
  const setLastListedAccounts = propSetLastListedAccounts !== undefined ? propSetLastListedAccounts : setLocalLastListed;

  // subkeyboards & Mini App states
  const [submenu, setSubmenu] = React.useState<'main' | 'settings'>('main');
  const [showMiniApp, setShowMiniApp] = React.useState(false);
  const [miniAppSearchQuery, setMiniAppSearchQuery] = React.useState('');
  const [miniAppPinInput, setMiniAppPinInput] = React.useState('');
  const [miniAppPinError, setMiniAppPinError] = React.useState(false);
  const [miniAppCopySuccessId, setMiniAppCopySuccessId] = React.useState<string | null>(null);
  const [miniAppExpandedAccountId, setMiniAppExpandedAccountId] = React.useState<string | null>(null);

  const chatEndRef = React.useRef<HTMLDivElement>(null);

  // Auto scroll chat to bottom
  React.useEffect(() => {
    chatEndRef.current?.scrollIntoView({ behavior: 'smooth' });
  }, [messages]);

  // Keep countdown timers ticking for self-destructive messages
  React.useEffect(() => {
    const timer = setInterval(() => {
      setMessages(prev => 
        prev.map(msg => {
          if (msg.isDestructive && msg.destructionSeconds !== undefined && msg.destructionSeconds > 0) {
            const nextSecs = msg.destructionSeconds - 1;
            if (nextSecs === 0) {
              return {
                ...msg,
                destructionSeconds: 0,
                isSelfDestroyed: true,
                text: `🔒 [TIN NHẮN ĐÃ TỰ HỦY] Mật khẩu và OTP của tài khoản ${activeWorkflowAccount?.alias || ''} đã được xóa sạch khỏi lịch sử chat để bảo vệ an toàn tối đa.`
              };
            }
            return {
              ...msg,
              destructionSeconds: nextSecs
            };
          }
          return msg;
        })
      );
    }, 1000);

    return () => clearInterval(timer);
  }, [activeWorkflowAccount]);

  // Initialize welcoming chat
  React.useEffect(() => {
    const nowStr = new Date().toLocaleTimeString([], { hour: '2-digit', minute: '2-digit' });
    setMessages([
      {
        id: 'welcome',
        sender: 'bot',
        text: `Xin chào! Tôi là **Vault Bot 🤖** bảo mật 24/7 của bạn.\n\nTôi chỉ phản hồi duy nhất tin nhắn của chủ sở hữu Telegram ID: **${telegramId}**.\n\n*⚡️ Các chức năng nhanh dưới bàn phím hệ thống:*\n🔍 **Tìm kiếm (Click \`🔍 Tìm kiếm\` hoặc gõ)**\n⚙️ **Cài đặt & Trợ giúp (Click \`⚙️ Cài đặt\` để kiểm tra PIN/Đặt lại / Xem Hướng dẫn)**\n🌐 **Mở Web Vault (Click \`🌐 Mở Web Vault\` để mở Mini App cực mượt ngay trong Telegram)**\n\nHoặc đơn giản nhất, bạn hãy gõ thẳng từ khóa (ví dụ: \`zalo\`, \`gg\`, \`fb\`) để truy vấn tài khoản siêu tốc!`,
        time: nowStr
      }
    ]);
  }, [telegramId]);

  const addBotMessage = (text: string, buttons?: ChatMessage['buttons'], isDestructive = false, duration = 30) => {
    const nowStr = new Date().toLocaleTimeString([], { hour: '2-digit', minute: '2-digit' });
    setMessages(prev => [
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

  const handleSendText = (textToSend: string) => {
    if (!textToSend.trim()) return;
    const nowStr = new Date().toLocaleTimeString([], { hour: '2-digit', minute: '2-digit' });
    
    setInputText('');

    const cleanCommandText = textToSend.trim().toLowerCase();

    // Cập nhật các trạng thái giao diện cục bộ của giả lập Telegram ngay lập tức khi nhấn/nhập lệnh tương ứng
    if (cleanCommandText === '1' || cleanCommandText.includes('tìm kiếm') || cleanCommandText === 'tìm' || cleanCommandText === 'tim') {
      const inputEl = document.getElementById('telegram-chat-input');
      if (inputEl) {
        (inputEl as HTMLInputElement).focus();
        (inputEl as HTMLInputElement).placeholder = "🔍 Nhập tên hoặc từ khóa để tìm... (Ví dụ: fb, gg, zalo)";
      }
    } else if (cleanCommandText.includes('cài đặt') || cleanCommandText.includes('settings')) {
      setSubmenu('settings');
    } else if (cleanCommandText === '↩️ quay lại menu chính' || cleanCommandText === 'quay lại' || cleanCommandText === 'back') {
      setSubmenu('main');
    } else if (cleanCommandText === '🌐 mở web vault' || cleanCommandText.includes('web vault') || cleanCommandText.includes('miniapp')) {
      setShowMiniApp(true);
    }

    // Đặt lại placeholder thông thường nếu tin nhắn tiếp theo là từ khóa tìm kiếm thực tế
    if (cleanCommandText !== '🔍 tìm kiếm' && !cleanCommandText.includes('tìm kiếm') && cleanCommandText !== 'tìm' && cleanCommandText !== 'tim') {
      const inputEl = document.getElementById('telegram-chat-input');
      if (inputEl) {
        (inputEl as HTMLInputElement).placeholder = "Nhập tin nhắn hoặc click nút nhanh...";
      }
    }

    if (onSendMessage) {
      onSendMessage(textToSend);
    } else {
      // Add user message to screen only when no custom onSendMessage handler is passed
      const userMsg: ChatMessage = {
        id: `user-${Date.now()}-${Math.random()}`,
        sender: 'user',
        text: textToSend,
        time: nowStr
      };
      setMessages(prev => [...prev, userMsg]);
      // Handle bot reaction delay logic
      setTimeout(() => {
        processCommand(textToSend);
      }, 600);
    }
  };

  // Bot Logic Engine
  const processCommand = (cmd: string) => {
    const cleanCmd = cmd.trim().toLowerCase();
    
    // 1. Check if we are waiting for a PIN input
    if (currentStep === 'waiting_pin') {
      if (cleanCmd === telegramPin) {
        setPinVerifiedToday(true);
        setCurrentStep('none');
        revealPasswordAndOtp(activeWorkflowAccount!);
      } else if (cleanCmd === 'hủy' || cleanCmd === 'huy' || cleanCmd === '/cancel' || cleanCmd === '2') {
        setCurrentStep('none');
        setActiveWorkflowAccount(null);
        addBotMessage("❌ Đã hủy quy trình xác thực PIN.");
      } else {
        addBotMessage("❌ Mã PIN nhập không chính xác! Vui lòng bấm phím trên bàn phím số phía dưới hoặc nhập lại mã PIN:");
      }
      return;
    }

    // 2. Check if we are in confirmation step
    if (currentStep === 'confirm_view') {
      if (cleanCmd === '1' || cleanCmd.includes('xác nhận') || cleanCmd.includes('xem')) {
        checkRequirementPin(activeWorkflowAccount!);
      } else if (cleanCmd === '2' || cleanCmd === 'hủy' || cleanCmd === 'huy' || cleanCmd === '/cancel') {
        setCurrentStep('none');
        setActiveWorkflowAccount(null);
        addBotMessage("❌ Đã hủy quy trình truy cập mật khẩu.");
      } else {
        addBotMessage("⚠️ Lựa chọn không hợp lệ. Vui lòng gõ `1` để Xem mật khẩu hoặc `2` để Hủy bỏ.");
      }
      return;
    }

    // 3. Check if we are in matches selection step
    if (currentStep === 'showing_matches') {
      const index = parseInt(cleanCmd);
      if (!isNaN(index) && index >= 1 && index <= lastListedAccounts.length) {
        const selected = lastListedAccounts[index - 1];
        handleSelectAccountFlow(selected);
      } else if (cleanCmd === 'hủy' || cleanCmd === 'huy' || cleanCmd === '/cancel') {
        setCurrentStep('none');
        addBotMessage("❌ Đã hủy quy trình hiển thị tài khoản.");
      } else {
        addBotMessage(`⚠️ Lựa chọn không hợp lệ. Vui lòng nhập số từ 1 đến ${lastListedAccounts.length} hoặc click nút dưới để chọn.`);
      }
      return;
    }

    // 4. Default Step Menu & Actions
    if (cleanCmd === '1' || cleanCmd.includes('tìm kiếm') || cleanCmd === 'tìm' || cleanCmd === 'tim') {
      addBotMessage("🔍 **BẮT ĐẦU TÌM KIẾM CHI TIẾT**\n\nHãy nhập tên viết tắt hoặc từ khóa bất kỳ của tài khoản cần tìm (Ví dụ: `gg`, `zalo`, `fb`...)\n\n👉 *Mẹo:* Bạn có thể gõ trực tiếp từ khóa này bất kỳ lúc nào mà không cần nhấp nút Tìm kiếm!");
    } else if (cleanCmd === '2' || cleanCmd.includes('cài đặt') || cleanCmd.includes('settings')) {
      setSubmenu('settings');
      addBotMessage("⚙️ **CÀI ĐẶT & TRỢ GIÚP BẢO MẬT**\n\nVui lòng lựa chọn các tùy chọn bảo mật hoặc xem hướng dẫn bằng các phím ở bàn phím bên dưới:");
    } else if (cleanCmd === '🔐 kiểm tra trạng thái pin' || cleanCmd === '/pin' || cleanCmd.includes('kiểm tra pin')) {
      if (pinVerifiedToday) {
        addBotMessage("🔐 *Trạng thái PIN hôm nay:* **ĐÃ XÁC THỰC ✅**\nBạn không cần nhập lại PIN để xem mật khẩu.");
      } else {
        addBotMessage("🔐 *Trạng thái PIN hôm nay:* **CHƯA XÁC THỰC ❌**\nBạn sẽ cần nhập PIN bảo mật 1 lần để mở khóa mật khẩu.");
      }
    } else if (cleanCmd === '🔑 reset khóa pin' || cleanCmd === '/resetpin' || cleanCmd.includes('reset pin')) {
      setPinVerifiedToday(false);
      addBotMessage("🔑 Đã reset trạng thái xác minh PIN hôm nay. Lần xem mật khẩu tiếp theo bạn sẽ cần nhập lại PIN.");
    } else if (cleanCmd === '🆘 trợ giúp sử dụng' || cleanCmd === '/start' || cleanCmd === '/help' || cleanCmd === '🆘' || cleanCmd.includes('trợ giúp') || cleanCmd.includes('hướng dẫn')) {
      addBotMessage(`🤖 *Secure Vault Bot* \nQuản lý mật khẩu bằng bàn phím hệ thống thông minh.\n\n*⚡️ Các phím chức năng chính:*\n🔍 **Tìm kiếm**: Nhập từ khóa truy tìm nhanh.\n⚙️ **Cài đặt & Trợ giúp**: Quản lý PIN khóa, xem trạng thái đăng nhập, và trợ giúp.\n🌐 **Web Vault**: Mở giao diện Web App đỉnh cao trực tiếp bên trong Telegram.\n\n*🔍 Tra cứu siêu tốc:*\nGõ thẳng từ khóa (ví dụ: \`gg\`, \`fb\`, \`zalo\`) để tìm tài khoản mà không cần gõ lệnh dài dòng.`);
    } else if (cleanCmd === '↩️ quay lại menu chính' || cleanCmd === 'quay lại' || cleanCmd === 'back') {
      setSubmenu('main');
      addBotMessage("↩️ Đã quay lại Menu chính.");
    } else if (cleanCmd === '🌐 mở web vault' || cleanCmd.includes('web vault') || cleanCmd.includes('miniapp')) {
      setShowMiniApp(true);
      addBotMessage("🌐 **Đang mở Web Vault...**\nGiao diện Web App tương tác tuyệt vời đang mở ngay bên trong Telegram của bạn.");
    } else {
      // Treat as custom text search query
      const matches = accounts.filter(
        acc => !acc.isDeleted && (
          acc.alias.toLowerCase().includes(cleanCmd) || 
          acc.appName.toLowerCase().includes(cleanCmd)
        )
      );

      if (matches.length === 0) {
        addBotMessage(`🔍 Không tìm thấy tài khoản nào liên quan đến từ khóa "${cmd}". Hãy thử lại hoặc nhập \`1\` để xem toàn bộ danh sách.`);
      } else if (matches.length === 1) {
        setLastListedAccounts(matches);
        handleSelectAccountFlow(matches[0]);
      } else {
        setLastListedAccounts(matches);
        setCurrentStep('showing_matches');
        const countText = `🔍 Tìm thấy ${matches.length} tài khoản cho từ khóa "${cmd}":\n\n`;
        const listText = matches.map((acc, idx) => {
          return `${idx + 1}. **${acc.alias}** (${acc.username})`;
        }).join('\n\n');

        addBotMessage(countText + listText + "\n\n👉 Vui lòng nhập số thứ tự tương ứng (ví dụ: `1`, `2`...) hoặc click bàn phím bên dưới để lựa chọn:");
      }
    }
  };

  const handleSelectAccountFlow = (account: Account) => {
    setActiveWorkflowAccount(account);
    setCurrentStep('confirm_view');
    const questionText = `Tài khoản: **${account.alias}**\nUsername: **${account.username}**\n\nBạn có muốn xem mật khẩu không?`;
    
    addBotMessage(questionText);
  };

  const checkRequirementPin = (account: Account) => {
    if (pinVerifiedToday) {
      revealPasswordAndOtp(account);
    } else {
      setCurrentStep('waiting_pin');
      addBotMessage("🔐 Vui lòng nhập mã PIN bảo mật của bạn để tiếp tục (hôm nay bạn chưa xác thực trong 24 giờ):");
    }
  };

  const revealPasswordAndOtp = (account: Account) => {
    // Increment access frequency query tracking
    if (onIncrementAccess) {
      onIncrementAccess(account.id);
    }

    // Deterministic OTP code creation
    const now = new Date();
    const currentSecs = now.getSeconds();
    const left = 30 - (currentSecs % 30);
    const baseNum = Math.floor(now.getTime() / 30000);
    const secretHash = account.id.charCodeAt(0) * 8219 + (account.otpSecret ? account.otpSecret.charCodeAt(0) * 1923 : 0);
    const codeNum = (baseNum * 313 + secretHash) % 900000 + 100000;
    const codeStr = codeNum.toString();
    const otpCodeString = account.otpSecret ? `${codeStr.slice(0, 3)} ${codeStr.slice(3)}` : 'Chưa thiết lập 2FA';

    const altPassSection = account.alternativePasswords && account.alternativePasswords.length > 0
      ? `\n**Mật khẩu phụ khác chung tài khoản:**\n` + account.alternativePasswords.map((p, idx) => `• Mật khẩu ${idx + 2}: \`${p}\``).join('\n')
      : '';

    const outputText = `🔓 **XÁC MINH PIN HỢP LỆ ✅**\nBạn sẽ không cần nhập PIN trong vòng 24h tới.\n\n` +
      `**Tài khoản:** ${account.alias}\n` +
      `**Username:** \`${account.username}\`\n\n` +
      `**Mật khẩu chính:** \`${account.passwordDecrypted}\`${altPassSection}\n` +
      `(Các mật khẩu đã được giải mã tạm thời)\n\n` +
      `**Mã OTP (2FA):** \`${otpCodeString}\`\n` +
      `(Hiệu lực: còn ${left} giây)\n\n` +
      `🚨 **Lưu ý bảo mật:** Tin nhắn này chứa thông tin mật, bộ đệm bảo vệ sẽ **TỰ ĐỘNG XÓA** tin nhắn sau 30 giây!`;

    addBotMessage(outputText, undefined, true, 30);
  };

  return (
    <div id="telegram-section-container" className="flex-1 max-w-lg mx-auto bg-slate-100 dark:bg-[#1C1C1E] p-4 rounded-3xl border-none shadow-[0_12px_40px_rgba(0,0,0,0.06)] flex flex-col h-[650px] relative">
      
      {/* Bot Phone Mock Frame Top Grill */}
      <div id="phone-camera-grill" className="w-1/3 h-5 bg-slate-800 dark:bg-slate-900 absolute top-2 left-1/2 -translate-x-1/2 rounded-full z-20 flex items-center justify-center">
        <span className="w-2.5 h-2.5 rounded-full bg-slate-950 mr-1.5 border border-slate-850"></span>
        <span className="w-8 h-1 bg-slate-700 rounded-full"></span>
      </div>

      {/* Blue Header (Classic Telegram styling in dark blue) */}
      <div id="telegram-header" className="bg-[#1c7bb6] text-white p-3 pt-7 rounded-t-2xl flex items-center gap-3 shadow-md z-10 shrink-0">
        <div id="bot-avatar" className="w-10 h-10 rounded-full border border-blue-400 overflow-hidden shrink-0 shadow-inner p-0 bg-blue-900">
          <img 
            src="https://images.unsplash.com/photo-1517841905240-472988babdf9?auto=format&fit=crop&w=100&h=100&q=80"
            alt="Vault Bot Avatar"
            referrerPolicy="no-referrer"
            className="w-full h-full object-cover"
          />
        </div>
        <div className="flex-1 min-w-0">
          <div className="flex items-center gap-1.5">
            <h3 id="bot-name" className="font-sans font-bold text-sm">Vault Bot</h3>
            <span id="bot-verified-badge" className="w-3.5 h-3.5 rounded-full bg-white text-[#1c7bb6] flex items-center justify-center text-[8px] font-extrabold" title="Verified Bot">✓</span>
          </div>
          <span id="bot-status" className="text-[10px] text-blue-100 block">bot</span>
        </div>
        
        {/* Verification Status Badge */}
        <div id="verified-state-indicator" className="text-[10px] px-2 py-1 rounded-full bg-blue-500/30 border border-blue-400 flex items-center gap-1 whitespace-nowrap">
          <span className={`w-1.5 h-1.5 rounded-full ${pinVerifiedToday ? 'bg-emerald-400' : 'bg-red-400 animate-pulse'}`}></span>
          <span className="font-semibold text-[9px]">{pinVerifiedToday ? 'Đã Xác Thực PIN' : 'Chưa Nhập PIN'}</span>
        </div>
      </div>

      {/* Chat messages viewport */}
      <div id="chat-messages-viewport" className="flex-1 bg-[#f3f7fa] dark:bg-slate-900 overflow-y-auto p-3.5 space-y-4">
        
        {messages.map((msg) => {
          const isBot = msg.sender === 'bot';
          return (
            <div
              id={`chat-msg-row-${msg.id}`}
              key={msg.id}
              className={`flex items-start gap-2.5 max-w-[85%] ${
                isBot ? 'mr-auto' : 'ml-auto flex-row-reverse'
              }`}
            >
              {/* Profile letter bubbles */}
              <div id={`chat-avatar-${msg.id}`} className="w-7 h-7 rounded-full select-none shadow-sm overflow-hidden shrink-0 bg-slate-200">
                {isBot ? (
                  <img 
                    src="https://images.unsplash.com/photo-1517841905240-472988babdf9?auto=format&fit=crop&w=80&h=80&q=80"
                    alt="Bot Avatar"
                    referrerPolicy="no-referrer"
                    className="w-full h-full object-cover"
                  />
                ) : (
                  <div className="w-full h-full bg-emerald-600 text-white flex items-center justify-center text-xs">
                    <User size={12} />
                  </div>
                )}
              </div>

              {/* Message Content Bubble */}
              <div id={`chat-bubble-bg-${msg.id}`} className="space-y-1">
                <div id={`chat-bubble-${msg.id}`} className={`p-3 rounded-2xl shadow-sm text-xs selection:bg-blue-200 ${
                  isBot 
                    ? msg.isDestructive 
                      ? 'bg-amber-50 dark:bg-amber-950/20 text-slate-800 dark:text-slate-100 dark:border dark:border-amber-900 border border-amber-200' 
                      : 'bg-white dark:bg-slate-800 text-slate-800 dark:text-slate-150' 
                    : 'bg-[#effdde] dark:bg-emerald-950/40 text-slate-800 dark:text-slate-150 border border-emerald-100 dark:border-emerald-900'
                }`}>
                  {/* Process formatting markdown nicely */}
                  <p className="whitespace-pre-line leading-relaxed">
                    {msg.text}
                  </p>

                  <div className="flex items-center justify-end gap-1.5 mt-1 text-[9px] text-slate-400 select-none font-semibold">
                    {msg.isDestructive && msg.destructionSeconds !== undefined && msg.destructionSeconds > 0 && (
                      <span className="text-red-500 flex items-center gap-0.5 animate-pulse">
                        <Clock size={8} /> Tự hủy sau {msg.destructionSeconds}s
                      </span>
                    )}
                    <span>{msg.time}</span>
                    {!isBot && <Check size={8} className="text-emerald-500 font-bold" />}
                  </div>
                </div>

                {/* Inline Message Action Buttons (Telegram style) */}
                {msg.buttons && msg.buttons.length > 0 && !msg.isSelfDestroyed && (
                  <div className="grid grid-cols-1 gap-1.5 pt-1">
                    {msg.buttons.map((btn, bidx) => (
                      <button
                        key={bidx}
                        onClick={btn.action}
                        className={`w-full py-2 px-3 text-[11px] font-bold rounded-xl text-center shadow-sm select-none transition-all active:scale-[0.98] cursor-pointer ${
                          btn.variant === 'success' 
                            ? 'bg-emerald-600 hover:bg-emerald-700 text-white' 
                            : btn.variant === 'danger' 
                              ? 'bg-rose-600 hover:bg-rose-700 text-white'
                              : 'bg-slate-200 hover:bg-slate-300 dark:bg-slate-800 dark:hover:bg-slate-700 text-slate-700 dark:text-slate-200'
                        }`}
                      >
                        {btn.text}
                      </button>
                    ))}
                  </div>
                )}
              </div>
            </div>
          );
        })}
        <div ref={chatEndRef} />
      </div>

      {/* Input Message Footer Area */}
      <div id="telegram-chat-footer" className="p-2.5 bg-white dark:bg-slate-900 rounded-b-2xl border-t border-slate-50 dark:border-[#2C2C2E] space-y-2 shrink-0 font-sans">
        
        {/* Help tip details wrapper */}
        <div className="flex items-center justify-between text-[10px] text-slate-400 px-1 font-semibold">
          <div className="flex items-center gap-1">
            <Info size={11} className="text-sky-500 shrink-0" />
            <span>
              {currentStep === 'waiting_pin' ? 'Click mã PIN bảo mật 4 chữ số' :
               currentStep === 'confirm_view' ? 'Xác nhận để mở khóa xem mật khẩu' :
               currentStep === 'showing_matches' ? 'Chọn tài khoản tương ứng' :
               'Hãy bấm nút nhanh bên dưới hoặc gõ trực tiếp'}
            </span>
          </div>
          {currentStep !== 'none' && (
            <button 
              type="button" 
              onClick={() => {
                setCurrentStep('none');
                setActiveWorkflowAccount(null);
                addBotMessage("❌ Đã hủy quy trình hiện tại.");
              }}
              className="text-red-500 hover:underline cursor-pointer"
            >
              Hủy nhanh
            </button>
          )}
        </div>

        {/* TELEGRAM STYLE CUSTOM REPLY KEYBOARD MARKUP */}
        <div id="telegram-reply-keyboard" className="bg-slate-50 dark:bg-slate-950 p-2 rounded-2xl border border-slate-100 dark:border-slate-800 space-y-1.5">
          {currentStep === 'waiting_pin' && (
            <div className="space-y-1.5">
              <div className="text-[10px] text-center font-bold text-slate-500 uppercase tracking-wider mb-1">
                Bàn Phím PIN Bảo Mật
              </div>
              <div className="grid grid-cols-3 gap-1 max-w-[210px] mx-auto">
                {[1, 2, 3, 4, 5, 6, 7, 8, 9].map((num) => (
                  <button
                    key={num}
                    type="button"
                    onClick={() => {
                      if (inputText.length < 6) {
                        setInputText(prev => prev + num);
                      }
                    }}
                    className="py-2 bg-white dark:bg-slate-850 hover:bg-slate-100 dark:hover:bg-slate-700 text-xs font-bold text-slate-700 dark:text-slate-200 rounded-lg border border-slate-100 dark:border-slate-700 shadow-sm active:scale-95 transition-all cursor-pointer"
                  >
                    {num}
                  </button>
                ))}
                <button
                  type="button"
                  onClick={() => {
                    setCurrentStep('none');
                    setActiveWorkflowAccount(null);
                    addBotMessage("❌ Đã dừng quy trình nhập mã PIN.");
                  }}
                  className="py-2 bg-rose-50 dark:bg-rose-950/20 hover:bg-rose-100 text-[10px] font-bold text-rose-600 dark:text-rose-400 rounded-lg border border-rose-100 dark:border-rose-900/40 shadow-sm active:scale-95 transition-all cursor-pointer"
                >
                  HỦY
                </button>
                <button
                  type="button"
                  onClick={() => {
                    if (inputText.length < 6) {
                      setInputText(prev => prev + '0');
                    }
                  }}
                  className="py-2 bg-white dark:bg-slate-850 hover:bg-slate-100 dark:hover:bg-slate-700 text-xs font-bold text-slate-700 dark:text-slate-200 rounded-lg border border-slate-100 dark:border-slate-700 shadow-sm active:scale-95 transition-all cursor-pointer"
                >
                  0
                </button>
                <button
                  type="button"
                  onClick={() => {
                    setInputText('');
                  }}
                  className="py-2 bg-amber-50 dark:bg-amber-950/20 hover:bg-amber-100 text-[10px] font-bold text-amber-600 dark:text-amber-400 rounded-lg border border-amber-100 dark:border-amber-900/40 shadow-sm active:scale-95 transition-all cursor-pointer"
                >
                  XÓA
                </button>
              </div>
            </div>
          )}

          {currentStep === 'confirm_view' && (
            <div className="grid grid-cols-2 gap-1.5">
              <button
                type="button"
                onClick={() => handleSendText('1')}
                className="py-2 bg-emerald-600 hover:bg-emerald-700 text-white text-xs font-bold rounded-xl shadow-sm active:scale-[0.98] transition-all flex items-center justify-center gap-1.5 cursor-pointer"
              >
                <span>🚀 1. Xem mật khẩu</span>
              </button>
              <button
                type="button"
                onClick={() => handleSendText('2')}
                className="py-2 bg-slate-200 hover:bg-slate-300 dark:bg-slate-800 dark:hover:bg-slate-700 text-slate-750 dark:text-slate-200 text-xs font-bold rounded-xl shadow-sm active:scale-[0.98] transition-all flex items-center justify-center gap-1.5 cursor-pointer"
              >
                <span>❌ 2. Hủy bỏ</span>
              </button>
            </div>
          )}

          {currentStep === 'showing_matches' && (
            <div className="space-y-1.5">
              <div className="max-h-[120px] overflow-y-auto space-y-1 pr-0.5">
                {lastListedAccounts.map((acc, idx) => (
                  <button
                    key={acc.id}
                    type="button"
                    onClick={() => handleSendText(String(idx + 1))}
                    className="w-full text-left py-1.5 px-3 bg-white dark:bg-slate-850 hover:bg-slate-100 dark:hover:bg-slate-750 rounded-lg border border-slate-100 dark:border-slate-750 flex items-center justify-between shadow-sm active:translate-x-0.5 transition-all cursor-pointer"
                  >
                    <div className="flex items-center gap-1.5 min-w-0">
                      <span className="w-4 h-4 rounded-full bg-blue-50 dark:bg-blue-950 flex items-center justify-center text-[9px] font-bold text-blue-600 dark:text-blue-400 shrink-0">
                        {idx + 1}
                      </span>
                      <span className="text-xs font-bold text-slate-700 dark:text-slate-200 truncate">{acc.alias}</span>
                    </div>
                    <span className="text-[10px] text-slate-400 truncate max-w-[120px]">{acc.username}</span>
                  </button>
                ))}
              </div>
              <button
                type="button"
                onClick={() => handleSendText('hủy')}
                className="w-full py-1.5 bg-rose-55 hover:bg-rose-100 dark:bg-rose-950/20 text-rose-600 dark:text-rose-400 rounded-lg text-xs font-bold border border-rose-100 dark:border-rose-900/30 text-center cursor-pointer"
              >
                ❌ Hủy bỏ quy trình lựa chọn
              </button>
            </div>
          )}

          {currentStep === 'none' && (
            <div className="space-y-1.5">
              {submenu === 'main' ? (
                <div className="space-y-1.5">
                  {/* Single Row: 3 modern action buttons side-by-side with zero icon duplication */}
                  <div className="grid grid-cols-3 gap-1.5">
                    <button
                      type="button"
                      onClick={() => handleSendText('🔍 Tìm kiếm')}
                      className="py-2 bg-[#1c7bb6] hover:bg-[#156190] text-white text-[11px] font-bold rounded-xl shadow-sm active:scale-[0.98] transition-all flex items-center justify-center gap-1 cursor-pointer"
                    >
                      <Search size={13} className="shrink-0" />
                      <span className="truncate">Tìm kiếm</span>
                    </button>
                    <button
                      type="button"
                      onClick={() => handleSendText('⚙️ Cài đặt & Trợ giúp')}
                      className="py-2 bg-slate-200 hover:bg-slate-300 dark:bg-slate-800 dark:hover:bg-slate-700 text-slate-700 dark:text-slate-200 text-[11px] font-bold rounded-xl shadow-sm active:scale-[0.98] transition-all flex items-center justify-center gap-1 cursor-pointer"
                    >
                      <Settings size={13} className="shrink-0" />
                      <span className="truncate">Cài đặt</span>
                    </button>
                    <button
                      type="button"
                      onClick={() => {
                        handleSendText('🌐 Mở Web Vault');
                      }}
                      className="py-2 bg-emerald-600 hover:bg-emerald-700 text-white text-[11px] font-bold rounded-xl shadow-sm active:scale-[0.98] transition-all flex items-center justify-center gap-1 cursor-pointer"
                    >
                      <Globe size={13} className="shrink-0" />
                      <span className="truncate">Web Vault</span>
                    </button>
                  </div>
                </div>
              ) : (
                <div className="space-y-1.5 animate-[fadeIn_0.15s_ease-out]">
                  {/* Settings Sub-keyboard */}
                  <div className="grid grid-cols-2 gap-1.5">
                    <button
                      type="button"
                      onClick={() => handleSendText('🔐 Kiểm tra trạng thái PIN')}
                      className="py-1.5 bg-slate-200 hover:bg-slate-300 dark:bg-slate-800 dark:hover:bg-slate-700 text-slate-700 dark:text-slate-200 text-[11px] font-semibold rounded-xl active:scale-[0.98] transition-all flex items-center justify-center gap-1 cursor-pointer"
                    >
                      <Lock size={12} />
                      <span>Kiểm tra PIN</span>
                    </button>
                    <button
                      type="button"
                      onClick={() => handleSendText('🔑 Reset khóa PIN')}
                      className="py-1.5 bg-slate-200 hover:bg-slate-300 dark:bg-slate-800 dark:hover:bg-slate-700 text-slate-700 dark:text-slate-200 text-[11px] font-semibold rounded-xl active:scale-[0.98] transition-all flex items-center justify-center gap-1 cursor-pointer"
                    >
                      <Key size={12} />
                      <span>Reset PIN</span>
                    </button>
                  </div>
                  <div className="grid grid-cols-2 gap-1.5">
                    <button
                      type="button"
                      onClick={() => handleSendText('🆘 Trợ giúp sử dụng')}
                      className="py-1.5 bg-indigo-50 dark:bg-indigo-950/20 hover:bg-indigo-100 text-indigo-600 dark:text-indigo-400 text-[11px] font-semibold rounded-xl active:scale-[0.98] transition-all flex items-center justify-center gap-1 cursor-pointer"
                    >
                      <Info size={12} />
                      <span>Trợ giúp</span>
                    </button>
                    <button
                      type="button"
                      onClick={() => handleSendText('↩️ Quay lại Menu chính')}
                      className="py-1.5 bg-slate-100 dark:bg-slate-900 border border-slate-200 dark:border-slate-800 text-slate-500 text-[11px] font-semibold rounded-xl active:scale-[0.98] transition-all flex items-center justify-center gap-1 cursor-pointer"
                    >
                      <ArrowLeft size={12} />
                      <span>Quay lại</span>
                    </button>
                  </div>
                </div>
              )}

              {/* Smart Quick Suggestions by search/access frequency (Vietnamese: "Tra nhanh") */}
              {frequentAccounts && frequentAccounts.length > 0 && (
                <div className="flex items-center gap-1 overflow-x-auto py-0.5 whitespace-nowrap scrollbar-none">
                  <span className="text-[9px] text-slate-400 font-bold shrink-0">Hay dùng ⭐:</span>
                  {frequentAccounts.slice(0, 5).map(acc => (
                    <button
                      key={acc.id}
                      type="button"
                      onClick={() => handleSendText(acc.alias)}
                      className="px-2 py-0.5 bg-white dark:bg-slate-800 hover:bg-blue-50 dark:hover:bg-slate-700 text-[10px] text-slate-650 dark:text-slate-300 font-bold rounded border border-slate-150 dark:border-slate-750 shadow-sm shrink-0 cursor-pointer"
                    >
                      {acc.alias}
                    </button>
                  ))}
                </div>
              )}
            </div>
          )}
        </div>

        {/* Input box row */}
        <form
          id="telegram-chat-form"
          onSubmit={(e) => {
            e.preventDefault();
            handleSendText(inputText);
          }}
          className="flex items-center gap-2"
        >
          <input
            id="telegram-chat-input"
            type="text"
            value={inputText}
            onChange={(e) => setInputText(e.target.value)}
            placeholder={
              currentStep === 'waiting_pin' 
                ? "Nhập hoặc click PIN gồm 4 số phía trên..." 
                : currentStep === 'confirm_view' 
                  ? "Nhập '1' để xem, '2' để hủy..." 
                  : currentStep === 'showing_matches' 
                    ? "Nhập số thứ tự để lựa chọn..." 
                    : "Nhập tin nhắn hoặc click nút nhanh..."
            }
            className="flex-1 bg-slate-50 dark:bg-[#1C1C1E] px-3.5 py-2 text-xs text-slate-800 dark:text-slate-100 border-none rounded-full focus:outline-none focus:ring-1 focus:ring-[#1c7bb6] transition-all font-sans"
          />
          <button
            id="telegram-chat-submit"
            type="submit"
            className="w-8 h-8 rounded-full bg-[#1c7bb6] text-white flex items-center justify-center shadow-md shadow-[#1c7bb6]/20 hover:bg-blue-600 active:scale-95 transition-all cursor-pointer shrink-0"
          >
            <Send size={13} className="stroke-[2.5]" />
          </button>
        </form>
      </div>
      
      {/* Phone Bot Guard Label Footer */}
      <div className="text-[10px] text-slate-400 text-center py-1 bg-slate-200/50 dark:bg-slate-900 rounded-b-3xl">
        <span>Bot chỉ hoạt động với Telegram ID của bạn. Nếu người khác chat, bot sẽ không phản hồi.</span>
      </div>

      {/* 🌐 MOCK EMBEDDED TELEGRAM WEB APP (MINI APP) OVERLAY */}
      {showMiniApp && (
        <div id="embedded-tg-miniapp" className="absolute inset-0 bg-[#F5F5F7] dark:bg-[#121214] rounded-3xl z-30 flex flex-col overflow-hidden animate-[slideUp_0.25s_ease-out]">
          {/* Header of Telegram Web App Web View */}
          <div className="bg-[#1c7bb6] dark:bg-[#1E1F24] text-white px-4 py-3 flex items-center justify-between shadow-sm border-b border-sky-700/10 dark:border-slate-800 shrink-0">
            <div className="flex items-center gap-2 min-w-0">
              <button 
                type="button"
                onClick={() => setShowMiniApp(false)}
                className="p-1 hover:bg-white/10 dark:hover:bg-slate-800 rounded-full transition-colors cursor-pointer"
                title="Đóng Web Vault"
              >
                <X size={18} />
              </button>
              <div className="min-w-0">
                <div className="flex items-center gap-1.5 font-bold text-xs">
                  <span className="w-1.5 h-1.5 rounded-full bg-emerald-450 animate-pulse bg-emerald-400"></span>
                  <span className="truncate">Kraft Web Vault</span>
                </div>
                <div className="text-[9px] text-sky-100 dark:text-slate-400 font-medium truncate">
                  {window.location.host || 'vault.local'} • t.me/VaultBot
                </div>
              </div>
            </div>
            
            <div className="flex items-center gap-1.5">
              <button 
                type="button" 
                onClick={() => {
                  setMiniAppPinInput('');
                  setMiniAppPinError(false);
                }} 
                className="p-1 text-sky-100 hover:text-white dark:text-slate-400 dark:hover:text-slate-200 transition-colors cursor-pointer"
                title="Tải lại trang"
              >
                <RefreshCw size={14} />
              </button>
            </div>
          </div>

          {/* Body of Web App */}
          <div className="flex-1 flex flex-col min-h-0 bg-slate-50 dark:bg-[#151518] p-0 text-slate-800 dark:text-slate-100 overflow-hidden relative">
            {!pinVerifiedToday ? (
              /* If Today is not PIN authorized yet, prompt PIN screen inside the web app for security */
              <div className="flex-1 flex flex-col items-center justify-center max-w-sm mx-auto w-full text-center py-6 px-4 space-y-4">
                <div className="p-3 bg-sky-50 dark:bg-sky-950/20 text-[#1c7bb6] dark:text-sky-400 rounded-full">
                  <LockKeyhole size={36} className="animate-bounce" />
                </div>
                <div>
                  <h4 className="font-display font-bold text-sm text-slate-800 dark:text-slate-200">Xác thực mã PIN Web Vault</h4>
                  <p className="text-[11px] text-slate-400 mt-1">
                    Vui lòng nhập mã PIN bảo mật đã thiết lập của bạn để truy cập cơ sở dữ liệu mật khẩu an toàn.
                  </p>
                </div>
                
                <div className="w-full space-y-2">
                  <input
                    type="password"
                    maxLength={4}
                    placeholder="• • • •"
                    value={miniAppPinInput}
                    onChange={(e) => {
                      const val = e.target.value.replace(/\D/g, '');
                      setMiniAppPinInput(val);
                      setMiniAppPinError(false);
                      if (val === telegramPin) {
                        setPinVerifiedToday(true);
                      } else if (val.length === 4) {
                        setMiniAppPinError(true);
                      }
                    }}
                    className={`w-full text-center py-2 px-4 bg-white dark:bg-slate-900 border ${miniAppPinError ? 'border-rose-500 bg-rose-50/10' : 'border-slate-200 dark:border-slate-800'} text-lg font-bold tracking-[0.5em] rounded-xl focus:ring-2 focus:ring-sky-500 focus:outline-[#1c7bb6] focus:outline-none`}
                    readOnly
                  />
                  {miniAppPinError && (
                    <div className="text-[10px] text-rose-500 font-bold">Mã PIN không chính xác! Hãy thử lại.</div>
                  )}
                  <div className="text-[10px] text-slate-400">
                    💡 PIN mặc định của bot: <span className="font-bold">{telegramPin}</span>
                  </div>
                </div>

                <div className="grid grid-cols-3 gap-1.5 w-full max-w-[200px] pt-1">
                  {[1, 2, 3, 4, 5, 6, 7, 8, 9].map(n => (
                    <button
                      key={n}
                      type="button"
                      onClick={() => {
                        const nextVal = (miniAppPinInput + n).slice(0, 4);
                        setMiniAppPinInput(nextVal);
                        if (nextVal === telegramPin) {
                          setPinVerifiedToday(true);
                        } else if (nextVal.length === 4) {
                          setMiniAppPinError(true);
                        }
                      }}
                      className="py-2 bg-white dark:bg-slate-900 text-xs font-bold text-slate-700 dark:text-slate-300 rounded-lg hover:bg-slate-105 dark:hover:bg-slate-800 border border-slate-150 dark:border-slate-800 active:scale-95 transition-all cursor-pointer animate-none"
                    >
                      {n}
                    </button>
                  ))}
                  <button
                    type="button"
                    onClick={() => setMiniAppPinInput('')}
                    className="py-2 bg-slate-100 dark:bg-slate-800 text-[10px] font-bold text-slate-500 dark:text-slate-400 rounded-lg hover:bg-slate-200 active:scale-95 transition-all cursor-pointer"
                  >
                    Clear
                  </button>
                  <button
                    key={0}
                    type="button"
                    onClick={() => {
                      const nextVal = (miniAppPinInput + '0').slice(0, 4);
                      setMiniAppPinInput(nextVal);
                      if (nextVal === telegramPin) {
                        setPinVerifiedToday(true);
                      } else if (nextVal.length === 4) {
                        setMiniAppPinError(true);
                      }
                    }}
                    className="py-2 bg-white dark:bg-slate-900 text-xs font-bold text-slate-705 dark:text-slate-305 rounded-lg hover:bg-slate-105 dark:hover:bg-slate-800 border border-slate-150 dark:border-slate-800 active:scale-95 transition-all cursor-pointer"
                  >
                    0
                  </button>
                  <button
                    type="button"
                    onClick={() => setShowMiniApp(false)}
                    className="py-2 bg-slate-100 dark:bg-slate-800 text-[10px] font-bold text-rose-500 rounded-lg hover:bg-rose-50 cursor-pointer"
                  >
                    Exit
                  </button>
                </div>
              </div>
            ) : (
              /* Highly optimized real web app iframe embed view */
              <div className="flex-1 flex flex-col min-h-0 bg-slate-50 dark:bg-[#151518] relative w-full h-full">
                <iframe
                  src={`${window.location.origin}/?tg_webapp=true`}
                  className="w-full h-full border-none bg-slate-50 dark:bg-[#151518] rounded-b-3xl relative z-10"
                  title="Kraft Web Vault Mini App"
                />
              </div>
            )}
          </div>
        </div>
      )}
    </div>
  );
}
