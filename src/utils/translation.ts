/**
 * Bill classification rules and position translation data.
 *
 * 数据来源优先级：
 * 1. 后端 get_classification_rules 命令（运行时从本地/GitHub 加载）
 * 2. 内置默认数据（仅在无法连接后端时使用）
 *
 * 调用 initTranslationData() 可从后端动态加载最新规则。
 */

// ========== 类型定义 ==========

interface PositionKeyword {
  position: string;
  room: string;
}

interface TypeRuleData {
  name?: string[];
  target?: string[];
  match_field?: string;
  match_names?: string[];
  match_targets?: string[];
}

interface ClassificationRulesFromBackend {
  type?: Record<string, TypeRuleData> | null;
  type_rules?: Record<string, TypeRuleData> | null;
  position?: {
    field: string;
    keywords?: Record<string, { building: string; room: string }> | null;
  } | null;
}

// ========== 可变的运行时数据（可通过 initTranslationData 覆盖） ==========

let POSITION_KEYWORDS: Record<string, PositionKeyword> = {
  "A食堂1楼大餐厅": { position: "海馨楼", room: "海馨第1食堂" },
  "A食堂1楼小餐厅": { position: "海馨楼", room: "海馨第3食堂" },
  "A食堂1楼清真餐厅": { position: "海馨楼", room: "海馨第5食堂(清真)" },
  "A食堂2楼大餐厅": { position: "海馨楼", room: "海馨第2食堂" },
  "A食堂2楼小餐厅": { position: "海馨楼", room: "海馨第4食堂" },
  "B食堂1楼": { position: "海琴楼", room: "海琴1楼" },
  "B食堂2楼": { position: "海琴楼", room: "海琴2楼" },
  "C1大餐厅": { position: "海联楼", room: "海联1楼" },
  "C食堂2楼": { position: "海联楼", room: "海联2楼" },
  淋浴: { position: "公共浴室", room: "浴室" },
  热水: { position: "公共浴室", room: "浴室" },
  北区西点房: { position: "海馨楼", room: "西点房" },
  图书馆: { position: "图书馆", room: "图书馆" },
  校医院: { position: "校医院", room: "校医院" },
  教育超市: { position: "校园商业", room: "教育超市" },
};

let POSITION_FUZZY: Record<string, PositionKeyword> = {
  食堂: { position: "食堂", room: "食堂" },
  餐厅: { position: "食堂", room: "食堂" },
  超市: { position: "校园商业", room: "超市" },
  洗衣: { position: "公共浴室", room: "洗衣房" },
  公交: { position: "交通", room: "公交" },
  地铁: { position: "交通", room: "地铁" },
};

let TYPE_RULES: Record<string, { name?: string[]; target?: string[] }> = {
  deposit: { name: ["中行云充值", "微信充值"] },
  electricity: { name: ["电费缴费"] },
  bath: { target: ["淋浴", "热水"] },
  hot_water: { name: ["水控转账"] },
  cake: { target: ["北区西点房"] },
  canteen: { target: ["食堂", "餐厅"] },
  library: { target: ["图书馆"] },
  hospital: { target: ["校医院"] },
  shop: { target: ["超市", "教育超市"] },
  laundry: { target: ["洗衣"] },
  network: { name: ["网络缴费", "网费"] },
  transport: { target: ["公交", "地铁", "交通"] },
};

// Category display names (Chinese)
const CATEGORY_DISPLAY_NAMES: Record<string, string> = {
  deposit: "充值",
  electricity: "电费",
  bath: "淋浴",
  hot_water: "热水",
  cake: "西点",
  canteen: "食堂",
  library: "图书馆",
  hospital: "校医院",
  shop: "超市",
  laundry: "洗衣",
  network: "网络",
  transport: "交通",
  other: "其他",
};

// Category colors for charts
const CATEGORY_COLORS: Record<string, string> = {
  deposit: "#107C10",
  electricity: "#D13438",
  bath: "#0078D4",
  hot_water: "#00B7C3",
  cake: "#FF8C00",
  canteen: "#8764B8",
  library: "#FFB900",
  hospital: "#E81123",
  shop: "#0098BC",
  laundry: "#881798",
  network: "#498205",
  transport: "#00B7C3",
  other: "#8E8E8E",
};

// ========== Schedule (from schedule.json) ==========

interface MealSlot {
  name: string;
  start_time: string;
  end_time: string;
}

const DEFAULT_SCHEDULE: Record<string, MealSlot> = {
  breakfast: { name: "早餐", start_time: "6:30", end_time: "8:30" },
  lunch: { name: "午餐", start_time: "10:45", end_time: "12:30" },
  dinner: { name: "晚餐", start_time: "16:30", end_time: "18:15" },
  midnight_snack: { name: "夜宵", start_time: "18:15", end_time: "21:00" },
};

