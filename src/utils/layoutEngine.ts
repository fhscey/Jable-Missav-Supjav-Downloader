import {
  Camera,
  CELL_H,
  CELL_W,
  ClusterBounds,
  GAP,
  VideoItem,
  Viewport,
} from '../store/canvasStore';
import { VideoInfo } from '../types';


// 不是点坐标(point index)，而是块坐标 (block index)
//        -1 (列)         0 (列)
//    ┌─────────────┬─────────────┐
// -1 │  (-1, -1)   │   (0, -1)   │  <- 第 1 行 (上)
//    ├─────────────┼─────────────┤
//  0 │  (-1,  0)   │   (0,  0)   │  <- 第 2 行 (下)
//    └─────────────┴─────────────┘
//           ↑             ↑
//           横向 2 列 × 纵向 2 行 = 4 个宏块
const CENTER_BLOCKS: readonly [number, number][] = [
  [-1, -1],
  [0, -1],
  [-1, 0],
  [0, 0],
];

/**
 * 宏块索引位运算压包：将 (bx, by) 两个 16 位有符号整数打包为一个 32 位 int。
 * 坐标范围：-32,768 ~ +32,767，极限可支撑 30 万+ 卡片。
 */
function packBlock(bx: number, by: number): number {
  return ((bx & 0xffff) << 16) | (by & 0xffff);
}

/**
 * 宏块索引解包：从 32 位 int 还原为 [bx, by] 宏块坐标。
 * 利用符号位算术右移（>> 16）保持负数正确还原。
 */
function unpackBlock(key: number): [number, number] {
  return [key >> 16, (key << 16) >> 16];
}

const INITIAL_BLOCK_KEYS: readonly number[] = CENTER_BLOCKS.map(([bx, by]) => packBlock(bx, by));

// 宏块周围的 8 个邻居偏移量（上、下、左、右及 4 个斜角）
const NEIGHBORS: readonly [number, number][] = [
  [1, 0],
  [-1, 0],
  [0, 1],
  [0, -1],
  [1, 1],
  [-1, 1],
  [1, -1],
  [-1, -1],
];

// 4 种互锁的 4x4 砌砖宏块变体，彼此拼接后严丝合缝、无网格空隙
const BLOCK_VARIANTS: Cell[][] = [
  // 中心聚焦：2x2 核心 + 四周横卡/竖卡/角标卡
  [
    [2, 2, 1, 1],
    [2, 1, 1, 0],
    [2, 1, 1, 3],
    [1, 2, 0, 1],
    [1, 2, 3, 1],
    [1, 1, 0, 0],
    [1, 1, 3, 0],
    [1, 1, 0, 3],
    [1, 1, 3, 3],
  ],
  // 左上锚定
  [
    [2, 2, 0, 0],
    [2, 1, 2, 0],
    [1, 1, 2, 1],
    [1, 2, 3, 1],
    [1, 2, 0, 2],
    [2, 1, 1, 2],
    [2, 1, 1, 3],
    [1, 1, 3, 3],
  ],
  // 右下锚定（180° 翻转互锁）
  [
    [2, 2, 2, 2],
    [2, 1, 0, 3],
    [1, 1, 1, 2],
    [1, 2, 0, 1],
    [1, 2, 3, 0],
    [2, 1, 1, 1],
    [2, 1, 1, 0],
    [1, 1, 0, 0],
  ],
  // 双立绘交错穿插
  [
    [1, 2, 0, 0],
    [2, 1, 1, 0],
    [1, 1, 3, 0],
    [2, 2, 1, 1],
    [1, 2, 3, 1],
    [1, 1, 0, 2],
    [2, 1, 0, 3],
    [1, 1, 2, 3],
    [1, 1, 3, 3],
  ],
];

// 伪随机数生成器相关的质数（出自 Matthias Müller 2003 年著名空间哈希论文）
const PRIME_X = 73856093;
const PRIME_Y = 19349663;

export interface Point {
  x: number;
  y: number;
}

interface SlotDef {
  slotIndex: number;
  x: number;
  y: number;
  width: number;
  height: number;
}

// 每个格子用元组表示：[跨列数, 跨行数, 列偏移, 行偏移]
type Cell = [spanX: number, spanY: number, col: number, row: number];


function toPixel(col: number, row: number, spanX: number, spanY: number) {
  return {
    x: col * (CELL_W + GAP),
    y: row * (CELL_H + GAP),
    width: spanX * CELL_W + (spanX - 1) * GAP,
    height: spanY * CELL_H + (spanY - 1) * GAP,
  };
}

// 根据宏块坐标确定性地选择一种拼接变体（原点固定用变体 0）
function pickVariant(bx: number, by: number): Cell[] {
  if (bx === 0 && by === 0) return BLOCK_VARIANTS[0];
  const idx = Math.abs((bx * PRIME_X) ^ (by * PRIME_Y)) % BLOCK_VARIANTS.length;
  return BLOCK_VARIANTS[idx];
}

// 获取宏块在世界坐标系中的几何中心点
function getBlockCenter(bx: number, by: number): Point {
  const colOffset = Math.abs(by) % 2 === 1 ? 2 : 0;
  const baseCol = bx * 4 - 2 + colOffset;
  const baseRow = by * 4 - 2;
  return {
    x: (baseCol + 2) * (CELL_W + GAP),
    y: (baseRow + 2) * (CELL_H + GAP),
  };
}

