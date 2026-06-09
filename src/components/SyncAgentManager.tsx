/**
 * @license
 * SPDX-License-Identifier: Apache-2.0
 */

import React from 'react';
import { 
  Laptop, 
  Chrome, 
  Terminal, 
  Play, 
  RefreshCw, 
  Check, 
  Trash2, 
  Zap, 
  CheckCircle,
  Plus,
  Compass,
  Database
} from 'lucide-react';
import { SyncDevice, SyncLog, Account } from '../types';

interface SyncAgentManagerProps {
  devices: SyncDevice[];
  logs: SyncLog[];
  onTriggerSync: (deviceId: string) => void;
  onAddDevice: () => void;
  onDeleteDevice: (id: string) => void;
}

export default function SyncAgentManager({
  devices,
  logs,
  onTriggerSync,
  onAddDevice,
  onDeleteDevice
}: SyncAgentManagerProps) {
  const [syncingId, setSyncingId] = React.useState<string | null>(null);

  const handleSyncClick = (deviceId: string) => {
    setSyncingId(deviceId);
    onTriggerSync(deviceId);
    setTimeout(() => {
      setSyncingId(null);
    }, 2000);
  };

  const getBrowserIcon = (browser: string) => {
    switch (browser) {
      case 'Chrome':
        return <Chrome size={18} className="text-red-500" />;
      case 'Edge':
        return <Compass size={18} className="text-blue-500" />;
      default:
        return <Compass size={18} className="text-orange-500" />;
    }
  };

  return (
    <div id="sync-manager-container" className="flex-1 overflow-y-auto bg-[#F5F5F7] dark:bg-[#1A1A1E] p-4 sm:p-6 space-y-6 select-none">
      
      {/* Title Header */}
      <div id="sync-title-header" className="flex flex-col sm:flex-row sm:items-center justify-between gap-4">
        <div>
          <h2 id="sync-headline" className="text-xl font-display font-bold text-slate-800 dark:text-slate-100">
            Đồng bộ trình duyệt tự động (Sync Agent)
          </h2>
          <p id="sync-description" className="text-xs text-slate-400 mt-1">
            Quản lý các thiết bị Windows 24/7 đang chạy Sync Agent và thu thập mật khẩu tự động từ Chrome, Edge, Firefox.
          </p>
        </div>

        <button
          id="btn-register-device"
          onClick={onAddDevice}
          className="flex items-center justify-center gap-1.5 px-4 py-2 text-sm font-semibold text-white bg-blue-600 hover:bg-blue-700 rounded-xl shadow-md shadow-blue-500/10 cursor-pointer transition-all active:scale-[0.98] w-full sm:w-auto shrink-0"
        >
          <Plus size={16} className="stroke-[2.5]" />
          <span>Liên kết thiết bị mới</span>
        </button>
      </div>

      <div id="sync-view-grid" className="grid grid-cols-1 xl:grid-cols-3 gap-6">
        
        {/* Left / Middle: Connected agent devices list */}
        <div id="devices-listing-column" className="xl:col-span-2 space-y-4">
          <div className="bg-white dark:bg-[#1C1C1E] rounded-2xl p-5 shadow-[0_8px_30px_rgba(0,0,0,0.03)] space-y-4">
            <h3 className="font-display font-bold text-sm text-slate-800 dark:text-slate-200 uppercase tracking-wider flex items-center gap-2">
              <Laptop size={16} className="text-blue-500" />
              <span>Thiết bị đang kết nối ({devices.length})</span>
            </h3>

            <div className="divide-y divide-slate-100 dark:divide-slate-800/80 space-y-3">
              {devices.map((dev) => (
                <div 
                  id={`device-row-${dev.id}`}
                  key={dev.id} 
                  className="flex flex-col sm:flex-row sm:items-center justify-between pt-3 first:pt-0 gap-3"
                >
                  <div className="flex items-center gap-4">
                    <div className="w-10 h-10 rounded-xl bg-slate-50 dark:bg-[#2C2C2E]/60 flex items-center justify-center shrink-0">
                      {getBrowserIcon(dev.browser)}
                    </div>
                    <div>
                      <h4 className="font-semibold text-sm text-slate-800 dark:text-slate-200">
                        {dev.name}
                      </h4>
                      <div className="flex items-center gap-2 mt-1">
                        <span className={`w-2 h-2 rounded-full ${dev.status === 'active' ? 'bg-emerald-500' : 'bg-slate-300'}`}></span>
                        <span className="text-[11px] font-medium text-slate-400">
                          {dev.status === 'active' ? 'Trực tuyến' : 'Ngoại tuyến'} • Đồng bộ lần cuối: {new Date(dev.lastSyncedAt).toLocaleTimeString([], { hour: '2-digit', minute: '2-digit' })}
                        </span>
                      </div>
                    </div>
                  </div>

                  <div className="flex items-center gap-2 self-end sm:self-auto">
                    <button
                      id={`btn-manual-sync-${dev.id}`}
                      onClick={() => handleSyncClick(dev.id)}
                      disabled={syncingId === dev.id || dev.status !== 'active'}
                      className="px-3 py-1.5 bg-blue-50 hover:bg-blue-100 dark:bg-blue-950/40 dark:hover:bg-blue-950/70 text-blue-600 dark:text-blue-400 font-bold rounded-lg text-xs flex items-center gap-1.5 transition-colors cursor-pointer disabled:opacity-40 disabled:cursor-not-allowed"
                    >
                      <RefreshCw size={12} className={syncingId === dev.id ? 'animate-spin' : ''} />
                      <span>{syncingId === dev.id ? 'Đang đồng bộ...' : 'Quét mật khẩu'}</span>
                    </button>

                    <button
                      id={`btn-unlink-device-${dev.id}`}
                      onClick={() => onDeleteDevice(dev.id)}
                      className="p-1.5 rounded-lg text-slate-400 hover:text-red-500 hover:bg-slate-50 dark:hover:bg-slate-800 transition-colors"
                      title="Hủy liên kết agent"
                    >
                      <Trash2 size={14} />
                    </button>
                  </div>
                </div>
              ))}
            </div>
          </div>

          {/* Theoretical System instructions architecture */}
          <div className="bg-gradient-to-r from-blue-600 to-indigo-700 text-white p-6 rounded-2xl shadow-md shadow-blue-500/10 space-y-4">
            <h4 className="font-display font-bold text-base flex items-center gap-2">
              <Zap size={18} className="text-yellow-300 fill-yellow-300" />
              <span>Làm thế nào để cài đặt Sync Agent trên máy tính của bạn?</span>
            </h4>
            <p className="text-xs text-blue-100 leading-relaxed text-slate-250">
              Sync Agent là một ứng dụng nền bảo mật nhẹ được viết bằng Rust/Python chạy trên thiết bị Windows 24/7 của bạn. Agent sẽ tự động giải mã DPAPI mật khẩu cục bộ của Chrome, Edge, Firefox sau đấy gửi trực tiếp lên Web API an toàn này qua kết nối mã hóa.
            </p>
            <div className="grid grid-cols-1 md:grid-cols-3 gap-3 pt-2">
              <div className="bg-white/10 p-3 rounded-xl border border-white/10">
                <span className="text-[11px] font-bold text-yellow-300 block mb-1">BƯỚC 1</span>
                <p className="text-[10px] text-blue-50">Tải về tệp cài đặt `VaultSync.exe` từ trang cài đặt chính của hệ thống.</p>
              </div>
              <div className="bg-white/10 p-3 rounded-xl border border-white/10">
                <span className="text-[11px] font-bold text-yellow-300 block mb-1">BƯỚC 2</span>
                <p className="text-[10px] text-blue-50">Cấu hình khóa liên kết API Endpoint: `https://ais-dev.../api/sync` vào cài đặt.</p>
              </div>
              <div className="bg-white/10 p-3 rounded-xl border border-white/10">
                <span className="text-[11px] font-bold text-yellow-300 block mb-1">BƯỚC 3</span>
                <p className="text-[10px] text-blue-50">Chạy ngầm và nhận dữ liệu đồng bộ tức thì, không cần copy nhập thủ công.</p>
              </div>
            </div>
          </div>
        </div>

        {/* Right side: Realtime agent updates log */}
        <div id="logs-column" className="space-y-4">
          <div className="bg-white dark:bg-[#1C1C1E] rounded-2xl p-5 shadow-[0_8px_30px_rgba(0,0,0,0.03)] flex flex-col h-[400px]">
            <h3 className="font-display font-bold text-sm text-slate-800 dark:text-slate-200 uppercase tracking-wider flex items-center gap-2 mb-4">
              <Terminal size={16} className="text-slate-500" />
              <span>Nhật ký đồng bộ trực tuyến</span>
            </h3>

            {/* Realtime Terminal display area */}
            <div className="flex-1 bg-slate-950 text-slate-350 p-4 rounded-xl font-mono text-[11px] overflow-y-auto space-y-2.5 shadow-inner border-none">
              {logs.map((log) => (
                <div key={log.id} className="border-b border-slate-900/60 pb-2 last:border-0 last:pb-0">
                  <div className="flex items-center justify-between text-[10px] text-slate-500 mb-0.5">
                    <span>{new Date(log.timestamp).toLocaleTimeString()}</span>
                    <span className={`px-1 py-0.2 rounded font-sans font-bold text-[8px] ${
                      log.type === 'ADDED' ? 'bg-emerald-950/60 text-emerald-400' :
                      log.type === 'UPDATED' ? 'bg-blue-950/60 text-blue-400' :
                      'bg-slate-800 text-slate-400'
                    }`}>
                      {log.type}
                    </span>
                  </div>
                  <p className="text-slate-300 leading-normal">
                    <span className="text-blue-400">{log.deviceName}:</span> {log.message}
                  </p>
                </div>
              ))}
            </div>
          </div>
        </div>
      </div>
    </div>
  );
}
