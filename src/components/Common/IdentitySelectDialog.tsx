import React, { useEffect, useState } from 'react';
import {
  Button,
  Card,
  Dialog,
  DialogActions,
  DialogBody,
  DialogContent,
  DialogSurface,
  DialogTitle,
  Radio,
  RadioGroup,
  Spinner,
  Text,
} from '@fluentui/react-components';
import { PersonArrowRight24Regular } from '@fluentui/react-icons';
import { useAppStore } from '../../stores/appStore';
import * as tauri from '../../services/tauri';
import {
  CardEnterMotion,
  PageEnterMotion,
  SectionEnterMotion,
  getStaggerDelay,
} from './motion';

function pillStyle(background: string, color: string) {
  return {
    display: 'inline-flex',
    alignItems: 'center',
    padding: '2px 8px',
    borderRadius: 999,
    background,
    color,
    fontSize: 12,
    lineHeight: 1.4,
    whiteSpace: 'nowrap' as const,
  };
}

export const IdentitySelectDialog: React.FC = () => {
  const setShowIdentitySelectDialog = useAppStore((s) => s.setShowIdentitySelectDialog);
  const setShowIdentityManagerDialog = useAppStore((s) => s.setShowIdentityManagerDialog);
  const activateIdentity = useAppStore((s) => s.activateIdentity);
  const currentIdentity = useAppStore((s) => s.currentIdentity);
  const config = useAppStore((s) => s.config);
  const loadConfig = useAppStore((s) => s.loadConfig);
  const loadIdentities = useAppStore((s) => s.loadIdentities);
  const identities = useAppStore((s) => s.identities);

  const [selectedId, setSelectedId] = useState<string>('');
  const [defaultIdentityId, setDefaultIdentityId] = useState<number | null>(null);
  const [accountCounts, setAccountCounts] = useState<Record<number, number>>({});
  const [isLoading, setIsLoading] = useState(true);

  const enabledIdentities = identities.filter((identity) => identity.enable);
  const selectedIdentity =
    enabledIdentities.find((identity) => identity.id.toString() === selectedId) ?? null;

  useEffect(() => {
    let cancelled = false;

    const load = async () => {
      setIsLoading(true);

      try {
        await loadIdentities();

        const latestIdentities = useAppStore
          .getState()
          .identities.filter((identity) => identity.enable);

        const counts = await Promise.all(
          latestIdentities.map(async (identity) => {
            try {
              const accounts = await tauri.list_accounts(identity.id);
              return [identity.id, accounts.filter((account) => account.enable).length] as const;
            } catch {
              return [identity.id, 0] as const;
            }
          })
        );

        const defaultId = await tauri.get_default_identity();

        if (cancelled) return;

        setAccountCounts(Object.fromEntries(counts));
        setDefaultIdentityId(defaultId);

        if (currentIdentity && currentIdentity.enable) {
          setSelectedId(currentIdentity.id.toString());
          return;
        }

        if (defaultId !== null) {
          const defaultIdentity = latestIdentities.find((identity) => identity.id === defaultId);
          if (defaultIdentity) {
            setSelectedId(defaultIdentity.id.toString());
            return;
          }
        }

        if (latestIdentities.length > 0) {
          setSelectedId(latestIdentities[0].id.toString());
        }
      } finally {
        if (!cancelled) {
          setIsLoading(false);
        }
      }
    };

    load();

    return () => {
      cancelled = true;
    };
  }, [currentIdentity, loadIdentities]);

  const handleEnter = async () => {
    const identity = identities.find((item) => item.id.toString() === selectedId);
    if (!identity) return;

    await activateIdentity(identity);
    setShowIdentitySelectDialog(false);
  };

  const handleManage = () => {
    setShowIdentitySelectDialog(false);
    setShowIdentityManagerDialog(true);
  };

  const handleSetDefaultIdentity = async () => {
    if (!selectedIdentity) return;

    await tauri.set_default_identity(selectedIdentity.id);
    await loadConfig();
    setDefaultIdentityId(selectedIdentity.id);
  };

  return (
    <Dialog open>
      <DialogSurface style={{ width: 'min(92vw, 620px)' }}>
        <DialogBody>
          <DialogTitle>
            <PersonArrowRight24Regular style={{ marginRight: 8 }} />
            选择身份
          </DialogTitle>
          <DialogContent>
            <div style={{ display: 'grid', gap: 16 }}>
              <SectionEnterMotion>
                <div
                  className="motion-sheen"
                  style={{
                    padding: '16px 18px',
                    borderRadius: 14,
                    border: '1px solid color-mix(in srgb, var(--colorBrandBackground) 24%, var(--colorNeutralStroke2))',
                    background:
                      'linear-gradient(135deg, color-mix(in srgb, var(--colorBrandBackground2) 72%, white), var(--colorNeutralBackground2))',
                  }}
                >
                  <Text weight="semibold" size={400} block style={{ marginBottom: 6 }}>
                    账单、账号和同步范围会跟随身份切换
                  </Text>
                  <Text size={200} style={{ color: 'var(--colorNeutralForeground3)' }}>
                    先确认你要进入的是谁的数据，再执行同步或查看记录。
                  </Text>
                </div>
              </SectionEnterMotion>

              {isLoading ? (
                <div
                  style={{
                    minHeight: 220,
                    display: 'flex',
                    alignItems: 'center',
                    justifyContent: 'center',
                  }}
                >
                  <Spinner label="正在加载身份..." />
                </div>
              ) : enabledIdentities.length === 0 ? (
                <div
                  style={{
                    padding: 28,
                    borderRadius: 14,
                    border: '1px dashed var(--colorNeutralStroke2)',
                    background: 'var(--colorNeutralBackground2)',
                    textAlign: 'center',
                  }}
                >
                  <Text weight="semibold" block style={{ marginBottom: 6 }}>
                    还没有可进入的身份
                  </Text>
                  <Text size={200} style={{ color: 'var(--colorNeutralForeground3)' }}>
                    先到“管理身份”里创建并启用一个身份。
                  </Text>
                </div>
              ) : (
                <>
                  <RadioGroup
                    value={selectedId}
                    onChange={(_, data) => setSelectedId(data.value)}
                    style={{ display: 'grid', gap: 10, maxHeight: 320, overflowY: 'auto', paddingRight: 4 }}
                  >
                    {enabledIdentities.map((identity, index) => {
                      const isSelected = selectedId === identity.id.toString();
                      const isCurrent = currentIdentity?.id === identity.id;
                      const isDefault = defaultIdentityId === identity.id;
                      const accountCount = accountCounts[identity.id] ?? 0;

                      return (
                        <CardEnterMotion key={identity.id} delay={getStaggerDelay(index, 55, 60)}>
                          <Card
                            className="motion-hover-lift motion-sheen"
                            onClick={() => setSelectedId(identity.id.toString())}
                            style={{
                              cursor: 'pointer',
                              padding: 16,
                              borderRadius: 14,
                              border: isSelected
                                ? '1px solid var(--colorBrandStroke1)'
                                : '1px solid var(--colorNeutralStroke2)',
                              boxShadow: isSelected
                                ? '0 0 0 3px color-mix(in srgb, var(--colorBrandStroke1) 18%, transparent)'
                                : 'none',
                              background: isSelected
                                ? 'color-mix(in srgb, var(--colorBrandBackground2) 65%, var(--colorNeutralBackground1))'
                                : 'var(--colorNeutralBackground1)',
                            }}
                          >
                            <div style={{ display: 'flex', gap: 12, alignItems: 'flex-start' }}>
                              <div style={{ paddingTop: 2 }}>
                                <Radio value={identity.id.toString()} label="" />
                              </div>
                              <div style={{ flex: 1, minWidth: 0, display: 'grid', gap: 10 }}>
                                <div
                                  style={{
                                    display: 'flex',
                                    justifyContent: 'space-between',
                                    gap: 12,
                                    alignItems: 'flex-start',
                                    flexWrap: 'wrap',
                                  }}
                                >
                                  <div style={{ minWidth: 0 }}>
                                    <Text weight="semibold" size={400} block>
                                      {identity.name}
                                    </Text>
                                    <Text size={200} style={{ color: 'var(--colorNeutralForeground3)' }}>
                                      身份 ID #{identity.id}
                                    </Text>
                                  </div>
                                  <div style={{ display: 'flex', gap: 6, flexWrap: 'wrap' }}>
                                    {isCurrent && (
                                      <span
                                        style={pillStyle(
                                          'var(--colorBrandBackground)',
                                          'var(--colorNeutralForegroundOnBrand)'
                                        )}
                                      >
                                        当前身份
                                      </span>
                                    )}
                                    {isDefault && (
                                      <span
                                        style={pillStyle(
                                          'var(--colorPaletteLightGreenBackground2)',
                                          'var(--colorPaletteGreenForeground2)'
                                        )}
                                      >
                                        默认进入
                                      </span>
                                    )}
                                  </div>
                                </div>

                                <div style={{ display: 'flex', gap: 8, flexWrap: 'wrap' }}>
                                  <span
                                    style={pillStyle(
                                      'var(--colorNeutralBackground3)',
                                      'var(--colorNeutralForeground2)'
                                    )}
                                  >
                                    {accountCount} 个启用账号
                                  </span>
                                </div>
                              </div>
                            </div>
                          </Card>
                        </CardEnterMotion>
                      );
                    })}
                  </RadioGroup>

                  <PageEnterMotion key={selectedIdentity?.id ?? 'empty'}>
                    <div
                      style={{
                        padding: '14px 16px',
                        borderRadius: 14,
                        border: '1px solid var(--colorNeutralStroke2)',
                        background: 'var(--colorNeutralBackground2)',
                      }}
                    >
                      <Text weight="semibold" block style={{ marginBottom: 4 }}>
                        {selectedIdentity ? `即将进入：${selectedIdentity.name}` : '请选择一个身份'}
                      </Text>
                      <Text size={200} style={{ color: 'var(--colorNeutralForeground3)' }}>
                        {selectedIdentity
                          ? `将加载该身份下的 ${accountCounts[selectedIdentity.id] ?? 0} 个启用账号，并切换后续账单与同步视图。`
                          : '未选择身份时无法进入。'}
                      </Text>
                      {selectedIdentity && (
                        <div style={{ display: 'flex', alignItems: 'center', gap: 10, marginTop: 12, flexWrap: 'wrap' }}>
                          <Button
                            appearance="subtle"
                            size="small"
                            onClick={handleSetDefaultIdentity}
                            disabled={defaultIdentityId === selectedIdentity.id}
                          >
                            {defaultIdentityId === selectedIdentity.id ? '已设为默认身份' : '设为默认身份'}
                          </Button>
                          <Text size={100} style={{ color: 'var(--colorNeutralForeground3)' }}>
                            {config?.identity.remember_default
                              ? '当前启动策略会优先加载默认身份。'
                              : '当前启动策略是优先加载上一次使用的身份。'}
                          </Text>
                        </div>
                      )}
                    </div>
                  </PageEnterMotion>
                </>
              )}
            </div>
          </DialogContent>
          <DialogActions>
            <Button appearance="secondary" onClick={handleManage}>
              管理身份
            </Button>
            <Button appearance="primary" onClick={handleEnter} disabled={!selectedId || isLoading}>
              进入
            </Button>
          </DialogActions>
        </DialogBody>
      </DialogSurface>
    </Dialog>
  );
};
