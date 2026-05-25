use std::path::Path;

use serde::{Deserialize, Serialize};
use shmtu_cas::classifier::PositionTranslator;

use crate::classification::{BillClassifier, ClassificationResult};
use crate::db::{BillStoreImpl, DatabaseManager};
use crate::error::AppResult;
use crate::models::{BillMerged, BillOriginal};

/// 导出格式
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExportFormat {
    Csv,
    Json,
    Qianji,
}

/// 导出选项
#[derive(Debug, Clone)]
pub struct ExportOptions {
    pub format: ExportFormat,
    pub output_path: String,
    pub include_classification: bool,
    /// 时间范围过滤（可选）
    pub start_timestamp: Option<i64>,
    pub end_timestamp: Option<i64>,
}

/// JSON 导出结构
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonExport {
    pub export_time: String,
    pub identity_name: String,
    pub source: String,
    pub bills: Vec<JsonBillItem>,
}

/// JSON 单条账单
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonBillItem {
    pub date_time_formatted: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub time_str_formatted: Option<String>,
    pub item_type: Option<String>,
    pub number: Option<String>,
    pub number_list: Option<Vec<String>>,
    pub target_user: Option<String>,
    pub money_str: Option<String>,
    pub money: Option<f64>,
    pub method: Option<String>,
    pub status_str: Option<String>,
    pub is_combined: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub classification: Option<ClassificationResult>,
}

/// 钱迹格式单条记录
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QianjiItem {
    /// 0=支出，1=收入
    pub r#type: u8,
    /// 正数金额
    pub money: f64,
    /// 分类
    pub category: String,
    /// 账户
    pub account: String,
    /// 备注
    pub remark: String,
    /// Unix 时间戳
    pub time: i64,
}

/// 数据导入结构
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonImport {
    pub export_time: String,
    pub identity_name: String,
    pub source: String,
    pub bills: Vec<JsonBillItem>,
}

/// 数据导入导出服务
pub struct ExportService {
    db_manager: DatabaseManager,
    translator: PositionTranslator,
}

impl ExportService {
    pub fn new(db_manager: DatabaseManager, translator: PositionTranslator) -> Self {
        Self { db_manager, translator }
    }

    /// 导出身份合并数据
    pub async fn export_identity_bills(
        &self,
        identity_id: i64,
        identity_name: &str,
        options: &ExportOptions,
    ) -> AppResult<()> {
        tracing::info!(
            "[Export] export_identity_bills: identity_id={}, format={:?}, path={}",
            identity_id,
            options.format,
            options.output_path
        );

        let store = BillStoreImpl::new(self.db_manager.db().clone(), "", identity_id, self.translator.clone()).await?;
        let bills = store.get_all_merged_bills(identity_id).await?;

        tracing::debug!("[Export] loaded {} merged bills for export", bills.len());

        // 时间范围过滤
        let filtered: Vec<&BillMerged> = bills
            .iter()
            .filter(|b| {
                if let (Some(start), Some(ts)) = (options.start_timestamp, b.timestamp) {
                    if ts < start {
                        return false;
                    }
                }
                if let (Some(end), Some(ts)) = (options.end_timestamp, b.timestamp) {
                    if ts > end {
                        return false;
                    }
                }
                true
            })
            .collect();

        tracing::info!(
            "[Export] filtered: {} -> {} bills (date range filter)",
            bills.len(),
            filtered.len()
        );

        match options.format {
            ExportFormat::Csv => self.export_csv(&filtered, &options.output_path)?,
            ExportFormat::Json => {
                self.export_json(&filtered, identity_name, "merged", options, &filtered.len())?
            }
            ExportFormat::Qianji => self.export_qianji(&filtered, &options.output_path)?,
        }

        tracing::info!(
            "[Export] export_identity_bills completed: {} bills written to {}",
            filtered.len(),
            options.output_path
        );

        Ok(())
    }

