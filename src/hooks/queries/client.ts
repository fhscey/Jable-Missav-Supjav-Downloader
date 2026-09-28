import { QueryClient } from '@tanstack/react-query';

/**
 * 桌面端（Tauri）专用的 QueryClient 配置
 * 
 * 关键策略：
 * 1. 禁用 refetchOnWindowFocus：桌面端用户在不同窗口切来切去是极高频操作，避免切窗口就重新请求
 * 2. 较保守的重试策略（retry: 1）：解析器若失败大多是防盗链或规则变化，频繁重试没有意义
 * 3. 合理的默认过期时间（staleTime: 5分钟）
 */
export const queryClient = new QueryClient({
  defaultOptions: {
    queries: {
      staleTime: 5 * 60 * 1000, // 5 分钟内默认认为数据新鲜
      gcTime: 30 * 60 * 1000,    // 30 分钟垃圾回收未激活的缓存
      refetchOnWindowFocus: false, // 桌面端关闭切屏刷新
      retry: 1,                    // 失败仅重试 1 次
    },
  },
});
