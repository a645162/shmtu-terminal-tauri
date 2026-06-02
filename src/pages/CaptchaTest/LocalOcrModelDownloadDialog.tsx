import React from 'react';
import {
  Button,
  Dialog,
  DialogActions,
  DialogBody,
  DialogContent,
  DialogSurface,
  DialogTitle,
  ProgressBar,
  Text,
} from '@fluentui/react-components';
import { ArrowDownload24Regular } from '@fluentui/react-icons';
import type { LocalOcrModelDownloadProgress } from '../../types';

interface LocalOcrModelDownloadDialogProps {
  open: boolean;
  progress: LocalOcrModelDownloadProgress | null;
  cancelling: boolean;
  onCancel: () => void;
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

export const LocalOcrModelDownloadDialog: React.FC<LocalOcrModelDownloadDialogProps> = ({
  open,
  progress,
  cancelling,
  onCancel,
}) => {
  const totalProgress = progress?.overall_progress ?? 0;
  const currentFileProgress = progress?.current_file_progress ?? 0;
  const currentFileLabel = progress?.current_file_name ?? '等待下载';
  const canCancel = progress?.phase === 'checking' || progress?.phase === 'downloading';

  return (
    <Dialog open={open}>
      <DialogSurface style={{ width: 'min(92vw, 560px)' }}>
        <DialogBody>
          <DialogTitle>
            <ArrowDownload24Regular style={{ marginRight: 8 }} />
            下载本地 OCR 模型
          </DialogTitle>
          <DialogContent>
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
          </DialogContent>
          <DialogActions>
            <Button appearance="secondary" onClick={onCancel} disabled={!canCancel || cancelling}>
              {cancelling ? '正在取消...' : '取消下载'}
            </Button>
          </DialogActions>
        </DialogBody>
      </DialogSurface>
    </Dialog>
  );
};