// 生成单个宏块内的所有卡片槽位
function generateBlockSlots(bx: number, by: number): SlotDef[] {
  const colOffset = Math.abs(by) % 2 === 1 ? 2 : 0;
  const baseCol = bx * 4 - 2 + colOffset;
  const baseRow = by * 4 - 2;

  const slots: SlotDef[] = [];
  const cells = pickVariant(bx, by);

  for (const [spanX, spanY, c, r] of cells) {
    const col = baseCol + c;
    const row = baseRow + r;
    slots.push({
      slotIndex: 0,
      ...toPixel(col, row, spanX, spanY),
    });
  }
  return slots;
}

// 惰性单例缓存
let centerSlots: SlotDef[] | null = null;

function getCenterSlots(): SlotDef[] {
  if (!centerSlots) {
    const raw: { slot: SlotDef; dist: number }[] = [];
    for (const [bx, by] of CENTER_BLOCKS) {
      for (const s of generateBlockSlots(bx, by)) {
        const cx = s.x + s.width / 2;
        const cy = s.y + s.height / 2;
        raw.push({ slot: s, dist: Math.hypot(cx, cy * 1.35) });
      }
    }
    raw.sort((a, b) => a.dist - b.dist);
    centerSlots = raw.map((item, idx) => ({ ...item.slot, slotIndex: idx }));
  }
  return centerSlots;
}

interface LayoutEngineState {
  firstCardId: string | null;
  allocatedBlocks: Set<number>;
  slotCache: SlotDef[];
}

const layoutState: LayoutEngineState = {
  firstCardId: null,
  allocatedBlocks: new Set<number>(),
  slotCache: [],
};

function resetToCenter() {
  layoutState.allocatedBlocks = new Set(INITIAL_BLOCK_KEYS);
  layoutState.slotCache = [...getCenterSlots()];
}

function getCandidateNeighbors(): [number, number][] {
  const candidates = new Map<number, [number, number]>();
  for (const key of layoutState.allocatedBlocks) {
    const [bx, by] = unpackBlock(key);
    for (const [dx, dy] of NEIGHBORS) {
      const nbx = bx + dx;
      const nby = by + dy;
      const nkey = packBlock(nbx, nby);
      if (!layoutState.allocatedBlocks.has(nkey)) {
        candidates.set(nkey, [nbx, nby]);
      }
    }
  }
  return Array.from(candidates.values());
}

/** 取离视口中心最近的候选宏块进行填充（开迷雾） */
function allocateNextBestBlock(focus: Point) {
  const candidates = getCandidateNeighbors();
  if (candidates.length === 0) return;

  let bestBlock = candidates[0];
  let bestDist = Infinity;

  for (const [bx, by] of candidates) {
    const center = getBlockCenter(bx, by);
    // 加权欧氏距离：对垂直分量缩放（1.35），以契合宽屏显示器的视觉长宽比
    // TODO: 比例应该是 window 的比例（或者 viewport 的比例），而不是显示器的比例
    const dist = Math.hypot(center.x - focus.x, (center.y - focus.y) * 1.35);
    if (dist < bestDist) {
      bestDist = dist;
      bestBlock = [bx, by];
    }
  }

  const [bx, by] = bestBlock;
  layoutState.allocatedBlocks.add(packBlock(bx, by));

  const newSlots = generateBlockSlots(bx, by);
  const baseIndex = layoutState.slotCache.length;
  newSlots.forEach((s, idx) => {
    layoutState.slotCache.push({ ...s, slotIndex: baseIndex + idx });
  });
}

export interface LayoutResult {
  items: VideoItem[];
  bounds: ClusterBounds;
}

// 单次遍历同时计算卡片布局与整体包围盒
export function computeLayout(
  cards: VideoInfo[],
  focusPoint?: Point | null
): LayoutResult {
  if (cards.length === 0) {
    return { items: [], bounds: { minX: 0, maxX: 0, minY: 0, maxY: 0, width: 0, height: 0 } };
  }

  const currentFirstId = cards[0]?.id || 'initial';

  // 检测到全新数据流（例如切换站点、分类或输入新关键词搜索），重置排版状态为中心原点
  if (layoutState.firstCardId !== currentFirstId) {
    layoutState.firstCardId = currentFirstId;
    resetToCenter();
  }

  const focus = focusPoint || { x: 0, y: 0 };

  // 槽位不够时，按距离视口中心最近的空闲相邻宏块动态追加，直到满足卡片数量
  while (layoutState.slotCache.length < cards.length) {
    allocateNextBestBlock(focus);
  }

  let minX = Infinity;
  let maxX = -Infinity;
  let minY = Infinity;
  let maxY = -Infinity;

  const items: VideoItem[] = cards.map((card, i) => {
    const slot = layoutState.slotCache[i];
    minX = Math.min(minX, slot.x);
    maxX = Math.max(maxX, slot.x + slot.width);
    minY = Math.min(minY, slot.y);
    maxY = Math.max(maxY, slot.y + slot.height);

    return {
      id: `slot_${i}_${card.id || i}`,
      slotIndex: i,
      x: slot.x,
      y: slot.y,
      width: slot.width,
      height: slot.height,
      payload: card,
    };
  });

  return { items, bounds: { minX, maxX, minY, maxY, width: maxX - minX, height: maxY - minY } };
}

/** 视口裁剪：只保留视野及缓冲带内的卡片 */
export function getVisibleItems(
  camera: Camera,
  viewport: Viewport,
  items: VideoItem[]
): VideoItem[] {
  const buffer = 600; // 约 1.5 张卡片宽度，避免滚动时出现空白
  const left = camera.x - buffer;
  const right = camera.x + viewport.width / camera.zoom + buffer;
  const top = camera.y - buffer;
  const bottom = camera.y + viewport.height / camera.zoom + buffer;

  return items.filter(
    (item) =>
      item.x + item.width >= left &&
      item.x <= right &&
      item.y + item.height >= top &&
      item.y <= bottom
  );
}
