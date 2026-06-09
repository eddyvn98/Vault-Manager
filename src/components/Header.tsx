/**
 * @license
 * SPDX-License-Identifier: Apache-2.0
 */

import React from 'react';
import { Search, User, Check, Laptop, Shield, Bot, Menu } from 'lucide-react';

interface HeaderProps {
  searchQuery: string;
  setSearchQuery: (query: string) => void;
  telegramPin: string;
  telegramId: string;
  setTelegramPin: (pin: string) => void;
  devicesCount: number;
  onToggleSidebar?: () => void;
}

export default function Header({
  searchQuery,
  setSearchQuery,
  telegramPin,
  telegramId,
  setTelegramPin,
  devicesCount,
  onToggleSidebar
}: HeaderProps) {
  const [showProfileMenu, setShowProfileMenu] = React.useState(false);
  const [pinInput, setPinInput] = React.useState(telegramPin);
  const [savedMessage, setSavedMessage] = React.useState(false);

  const handleUpdatePin = (e: React.FormEvent) => {
    e.preventDefault();
    if (pinInput.length >= 4) {
      setTelegramPin(pinInput);
      setSavedMessage(true);
      setTimeout(() => setSavedMessage(false), 2000);
    }
  };

  return (
    <header id="header-container" className="h-16 border-none bg-transparent px-4 sm:px-6 flex items-center justify-between select-none shrink-0 relative z-10 w-full gap-3">
      {/* Central Search Bar & Hamburger */}
      <div className="flex items-center gap-3 flex-1 max-w-xl">
        <button
          onClick={onToggleSidebar}
          className="lg:hidden p-2 -ml-1 text-slate-500 hover:text-slate-800 dark:text-slate-400 dark:hover:text-slate-100 rounded-lg hover:bg-slate-50 dark:hover:bg-slate-800 transition-colors shrink-0"
          title="Mở menu"
        >
          <Menu size={20} />
        </button>

        <div id="search-input-box" className="relative group flex-1">
          <span className="absolute inset-y-0 left-0 flex items-center pl-3 text-slate-400">
            <Search size={16} className="transition-colors group-focus-within:text-[#007AFF]" />
          </span>
          <input
            id="global-search-input"
            type="text"
            value={searchQuery}
            onChange={(e) => setSearchQuery(e.target.value)}
            placeholder="Tìm kiếm tài khoản..."
            className="w-full bg-[#E5E5EA]/70 dark:bg-[#2C2C2E] text-xs text-slate-800 dark:text-zinc-100 placeholder-slate-400/80 pl-9 pr-4 py-2 rounded-lg border-none focus:bg-white dark:focus:bg-[#1E1E1E] focus:outline-none focus:ring-2 focus:ring-[#007AFF]/35 dark:focus:ring-[#0A84FF]/35 transition-all font-sans"
          />
        </div>
      </div>

      {/* Right Side Utility Actions */}
      <div id="header-utils" className="flex items-center gap-4">
        {/* Sync Agent Pulse Info */}
        <div id="sync-agents-header-badge" className="hidden md:flex items-center gap-2 px-3 py-1.5 rounded-full bg-slate-50 dark:bg-slate-800 text-xs text-slate-500 dark:text-slate-400 border border-slate-100 dark:border-slate-700/50">
          <span className="w-1.5 h-1.5 rounded-full bg-emerald-500 animate-ping"></span>
          <span className="font-medium">{devicesCount} Sync Agents Online</span>
        </div>

        {/* User Account / Admin Panel */}
        <div id="admin-profile-wrapper" className="relative">
          <button
            id="btn-profile-dropdown"
            onClick={() => setShowProfileMenu(!showProfileMenu)}
            className="flex items-center gap-2 p-1.5 pl-2.5 pr-3 rounded-full hover:bg-slate-50 dark:hover:bg-slate-800 border border-slate-100 dark:border-slate-800 transition-colors"
          >
            <div id="profile-avatar-bg" className="w-6 h-6 rounded-full bg-blue-100 dark:bg-blue-900/60 text-blue-600 dark:text-blue-300 flex items-center justify-center">
              <User size={14} className="stroke-[2.5]" />
            </div>
            <span id="profile-display-name" className="text-sm font-semibold text-slate-700 dark:text-slate-200 hidden sm:inline">
              Admin
            </span>
          </button>

          {/* Profile Dropdown Menu */}
          {showProfileMenu && (
            <>
              <div id="dropdown-backdrop" className="fixed inset-0 z-10" onClick={() => setShowProfileMenu(false)} />
              <div id="profile-dropdown-box" className="absolute right-0 mt-2 w-72 bg-white dark:bg-[#1C1C1E] rounded-xl shadow-[0_12px_40px_rgba(0,0,0,0.12)] p-4 space-y-4 z-20 animate-in fade-in slide-in-from-top-2 duration-150 border border-slate-100/50 dark:border-zinc-800/85">
                <div id="dropdown-profile-header" className="flex items-center gap-3 border-b border-slate-100/65 dark:border-zinc-800/60 pb-3">
                  <div id="avatar-logo-sub" className="w-10 h-10 rounded-full bg-[#007AFF] flex items-center justify-center text-white font-bold">
                    A
                  </div>
                  <div>
                    <h4 id="dropdown-user-title" className="font-semibold text-sm text-slate-800 dark:text-slate-100 leading-none">Administrator</h4>
                    <span id="dropdown-user-email" className="text-xs text-slate-400 dark:text-slate-500">eddyvn98@gmail.com</span>
                  </div>
                </div>

                <div id="dropdown-telegram-info" className="space-y-2.5">
                  <div className="flex items-center gap-2 text-[10px] font-bold text-slate-450 dark:text-zinc-500 uppercase tracking-widest">
                    <Bot size={13} className="text-[#007AFF] dark:text-[#0A84FF]" />
                    <span>Telegram Integration</span>
                  </div>
                  
                  <div className="bg-[#F2F2F7] dark:bg-[#2C2C2E]/60 p-3 rounded-lg space-y-1 my-1 border border-slate-100/40 dark:border-zinc-800/10">
                    <div className="flex justify-between text-xs">
                      <span className="text-slate-500">Telegram ID:</span>
                      <span className="font-mono text-slate-800 dark:text-zinc-350 font-semibold">{telegramId}</span>
                    </div>
                    <div className="flex justify-between text-xs">
                      <span className="text-slate-500">Ủy quyền PIN:</span>
                      <span className="font-mono text-emerald-650 dark:text-[#30D158] font-bold">24 Giờ/Lần</span>
                    </div>
                  </div>

                  <form onSubmit={handleUpdatePin} className="space-y-1.5 animate-none">
                    <label className="block text-[11px] font-semibold text-slate-500 dark:text-zinc-400 pl-0.5">
                      Mã PIN bảo mật Telegram
                    </label>
                    <div className="flex gap-2">
                      <input
                        type="text"
                        maxLength={6}
                        value={pinInput}
                        onChange={(e) => setPinInput(e.target.value.replace(/\D/g, ''))}
                        placeholder="Mã PIN (v.v 2580)"
                        className="flex-1 bg-[#F2F2F7] dark:bg-[#2C2C2E] text-xs px-2.5 py-1.5 rounded-lg border border-[#E5E5EA] dark:border-[#3A3A3C] focus:bg-white dark:focus:bg-[#1C1C1E] focus:outline-none focus:ring-2 focus:ring-[#007AFF]/35 dark:focus:ring-[#0A84FF]/35 transition-all font-mono text-center font-bold text-slate-800 dark:text-zinc-150"
                      />
                      <button
                        type="submit"
                        className="px-4.5 bg-[#007AFF] hover:bg-[#0063E1] dark:bg-[#0A84FF] dark:hover:bg-[#0072E3] text-white rounded-lg flex items-center justify-center transition-all shadow-sm text-xs py-1.5 active:scale-[0.98] font-semibold cursor-pointer shrink-0"
                      >
                        {savedMessage ? <Check size={14} /> : 'Lưu'}
                      </button>
                    </div>
                    {savedMessage && (
                      <span className="text-[10px] text-emerald-600 dark:text-[#30D158] font-semibold block text-right animate-pulse">
                        Đã cập nhật mã PIN thành công!
                      </span>
                    )}
                  </form>
                </div>

                <div className="border-t border-slate-100/50 dark:border-zinc-850/60 pt-3 flex gap-2">
                  <div className="w-full flex items-center gap-1.5 text-[10px] text-slate-400">
                    <Shield size={12} className="text-[#30D158]" />
                    <span>Server: Local Host (Secure)</span>
                  </div>
                </div>
              </div>
            </>
          )}
        </div>
      </div>
    </header>
  );
}
