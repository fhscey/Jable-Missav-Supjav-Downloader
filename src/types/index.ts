export type ProxyMode = { type: 'System' } | { type: 'Direct' } | { type: 'Custom'; url: string };

export type Language = 'en' | 'ja' | 'zh-CN' | 'zh-TW';

export type PreferredQuality = 'highest' | '1080p' | '720p' | '480p' | 'lowest';

export type DeleteFileMode = 'Always' | 'Never' | 'Ask';

export type ProviderSite = 'jable' | 'missav' | 'supjav';

export interface AppConfig {
  config_path: string;
  download_dir: string;
  max_concurrent_tasks: number;
  max_download_speed: number;
  preferred_quality: PreferredQuality;
  language: Language;
  enable_logging: boolean;
  proxy_mode: ProxyMode;
  auto_check_update: boolean;
  delete_file_on_remove?: boolean;
  site_domains?: Record<ProviderSite, string[]>;
}

export interface DiskSpace {
  total: number;
  free: number;
}

export interface UpdateInfo {
  currentVersion: string;
  latestVersion: string;
  updateAvailable: boolean;
  changelog: string;
  releaseUrl: string;
  publishedAt?: string | null;
}

export interface NavItem {
  name: string;
  url: string;
}

export interface TagGroup {
  group: string;
  items: NavItem[];
}

export interface SortOption {
  name: string;
  value: string;
}

export interface SortRule {
  match_patterns: string[];
  options: SortOption[];
}

export interface SiteManifest {
  site: ProviderSite;
  name: string;
  primary_domain: string;
  available_domains: string[];
  default_url: string;
  quick_links: NavItem[];
  categories: NavItem[];
  tags: TagGroup[];
  supported_languages?: Record<string, string>;
  sort_rules?: SortRule[];
}

export interface VideoInfo {
  id: string;
  title: string;
  cover_url: string;
  detail_page_url: string;
  duration?: string;
  preview_url?: string;
  referer?: string;
  ua?: string;
}

export interface MediaDetail {
  id: string;
  title: string;
  cover_url: string;
  detail_page_url: string;
  stream_url: string;
  referer?: string;
  categories: NavItem[];
  tags: NavItem[];
  actresses: NavItem[];
  directors?: NavItem[];
}

export interface VideoPage {
  items: VideoInfo[];
  page: number;
  total_pages: number;
}

export interface FetchParams {
  page?: number;
  lang?: Language;
  sort_by?: string;
}

export type TaskStatus = 'pending' | 'running' | 'paused' | 'completed' | 'failed' | 'incomplete';

export interface TaskRecord {
  id: string;
  title: string;
  stream_url: string;
  referer: string | null;
  save_dir: string;
  status: TaskStatus;
  completed_segments: number | null;
  total_segments: number | null;
  downloaded_bytes: number;
  total_bytes: number | null;
  created_at: number;
  updated_at: number;
}


export interface ProgressPayload {
  task_id: string;
  downloaded_bytes: number;
  total_bytes: number | null;
  completed_segments: number | null;
  total_segments: number | null;
  speed_bps: number;
}

export interface TaskStatusEvent {
  task_id: string;
  status: TaskStatus;
}
