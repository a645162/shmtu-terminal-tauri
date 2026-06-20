import React, { useEffect, useState } from 'react';
import {
  Button,
  Dialog,
  DialogActions,
  DialogBody,
  DialogContent,
  DialogSurface,
  DialogTitle,
  Dropdown,
  Field,
  Option,
  ProgressBar,
  Spinner,
  Table,
  TableBody,
  TableCell,
  TableHeader,
  TableHeaderCell,
  TableRow,
  Text,
} from '@fluentui/react-components';
import { ArrowDownload24Regular, ArrowSync24Regular } from '@fluentui/react-icons';
import type { LocalOcrModelDownloadProgress } from '../../types';
import * as tauri from '../../services/tauri';
import type { LocalModelEntry } from '../../services/tauri';

interface LocalOcrModelDownloadDialogProps {
  open: boolean;
  progress: LocalOcrModelDownloadProgress | null;
  cancelling: boolean;
  onCancel: () => void;
  /**
   * Whether this dialog is opened in "advanced settings" mode.
   * In that mode it shows tag / backbone / precision selectors and the user
   * actively chooses what to download. In the default progress-only mode it
   * just renders the in-flight download progress.
   */
  advanced?: boolean;
}

function formatBytes(value?: number): string {
  if (value === undefined || value === null || Number.isNaN(value)) {
    return '--';
  }
  if (value < 1024) {
    return `${value} B`;
  }
  if (value < 1024 * 1024) {
    return `${(value / 1024).toFixed(1)} KB`;
  }
  return `${(value / 1024 / 1024).toFixed(2)} MB`;
}

function formatAccuracy(value: number | null | undefined): string {
  if (value === null || value === undefined || Number.isNaN(value)) {
    return '--';
  }
  return `${(value * 100).toFixed(2)}%`;
}

function formatSizeMb(value: number | null | undefined): string {
  if (value === null || value === undefined || Number.isNaN(value)) {
    return '--';
  }
  return `${value.toFixed(2)} M`;
}

