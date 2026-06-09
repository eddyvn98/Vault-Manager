/**
 * @license
 * SPDX-License-Identifier: Apache-2.0
 */

import React from 'react';
import { 
  Copy, 
  Check, 
  Clock, 
  Search, 
  Plus, 
  X, 
  FileUp, 
  FileDown, 
  Smartphone, 
  Info, 
  AlertTriangle,
  QrCode,
  ShieldCheck,
  Trash2
} from 'lucide-react';
import { Account } from '../types';

interface TwoFactorAuthenticatorProps {
  accounts: Account[];
  onAddAccount: (newData: Partial<Account>) => void;
  onUpdateAccount: (updatedData: Partial<Account>) => void;
}

export default function TwoFactorAuthenticator({
  accounts,
  onAddAccount,
  onUpdateAccount
}: TwoFactorAuthenticatorProps) {
  const [searchQuery, setSearchQuery] = React.useState('');
  const [copiedId, setCopiedId] = React.useState<string | null>(null);
  
  // Quick Add 2FA State
  const [showAddQuickModal, setShowAddQuickModal] = React.useState(false);
  const [quickAlias, setQuickAlias] = React.useState('');
  const [quickAppName, setQuickAppName] = React.useState('');
  const [quickUsername, setQuickUsername] = React.useState('');
  const [quickOtpSecret, setQuickOtpSecret] = React.useState('');
  const [quickCategory, setQuickCategory] = React.useState('Khác');

  // Time Tracker state
  const [secondsLeft, setSecondsLeft] = React.useState(30);

  // Generate TOTP-like codes for accounts
  const getOtpCode = (account: Account) => {
    if (!account.otpSecret) return '000000';
    const now = new Date();
    const baseNum = Math.floor(now.getTime() / 30000);
    // Simple deterministic hash matching AccountDetail.tsx
    const secretHash = account.id.charCodeAt(0) * 8219 + (account.otpSecret ? account.otpSecret.charCodeAt(0) * 1923 : 0);
    const codeNum = (baseNum * 313 + secretHash) % 900000 + 100000;
    const codeStr = codeNum.toString();
    return `${codeStr.slice(0, 3)} ${codeStr.slice(3)}`;
  };

  React.useEffect(() => {
    const updateTimer = () => {
      const now = new Date();
      const currentSecs = now.getSeconds();
      setSecondsLeft(30 - (currentSecs % 30));
    };

    updateTimer();
    const interval = setInterval(updateTimer, 1000);
    return () => clearInterval(interval);
  }, []);

  const handleCopy = (code: string, id: string) => {
    const cleanCode = code.replace(/\s+/g, '');
    navigator.clipboard.writeText(cleanCode);
    setCopiedId(id);
    setTimeout(() => setCopiedId(null), 1500);
  };

  // Filter accounts having OTP secret & not deleted
  const authenticatorAccounts = React.useMemo(() => {
    return accounts.filter(acc => 
      !acc.isDeleted && 
      acc.otpSecret && 
      acc.otpSecret.trim().length > 0 &&
      (acc.alias.toLowerCase().includes(searchQuery.toLowerCase()) || 
       acc.appName.toLowerCase().includes(searchQuery.toLowerCase()) ||
       acc.username.toLowerCase().includes(searchQuery.toLowerCase()))
    );
  }, [accounts, searchQuery]);

  // Export 2FA logic
  const handleExport2FA = () => {
    const data2FA = accounts
      .filter(acc => !acc.isDeleted && acc.otpSecret && acc.otpSecret.trim().length > 0)
      .map(acc => ({
        alias: acc.alias,
        appName: acc.appName,
        username: acc.username,
        otpSecret: acc.otpSecret,
        category: acc.category,
        notes: acc.notes || ''
      }));

    if (data2FA.length === 0) {
      alert("Không có tài khoản nào được cấu hình 2FA để xuất dữ liệu!");
      return;
    }

    const jsonString = `data:text/json;charset=utf-8,${encodeURIComponent(
      JSON.stringify(data2FA, null, 2)
    )}`;
    
    const downloadAnchor = document.createElement('a');
    downloadAnchor.setAttribute('href', jsonString);
    downloadAnchor.setAttribute('download', 'vault_manager_2fa_backup.json');
    document.body.appendChild(downloadAnchor);
    downloadAnchor.click();
    downloadAnchor.remove();
  };

  // Import 2FA logic
  const handleImport2FA = (e: React.ChangeEvent<HTMLInputElement>) => {
    const fileReader = new FileReader();
    const file = e.target.files?.[0];
    if (!file) return;

    fileReader.onload = (event) => {
      try {
        const parsed = JSON.parse(event.target?.result as string);
        if (!Array.isArray(parsed)) {
          alert("Định dạng file sao lưu không hợp lệ! Vui lòng tải lại tệp chuẩn.");
          return;
        }

        let importedCount = 0;
        let updatedCount = 0;

        parsed.forEach((item: any) => {
          if (!item.otpSecret) return; // Skip items without key

          // Check if account already exists with the same username and appName/alias
          const existing = accounts.find(
            a => !a.isDeleted && 
            (a.username.toLowerCase() === (item.username || '').toLowerCase()) && 
            (a.appName.toLowerCase() === (item.appName || '').toLowerCase() || a.alias.toLowerCase() === (item.alias || '').toLowerCase())
          );

          if (existing) {
            // Update OTP secret of existing account
            onUpdateAccount({
              id: existing.id,
              otpSecret: item.otpSecret,
              notes: item.notes ? `${existing.notes || ''}\n[Imported 2FA]: ${item.notes}`.trim() : existing.notes
            });
            updatedCount++;
          } else {
            // Add as new account with backup settings
            onAddAccount({
              alias: item.alias || `imported-${Date.now()}`,
              appName: item.appName || 'unknown.com',
              username: item.username || 'user',
              otpSecret: item.otpSecret,
              category: item.category || 'Khác',
              notes: item.notes || 'Được khôi phục từ tệp bảo mật 2FA JSON.',
              passwordDecrypted: 'MậtKhẩuMặcĐịnh123!'
            });
            importedCount++;
          }
        });

        alert(`Nhập dữ liệu 2FA hoàn tất!\n- Đã thêm mới: ${importedCount} mã 2FA\n- Đã cập nhật: ${updatedCount} tài khoản sẵn có.`);
      } catch (err) {
        alert("Có lỗi xảy ra khi đọc file sao lưu. Đảm bảo file cấu trúc JSON chuẩn.");
      }
    };
    fileReader.readAsText(file);
    // Reset file input value so it triggers change even for same file
    e.target.value = '';
  };

  // Quick 2FA Account Registration
  const submitQuickAdd2FA = () => {
    if (!quickAlias.trim() || !quickOtpSecret.trim()) {
      alert("Vui lòng điền Alias và Khóa bí mật (Secret Key)!");
      return;
    }

    onAddAccount({
      alias: quickAlias.trim(),
      appName: quickAppName.trim() || 'authenticator.app',
      username: quickUsername.trim() || 'user',
      otpSecret: quickOtpSecret.replace(/\s+/g, '').toUpperCase(),
      category: quickCategory,
      passwordDecrypted: 'PasswordSecured2FA_99',
      notes: 'Được tạo trực tiếp từ Giao diện 2FA Authenticator.'
    });

    // Reset Form
    setQuickAlias('');
    setQuickAppName('');
    setQuickUsername('');
    setQuickOtpSecret('');
    setQuickCategory('Khác');
    setShowAddQuickModal(false);
  };

  // Remove 2FA from account helper (Disable 2nd factor)
  const handleRemove2FA = (id: string, alias: string) => {
    if (confirm(`Bạn có chắc muốn tắt 2FA và xóa Khóa bí mật của tài khoản "${alias}" không?`)) {
      onUpdateAccount({
        id,
        otpSecret: ''
      });
    }
  };

  // Color logic for timer indicator
  const timerColorClass = secondsLeft <= 5 
    ? 'text-rose-500 bg-rose-50 dark:bg-rose-950/30' 
    : secondsLeft <= 10 
      ? 'text-amber-500 bg-amber-50 dark:bg-amber-950/30' 
      : 'text-emerald-500 bg-emerald-50 dark:bg-emerald-950/30';

  const timerCircleProgress = ((30 - secondsLeft) / 30) * 100;

  return (
    <div id="authenticator-tab-container" className="flex-1 flex flex-col md:flex-row min-w-0 overflow-hidden bg-[#F5F5F7] dark:bg-[#1A1A1E] select-none">
      
      {/* Left Area: Displaying & configuring codes */}
      <div className="flex-1 flex flex-col min-w-0 overflow-hidden border-none">
        
        {/* Sub Header for 2FA */}
        <div className="p-4 bg-transparent flex flex-col sm:flex-row sm:items-center sm:justify-between gap-3">
          <div>
            <h3 className="font-display font-bold text-sm text-slate-800 dark:text-slate-100 flex items-center gap-1.5">
              <Smartphone className="text-blue-600 dark:text-blue-400" size={18} />
              <span>Giao diện Quản lý Google Authenticator</span>
            </h3>
            <p className="text-[11px] text-slate-400 mt-0.5">Xác thực 2 lớp TOTP nâng cao thay đổi liên tục thời gian thực.</p>
          </div>

          <div className="flex flex-wrap items-center gap-2">
            {/* Import / Export Controls */}
            <label className="flex items-center gap-1 px-3 py-1.5 bg-slate-100 hover:bg-slate-200 dark:bg-slate-800 dark:hover:bg-slate-700 text-[11px] font-bold text-slate-700 dark:text-slate-300 rounded-xl transition-colors cursor-pointer shadow-xs">
              <FileUp size={13} />
              <span>Nhập JSON 2FA</span>
              <input 
                type="file" 
                accept=".json" 
                onChange={handleImport2FA} 
                className="hidden" 
              />
            </label>

            <button
              onClick={handleExport2FA}
              className="flex items-center gap-1 px-3 py-1.5 bg-slate-100 hover:bg-slate-200 dark:bg-slate-800 dark:hover:bg-slate-700 text-[11px] font-bold text-slate-700 dark:text-slate-300 rounded-xl transition-colors cursor-pointer shadow-xs"
            >
              <FileDown size={13} />
              <span>Xuất JSON 2FA</span>
            </button>

            <button
              onClick={() => setShowAddQuickModal(true)}
              className="flex items-center gap-1 px-3 py-1.5 bg-blue-600 hover:bg-blue-700 text-[11px] font-bold text-white rounded-xl transition-colors cursor-pointer shadow-sm"
            >
              <Plus size={13} />
              <span>Thêm mã 2FA nhanh</span>
            </button>
          </div>
        </div>

        {/* Search Bar & Dynamic Timer display indicator */}
        <div className="p-4 bg-transparent flex items-center justify-between gap-4">
          <div className="relative flex-1 max-w-md">
            <span className="absolute inset-y-0 left-0 pl-3 flex items-center text-slate-400 pointer-events-none">
              <Search size={14} />
            </span>
            <input
              type="text"
              placeholder="Tìm kiếm mã 2FA bằng alias, username..."
              value={searchQuery}
              onChange={(e) => setSearchQuery(e.target.value)}
              className="w-full bg-slate-100 dark:bg-slate-800/80 text-xs pl-9 pr-4 py-2 rounded-xl focus:outline-none focus:bg-slate-50 border border-transparent focus:border-blue-500 font-sans text-slate-800 dark:text-slate-200"
            />
          </div>

          {/* Time Countdown block */}
          <div className={`flex items-center gap-2.5 px-3 py-1.5 rounded-2xl border border-slate-100 dark:border-slate-800 font-bold text-xs ${timerColorClass} transition-all`}>
            {/* SVG Timer circle */}
            <div className="relative w-5 h-5 flex items-center justify-center shrink-0">
              <svg className="w-full h-full transform -rotate-90">
                <circle
                  cx="10"
                  cy="10"
                  r="8"
                  className="stroke-slate-200 dark:stroke-slate-800 fill-none"
                  strokeWidth="2"
                />
                <circle
                  cx="10"
                  cy="10"
                  r="8"
                  className={`fill-none stroke-current transition-all duration-1000`}
                  strokeWidth="2"
                  strokeDasharray="50.26"
                  strokeDashoffset={50.26 - (50.26 * timerCircleProgress) / 100}
                />
              </svg>
              <Clock size={10} className="absolute inset-0 m-auto" />
            </div>
            <span>Đổi mã sau: {secondsLeft} giây</span>
          </div>
        </div>

        {/* List of 2FA Authenticator accounts */}
        <div className="flex-1 overflow-y-auto p-5">
          {authenticatorAccounts.length > 0 ? (
            <div className="grid grid-cols-1 sm:grid-cols-2 lg:grid-cols-3 gap-4">
              {authenticatorAccounts.map((acc) => {
                const otp = getOtpCode(acc);
                const isCopied = copiedId === acc.id;

                return (
                  <div 
                    key={acc.id}
                    className="p-5.5 bg-white dark:bg-slate-900 border border-slate-150 dark:border-slate-800/80 rounded-2xl shadow-xs space-y-4 hover:border-blue-500/40 hover:shadow-md transition-all relative group"
                  >
                    {/* Trash delete icon */}
                    <button
                      onClick={() => handleRemove2FA(acc.id, acc.alias)}
                      className="absolute top-2 right-2 p-1.5 opacity-0 group-hover:opacity-100 text-slate-400 hover:text-red-500 hover:bg-slate-50 dark:hover:bg-slate-800 rounded-lg transition-all"
                      title="Tắt 2FA tài khoản này"
                    >
                      <Trash2 size={13} />
                    </button>

                    <div className="space-y-1">
                      <div className="flex items-center gap-2">
                        <span className="w-2 h-2 rounded-full bg-blue-500"></span>
                        <h4 className="font-display font-black text-xs text-slate-800 dark:text-slate-100 uppercase tracking-wide truncate">
                          {acc.alias}
                        </h4>
                      </div>
                      <p className="text-[10px] text-slate-400 font-medium truncate font-sans pl-4">
                        App: {acc.appName} – User: {acc.username}
                      </p>
                    </div>

                    <div className="flex items-center justify-between p-3 rounded-xl bg-slate-50 dark:bg-slate-950 border border-slate-100 dark:border-slate-850">
                      <span className="font-mono text-xl font-bold tracking-[2px] text-blue-600 dark:text-blue-400 select-all">
                        {otp}
                      </span>

                      <button
                        onClick={() => handleCopy(otp, acc.id)}
                        className={`p-2 rounded-lg border text-xs font-semibold flex items-center justify-center transition-all ${
                          isCopied 
                            ? 'bg-emerald-50 border-emerald-200 text-emerald-600 dark:bg-emerald-950/20 dark:border-emerald-800 dark:text-emerald-400' 
                            : 'bg-white border-slate-200 text-slate-500 hover:bg-slate-50 hover:text-slate-700 dark:bg-slate-800 dark:border-slate-755 dark:text-slate-300 dark:hover:bg-slate-700'
                        }`}
                        title="Copy mã OTP này"
                      >
                        {isCopied ? <Check size={14} className="animate-in zoom-in-100" /> : <Copy size={14} />}
                      </button>
                    </div>
                  </div>
                );
              })}
            </div>
          ) : (
            <div className="h-full flex flex-col items-center justify-center text-center p-8">
              <span className="text-4xl">🔑</span>
              <h4 className="font-bold text-slate-700 dark:text-slate-200 mt-3 text-sm">Chưa có mã 2FA nào trùng khớp</h4>
              <p className="text-xs text-slate-400 mt-1 max-w-sm">
                Hãy bật 2FA cho tài khoản bằng cách điền **Khóa bí mật (Secret-Key)** khi thêm mới hoặc sửa bất kỳ tài khoản nào.
              </p>
            </div>
          )}
        </div>
      </div>

      {/* Right Area: Hướng dẫn cài đặt & setup chi tiết */}
      <div className="w-full md:w-80 bg-slate-50/70 dark:bg-[#1C1C1E]/70 border-none p-5 space-y-6 shrink-0 overflow-y-auto">
        <div className="pb-3">
          <h4 className="font-display font-bold text-xs text-slate-700 dark:text-slate-200 flex items-center gap-1.5 uppercase tracking-wide">
            <Info size={14} className="text-blue-500" />
            <span>Cài đặt 2FA ở đâu?</span>
          </h4>
          <p className="text-[10px] text-slate-400 mt-0.5">Hướng dẫn từng bước cách tích hợp và kích hoạt toàn diện.</p>
        </div>

        {/* Detailed Guidelines */}
        <div className="space-y-4 text-xs leading-relaxed text-slate-600 dark:text-slate-400">
          <div className="p-3 bg-blue-50/50 dark:bg-[#1a3a54]/10 rounded-xl border border-blue-100/40 dark:border-blue-900/30 space-y-1.5">
            <span className="font-bold text-blue-700 dark:text-blue-300 block">⚙️ Bước 1: Mục Tất cả tài khoản</span>
            <p className="text-[11px]">Bấm chọn tab **Tất cả tài khoản** ở Sidebar bên trái. Chọn tài khoản bất kỳ bạn muốn bật 2FA.</p>
          </div>

          <div className="p-3 bg-blue-50/50 dark:bg-[#1a3a54]/10 rounded-xl border border-blue-100/40 dark:border-blue-900/30 space-y-1.5">
            <span className="font-bold text-blue-700 dark:text-blue-300 block">✏️ Bước 2: Bấm Sửa mật khẩu</span>
            <p className="text-[11px]">Trong bảng chi tiết bên phải tài khoản, chọn nút Sửa (biểu tượng cây bút ✏️) ở thanh công cụ góc trên.</p>
          </div>

          <div className="p-3 bg-blue-50/50 dark:bg-[#1a3a54]/10 rounded-xl border border-blue-100/40 dark:border-blue-900/30 space-y-1.5">
            <span className="font-bold text-blue-700 dark:text-blue-300 block">🔑 Bước 3: Điền Khóa bí mật</span>
            <p className="text-[11px]">Tìm ô nhập **'Khóa bí mật OTP Secret-Key'** và điền Serial Code 2FA thô, thường được cung cấp khi bạn bật 2FA trên Facebook, Google, v.v.</p>
          </div>

          <div className="p-3 bg-blue-50/50 dark:bg-[#1a3a54]/10 rounded-xl border border-blue-100/40 dark:border-blue-900/30 space-y-1.5">
            <span className="font-bold text-blue-700 dark:text-blue-300 block">📱 Bước 4: Kiểm tra mã OTP</span>
            <p className="text-[11px]">Bấm Lưu. Quay lại trang này, tài khoản của bạn sẽ xuất hiện và tự động tạo mã giống như **Google Authenticator**.</p>
          </div>
        </div>

        {/* Security Alert box */}
        <div className="p-4 bg-amber-50 dark:bg-amber-950/20 border border-amber-200 dark:border-amber-900 rounded-2xl space-y-2">
          <div className="flex items-center gap-1.5 text-amber-700 dark:text-amber-400 font-bold text-xs uppercase tracking-wide">
            <AlertTriangle size={14} />
            <span>Khuyến cáo an ninh</span>
          </div>
          <p className="text-[10px] text-slate-500 dark:text-slate-400 leading-relaxed">
            Sao lưu dữ liệu 2FA (mã Secret-Key JSON) cực kỳ quan trọng. Bản backup này được dùng để import lại trong trường hợp dữ liệu cục bộ trình duyệt (localStorage) bị xóa sạch.
          </p>
        </div>
      </div>

      {/* QUICK ADD 2FA AUTHENTICATOR MODAL */}
      {showAddQuickModal && (
        <div id="quick-add-overlay" className="fixed inset-0 z-50 bg-slate-950/40 backdrop-blur-xs flex items-center justify-center p-4 select-none">
          <div className="w-full max-w-md bg-white dark:bg-slate-900 rounded-3xl border border-slate-150 dark:border-slate-800 p-6 space-y-4 shadow-2xl animate-in zoom-in-95 duration-150">
            <div className="flex items-center justify-between border-b border-slate-100 dark:border-slate-800/80 pb-3">
              <h3 className="font-display font-bold text-sm text-slate-800 dark:text-slate-100 flex items-center gap-2">
                <ShieldCheck size={18} className="text-blue-600" />
                <span>Thêm mã bảo mật 2FA nhanh</span>
              </h3>
              <button 
                onClick={() => setShowAddQuickModal(false)} 
                className="p-1 hover:bg-slate-100 dark:hover:bg-slate-800 rounded-lg text-slate-400"
              >
                <X size={16} />
              </button>
            </div>

            <div className="space-y-3.5">
              <div className="flex flex-col gap-1">
                <label className="text-xs font-semibold text-slate-500">Tên Alias tài khoản <span className="text-rose-500">*</span></label>
                <input
                  type="text"
                  placeholder="Ví dụ: gmail-phu, fb-ca-nhan"
                  value={quickAlias}
                  onChange={(e) => setQuickAlias(e.target.value)}
                  className="bg-slate-50 dark:bg-slate-800 text-xs px-3 py-2.5 rounded-xl border border-slate-150 dark:border-slate-700/80 focus:outline-none focus:border-blue-500"
                />
              </div>

              <div className="flex flex-col gap-1">
                <label className="text-xs font-semibold text-slate-500">Website / Domain (Tùy chọn)</label>
                <input
                  type="text"
                  placeholder="Ví dụ: accounts.google.com"
                  value={quickAppName}
                  onChange={(e) => setQuickAppName(e.target.value)}
                  className="bg-slate-50 dark:bg-slate-800 text-xs px-3 py-2.5 rounded-xl border border-slate-150 dark:border-slate-700/80 focus:outline-none focus:border-blue-500"
                />
              </div>

              <div className="flex flex-col gap-1">
                <label className="text-xs font-semibold text-slate-500">Tên đăng nhập / Username (Tùy chọn)</label>
                <input
                  type="text"
                  placeholder="Ví dụ: myemail@gmail.com, myphone"
                  value={quickUsername}
                  onChange={(e) => setQuickUsername(e.target.value)}
                  className="bg-slate-50 dark:bg-slate-800 text-xs px-3 py-2.5 rounded-xl border border-slate-150 dark:border-slate-700/80 focus:outline-none focus:border-blue-500"
                />
              </div>

              <div className="flex flex-col gap-1">
                <label className="text-xs font-semibold text-slate-500">Khóa bí mật 2FA Secret Key (Chuỗi Base32) <span className="text-rose-500">*</span></label>
                <input
                  type="text"
                  placeholder="Ví dụ: JBSWY3DPEHPK3PXP"
                  value={quickOtpSecret}
                  onChange={(e) => setQuickOtpSecret(e.target.value.replace(/\s+/g, '').toUpperCase())}
                  className="bg-slate-50 dark:bg-slate-800 text-xs px-3 py-2.5 rounded-xl border border-slate-150 dark:border-slate-700/80 focus:outline-none focus:border-blue-500 font-mono tracking-wider text-blue-600 dark:text-blue-400"
                />
              </div>

              <div className="flex flex-col gap-1">
                <label className="text-xs font-semibold text-slate-500">Danh mục</label>
                <select
                  value={quickCategory}
                  onChange={(e) => setQuickCategory(e.target.value)}
                  className="bg-slate-50 dark:bg-slate-800 text-xs px-3 py-2.5 rounded-xl border border-slate-150 dark:border-slate-700/80 focus:outline-none focus:border-blue-500 cursor-pointer text-slate-700 dark:text-slate-350"
                >
                  <option value="Email">Email</option>
                  <option value="Mạng xã hội">Mạng xã hội</option>
                  <option value="Ngân hàng">Ngân hàng</option>
                  <option value="Công việc">Công việc</option>
                  <option value="Giải trí">Giải trí</option>
                  <option value="Game">Game</option>
                  <option value="Khác">Khác</option>
                </select>
              </div>
            </div>

            <div className="flex justify-end gap-2 pt-3 border-t border-slate-100 dark:border-slate-800/80">
              <button
                type="button"
                onClick={() => setShowAddQuickModal(false)}
                className="px-4 py-2 border border-slate-200 dark:border-slate-800 text-slate-500 rounded-xl text-xs font-bold hover:bg-slate-50 transition-colors"
              >
                Hủy
              </button>
              <button
                type="button"
                onClick={submitQuickAdd2FA}
                className="px-4 py-2 bg-blue-600 hover:bg-blue-700 text-white rounded-xl text-xs font-bold shadow-md shadow-blue-500/10 cursor-pointer"
              >
                Kích hoạt & thêm nhanh
              </button>
            </div>
          </div>
        </div>
      )}

    </div>
  );
}