    /// 导出账号原始数据
    pub async fn export_account_bills(&self, account_id: &str, options: &ExportOptions) -> AppResult<()> {
        tracing::info!(
            "[Export] export_account_bills: account_id={}, format={:?}, path={}",
            account_id,
            options.format,
            options.output_path
        );

        let store = BillStoreImpl::new(self.db_manager.db().clone(), account_id, 0, self.translator.clone()).await?;
        let bills = store.get_all_original_bills(account_id).await?;

        tracing::debug!("[Export] loaded {} original bills for export", bills.len());

        match options.format {
            ExportFormat::Csv => self.export_original_csv(&bills, &options.output_path)?,
            ExportFormat::Json => self.export_original_json(&bills, account_id, options)?,
            ExportFormat::Qianji => self.export_original_qianji(&bills, &options.output_path)?,
        }

        tracing::info!(
            "[Export] export_account_bills completed: {} bills written to {}",
            bills.len(),
            options.output_path
        );

        Ok(())
    }

    /// CSV 导出（合并数据）— UTF-8 BOM
    fn export_csv(&self, bills: &[&BillMerged], path: &str) -> AppResult<()> {
        use std::fs::File;
        use std::io::Write;

        tracing::debug!("[Export] writing CSV to {}, {} rows", path, bills.len());

        let mut file = File::create(path)?;
        // UTF-8 BOM
        file.write_all(&[0xEF, 0xBB, 0xBF])?;

        writeln!(file, "日期时间,交易名称,交易号,对方账户,金额,支付方式,状态")?;

        for bill in bills {
            let numbers = bill.number_list.as_deref().unwrap_or("");
            writeln!(
                file,
                "{},{},{},{},{},{},{}",
                bill.date_time_formatted.as_deref().unwrap_or(""),
                bill.item_type.as_deref().unwrap_or(""),
                numbers,
                bill.target_user.as_deref().unwrap_or(""),
                bill.money_str.as_deref().unwrap_or(""),
                bill.method.as_deref().unwrap_or(""),
                bill.status_str.as_deref().unwrap_or(""),
            )?;
        }

        Ok(())
    }

    /// CSV 导出（原始数据）— UTF-8 BOM
    fn export_original_csv(&self, bills: &[BillOriginal], path: &str) -> AppResult<()> {
        use std::fs::File;
        use std::io::Write;

        tracing::debug!("[Export] writing original CSV to {}, {} rows", path, bills.len());

        let mut file = File::create(path)?;
        file.write_all(&[0xEF, 0xBB, 0xBF])?;

        writeln!(file, "日期时间,交易名称,交易号,对方账户,金额,支付方式,状态")?;

        for bill in bills {
            let numbers = bill.number_list.as_deref().unwrap_or("");
            writeln!(
                file,
                "{},{},{},{},{},{},{}",
                bill.date_time_formatted.as_deref().unwrap_or(""),
                bill.item_type.as_deref().unwrap_or(""),
                numbers,
                bill.target_user.as_deref().unwrap_or(""),
                bill.money_str.as_deref().unwrap_or(""),
                bill.method.as_deref().unwrap_or(""),
                bill.status_str.as_deref().unwrap_or(""),
            )?;
        }

        Ok(())
    }

    /// JSON 导出（合并数据）
    fn export_json(
        &self,
        bills: &[&BillMerged],
        identity_name: &str,
        source: &str,
        options: &ExportOptions,
        _total: &usize,
    ) -> AppResult<()> {
        tracing::debug!(
            "[Export] writing JSON, include_classification={}",
            options.include_classification
        );

        let classifier = if options.include_classification {
            // 尝试加载分类规则
            let rules_path = self.db_manager.data_dir().join("classification_rules.toml");
            if rules_path.exists() {
                tracing::debug!("[Export] loading classification rules from {:?}", rules_path);
                Some(BillClassifier::from_file(
                    rules_path.to_str().unwrap_or(""),
                )?)
            } else {
                tracing::warn!("[Export] classification_rules.toml not found, skipping classification");
                None
            }
        } else {
            None
        };

        let json_bills: Vec<JsonBillItem> = bills
            .iter()
            .map(|b| {
                let classification = classifier.as_ref().and_then(|c| {
                    let result = c.classify(
                        b.item_type.as_deref().unwrap_or(""),
                        b.target_user.as_deref().unwrap_or(""),
                        b.timestamp.unwrap_or(0),
                    );
                    if result.type_label.is_none()
                        && result.building.is_none()
                        && result.meal.is_none()
                    {
                        None
                    } else {
                        Some(result)
                    }
                });

                let number_list = b
                    .number_list
                    .as_deref()
                    .and_then(|s| serde_json::from_str::<Vec<String>>(s).ok());

                JsonBillItem {
                    date_time_formatted: b.date_time_formatted.clone(),
                    time_str_formatted: b.time_str_formatted.clone(),
                    item_type: b.item_type.clone(),
                    number: b.number.clone(),
                    number_list,
                    target_user: b.target_user.clone(),
                    money_str: b.money_str.clone(),
                    money: b.money,
                    method: b.method.clone(),
                    status_str: b.status_str.clone(),
                    is_combined: b.is_combined,
                    classification,
                }
            })
            .collect();

        let export = JsonExport {
            export_time: chrono::Local::now().to_rfc3339(),
            identity_name: identity_name.to_string(),
            source: source.to_string(),
            bills: json_bills,
        };

        let json_str = serde_json::to_string_pretty(&export)?;
        std::fs::write(&options.output_path, json_str)?;

        Ok(())
    }