export const LocalOcrModelDownloadDialog: React.FC<LocalOcrModelDownloadDialogProps> = ({
  open,
  progress,
  cancelling,
  onCancel,
  advanced = false,
}) => {
  // Catalog & selection state (advanced mode only).
  const [tags, setTags] = useState<tauri.OcrV2TagCatalogEntry[]>([]);
  const [catalogLoading, setCatalogLoading] = useState(false);
  const [selectedTag, setSelectedTag] = useState<string>('');
  const [models, setModels] = useState<tauri.OcrV2ModelInfo[]>([]);
  const [modelsLoading, setModelsLoading] = useState(false);
  const [selectedBackbone, setSelectedBackbone] = useState<string>('');
  const [selectedPrecision, setSelectedPrecision] = useState<string>('fp16');
  const [currentConfig, setCurrentConfig] = useState<tauri.OcrV2Config | null>(null);
  const [submitting, setSubmitting] = useState(false);
  const [submitError, setSubmitError] = useState('');
  const [activeTab, setActiveTab] = useState<'download' | 'local'>('download');
  const [localModels, setLocalModels] = useState<LocalModelEntry[]>([]);
  const [localModelsLoading, setLocalModelsLoading] = useState(false);
  const [loadingModelKey, setLoadingModelKey] = useState<string | null>(null);

  const totalProgress = progress?.overall_progress ?? 0;
  const currentFileProgress = progress?.current_file_progress ?? 0;
  const currentFileLabel = progress?.current_file_name ?? '等待下载';
  const canCancel = progress?.phase === 'checking' || progress?.phase === 'downloading';

  const refreshCatalog = async () => {
    setCatalogLoading(true);
    setSubmitError('');
    try {
      const catalog = await tauri.refresh_ocr_v2_tag_catalog();
      setTags(catalog);
      const cfg = await tauri.get_ocr_v2_config();
      setCurrentConfig(cfg);
      if (!selectedTag) {
        setSelectedTag(cfg.tag);
      }
    } catch (error) {
      setSubmitError(String(error));
    } finally {
      setCatalogLoading(false);
    }
  };

  useEffect(() => {
    if (!advanced || !open) {
      return;
    }
    void refreshCatalog();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [advanced, open]);

  useEffect(() => {
    if (!open) return;
    let disposed = false;
    setLocalModelsLoading(true);
    tauri
      .scan_local_ocr_models()
      .then((items) => {
        if (!disposed) setLocalModels(items);
      })
      .catch((error) => {
        if (!disposed) setSubmitError(String(error));
      })
      .finally(() => {
        if (!disposed) setLocalModelsLoading(false);
      });
    return () => { disposed = true; };
  }, [open]);

  useEffect(() => {
    if (!advanced || !open || !selectedTag) {
      return;
    }
    let disposed = false;
    setModelsLoading(true);
    setModels([]);
    tauri
      .list_ocr_v2_models(selectedTag)
      .then((items) => {
        if (disposed) {
          return;
        }
        setModels(items);
        // Keep current backbone if still available; otherwise fall back to current config.
        const exists = items.some((m) => m.backbone === selectedBackbone);
        if (!exists) {
          const fallback =
            items.find((m) => m.backbone === currentConfig?.backbone)?.backbone ?? items[0]?.backbone ?? '';
          setSelectedBackbone(fallback);
        }
      })
      .catch((error) => {
        if (!disposed) {
          setSubmitError(String(error));
        }
      })
      .finally(() => {
        if (!disposed) {
          setModelsLoading(false);
        }
      });
    return () => {
      disposed = true;
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [advanced, open, selectedTag]);

  const handleApply = async () => {
    if (!selectedTag || !selectedBackbone || !selectedPrecision) {
      setSubmitError('请先选择 tag / backbone / precision');
      return;
    }
    setSubmitting(true);
    setSubmitError('');
    try {
      await tauri.set_ocr_v2_model_tag(selectedTag);
      await tauri.set_ocr_v2_backbone(selectedBackbone);
      await tauri.set_ocr_v2_precision(selectedPrecision);
      // Trigger the actual model download via the standard helper, which keeps
      // the existing progress event channel intact.
      const status = await tauri.get_local_ocr_model_status();
      if (status.ready) {
        setCurrentConfig({ tag: selectedTag, backbone: selectedBackbone, precision: selectedPrecision });
      } else {
        await tauri.ensure_local_ocr_models();
        setCurrentConfig({ tag: selectedTag, backbone: selectedBackbone, precision: selectedPrecision });
      }
    } catch (error) {
      setSubmitError(String(error));
    } finally {
      setSubmitting(false);
    }
  };

  const handleLoadModel = async (model: LocalModelEntry) => {
    const key = `${model.version}:${model.backbone}:${model.precision}:${model.file_name}`;
    setLoadingModelKey(key);
    setSubmitError('');
    try {
      await tauri.select_and_load_local_ocr_model(model.version, model.backbone, model.precision);
      // Refresh the list to reflect the loaded state
      const items = await tauri.scan_local_ocr_models();
      setLocalModels(items);
    } catch (error) {
      setSubmitError(String(error));
    } finally {
      setLoadingModelKey(null);
    }
  };

  const renderProgress = () => (
    <div style={{ display: 'grid', gap: 16 }}>
      <Text>{progress?.message ?? '正在准备下载模型...'}</Text>
      <div style={{ display: 'grid', gap: 8 }}>
        <Text weight="semibold">
          总进度
          {progress ? ` ${progress.completed_files}/${progress.total_files} 个文件` : ''}
        </Text>
        <ProgressBar value={totalProgress} thickness="large" />
        <Text size={200} style={{ color: 'var(--colorNeutralForeground3)' }}>
          {Math.round(totalProgress * 100)}%
          {progress?.model_dir ? ` · ${progress.model_dir}` : ''}
        </Text>
      </div>
      <div style={{ display: 'grid', gap: 8 }}>
        <Text weight="semibold">
          当前文件
          {progress?.current_file_index ? ` ${progress.current_file_index}/${progress.total_files}` : ''}
        </Text>
        <Text size={200}>{currentFileLabel}</Text>
        <ProgressBar value={currentFileProgress} />
        <Text size={200} style={{ color: 'var(--colorNeutralForeground3)' }}>
          {Math.round(currentFileProgress * 100)}%
          {(progress?.downloaded_bytes !== undefined || progress?.total_bytes !== undefined) &&
            ` · ${formatBytes(progress?.downloaded_bytes)} / ${formatBytes(progress?.total_bytes)}`}
        </Text>
      </div>
    </div>
  );

  const renderAdvanced = () => (
    <div style={{ display: 'grid', gap: 16 }}>
      <div
        style={{
          display: 'grid',
          gridTemplateColumns: 'minmax(0, 1fr) minmax(0, 1fr)',
          gap: 12,
        }}
      >
        <Field label="Release Tag">
          <div style={{ display: 'flex', gap: 8 }}>
            <Dropdown
              value={selectedTag || '请选择 tag'}
              selectedOptions={selectedTag ? [selectedTag] : []}
              onOptionSelect={(_, data) => setSelectedTag(data.optionValue as string)}
              disabled={catalogLoading || tags.length === 0 || submitting || !!progress}
              style={{ flex: 1, minWidth: 0 }}
            >
              {tags.map((tag) => (
                <Option key={tag.tag} value={tag.tag} text={tag.tag}>
                  {tag.tag}
                  {tag.prerelease ? ' (pre)' : ''}
                </Option>
              ))}
            </Dropdown>
            <Button
              appearance="subtle"
              icon={<ArrowSync24Regular />}
              onClick={() => void refreshCatalog()}
              disabled={catalogLoading || !!progress}
              title="刷新 tag 列表"
            >
              刷新
            </Button>
          </div>
        </Field>
        <Field
          label="Precision"
          hint="fp16 默认，fp32 体积更大但精度略高"
        >
          <Dropdown
            value={selectedPrecision}
            selectedOptions={[selectedPrecision]}
            onOptionSelect={(_, data) => setSelectedPrecision(data.optionValue as string)}
            disabled={submitting || !!progress}
          >
            <Option value="fp16">fp16（默认，体积小）</Option>
            <Option value="fp32">fp32（备用，体积大）</Option>
          </Dropdown>
        </Field>
      </div>

      <div style={{ display: 'grid', gap: 8 }}>
        <div style={{ display: 'flex', alignItems: 'baseline', justifyContent: 'space-between' }}>
          <Text weight="semibold">Backbone 列表</Text>
          {modelsLoading && (
            <span style={{ display: 'inline-flex', alignItems: 'center', gap: 6 }}>
              <Spinner size="tiny" />
              <Text size={200}>加载中...</Text>
            </span>
          )}
        </div>
        <div
          style={{
            border: '1px solid var(--colorNeutralStroke2)',
            borderRadius: 8,
            background: 'var(--colorNeutralBackground1)',
            maxHeight: 240,
            overflow: 'auto',
          }}
        >
          <Table>
            <TableHeader>
              <TableRow>
                <TableHeaderCell>选择</TableHeaderCell>
                <TableHeaderCell>Backbone</TableHeaderCell>
                <TableHeaderCell>显示名</TableHeaderCell>
                <TableHeaderCell>参数量</TableHeaderCell>
                <TableHeaderCell>Val Acc</TableHeaderCell>
                <TableHeaderCell>Test Acc</TableHeaderCell>
              </TableRow>
            </TableHeader>
            <TableBody>
              {models.length === 0 && !modelsLoading && (
                <TableRow>
                  <TableCell colSpan={6}>
                    <Text size={200} style={{ color: 'var(--colorNeutralForeground3)' }}>
                      该 tag 下没有可用的 backbone。
                    </Text>
                  </TableCell>
                </TableRow>
              )}
              {models.map((m) => (
                <TableRow key={m.backbone}>
                  <TableCell>
                    <input
                      type="radio"
                      name="ocr-v2-backbone"
                      checked={selectedBackbone === m.backbone}
                      onChange={() => setSelectedBackbone(m.backbone)}
                      disabled={submitting || !!progress}
                    />
                  </TableCell>
                  <TableCell>
                    <code>{m.backbone}</code>
                  </TableCell>
                  <TableCell>{m.display_name}</TableCell>
                  <TableCell>{formatSizeMb(m.model_size_m)}</TableCell>
                  <TableCell>{formatAccuracy(m.val_acc_expression)}</TableCell>
                  <TableCell>{formatAccuracy(m.test_acc_expression)}</TableCell>
                </TableRow>
              ))}
            </TableBody>
          </Table>
        </div>
      </div>

      {currentConfig && (
        <Text size={200} style={{ color: 'var(--colorNeutralForeground3)' }}>
          当前配置: tag=<code>{currentConfig.tag}</code> · backbone=<code>{currentConfig.backbone}</code> ·
          precision=<code>{currentConfig.precision}</code>
        </Text>
      )}

      {submitError && (
        <div
          style={{
            padding: 10,
            borderRadius: 8,
            background: 'var(--colorPaletteRedBackground1)',
            border: '1px solid var(--colorPaletteRedBorder2)',
            color: 'var(--colorPaletteRedForeground1)',
            fontSize: 12,
          }}
        >
          {submitError}
        </div>
      )}

      {progress && <div style={{ borderTop: '1px solid var(--colorNeutralStroke2)', paddingTop: 12 }}>{renderProgress()}</div>}
    </div>
  );

  const renderLocalModels = () => (
    <div style={{ display: 'grid', gap: 12 }}>
      <div style={{ display: 'flex', alignItems: 'baseline', justifyContent: 'space-between' }}>
        <Text weight="semibold">已下载的模型</Text>
        {localModelsLoading && (
          <span style={{ display: 'inline-flex', alignItems: 'center', gap: 6 }}>
            <Spinner size="tiny" />
            <Text size={200}>扫描中...</Text>
          </span>
        )}
      </div>
      {localModels.length === 0 && !localModelsLoading && (
        <Text size={200} style={{ color: 'var(--colorNeutralForeground3)' }}>
          未找到本地模型文件。请先下载模型。
        </Text>
      )}
      {localModels.length > 0 && (
        <div
          style={{
            border: '1px solid var(--colorNeutralStroke2)',
            borderRadius: 8,
            background: 'var(--colorNeutralBackground1)',
            maxHeight: 320,
            overflow: 'auto',
          }}
        >
          <Table>
            <TableHeader>
              <TableRow>
                <TableHeaderCell>版本</TableHeaderCell>
                <TableHeaderCell>显示名</TableHeaderCell>
                <TableHeaderCell>Backbone</TableHeaderCell>
                <TableHeaderCell>Precision</TableHeaderCell>
                <TableHeaderCell>大小</TableHeaderCell>
                <TableHeaderCell>操作</TableHeaderCell>
              </TableRow>
            </TableHeader>
            <TableBody>
              {localModels.map((m) => {
                const key = `${m.version}:${m.backbone}:${m.precision}:${m.file_name}`;
                const isLoading = loadingModelKey === key;
                return (
                  <TableRow key={key}>
                    <TableCell>
                      <code>{m.version}</code>
                    </TableCell>
                    <TableCell>{m.display_name}</TableCell>
                    <TableCell>
                      {m.backbone ? <code>{m.backbone}</code> : '--'}
                    </TableCell>
                    <TableCell>
                      {m.precision ? <code>{m.precision}</code> : '--'}
                    </TableCell>
                    <TableCell>{formatBytes(m.file_size_bytes)}</TableCell>
                    <TableCell>
                      <Button
                        size="small"
                        appearance="primary"
                        onClick={() => void handleLoadModel(m)}
                        disabled={isLoading || !!loadingModelKey}
                      >
                        {isLoading ? <Spinner size="tiny" /> : '加载'}
                      </Button>
                    </TableCell>
                  </TableRow>
                );
              })}
            </TableBody>
          </Table>
        </div>
      )}

      {submitError && (
        <div
          style={{
            padding: 10,
            borderRadius: 8,
            background: 'var(--colorPaletteRedBackground1)',
            border: '1px solid var(--colorPaletteRedBorder2)',
            color: 'var(--colorPaletteRedForeground1)',
            fontSize: 12,
          }}
        >
          {submitError}
        </div>
      )}
    </div>
  );

  return (
    <Dialog open={open}>
      <DialogSurface style={{ width: 'min(92vw, 720px)' }}>
        <DialogBody>
          <DialogTitle>
            <ArrowDownload24Regular style={{ marginRight: 8 }} />
            {advanced ? 'OCR 模型高级设置 (v2)' : '下载本地 OCR 模型'}
          </DialogTitle>
          <DialogContent>
            {advanced ? (
              <div style={{ display: 'grid', gap: 12 }}>
                <div style={{ display: 'flex', gap: 8 }}>
                  <Button
                    appearance={activeTab === 'download' ? 'primary' : 'secondary'}
                    onClick={() => setActiveTab('download')}
                    size="small"
                  >
                    下载设置
                  </Button>
                  <Button
                    appearance={activeTab === 'local' ? 'primary' : 'secondary'}
                    onClick={() => setActiveTab('local')}
                    size="small"
                  >
                    已下载的模型
                  </Button>
                </div>
                {activeTab === 'download' ? renderAdvanced() : renderLocalModels()}
              </div>
            ) : (
              renderProgress()
            )}
          </DialogContent>
          <DialogActions>
            {advanced ? (
              <>
                <Button
                  appearance="secondary"
                  onClick={onCancel}
                  disabled={submitting || (!!progress && canCancel)}
                >
                  {progress && canCancel ? (cancelling ? '正在取消...' : '取消下载') : '关闭'}
                </Button>
                <Button
                  appearance="primary"
                  onClick={() => void handleApply()}
                  disabled={
                    submitting ||
                    !selectedTag ||
                    !selectedBackbone ||
                    !selectedPrecision ||
                    (!!progress && canCancel)
                  }
                >
                  {submitting ? <Spinner size="tiny" /> : '应用并下载'}
                </Button>
              </>
            ) : (
              <Button appearance="secondary" onClick={onCancel} disabled={!canCancel || cancelling}>
                {cancelling ? '正在取消...' : '取消下载'}
              </Button>
            )}
          </DialogActions>
        </DialogBody>
      </DialogSurface>
    </Dialog>
  );
};