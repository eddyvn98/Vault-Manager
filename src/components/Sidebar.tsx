/**
 * @license
 * SPDX-License-Identifier: Apache-2.0
 */

import React from 'react';
import { 
  ShieldCheck, 
  Layers, 
  Trash2, 
  Star, 
  Mail, 
  Share2, 
  Coins, 
  Briefcase, 
  Heart, 
  Gamepad2, 
  MoreHorizontal, 
  Plus, 
  KeyRound, 
  ShieldAlert, 
  RefreshCw, 
  FileUp, 
  FileDown, 
  Settings,
  Bot,
  Smartphone,
  X,
  ChevronDown,
  ChevronLeft,
  ChevronRight
} from 'lucide-react';
import { Account } from '../types';

interface SidebarProps {
  currentTab: string;
  setCurrentTab: (tab: string) => void;
  selectedCategory: string | null;
  setSelectedCategory: (cat: string | null) => void;
  accounts: Account[];
  onAddCategory: () => void;
  categories: { name: string; icon: React.ReactNode; color: string }[];
  isOpen: boolean;
  onClose: () => void;
  isMinified: boolean;
  setIsMinified: (minified: boolean) => void;
}

export default function Sidebar({
  currentTab,
  setCurrentTab,
  selectedCategory,
  setSelectedCategory,
  accounts,
  onAddCategory,
  categories,
  isOpen,
  onClose,
  isMinified,
  setIsMinified
}: SidebarProps) {
  
  // Collapsible Advanced Settings & Tools state
  const isAdvancedActive = ['security', 'sync', 'telegram', 'import', 'export', 'settings'].includes(currentTab);
  const [isCollapsed, setIsCollapsed] = React.useState(!isAdvancedActive);

  React.useEffect(() => {
    if (isAdvancedActive) {
      setIsCollapsed(false);
    }
  }, [currentTab, isAdvancedActive]);

  // Count accounts by criteria
  const totalCount = accounts.filter(a => !a.isDeleted).length;
  const favoriteCount = accounts.filter(a => a.isFavorite && !a.isDeleted).length;
  const deletedCount = accounts.filter(a => a.isDeleted).length;

  const getCategoryCount = (categoryName: string) => {
    return accounts.filter(a => a.category === categoryName && !a.isDeleted).length;
  };

  const handleMainMenuClick = (tab: string) => {
    setCurrentTab(tab);
    setSelectedCategory(null);
    onClose();
  };

  const handleCategoryClick = (categoryName: string) => {
    setCurrentTab('accounts');
    setSelectedCategory(categoryName);
    onClose();
  };

  return (
    <>
      {/* Mobile sidebar backdrop overlay */}
      {isOpen && (
        <div 
          onClick={onClose} 
          className="fixed inset-0 z-30 bg-slate-950/45 backdrop-blur-xs lg:hidden"
        />
      )}

      <aside 
        id="sidebar-container" 
        className={`fixed inset-y-0 left-0 z-40 lg:static lg:flex flex-col h-full select-none shrink-0 bg-[#F2F2F7] dark:bg-[#1C1C1E] transition-all duration-300 ease-in-out ${
          isMinified ? 'lg:w-[72px]' : 'lg:w-64'
        } ${
          isOpen ? 'translate-x-0 w-64' : '-translate-x-full lg:translate-x-0 lg:overflow-visible'
        }`}
      >
        {/* macOS Traffic Lights and desktop fold button */}
        {!isMinified ? (
          <div className="px-5 pt-4 flex items-center justify-between shrink-0 select-none pb-1">
            <div className="flex items-center gap-1.5">
              <div className="w-3 h-3 rounded-full bg-[#FF5F56] border border-[#FF5F56]/15 hover:opacity-85 transition-opacity" />
              <div className="w-3 h-3 rounded-full bg-[#FFBD2E] border border-[#FFBD2E]/15 hover:opacity-85 transition-opacity" />
              <div className="w-3 h-3 rounded-full bg-[#27C93F] border border-[#27C93F]/15 hover:opacity-85 transition-opacity" />
            </div>
            <button
              id="sidebar-desktop-collapse-btn"
              onClick={() => setIsMinified(true)}
              className="hidden lg:flex p-1 rounded-md hover:bg-slate-200 dark:hover:bg-slate-800 text-slate-400 hover:text-slate-600 transition-colors"
              title="Thu nhỏ thanh bên"
            >
              <ChevronLeft size={16} />
            </button>
          </div>
        ) : (
          <div className="pt-4 flex justify-center shrink-0 select-none pb-1">
            <button
              id="sidebar-desktop-expand-btn"
              onClick={() => setIsMinified(false)}
              className="hidden lg:flex p-1.5 rounded-lg bg-white dark:bg-zinc-800 border border-slate-200/60 dark:border-zinc-700/60 shadow-xs text-slate-500 hover:text-slate-800 dark:text-slate-400 dark:hover:text-slate-100 transition-colors"
              title="Mở rộng thanh bên"
            >
              <ChevronRight size={16} />
            </button>
          </div>
        )}

        {/* Brand Header */}
        <div id="brand-header" className={`p-5 pt-3 pb-4 flex items-center justify-between gap-3 ${isMinified ? 'lg:justify-center lg:px-2 lg:pb-2' : ''}`}>
          <div className={`flex items-center gap-3 ${isMinified ? 'lg:flex-col lg:gap-1 lg:items-center' : ''}`}>
            <div id="brand-logo-bg" className="w-10 h-10 rounded-xl bg-blue-600 flex items-center justify-center text-white shadow-md shadow-blue-500/20 shrink-0">
              <ShieldCheck id="brand-logo" size={22} className="stroke-[2.5]" />
            </div>
            <div className={`${isMinified ? 'lg:hidden' : 'block'}`}>
              <h1 id="brand-title" className="font-display font-bold text-lg text-slate-800 dark:text-slate-100 leading-tight">
                Vault Manager
              </h1>
              <span id="brand-subtitle" className="text-xs font-medium text-slate-400 dark:text-slate-500 block">
                Offline 24/7 Sync Bot
              </span>
            </div>
          </div>

          <button
            onClick={onClose}
            className={`${isMinified ? 'lg:hidden' : ''} lg:hidden p-1.5 rounded-lg hover:bg-slate-100 dark:hover:bg-slate-800 text-slate-500 hover:text-slate-700 dark:text-slate-400 transition-colors`}
            title="Đóng menu"
          >
            <X size={18} />
          </button>
        </div>

      {/* Main Navigation Scroll Area */}
      <div id="sidebar-scroll" className={`flex-1 overflow-y-auto p-4 space-y-6 ${isMinified ? 'lg:p-2 lg:space-y-4' : ''}`}>
        
        {/* Main Sections */}
        <div id="sidebar-section-main" className="space-y-1">
          <button
            id="menu-btn-all-accounts"
            onClick={() => handleMainMenuClick('accounts')}
            title="Tất cả tài khoản"
            className={`w-full flex items-center justify-between px-3 py-2.5 rounded-lg text-sm font-medium transition-colors ${
              currentTab === 'accounts' && selectedCategory === null
                ? 'bg-blue-50 text-blue-600 dark:bg-blue-950/40 dark:text-blue-400'
                : 'text-slate-600 hover:bg-slate-50 dark:text-slate-400 dark:hover:bg-slate-800/60'
            } ${isMinified ? 'lg:justify-center lg:px-0 lg:py-3' : ''}`}
          >
            <div id="menu-btn-all-accounts-label" className={`flex items-center gap-2.5 ${isMinified ? 'lg:flex-col lg:gap-0' : ''}`}>
              <Layers id="icon-all-accounts" size={17} />
              <span className={`${isMinified ? 'lg:hidden' : 'block'}`}>Tất cả tài khoản</span>
            </div>
            <span id="count-all-accounts" className={`text-xs px-2 py-0.5 rounded-full bg-slate-100 dark:bg-slate-800 text-slate-500 font-semibold ${isMinified ? 'lg:hidden' : 'block'}`}>
              {totalCount}
            </span>
          </button>

          <button
            id="menu-btn-favorites"
            onClick={() => handleMainMenuClick('favorites')}
            title="Ưa thích"
            className={`w-full flex items-center justify-between px-3 py-2.5 rounded-lg text-sm font-medium transition-colors ${
              currentTab === 'favorites'
                ? 'bg-blue-50 text-blue-600 dark:bg-blue-950/40 dark:text-blue-400'
                : 'text-slate-600 hover:bg-slate-50 dark:text-slate-400 dark:hover:bg-slate-800/60'
            } ${isMinified ? 'lg:justify-center lg:px-0 lg:py-3' : ''}`}
          >
            <div id="menu-btn-favorites-label" className={`flex items-center gap-2.5 ${isMinified ? 'lg:flex-col lg:gap-0' : ''}`}>
              <Star id="icon-favorites" size={17} />
              <span className={`${isMinified ? 'lg:hidden' : 'block'}`}>Ưa thích</span>
            </div>
            <span id="count-favorites" className={`text-xs px-2 py-0.5 rounded-full bg-slate-100 dark:bg-slate-800 text-slate-500 font-semibold ${isMinified ? 'lg:hidden' : 'block'}`}>
              {favoriteCount}
            </span>
          </button>

          <button
            id="menu-btn-trash"
            onClick={() => handleMainMenuClick('trash')}
            title="Thùng rác"
            className={`w-full flex items-center justify-between px-3 py-2.5 rounded-lg text-sm font-medium transition-colors ${
              currentTab === 'trash'
                ? 'bg-red-50 text-red-600 dark:bg-red-950/20 dark:text-red-400'
                : 'text-slate-600 hover:bg-slate-50 dark:text-slate-400 dark:hover:bg-slate-800/60'
            } ${isMinified ? 'lg:justify-center lg:px-0 lg:py-3' : ''}`}
          >
            <div id="menu-btn-trash-label" className={`flex items-center gap-2.5 ${isMinified ? 'lg:flex-col lg:gap-0' : ''}`}>
              <Trash2 id="icon-trash" size={17} />
              <span className={`${isMinified ? 'lg:hidden' : 'block'}`}>Thùng rác</span>
            </div>
            <span id="count-trash" className={`text-xs px-2 py-0.5 rounded-full bg-red-100/60 dark:bg-red-950/40 text-red-600 dark:text-red-400 font-semibold ${isMinified ? 'lg:hidden' : 'block'}`}>
              {deletedCount}
            </span>
          </button>
        </div>


        {/* Tools Section */}
        <div id="sidebar-section-tools" className="space-y-1.5 animate-duration-150">
          <div id="tools-header" className={`px-3 text-[11px] font-bold text-slate-400 dark:text-slate-500 tracking-wider uppercase ${isMinified ? 'lg:hidden' : 'block'}`}>
            CÔNG CỤ
          </div>
          
          <div className={`border-t border-slate-200/50 dark:border-slate-800/50 my-1 ${isMinified ? 'hidden lg:block' : 'hidden'}`} />

          <div id="tools-list" className="space-y-0.5">
            <button
              id="tool-btn-authenticator"
              onClick={() => handleMainMenuClick('authenticator')}
              title="2FA Authenticator"
              className={`w-full flex items-center justify-between px-3 py-2 rounded-lg text-sm font-semibold transition-colors ${
                currentTab === 'authenticator'
                  ? 'bg-blue-50 text-blue-600 dark:bg-blue-950/40 dark:text-blue-400'
                  : 'text-slate-600 hover:bg-slate-50 dark:text-slate-400 dark:hover:bg-slate-800/60'
              } ${isMinified ? 'lg:justify-center lg:px-0 lg:py-2.5' : ''}`}
            >
              <div className={`flex items-center gap-2.5 ${isMinified ? 'lg:flex-col lg:gap-0' : ''}`}>
                <Smartphone id="icon-authenticator" size={16} className="text-blue-500" />
                <span className={`font-semibold text-slate-700 dark:text-slate-200 ${isMinified ? 'lg:hidden' : 'block'}`}>2FA Authenticator</span>
              </div>
              {accounts.filter(a => a.otpSecret && !a.isDeleted).length > 0 && (
                <span className={`text-[10px] px-1.5 py-0.5 rounded-full bg-blue-100 text-blue-600 dark:bg-blue-950 dark:text-blue-400 font-bold ${isMinified ? 'lg:hidden' : 'block'}`}>
                  {accounts.filter(a => a.otpSecret && !a.isDeleted).length}
                </span>
              )}
            </button>

            <button
              id="tool-btn-generator"
              onClick={() => handleMainMenuClick('generator')}
              title="Trình tạo mật khẩu"
              className={`w-full flex items-center gap-2.5 px-3 py-2 rounded-lg text-sm font-medium transition-colors ${
                currentTab === 'generator'
                  ? 'bg-blue-50 text-blue-600 dark:bg-blue-950/40 dark:text-blue-400'
                  : 'text-slate-600 hover:bg-slate-50 dark:text-slate-400 dark:hover:bg-slate-800/60'
              } ${isMinified ? 'lg:justify-center lg:px-0 lg:py-2.5' : ''}`}
            >
              <KeyRound id="icon-generator" size={16} className="text-slate-400" />
              <span className={`${isMinified ? 'lg:hidden' : 'block'}`}>Trình tạo mật khẩu</span>
            </button>

            {/* Collapsible toggle for advanced/less used utilities */}
            <div className={`pt-2 border-t border-slate-200/50 dark:border-slate-800/50 mt-2 ${isMinified ? 'lg:border-t-0 lg:pt-0 lg:mt-0' : ''}`}>
              {!isMinified && (
                <button
                  id="tool-btn-toggle-advanced"
                  onClick={() => setIsCollapsed(!isCollapsed)}
                  className="w-full flex items-center justify-between px-3 py-1.5 text-[10px] font-bold text-slate-400 hover:text-slate-600 dark:text-slate-500 dark:hover:text-slate-300 transition-colors uppercase tracking-wider"
                >
                  <span>Hệ thống & Đồng bộ</span>
                  <ChevronDown 
                    size={12} 
                    className={`transform transition-transform duration-200 ${isCollapsed ? '' : 'rotate-180'}`} 
                  />
                </button>
              )}
              
              {(!isCollapsed || isMinified) && (
                <div 
                  id="advanced-tools-list" 
                  className={`mt-1 space-y-0.5 ml-3.5 pl-1.5 ${
                    isMinified 
                      ? 'lg:ml-0 lg:pl-0 lg:mt-2 lg:space-y-2' 
                      : 'border-l border-slate-200/80 dark:border-slate-800/80'
                  }`}
                >
                  <button
                    id="tool-btn-security"
                    onClick={() => handleMainMenuClick('security')}
                    title="Kiểm tra bảo mật"
                    className={`w-full flex items-center gap-2.5 px-2.5 py-1.5 rounded-lg text-xs font-medium transition-colors ${
                      currentTab === 'security'
                        ? 'bg-blue-50 text-blue-600 dark:bg-blue-950/40 dark:text-blue-400'
                        : 'text-slate-600 hover:bg-slate-50 dark:text-slate-400 dark:hover:bg-slate-800/60'
                    } ${isMinified ? 'lg:justify-center lg:px-0 lg:py-2.5' : ''}`}
                  >
                    <ShieldAlert id="icon-security-check" size={14} className="text-slate-400" />
                    <span className={`${isMinified ? 'lg:hidden' : 'block'}`}>Kiểm tra bảo mật</span>
                  </button>

                  <button
                    id="tool-btn-sync"
                    onClick={() => handleMainMenuClick('sync')}
                    title="Đồng bộ trình duyệt"
                    className={`w-full flex items-center gap-2.5 px-2.5 py-1.5 rounded-lg text-xs font-medium transition-colors ${
                      currentTab === 'sync'
                        ? 'bg-blue-50 text-blue-600 dark:bg-blue-950/40 dark:text-blue-400'
                        : 'text-slate-600 hover:bg-slate-50 dark:text-slate-400 dark:hover:bg-slate-800/60'
                    } ${isMinified ? 'lg:justify-center lg:px-0 lg:py-2.5' : ''}`}
                  >
                    <RefreshCw id="icon-sync-agent" size={14} className="text-slate-400" />
                    <span className={`${isMinified ? 'lg:hidden' : 'block'}`}>Đồng bộ trình duyệt</span>
                  </button>

                  <button
                    id="tool-btn-telegram"
                    onClick={() => handleMainMenuClick('telegram')}
                    title="Giả lập Telegram Bot"
                    className={`w-full flex items-center gap-2.5 px-2.5 py-1.5 rounded-lg text-xs font-semibold transition-colors ${
                      currentTab === 'telegram'
                        ? 'bg-blue-50 text-blue-600 dark:bg-blue-950/40 dark:text-blue-400'
                        : 'text-slate-600 hover:bg-slate-50 dark:text-slate-400 dark:hover:bg-slate-800/60'
                    } ${isMinified ? 'lg:justify-center lg:px-0 lg:py-2.5' : ''}`}
                  >
                    <Bot id="icon-telegram-bot" size={14} className="text-sky-500" />
                    <span className={`font-semibold text-slate-700 dark:text-slate-200 ${isMinified ? 'lg:hidden' : 'block'}`}>Giả lập Telegram Bot</span>
                  </button>

                  <button
                    id="tool-btn-import"
                    onClick={() => handleMainMenuClick('import')}
                    title="Nhập tài khoản"
                    className={`w-full flex items-center gap-2.5 px-2.5 py-1.5 rounded-lg text-xs font-medium transition-colors ${
                      currentTab === 'import'
                        ? 'bg-blue-50 text-blue-600 dark:bg-blue-950/40 dark:text-blue-400'
                        : 'text-slate-600 hover:bg-slate-50 dark:text-slate-400 dark:hover:bg-slate-800/60'
                    } ${isMinified ? 'lg:justify-center lg:px-0 lg:py-2.5' : ''}`}
                  >
                    <FileUp id="icon-import" size={14} className="text-slate-400" />
                    <span className={`${isMinified ? 'lg:hidden' : 'block'}`}>Nhập tài khoản</span>
                  </button>

                  <button
                    id="tool-btn-export"
                    onClick={() => handleMainMenuClick('export')}
                    title="Xuất dữ liệu"
                    className={`w-full flex items-center gap-2.5 px-2.5 py-1.5 rounded-lg text-xs font-medium transition-colors ${
                      currentTab === 'export'
                        ? 'bg-blue-50 text-blue-600 dark:bg-blue-950/40 dark:text-blue-400'
                        : 'text-slate-600 hover:bg-slate-50 dark:text-slate-400 dark:hover:bg-slate-800/60'
                    } ${isMinified ? 'lg:justify-center lg:px-0 lg:py-2.5' : ''}`}
                  >
                    <FileDown id="icon-export" size={14} className="text-slate-400" />
                    <span className={`${isMinified ? 'lg:hidden' : 'block'}`}>Xuất dữ liệu</span>
                  </button>

                  <button
                    id="tool-btn-settings"
                    onClick={() => handleMainMenuClick('settings')}
                    title="Cài đặt"
                    className={`w-full flex items-center gap-2.5 px-2.5 py-1.5 rounded-lg text-xs font-medium transition-colors ${
                      currentTab === 'settings'
                        ? 'bg-blue-50 text-blue-600 dark:bg-blue-950/40 dark:text-blue-400'
                        : 'text-slate-600 hover:bg-slate-50 dark:text-slate-400 dark:hover:bg-slate-800/60'
                    } ${isMinified ? 'lg:justify-center lg:px-0 lg:py-2.5' : ''}`}
                  >
                    <Settings id="icon-settings" size={14} className="text-slate-400" />
                    <span className={`${isMinified ? 'lg:hidden' : 'block'}`}>Cài đặt</span>
                  </button>
                </div>
              )}
            </div>
          </div>
        </div>
      </div>

      {/* Footer info lock indicator */}
      <div id="sidebar-footer" className={`p-4 bg-transparent flex flex-col justify-center items-center gap-1 mt-auto ${isMinified ? 'lg:p-2 lg:gap-0' : ''}`}>
        <div 
          id="footer-status-badge" 
          title="Bảo mật AES-256 kích hoạt"
          className={`flex items-center gap-1.5 text-xs text-emerald-600 dark:text-emerald-500 font-medium ${isMinified ? 'lg:justify-center' : ''}`}
        >
          <span className="w-2 h-2 rounded-full bg-emerald-500 animate-pulse shrink-0"></span>
          <span className={`${isMinified ? 'lg:hidden' : 'block'}`}>Bảo mật AES-256 kích hoạt</span>
        </div>
        <p className={`text-[10px] text-slate-400 text-center ${isMinified ? 'lg:hidden' : 'block'}`}>Version 1.4.2-STABLE</p>
      </div>
    </aside>
  </>
  );
}