    /// JSON 导出（原始数据）
    fn export_original_json(
        &self,
        bills: &[BillOriginal],
        account_id: &str,
        options: &ExportOptions,
    ) -> AppResult<()> {
        tracing::debug!("[Export] writing original JSON for account={}", account_id);

        let json_bills: Vec<JsonBillItem> = bills
            .iter()
            .map(|b| {
                let number_list = b
                    .number_list
                    .as_deref()
                    .and_then(|s| serde_json::from_str::<Vec<String>>(s).ok());

                JsonBillItem {
                    date_time_formatted: b.date_time_formatted.clone(),
                    time_str_formatted: b.time_str_formatted.clone(),
                    item_type: b.item_type.clone(),
                    number: b.number.clone(),
                    number_list,
                    target_user: b.target_user.clone(),
                    money_str: b.money_str.clone(),
                    money: b.money,
                    method: b.method.clone(),
                    status_str: b.status_str.clone(),
                    is_combined: b.is_combined,
                    classification: None,
                }
            })
            .collect();

        let export = JsonExport {
            export_time: chrono::Local::now().to_rfc3339(),
            identity_name: account_id.to_string(),
            source: "original".to_string(),
            bills: json_bills,
        };

        let json_str = serde_json::to_string_pretty(&export)?;
        std::fs::write(&options.output_path, json_str)?;

        Ok(())
    }

    /// 钱迹格式导出（合并数据）
    fn export_qianji(&self, bills: &[&BillMerged], path: &str) -> AppResult<()> {
        tracing::debug!("[Export] writing Qianji format to {}, {} rows", path, bills.len());

        let items: Vec<QianjiItem> = bills
            .iter()
            .filter_map(|b| {
                let money = b.money.unwrap_or(0.0);
                let timestamp = b.timestamp.unwrap_or(0);
                if timestamp == 0 {
                    return None;
                }

                let (type_val, abs_money) = if money >= 0.0 {
                    (1, money) // 收入
                } else {
                    (0, -money) // 支出
                };

                let target = b.target_user.as_deref().unwrap_or("");
                let item_type = b.item_type.as_deref().unwrap_or("");

                // 简单分类映射
                let category = if money >= 0.0 {
                    "其他收入".to_string()
                } else if target.contains("食堂") || target.contains("餐厅") {
                    "餐饮".to_string()
                } else if item_type.contains("充值") {
                    "其他收入".to_string()
                } else {
                    "其他支出".to_string()
                };

                let remark = format!("{}-{}", target, b.item_type.as_deref().unwrap_or(""));

                Some(QianjiItem {
                    r#type: type_val,
                    money: abs_money,
                    category,
                    account: "校园卡".to_string(),
                    remark,
                    time: timestamp,
                })
            })
            .collect();

        tracing::debug!("[Export] Qianji format: {} items after filtering", items.len());

        let json_str = serde_json::to_string_pretty(&items)?;
        std::fs::write(path, json_str)?;

        Ok(())
    }

