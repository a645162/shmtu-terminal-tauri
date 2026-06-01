import React, { useMemo, useState } from 'react';
import {
  Dialog,
  DialogSurface,
  DialogBody,
  DialogTitle,
  DialogContent,
  DialogActions,
  Button,
  Text,
} from '@fluentui/react-components';
import { Copy24Regular, ChevronRight12Regular } from '@fluentui/react-icons';

export interface TransactionDetailField {
  label: string;
  value: React.ReactNode;
}

interface TransactionDetailDialogProps {
  open: boolean;
  title?: string;
  fields: TransactionDetailField[];
  rawPayload?: unknown;
  copyText?: string;
  extraContent?: React.ReactNode;
  onClose: () => void;
}

export const TransactionDetailDialog: React.FC<TransactionDetailDialogProps> = ({
  open,
  title = '交易详情',
  fields,
  rawPayload,
  copyText,
  extraContent,
  onClose,
}) => {
  const [showJsonPreview, setShowJsonPreview] = useState(false);
  const jsonText = useMemo(
    () => (rawPayload === undefined ? '' : JSON.stringify(rawPayload, null, 2)),
    [rawPayload]
  );

  return (
    <Dialog open={open} onOpenChange={(_, data) => !data.open && onClose()}>
      <DialogSurface>
        <DialogBody>
          <DialogTitle>{title}</DialogTitle>
          <DialogContent>
            <div style={{ display: 'flex', flexDirection: 'column', gap: 8 }}>
              <div style={{ display: 'grid', gridTemplateColumns: '120px 1fr', gap: '8px 16px' }}>
                {fields.map((field) => (
                  <React.Fragment key={field.label}>
                    <Text size={200} weight="semibold" style={{ color: 'var(--colorNeutralForeground3)' }}>
                      {field.label}
                    </Text>
                    <Text size={200}>{field.value}</Text>
                  </React.Fragment>
                ))}
              </div>
              {extraContent}
              {jsonText && (
                <div
                  style={{
                    padding: 10,
                    borderRadius: 8,
                    background: 'var(--colorNeutralBackground3)',
                    border: '1px solid var(--colorNeutralStroke2)',
                  }}
                >
                  <div style={{ display: 'flex', alignItems: 'center', justifyContent: 'space-between' }}>
                    <Text size={100} style={{ color: 'var(--colorNeutralForeground3)' }}>
                      JSON 预览
                    </Text>
                    <Button
                      appearance="subtle"
                      size="small"
                      icon={
                        <ChevronRight12Regular
                          style={{
                            transform: showJsonPreview ? 'rotate(90deg)' : 'rotate(0deg)',
                            transition: 'transform 0.18s ease',
                          }}
                        />
                      }
                      onClick={() => setShowJsonPreview((prev) => !prev)}
                      style={{ minWidth: 28, padding: 0 }}
                      aria-label={showJsonPreview ? '收起 JSON 预览' : '展开 JSON 预览'}
                    />
                  </div>
                  {showJsonPreview && (
                    <Text
                      size={100}
                      style={{
                        display: 'block',
                        marginTop: 8,
                        fontFamily: 'ui-monospace, SFMono-Regular, Menlo, Consolas, monospace',
                        wordBreak: 'break-all',
                      }}
                    >
                      {jsonText}
                    </Text>
                  )}
                </div>
              )}
            </div>
          </DialogContent>
          <DialogActions>
            {copyText && (
              <Button
                appearance="secondary"
                icon={<Copy24Regular />}
                style={{ whiteSpace: 'nowrap' }}
                onClick={async () => {
                  await navigator.clipboard.writeText(copyText);
                }}
              >
                复制字符串
              </Button>
            )}
            {jsonText && (
              <Button
                appearance="secondary"
                icon={<Copy24Regular />}
                style={{ whiteSpace: 'nowrap' }}
                onClick={async () => {
                  await navigator.clipboard.writeText(jsonText);
                }}
              >
                复制JSON
              </Button>
            )}
            <Button appearance="secondary" onClick={onClose}>
              关闭
            </Button>
          </DialogActions>
        </DialogBody>
      </DialogSurface>
    </Dialog>
  );
};
