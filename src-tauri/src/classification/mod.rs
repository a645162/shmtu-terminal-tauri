use serde::{Deserialize, Serialize};

use crate::error::{AppError, AppResult};

/// 分类结果
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ClassificationResult {
    /// 消费类型（充值/电费/洗澡/热水/蛋糕/食堂/其他）
    pub type_label: Option<String>,
    /// 楼栋
    pub building: Option<String>,
    /// 房间/窗口
    pub room: Option<String>,
    /// 用餐时段（早餐/午餐/晚餐/夜宵/非用餐时段）
    pub meal: Option<String>,
}

/// 类型分类规则
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct TypeRule {
    pub name: String,
    pub match_field: String,
    #[serde(default)]
    pub match_names: Vec<String>,
    #[serde(default)]
    pub match_targets: Vec<String>,
}

/// 位置映射关键词规则
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct PositionKeyword {
    pub building: String,
    pub room: String,
}

/// 位置映射规则
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct PositionConfig {
    pub field: String,
    #[serde(default)]
    pub keywords: std::collections::HashMap<String, PositionKeyword>,
}

/// 用餐时段规则
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct MealSlot {
    pub name: String,
    pub start_time: String,
    pub end_time: String,
}

/// 用餐时段时间表
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Timetable {
    #[serde(default)]
    pub breakfast: Option<MealSlot>,
    #[serde(default)]
    pub lunch: Option<MealSlot>,
    #[serde(default)]
    pub dinner: Option<MealSlot>,
    #[serde(default)]
    pub midnight_snack: Option<MealSlot>,
}

impl Timetable {
    /// 获取所有时段
    pub fn all_slots(&self) -> Vec<&MealSlot> {
        let mut slots = Vec::new();
        if let Some(ref s) = self.breakfast {
            slots.push(s);
        }
        if let Some(ref s) = self.lunch {
            slots.push(s);
        }
        if let Some(ref s) = self.dinner {
            slots.push(s);
        }
        if let Some(ref s) = self.midnight_snack {
            slots.push(s);
        }
        slots
    }
}

/// 日期有效期
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ValidDate {
    pub start_date: String,
    pub end_date: String,
}

/// 日程规则
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ScheduleRule {
    pub valid_date: ValidDate,
    pub timetable: Timetable,
}

/// 完整的分类规则配置
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ClassificationRules {
    /// 类型分类规则
    #[serde(rename = "type", default)]
    pub type_rules: std::collections::HashMap<String, TypeRule>,
    /// 位置映射规则
    #[serde(default)]
    pub position: PositionConfig,
    /// 用餐时段规则
    #[serde(default)]
    pub schedule: Vec<ScheduleRule>,
}

/// 账单分类引擎
pub struct BillClassifier {
    rules: ClassificationRules,
}

impl BillClassifier {
    /// 从规则配置创建分类器
    pub fn new(rules: ClassificationRules) -> Self {
        Self { rules }
    }

    /// 从 TOML 文件加载规则并创建分类器
    pub fn from_file(path: &str) -> AppResult<Self> {
        let content = std::fs::read_to_string(path)?;
        Self::from_toml_str(&content)
    }

    /// 从 TOML 字符串解析规则并创建分类器
    pub fn from_toml_str(toml_str: &str) -> AppResult<Self> {
        let rules: ClassificationRules = toml::from_str(toml_str)?;
        Ok(Self::new(rules))
    }

    /// 对一条账单进行分类
    pub fn classify(
        &self,
        item_type: &str,
        target_user: &str,
        timestamp: i64,
    ) -> ClassificationResult {
        ClassificationResult {
            type_label: self.classify_type(item_type, target_user),
            building: None,
            room: None,
            meal: self.classify_meal(timestamp),
        }
        .with_position(self.classify_position(target_user))
    }

    /// 类型分类：按规则定义顺序匹配，首次匹配即返回
    fn classify_type(&self, item_type: &str, target_user: &str) -> Option<String> {
        for rule in self.rules.type_rules.values() {
            let matched = match rule.match_field.as_str() {
                "item_type" => rule.match_names.iter().any(|n| item_type.contains(n)),
                "target_user" | "target" => {
                    rule.match_targets.iter().any(|t| target_user.contains(t))
                }
                _ => false,
            };
            if matched {
                return Some(rule.name.clone());
            }
        }
        None
    }

    /// 位置映射：精确匹配 target_user
    fn classify_position(&self, target_user: &str) -> Option<(&str, &str)> {
        if let Some(kw) = self.rules.position.keywords.get(target_user) {
            Some((&kw.building, &kw.room))
        } else {
            None
        }
    }

    /// 用餐时段分类
    fn classify_meal(&self, timestamp: i64) -> Option<String> {
        use chrono::{Local, TimeZone};

        let dt = Local.timestamp_opt(timestamp, 0).single()?;
        let current_date = dt.format("%Y.%m.%d").to_string();
        let time_str = dt.format("%H:%M").to_string();

        for schedule in &self.rules.schedule {
            if !Self::is_date_valid(&current_date, &schedule.valid_date) {
                continue;
            }

            for slot in schedule.timetable.all_slots() {
                if time_str >= slot.start_time && time_str < slot.end_time {
                    return Some(slot.name.clone());
                }
            }
        }

        None
    }