    /// 钱迹格式导出（原始数据）
    fn export_original_qianji(&self, bills: &[BillOriginal], path: &str) -> AppResult<()> {
        tracing::debug!("[Export] writing original Qianji format to {}, {} rows", path, bills.len());

        let items: Vec<QianjiItem> = bills
            .iter()
            .filter_map(|b| {
                let money = b.money.unwrap_or(0.0);
                let timestamp = b.timestamp.unwrap_or(0);
                if timestamp == 0 {
                    return None;
                }

                let (type_val, abs_money) = if money >= 0.0 {
                    (1, money)
                } else {
                    (0, -money)
                };

                let target = b.target_user.as_deref().unwrap_or("");
                let item_type = b.item_type.as_deref().unwrap_or("");

                let category = if money >= 0.0 {
                    "其他收入".to_string()
                } else if target.contains("食堂") || target.contains("餐厅") {
                    "餐饮".to_string()
                } else {
                    "其他支出".to_string()
                };

                Some(QianjiItem {
                    r#type: type_val,
                    money: abs_money,
                    category,
                    account: "校园卡".to_string(),
                    remark: format!("{}-{}", target, item_type),
                    time: timestamp,
                })
            })
            .collect();

        let json_str = serde_json::to_string_pretty(&items)?;
        std::fs::write(path, json_str)?;

        Ok(())
    }

    /// 从 JSON 文件导入数据到身份合并数据库
    pub async fn import_json(&self, identity_id: i64, json_path: &str) -> AppResult<usize> {
        tracing::info!("[Export] import_json: identity_id={}, path={}", identity_id, json_path);

        use sea_orm::{EntityTrait, Set};

        let content = std::fs::read_to_string(json_path)?;
        let import: JsonImport = serde_json::from_str(&content)?;

        tracing::info!(
            "[Export] import_json: parsed {} bills from file",
            import.bills.len()
        );

        let now = chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string();
        let mut count = 0;

        for bill in &import.bills {
            let number_list_json = bill
                .number_list
                .as_ref()
                .map(|l| serde_json::to_string(l).unwrap_or_default())
                .unwrap_or_default();

            let model = crate::entity::bill_merged::ActiveModel {
                identity_id: Set(identity_id),
                date_str: Set(String::new()),
                time_str: Set(String::new()),
                time_str_formatted: Set(Some(bill.time_str_formatted.clone().unwrap_or_default())),
                date_time_formatted: Set(bill.date_time_formatted.clone()),
                timestamp: Set(Some(0)),
                item_type: Set(Some(bill.item_type.clone().unwrap_or_default())),
                number: Set(bill.number.clone()),
                number_list: Set(if number_list_json.is_empty() { None } else { Some(number_list_json) }),
                target_user: Set(bill.target_user.clone()),
                money_str: Set(bill.money_str.clone()),
                money: Set(bill.money),
                method: Set(bill.method.clone()),
                status_str: Set(bill.status_str.clone()),
                is_combined: Set(bill.is_combined),
                source_account_id: Set(None),
                is_manual: Set(true),
                synced_at: Set(Some(now.clone())),
                ..Default::default()
            };
            crate::entity::bill_merged::Entity::insert(model)
                .exec(self.db_manager.db())
                .await?;
            count += 1;
        }

        tracing::info!(
            "[Export] import_json completed: {} bills imported for identity_id={}",
            count,
            identity_id
        );

        Ok(count)
    }

    // === 数据快照 ===

    /// 创建数据快照
    pub fn create_snapshot(&self, max_keep: usize) -> AppResult<String> {
        tracing::info!("[Export] create_snapshot: max_keep={}", max_keep);

        let snapshot_dir = self.db_manager.snapshot_dir();
        std::fs::create_dir_all(&snapshot_dir)?;

        let timestamp = chrono::Local::now().format("%Y-%m-%d_%H-%M-%S").to_string();
        let snapshot_name = format!("{}.zip", timestamp);
        let snapshot_path = snapshot_dir.join(&snapshot_name);

        // 创建 ZIP 压缩包
        let file = std::fs::File::create(&snapshot_path)?;
        let mut zip = zip::ZipWriter::new(file);
        let options = zip::write::SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Deflated);

        self.add_directory_to_zip(
            &mut zip,
            self.db_manager.data_dir(),
            "Data",
            &options,
            &["snapshot", "models", "export"],
        )?;

        zip.finish()?;

        tracing::info!(
            "[Export] create_snapshot: saved to {}",
            snapshot_path.display()
        );

        // 清理超出的快照
        self.cleanup_snapshots(max_keep)?;