// ========== 动态初始化 ==========

/**
 * 从后端/GitHub 加载分类规则并覆盖本地默认值。
 * 应在应用启动时调用。数据来源于 database/bill/ 目录（本地不存在时自动从 GitHub 下载）。
 */
export async function initTranslationData(fetchRules: () => Promise<ClassificationRulesFromBackend>): Promise<void> {
  try {
    const rules = await fetchRules();
    const typeRules = rules.type_rules ?? rules.type ?? {};

    // 更新位置翻译表
    const newPosition: Record<string, PositionKeyword> = {};
    for (const [key, val] of Object.entries(rules.position?.keywords ?? {})) {
      newPosition[key] = { position: val.building, room: val.room };
    }
    if (Object.keys(newPosition).length > 0) {
      POSITION_KEYWORDS = newPosition;
    }

    // 更新类型规则（TOML 格式: type_rules[cat].match_names / match_targets）
    const newTypeRules: Record<string, { name?: string[]; target?: string[] }> = {};
    for (const [cat, rule] of Object.entries(typeRules)) {
      newTypeRules[cat] = {
        name: rule.match_names?.length ? rule.match_names : undefined,
        target: rule.match_targets?.length ? rule.match_targets : undefined,
      };
    }
    if (Object.keys(newTypeRules).length > 0) {
      TYPE_RULES = newTypeRules;
    }

    console.log("[translation] 分类规则已从后端更新");
  } catch (e) {
    console.warn("[translation] 无法从后端加载规则，使用内置默认值:", e);
  }
}

// ========== Translation Functions ==========

/**
 * Translate a target_user (merchant name) to position/room.
 * Returns the original string if no match found.
 */
export function translatePosition(targetUser: string): { position: string; room: string } {
  const exact = POSITION_KEYWORDS[targetUser];
  if (exact) return exact;

  for (const [keyword, result] of Object.entries(POSITION_FUZZY)) {
    if (targetUser.includes(keyword)) {
      return result;
    }
  }

  return { position: targetUser, room: targetUser };
}

export function getPosition(targetUser: string): string {
  return translatePosition(targetUser).position;
}

export function getRoom(targetUser: string): string {
  return translatePosition(targetUser).room;
}

/**
 * Classify a bill by its item_type and target_user.
 * Rules can be updated at runtime via initTranslationData().
 */
export function classifyBill(itemType: string, targetUser: string): string {
  for (const [category, rule] of Object.entries(TYPE_RULES)) {
    if (rule.name) {
      for (const keyword of rule.name) {
        if (itemType.includes(keyword)) return category;
      }
    }
    if (rule.target) {
      for (const keyword of rule.target) {
        if (targetUser.includes(keyword)) return category;
      }
    }
  }
  return "other";
}

export function getCategoryDisplayName(category: string): string {
  return CATEGORY_DISPLAY_NAMES[category] ?? category;
}

export function getCategoryColor(category: string): string {
  return CATEGORY_COLORS[category] ?? "#8E8E8E";
}

export function getMealPeriod(timeStr: string): string {
  if (!timeStr) return "非用餐时段";

  const [h, m] = timeStr.split(":").map(Number);
  const totalMinutes = h * 60 + m;

  for (const [, slot] of Object.entries(DEFAULT_SCHEDULE)) {
    const [sh, sm] = slot.start_time.split(":").map(Number);
    const [eh, em] = slot.end_time.split(":").map(Number);
    const startMin = sh * 60 + sm;
    const endMin = eh * 60 + em;

    if (totalMinutes >= startMin && totalMinutes <= endMin) {
      return slot.name;
    }
  }

  return "非用餐时段";
}

export function getAllCategories(): string[] {
  return Object.keys(TYPE_RULES);
}

export function getCategoryDisplayNames(): Record<string, string> {
  return { ...CATEGORY_DISPLAY_NAMES };
}

export function groupMerchantsByPosition(
  merchants: Array<{ merchant: string; amount: number; count: number }>
): Array<{ position: string; amount: number; count: number }> {
  const grouped = new Map<string, { amount: number; count: number }>();

  for (const item of merchants) {
    const { position } = translatePosition(item.merchant);
    const existing = grouped.get(position) ?? { amount: 0, count: 0 };
    existing.amount += item.amount;
    existing.count += item.count;
    grouped.set(position, existing);
  }

  return Array.from(grouped.entries())
    .map(([position, data]) => ({ position, ...data }))
    .sort((a, b) => b.amount - a.amount);
}