    /// 判断日期是否在有效范围内
    fn is_date_valid(current: &str, valid_date: &ValidDate) -> bool {
        let start_ok = current >= valid_date.start_date.as_str();
        let end_ok = if valid_date.end_date == "now" {
            true
        } else {
            current <= valid_date.end_date.as_str()
        };
        start_ok && end_ok
    }

    /// 获取当前规则引用
    pub fn rules(&self) -> &ClassificationRules {
        &self.rules
    }

    /// 从 GitHub 远程更新规则
    pub async fn update_rules_from_remote(url: &str, local_path: &str) -> AppResult<()> {
        let client = reqwest::Client::new();
        let response = client.get(url).send().await?;

        if !response.status().is_success() {
            return Err(AppError::Classification(format!(
                "下载规则文件失败，状态码: {}",
                response.status()
            )));
        }

        let content = response.text().await?;

        // 先验证 TOML 格式
        let _: ClassificationRules = toml::from_str(&content)?;

        // 备份当前规则文件（如果存在）
        if std::path::Path::new(local_path).exists() {
            let backup_path = format!("{}.bak", local_path);
            std::fs::copy(local_path, &backup_path)?;
        }

        // 写入新规则
        std::fs::write(local_path, &content)?;

        Ok(())
    }
}

impl ClassificationResult {
    /// 填充位置信息
    fn with_position(mut self, pos: Option<(&str, &str)>) -> Self {
        if let Some((building, room)) = pos {
            self.building = Some(building.to_string());
            self.room = Some(room.to_string());
        }
        self
    }
}

/// 将 BillMerged 转换为带分类信息的结果
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ClassifiedBill {
    pub date_time_formatted: Option<String>,
    pub end_date_time_formatted: Option<String>,
    pub item_type: Option<String>,
    pub number: Option<String>,
    pub number_list: Option<String>,
    pub target_user: Option<String>,
    pub money: Option<f64>,
    pub money_str: Option<String>,
    pub method: Option<String>,
    pub status_str: Option<String>,
    pub is_combined: bool,
    pub source_account_id: Option<String>,
    pub is_manual: bool,
    pub classification: ClassificationResult,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_rules_toml() -> &'static str {
        r#"
[type.deposit]
name = "充值"
match_field = "item_type"
match_names = ["中行云充值", "微信充值"]

[type.bath]
name = "洗澡"
match_field = "target_user"
match_targets = ["淋浴", "热水"]

[type.canteen]
name = "食堂"
match_field = "target_user"
match_targets = ["食堂", "餐厅"]

[position]
field = "target_user"

[position.keywords."A食堂1楼大餐厅"]
building = "海馨楼"
room = "海馨第1食堂"

[position.keywords."B食堂1楼"]
building = "海琴楼"
room = "海琴1楼"

[[schedule]]
[schedule.valid_date]
start_date = "2019.9.1"
end_date = "now"

[schedule.timetable.breakfast]
name = "早餐"
start_time = "6:30"
end_time = "8:30"

[schedule.timetable.lunch]
name = "午餐"
start_time = "10:45"
end_time = "12:30"

[schedule.timetable.dinner]
name = "晚餐"
start_time = "16:30"
end_time = "18:15"
"#
    }

    #[test]
    fn test_classify_type() {
        let classifier = BillClassifier::from_toml_str(sample_rules_toml()).unwrap();

        assert_eq!(
            classifier.classify_type("中行云充值", ""),
            Some("充值".to_string())
        );
        assert_eq!(
            classifier.classify_type("其他", "淋浴"),
            Some("洗澡".to_string())
        );
        assert_eq!(
            classifier.classify_type("消费", "海馨1楼食堂"),
            Some("食堂".to_string())
        );
        assert_eq!(classifier.classify_type("未知", "未知位置"), None);
    }

    #[test]
    fn test_classify_position() {
        let classifier = BillClassifier::from_toml_str(sample_rules_toml()).unwrap();

        let result = classifier.classify_position("A食堂1楼大餐厅");
        assert!(result.is_some());
        let (building, room) = result.unwrap();
        assert_eq!(building, "海馨楼");
        assert_eq!(room, "海馨第1食堂");

        assert!(classifier.classify_position("未知位置").is_none());
    }

    #[test]
    fn test_full_classify() {
        let classifier = BillClassifier::from_toml_str(sample_rules_toml()).unwrap();

        // 模拟一个午餐时段的时间戳：2024-03-15 12:00:00 CST
        let timestamp = 1710475200i64;
        let result = classifier.classify("消费", "A食堂1楼大餐厅", timestamp);

        assert_eq!(result.type_label, Some("食堂".to_string()));
        assert_eq!(result.building, Some("海馨楼".to_string()));
        assert_eq!(result.room, Some("海馨第1食堂".to_string()));
        assert!(result.meal.is_some());
    }
}