        Ok(snapshot_path.to_string_lossy().to_string())
    }

    /// 递归添加目录到 ZIP
    fn add_directory_to_zip(
        &self,
        zip: &mut zip::ZipWriter<std::fs::File>,
        base_dir: &Path,
        prefix: &str,
        options: &zip::write::SimpleFileOptions,
        exclude_dirs: &[&str],
    ) -> AppResult<()> {
        let entries = std::fs::read_dir(base_dir)?;
        for entry in entries {
            let entry = entry?;
            let path = entry.path();
            let name = entry.file_name().to_string_lossy().to_string();

            if path.is_dir() {
                // 跳过排除目录
                if exclude_dirs.contains(&name.as_str()) {
                    continue;
                }
                let dir_prefix = format!("{}/{}", prefix, name);
                zip.add_directory(&dir_prefix, *options)?;
                self.add_directory_to_zip(zip, &path, &dir_prefix, options, exclude_dirs)?;
            } else {
                let file_path = format!("{}/{}", prefix, name);
                zip.start_file(&file_path, *options)?;
                let mut f = std::fs::File::open(&path)?;
                std::io::copy(&mut f, zip)?;
            }
        }
        Ok(())
    }

    /// 从快照恢复数据
    pub fn restore_snapshot(&self, snapshot_path: &str) -> AppResult<()> {
        tracing::info!("[Export] restore_snapshot: path={}", snapshot_path);

        let data_dir = self.db_manager.data_dir();

        // 解压覆盖当前 Data 目录
        let file = std::fs::File::open(snapshot_path)?;
        let mut archive = zip::ZipArchive::new(file)?;

        let file_count = archive.len();
        tracing::debug!("[Export] restore_snapshot: {} files in archive", file_count);

        for i in 0..archive.len() {
            let mut file = archive.by_index(i)?;
            let outpath = match file.enclosed_name() {
                Some(path) => {
                    // 去掉 "Data/" 前缀，直接解压到 data_dir
                    let stripped = path.strip_prefix("Data").unwrap_or(&path);
                    data_dir.join(stripped)
                }
                None => continue,
            };

            if file.name().ends_with('/') {
                std::fs::create_dir_all(&outpath)?;
            } else {
                if let Some(p) = outpath.parent() {
                    if !p.exists() {
                        std::fs::create_dir_all(p)?;
                    }
                }
                let mut outfile = std::fs::File::create(&outpath)?;
                std::io::copy(&mut file, &mut outfile)?;
            }
        }

        tracing::info!(
            "[Export] restore_snapshot completed: {} files restored from {}",
            file_count,
            snapshot_path
        );

        Ok(())
    }

    /// 列出所有快照
    pub fn list_snapshots(&self) -> AppResult<Vec<SnapshotInfo>> {
        let snapshot_dir = self.db_manager.snapshot_dir();
        if !snapshot_dir.exists() {
            return Ok(Vec::new());
        }

        let mut snapshots = Vec::new();
        let entries = std::fs::read_dir(&snapshot_dir)?;

        for entry in entries {
            let entry = entry?;
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) == Some("zip") {
                let metadata = std::fs::metadata(&path)?;
                let name = path
                    .file_stem()
                    .and_then(|s| s.to_str())
                    .unwrap_or("")
                    .to_string();
                let size = metadata.len();

                snapshots.push(SnapshotInfo {
                    name,
                    path: path.to_string_lossy().to_string(),
                    size,
                    created_at: metadata
                        .modified()
                        .ok()
                        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                        .map(|d| d.as_secs() as i64),
                });
            }
        }

        // 按名称倒序（最新的在前）
        snapshots.sort_by(|a, b| b.name.cmp(&a.name));

        tracing::debug!("[Export] list_snapshots: {} snapshots found", snapshots.len());

        Ok(snapshots)
    }

    /// 清理超出保留数的快照
    fn cleanup_snapshots(&self, max_keep: usize) -> AppResult<()> {
        let mut snapshots = self.list_snapshots()?;

        if snapshots.len() > max_keep {
            let removed = snapshots.len() - max_keep;
            // 已按时间倒序排列，删除最旧的
            for snapshot in snapshots.drain(max_keep..) {
                tracing::info!("[Export] cleanup_snapshots: removing old snapshot {}", snapshot.name);
                let _ = std::fs::remove_file(&snapshot.path);
            }
            tracing::info!("[Export] cleanup_snapshots: removed {} old snapshots", removed);
        }

        Ok(())
    }
}

/// 快照信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SnapshotInfo {
    pub name: String,
    pub path: String,
    pub size: u64,
    pub created_at: Option<i64>,
}
