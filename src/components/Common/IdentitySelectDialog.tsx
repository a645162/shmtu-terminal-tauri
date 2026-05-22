import React, { useState, useEffect } from 'react';
import {
  Dialog,
  DialogSurface,
  DialogTitle,
  DialogBody,
  DialogContent,
  DialogActions,
  Button,
  Radio,
  RadioGroup,
  Checkbox,
  Text,
  Spinner,
  Card,
  CardPreview,
  CardHeader,
} from '@fluentui/react-components';
import { PersonArrowRight24Regular } from '@fluentui/react-icons';
import { useAppStore } from '../../stores/appStore';
import type { Identity } from '../../types';

export const IdentitySelectDialog: React.FC = () => {
  const setShowIdentitySelectDialog = useAppStore((s) => s.setShowIdentitySelectDialog);
  const setShowIdentityManagerDialog = useAppStore((s) => s.setShowIdentityManagerDialog);
  const setCurrentIdentity = useAppStore((s) => s.setCurrentIdentity);
  const loadAccounts = useAppStore((s) => s.loadAccounts);
  const loadBills = useAppStore((s) => s.loadBills);
  const identities = useAppStore((s) => s.identities);

  const [selectedId, setSelectedId] = useState<string>('');
  const [rememberDefault, setRememberDefault] = useState(false);

  const enabledIdentities = identities.filter((i) => i.enable);

  const handleEnter = () => {
    const identity = identities.find((i) => i.id.toString() === selectedId);
    if (identity) {
      setCurrentIdentity(identity);
      loadAccounts(identity.id);
      loadBills();
      setShowIdentitySelectDialog(false);
    }
  };

  const handleManage = () => {
    setShowIdentitySelectDialog(false);
    setShowIdentityManagerDialog(true);
  };

  return (
    <Dialog open>
      <DialogSurface>
        <DialogBody>
          <DialogTitle>
            <PersonArrowRight24Regular style={{ marginRight: 8 }} />
            选择身份
          </DialogTitle>
          <DialogContent>
            <Text block style={{ marginBottom: 16 }}>
              请选择要进入的身份：
            </Text>
            {enabledIdentities.length === 0 ? (
              <Text block style={{ textAlign: 'center', padding: 24, color: '#616161' }}>
                暂无已启用的身份，请先创建身份。
              </Text>
            ) : (
              <RadioGroup
                value={selectedId}
                onChange={(_, data) => setSelectedId(data.value)}
              >
                {enabledIdentities.map((identity) => (
                  <Card
                    key={identity.id}
                    style={{ marginBottom: 8, cursor: 'pointer' }}
                    onClick={() => setSelectedId(identity.id.toString())}
                  >
                    <CardHeader>
                      <Radio value={identity.id.toString()} label={identity.name} />
                    </CardHeader>
                  </Card>
                ))}
              </RadioGroup>
            )}
            <Checkbox
              label="记住默认选择"
              checked={rememberDefault}
              onChange={(_, data) => setRememberDefault(data.checked as boolean)}
              style={{ marginTop: 16 }}
            />
          </DialogContent>
          <DialogActions>
            <Button appearance="secondary" onClick={handleManage}>
              管理身份
            </Button>
            <Button
              appearance="primary"
              onClick={handleEnter}
              disabled={!selectedId}
            >
              进入
            </Button>
          </DialogActions>
        </DialogBody>
      </DialogSurface>
    </Dialog>
  );
};
