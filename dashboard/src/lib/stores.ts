import { writable } from 'svelte/store';

export interface WafLog {
  timestamp: string;
  client_ip: string;
  method: string;
  path: string;
  action: string;
  rule_id: string;
  reason: string;
  expanded?: boolean;
}

export interface Stats {
  total_requests: number;
  blocked: number;
  rate_limited: number;
}

export const connectionStatus = writable<'connecting' | 'online' | 'offline'>('connecting');
export const logs = writable<WafLog[]>([]);
export const latestLog = writable<WafLog | null>(null);
export const stats = writable<Stats>({ total_requests: 0, blocked: 0, rate_limited: 0 });
export const dbSize = writable<string>('0.0 KB');
export const vhostsCount = writable<number>(0);

let eventSource: EventSource | null = null;
let statsInterval: ReturnType<typeof setInterval>;
let flushInterval: ReturnType<typeof setInterval>;
let incomingQueue: WafLog[] = [];
let isInitialized = false;

export function initGlobalStore(controllerUrl: string) {
  if (isInitialized) return;
  isInitialized = true;

  const fetchStats = async () => {
    try {
      const res = await fetch(`${controllerUrl}/api/v1/stats`);
      if (res.ok) {
        const data = await res.json();
        stats.set(data);
      }
    } catch (e) {
      // connecting or offline
    }
  };
  
  fetchStats();
  statsInterval = setInterval(fetchStats, 5000);

  const sseUrl = `${controllerUrl}/api/v1/logs/stream`;
  eventSource = new EventSource(sseUrl);

  eventSource.onopen = () => {
    connectionStatus.set('online');
  };

  eventSource.onerror = () => {
    connectionStatus.set('offline');
  };

  eventSource.onmessage = (event) => {
    try {
      const log: WafLog = JSON.parse(event.data);
      log.expanded = false;
      incomingQueue.push(log);
      latestLog.set(log); // publish to single log subscribers
    } catch (e) {
      // Ignored
    }
  };

  flushInterval = setInterval(() => {
    if (incomingQueue.length > 0) {
      logs.update(currentLogs => {
        const newLogs = [...incomingQueue.reverse(), ...currentLogs];
        return newLogs.slice(0, 500); // retain 500 max
      });
      incomingQueue = [];
    }
  }, 200);
}

export function cleanupGlobalStore() {
  if (eventSource) {
    eventSource.close();
    eventSource = null;
  }
  if (statsInterval) clearInterval(statsInterval);
  if (flushInterval) clearInterval(flushInterval);
  isInitialized = false;
}
