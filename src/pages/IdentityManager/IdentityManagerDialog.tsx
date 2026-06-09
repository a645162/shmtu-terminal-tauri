import React, { useEffect, useMemo, useRef, useState } from 'react';
import {
  Badge,
  Button,
  Card,
  Dialog,
  DialogActions,
  DialogBody,
  DialogContent,
  DialogSurface,
  DialogTitle,
  Divider,
  Field,
  Input,
  MessageBar,
  MessageBarBody,
  Spinner,
  Switch,
  Text,
} from '@fluentui/react-components';
import {
  ArrowSync24Regular,
  Delete24Regular,
  Edit24Regular,
  Money24Regular,
  PersonAdd24Regular,
  Shield24Regular,
} from '@fluentui/react-icons';
import { useAppStore } from '../../stores/appStore';
import type { Account, Identity, PersonAccountInfo } from '../../types';
import * as tauri from '../../services/tauri';
import { PageEnterMotion, SectionEnterMotion } from '../../components/Common/motion';

interface PersonAccountCaptchaState {
  accountDbId: number;
  accountName: string;
  image: string;
  execution: string;
}

export const IdentityManagerDialog: React.FC = () => {
  const showIdentityManagerDialog = useAppStore((s) => s.showIdentityManagerDialog);
  const setShowIdentityManagerDialog = useAppStore((s) => s.setShowIdentityManagerDialog);
  const identities = useAppStore((s) => s.identities);
  const loadIdentities = useAppStore((s) => s.loadIdentities);
  const currentIdentity = useAppStore((s) => s.currentIdentity);
  const activateIdentity = useAppStore((s) => s.activateIdentity);
  const setCurrentIdentity = useAppStore((s) => s.setCurrentIdentity);

  const [selectedIdentity, setSelectedIdentity] = useState<Identity | null>(null);
  const [selectedAccount, setSelectedAccount] = useState<Account | null>(null);
  const [accounts, setAccounts] = useState<Account[]>([]);
  const [accountCounts, setAccountCounts] = useState<Record<number, number>>({});
  const [isLoadingCounts, setIsLoadingCounts] = useState(false);
  const [isAddingIdentity, setIsAddingIdentity] = useState(false);
  const [addingIdentityName, setAddingIdentityName] = useState('');
  const [addingIdentityLoading, setAddingIdentityLoading] = useState(false);
  const [editingIdentityId, setEditingIdentityId] = useState<number | null>(null);
  const [editingIdentityName, setEditingIdentityName] = useState('');
  const [notice, setNotice] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [personAccountCaptcha, setPersonAccountCaptcha] = useState<PersonAccountCaptchaState | null>(null);
  const [personAccountCaptchaCode, setPersonAccountCaptchaCode] = useState('');
  const [submittingPersonAccountCaptcha, setSubmittingPersonAccountCaptcha] = useState(false);

  const initialSelected = useRef(false);

  const [accountForm, setAccountForm] = useState({
    account_name: '',
    account_id: '',
    password: '',
    enable: true,
    enable_update: true,
    admission_date: '',
    graduation_date: '',
    graduation_to_present: true,
  });

  const [personAccounts, setPersonAccounts] = useState<Record<number, PersonAccountInfo | null>>({});
  const [personAccountLoading, setPersonAccountLoading] = useState<Record<number, boolean>>({});
  const [personAccountError, setPersonAccountError] = useState<Record<number, string | null>>({});

  useEffect(() => {
    if (!showIdentityManagerDialog) {
      initialSelected.current = false;
      setSelectedIdentity(null);
      setSelectedAccount(null);
      setAccounts([]);
      setNotice(null);
      setError(null);
      setPersonAccountCaptcha(null);
      setPersonAccountCaptchaCode('');
      return;
    }
    if (selectedIdentity) return;
    const identity = currentIdentity ?? (identities.length > 0 ? identities[0] : null);
    if (identity && !initialSelected.current) {
      initialSelected.current = true;
      void handleSelectIdentity(identity);
    }
  }, [showIdentityManagerDialog, currentIdentity, identities, selectedIdentity]);

  useEffect(() => {
    if (!showIdentityManagerDialog) return;
    let cancelled = false;

    const loadCounts = async () => {
      setIsLoadingCounts(true);
      try {
        const pairs = await Promise.all(
          identities.map(async (identity) => {
            try {
              const list = await tauri.list_accounts(identity.id);
              return [identity.id, list.length] as const;
            } catch {
              return [identity.id, 0] as const;
            }
          })
        );
        if (!cancelled) {
          setAccountCounts(Object.fromEntries(pairs));
        }
      } finally {
        if (!cancelled) {
          setIsLoadingCounts(false);
        }
      }
    };

    void loadCounts();
    return () => {
      cancelled = true;
    };
  }, [showIdentityManagerDialog, identities]);

  useEffect(() => {
    if (!selectedIdentity || accounts.length === 0) {
      setPersonAccounts({});
      return;
    }
    let cancelled = false;
    (async () => {
      try {
        const ids = accounts.map((a) => a.id);
        const cached = await tauri.list_cached_person_accounts(ids);
        if (cancelled) return;
        const map: Record<number, PersonAccountInfo> = {};
        const studentToId: Record<string, number> = {};
        accounts.forEach((a) => {
          studentToId[a.account_id] = a.id;
        });
        cached.forEach((info) => {
          const dbId = studentToId[info.account_id];
          if (typeof dbId === 'number') {
            map[dbId] = info;
          }
        });
        setPersonAccounts(map);

        const idsNeedingRefresh = accounts.filter((a) => !map[a.id]).map((a) => a.id);
        for (const dbId of idsNeedingRefresh) {
          void tauri.fetch_person_account(dbId)
            .then((info) => {
              if (!cancelled) {
                setPersonAccounts((s) => ({ ...s, [dbId]: info }));
              }
            })
            .catch(() => {
              // 静默失败，用户可手动刷新
            });
        }
      } catch {
        if (!cancelled) {
          setPersonAccounts({});
        }
      }
    })();
    return () => {
      cancelled = true;
    };
  }, [selectedIdentity, accounts]);

  const sortedIdentities = useMemo(() => {
    if (!currentIdentity) return identities;
    const idx = identities.findIndex((item) => item.id === currentIdentity.id);
    if (idx <= 0) return identities;
    const list = [...identities];
    const [current] = list.splice(idx, 1);
    list.unshift(current);
    return list;
  }, [identities, currentIdentity]);

  const totalAccounts = useMemo(
    () => Object.values(accountCounts).reduce((sum, count) => sum + count, 0),
    [accountCounts]
  );

  const selectedIdentityPersonAccounts = useMemo(
    () => accounts.map((account) => personAccounts[account.id]).filter(Boolean) as PersonAccountInfo[],
    [accounts, personAccounts]
  );

  const selectedIdentityBalanceSum = useMemo(
    () => selectedIdentityPersonAccounts.reduce((sum, item) => sum + (item.cash_balance || 0), 0),
    [selectedIdentityPersonAccounts]
  );

  const selectedIdentityProfileCoverage = useMemo(
    () => selectedIdentityPersonAccounts.length,
    [selectedIdentityPersonAccounts]
  );

  const handleSelectIdentity = async (identity: Identity) => {
    setSelectedIdentity(identity);
    setSelectedAccount(null);
    setNotice(null);
    setError(null);
    try {
      const accList = await tauri.list_accounts(identity.id);
      setAccounts(accList);
    } catch (e) {
      setAccounts([]);
      setError(extractErrorMessage(e, '加载账号列表失败'));
    }
  };

  const handleRefreshAccountCounts = async () => {
    setIsLoadingCounts(true);
    try {
      const entries = await Promise.all(
        identities.map(async (identity) => {
          const list = await tauri.list_accounts(identity.id);
          return [identity.id, list.length] as const;
        })
      );
      setAccountCounts(Object.fromEntries(entries));
      setNotice('身份统计已刷新');
    } catch (e) {
      setError(extractErrorMessage(e, '刷新身份统计失败'));
    } finally {
      setIsLoadingCounts(false);
    }
  };

  const handleAddIdentity = async () => {
    if (!addingIdentityName.trim()) return;
    setAddingIdentityLoading(true);
    try {
      const created = await tauri.create_identity(addingIdentityName.trim());
      setAddingIdentityName('');
      setIsAddingIdentity(false);
      await loadIdentities();
      await handleSelectIdentity(created);
      setNotice(`已创建身份「${created.name}」`);
    } catch (e) {
      setError(extractErrorMessage(e, '创建身份失败'));
    } finally {
      setAddingIdentityLoading(false);
    }
  };

  const handleDeleteIdentity = async (id: number) => {
    if (!confirm('确定要删除此身份吗？删除后数据无法恢复。')) return;
    try {
      await tauri.delete_identity(id);
      if (selectedIdentity?.id === id) {
        setSelectedIdentity(null);
        setSelectedAccount(null);
        setAccounts([]);
      }
      await loadIdentities();
      setNotice('身份已删除');
    } catch (e) {
      setError(extractErrorMessage(e, '删除身份失败'));
    }
  };

  const handleStartEditIdentity = (identity: Identity) => {
    setEditingIdentityId(identity.id);
    setEditingIdentityName(identity.name);
  };

  const handleSaveEditIdentity = async () => {
    if (!editingIdentityId || !editingIdentityName.trim()) return;
    try {
      const identity = identities.find((item) => item.id === editingIdentityId);
      if (!identity) return;
      const updatedIdentity = { ...identity, name: editingIdentityName.trim() };
      await tauri.update_identity(updatedIdentity);
      await loadIdentities();
      if (selectedIdentity?.id === editingIdentityId) {
        setSelectedIdentity(updatedIdentity);
      }
      if (currentIdentity?.id === editingIdentityId) {
        setCurrentIdentity(updatedIdentity);
      }
      setNotice('身份名称已更新');
    } catch (e) {
      setError(extractErrorMessage(e, '更新身份失败'));
    } finally {
      setEditingIdentityId(null);
      setEditingIdentityName('');
    }
  };

  const handleCancelEditIdentity = () => {
    setEditingIdentityId(null);
    setEditingIdentityName('');
  };

  const handleSelectAccount = (account: Account) => {
    setSelectedAccount(account);
    setAccountForm({
      account_name: account.account_name,
      account_id: account.account_id,
      password: '',
      enable: account.enable,
      enable_update: account.enable_update,
      admission_date: account.admission_date ?? '',
      graduation_date: account.graduation_date ?? '',
      graduation_to_present: !account.graduation_date,
    });
  };

  const handleSaveAccount = async () => {
    if (!selectedIdentity) return;
    try {
      if (selectedAccount && selectedAccount.id !== -1) {
        await tauri.update_account({
          id: selectedAccount.id,
          account_name: accountForm.account_name,
          account_id: accountForm.account_id,
          ...(accountForm.password ? { password: accountForm.password } : {}),
          enable: accountForm.enable,
          enable_update: accountForm.enable_update,
          admission_date: accountForm.admission_date || null,
          graduation_date: accountForm.graduation_to_present ? null : (accountForm.graduation_date || null),
          expire_date: accountForm.graduation_to_present ? '2099-12-31' : (accountForm.graduation_date || '2099-12-31'),
        });
        setNotice(`已更新账号「${accountForm.account_name}」`);
      } else {
        await tauri.create_account({
          identity_id: selectedIdentity.id,
          account_name: accountForm.account_name,
          account_id: accountForm.account_id,
          password: accountForm.password,
          enable: accountForm.enable,
          enable_update: accountForm.enable_update,
          admission_date: accountForm.admission_date || null,
          graduation_date: accountForm.graduation_to_present ? null : (accountForm.graduation_date || null),
          expire_date: accountForm.graduation_to_present ? '2099-12-31' : (accountForm.graduation_date || '2099-12-31'),
          last_update_time: '',
        });
        setNotice(`已添加账号「${accountForm.account_name}」`);
      }
      const accList = await tauri.list_accounts(selectedIdentity.id);
      setAccounts(accList);
      setAccountCounts((s) => ({ ...s, [selectedIdentity.id]: accList.length }));
      setSelectedAccount(null);
    } catch (e) {
      setError(extractErrorMessage(e, '保存账号失败'));
    }
  };

  const handleDeleteAccount = async (id: number) => {
    if (!confirm('确定要删除此账号吗？')) return;
    try {
      await tauri.delete_account(id);
      if (selectedIdentity) {
        const accList = await tauri.list_accounts(selectedIdentity.id);
        setAccounts(accList);
        setAccountCounts((s) => ({ ...s, [selectedIdentity.id]: accList.length }));
      }
      setSelectedAccount(null);
      setNotice('账号已删除');
    } catch (e) {
      setError(extractErrorMessage(e, '删除账号失败'));
    }
  };

  const handleRefreshPersonAccount = async (accountDbId: number) => {
    setPersonAccountLoading((s) => ({ ...s, [accountDbId]: true }));
    setPersonAccountError((s) => ({ ...s, [accountDbId]: null }));
    try {
      const info = await tauri.fetch_person_account(accountDbId);
      setPersonAccounts((s) => ({ ...s, [accountDbId]: info }));
      setNotice(`已刷新 ${accounts.find((item) => item.id === accountDbId)?.account_name ?? '账号'} 的一卡通详情`);
    } catch (e) {
      const marker = parsePersonAccountCaptchaMarker(e);
      if (marker) {
        const accountName = accounts.find((item) => item.id === accountDbId)?.account_name ?? `账号 ${accountDbId}`;
        setPersonAccountCaptcha({
          accountDbId,
          accountName,
          image: marker.image,
          execution: marker.execution,
        });
        setPersonAccountCaptchaCode('');
        return;
      }
      setPersonAccountError((s) => ({
        ...s,
        [accountDbId]: extractErrorMessage(e, '拉取失败'),
      }));
    } finally {
      setPersonAccountLoading((s) => ({ ...s, [accountDbId]: false }));
    }
  };

  const handleSubmitPersonAccountCaptcha = async () => {
    if (!personAccountCaptcha || !personAccountCaptchaCode.trim()) return;
    setSubmittingPersonAccountCaptcha(true);
    try {
      const info = await tauri.submit_person_account_captcha(
        personAccountCaptcha.accountDbId,
        personAccountCaptchaCode.trim(),
        personAccountCaptcha.execution
      );
      setPersonAccounts((s) => ({ ...s, [personAccountCaptcha.accountDbId]: info }));
      setPersonAccountError((s) => ({ ...s, [personAccountCaptcha.accountDbId]: null }));
      setNotice(`已完成 ${personAccountCaptcha.accountName} 的验证码登录并刷新详情`);
      setPersonAccountCaptcha(null);
      setPersonAccountCaptchaCode('');
    } catch (e) {
      const message = extractErrorMessage(e, '提交验证码失败');
      if (message.includes('VALIDATE_CODE_ERROR')) {
        setError('验证码错误，已为你重新获取新的验证码。');
        await handleRefreshPersonAccount(personAccountCaptcha.accountDbId);
        return;
      }
      setError(message);
    } finally {
      setSubmittingPersonAccountCaptcha(false);
    }
  };

  const selectedIdentityAccountCount = selectedIdentity ? (accountCounts[selectedIdentity.id] ?? accounts.length) : 0;

  return (
    <>
      <Dialog
        open={showIdentityManagerDialog}
        onOpenChange={(_, data) => !data.open && setShowIdentityManagerDialog(false)}
      >
        <DialogSurface
          style={{
            maxWidth: 1220,
            width: 'min(1220px, calc(100vw - 40px))',
            borderRadius: 28,
            overflow: 'hidden',
            background:
              'linear-gradient(180deg, color-mix(in srgb, var(--colorNeutralBackground1) 92%, white) 0%, color-mix(in srgb, var(--colorNeutralBackground2) 84%, white) 100%)',
            boxShadow: '0 28px 80px rgba(15, 23, 42, 0.18)',
          }}
        >
          <DialogBody>
            <DialogTitle>{selectedIdentity ? `${selectedIdentity.name} 的身份管理` : '身份与账号管理'}</DialogTitle>
            <DialogContent>
              <div className="identity-manager">
                {(notice || error) && (
                  <div style={{ display: 'grid', gap: 8 }}>
                    {notice && (
                      <MessageBar intent="success">
                        <MessageBarBody>{notice}</MessageBarBody>
                      </MessageBar>
                    )}
                    {error && (
                      <MessageBar intent="error">
                        <MessageBarBody>{error}</MessageBarBody>
                      </MessageBar>
                    )}
                  </div>
                )}

                <div
                  className="identity-manager__layout"
                >
                  <Card
                    className="identity-manager__sidebar"
                  >
                    <div style={{ display: 'grid', gap: 16 }}>
                      <div className="identity-manager__hero">
                        <Text size={200} className="identity-manager__eyebrow">
                          身份中心
                        </Text>
                        <Text weight="semibold" size={600} block style={{ marginTop: 8, lineHeight: 1.2 }}>
                          当前共管理 {identities.length} 个身份
                        </Text>
                        <Text size={200} className="identity-manager__hero-subtitle">
                          统一切换身份、维护账号，并集中查看一卡通余额与个人账户信息。
                        </Text>
                        <div className="identity-manager__metrics">
                          <MetricTile label="身份数" value={identities.length.toString()} />
                          <MetricTile
                            label="账号数"
                            value={isLoadingCounts ? '...' : totalAccounts.toString()}
                          />
                        </div>
                      </div>

                      <div className="identity-manager__sidebar-actions">
                        <Button
                          appearance="primary"
                          icon={<PersonAdd24Regular />}
                          onClick={() => setIsAddingIdentity(true)}
                          className="identity-manager__action-button"
                        >
                          添加身份
                        </Button>
                        <Button
                          appearance="secondary"
                          icon={<ArrowSync24Regular />}
                          onClick={() => void handleRefreshAccountCounts()}
                          disabled={isLoadingCounts}
                          className="identity-manager__action-button"
                        >
                          刷新统计
                        </Button>
                      </div>

                      {isAddingIdentity && (
                        <Card
                          style={{
                            padding: 14,
                            borderRadius: 14,
                            background: 'var(--colorNeutralBackground2)',
                            border: '1px solid var(--colorNeutralStroke2)',
                          }}
                        >
                          <div style={{ display: 'grid', gap: 10 }}>
                            <Field label="身份名称">
                              <Input
                                placeholder="例如：本科 / 研究生 / 家人"
                                value={addingIdentityName}
                                onChange={(e) => setAddingIdentityName(e.currentTarget.value)}
                                onKeyDown={(e) => {
                                  if (e.key === 'Enter') {
                                    void handleAddIdentity();
                                  }
                                }}
                                disabled={addingIdentityLoading}
                              />
                            </Field>
                            <div style={{ display: 'flex', gap: 8 }}>
                              <Button
                                appearance="primary"
                                onClick={() => void handleAddIdentity()}
                                disabled={addingIdentityLoading || !addingIdentityName.trim()}
                              >
                                {addingIdentityLoading ? '创建中...' : '确认创建'}
                              </Button>
                              <Button
                                appearance="secondary"
                                onClick={() => {
                                  setIsAddingIdentity(false);
                                  setAddingIdentityName('');
                                }}
                                disabled={addingIdentityLoading}
                              >
                                取消
                              </Button>
                            </div>
                          </div>
                        </Card>
                      )}

                      <div className="identity-manager__identity-list-shell">
                        <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center' }}>
                          <Text weight="semibold">身份列表</Text>
                          {isLoadingCounts && <Spinner size="tiny" />}
                        </div>
                        <div className="identity-manager__identity-list">
                          {sortedIdentities.map((identity) => {
                            const isSelected = selectedIdentity?.id === identity.id;
                            const isCurrent = currentIdentity?.id === identity.id;
                            const count = accountCounts[identity.id] ?? 0;
                            return (
                              <Card
                                key={identity.id}
                                className={`motion-hover-lift motion-sheen identity-manager__identity-card${
                                  isSelected ? ' is-selected' : ''
                                }${isCurrent ? ' is-current' : ''}`}
                                onClick={() => void handleSelectIdentity(identity)}
                              >
                                <div style={{ display: 'grid', gap: 10 }}>
                                  <div style={{ display: 'flex', justifyContent: 'space-between', gap: 12, alignItems: 'flex-start' }}>
                                    <div style={{ display: 'flex', gap: 12, minWidth: 0 }}>
                                      <div className="identity-manager__identity-avatar">
                                        {identity.name.slice(0, 1).toUpperCase()}
                                      </div>
                                      <div style={{ minWidth: 0 }}>
                                        <Text weight="semibold" size={400} block>
                                          {identity.name}
                                        </Text>
                                        <Text size={200} style={{ color: 'var(--colorNeutralForeground3)' }}>
                                          身份 ID #{identity.id}
                                        </Text>
                                      </div>
                                    </div>
                                    <div style={{ display: 'flex', gap: 4 }}>
                                      <Button
                                        appearance="subtle"
                                        icon={<Edit24Regular />}
                                        size="small"
                                        onClick={(e) => {
                                          e.stopPropagation();
                                          handleStartEditIdentity(identity);
                                        }}
                                      />
                                      <Button
                                        appearance="subtle"
                                        icon={<Delete24Regular />}
                                        size="small"
                                        onClick={(e) => {
                                          e.stopPropagation();
                                          void handleDeleteIdentity(identity.id);
                                        }}
                                      />
                                    </div>
                                  </div>
                                  <div style={{ display: 'flex', gap: 6, flexWrap: 'wrap' }}>
                                    {isCurrent && (
                                      <Badge appearance="filled" color="brand">
                                        当前身份
                                      </Badge>
                                    )}
                                    {isSelected && (
                                      <Badge appearance="tint" color="informative">
                                        已选中
                                      </Badge>
                                    )}
                                    <Badge appearance="outline">{count} 个账号</Badge>
                                  </div>
                                </div>
                              </Card>
                            );
                          })}
                        </div>
                      </div>
                    </div>
                  </Card>

                  <div className="identity-manager__content">
                    {selectedIdentity ? (
                      <>
                        <Card
                          className="identity-manager__summary-card"
                        >
                          <div style={{ display: 'grid', gap: 14 }}>
                            <div
                              style={{
                                display: 'flex',
                                justifyContent: 'space-between',
                                gap: 16,
                                alignItems: 'flex-start',
                                flexWrap: 'wrap',
                              }}
                            >
                              <div style={{ minWidth: 0 }}>
                                <Text size={200} style={{ color: 'var(--colorNeutralForeground3)' }}>
                                  身份概览
                                </Text>
                                <Text weight="semibold" size={600} block style={{ marginTop: 4 }}>
                                  {selectedIdentity.name}
                                </Text>
                                <Text size={200} style={{ color: 'var(--colorNeutralForeground3)', marginTop: 6 }}>
                                  当前身份下共 {selectedIdentityAccountCount} 个账号，其中 {selectedIdentityProfileCoverage} 个账号已缓存一卡通个人账户信息。
                                </Text>
                              </div>
                              <div style={{ display: 'flex', gap: 8, flexWrap: 'wrap' }}>
                                <Button
                                  appearance="primary"
                                  disabled={selectedIdentity.id === currentIdentity?.id}
                                  onClick={() => {
                                    activateIdentity(selectedIdentity).catch(console.error);
                                    setShowIdentityManagerDialog(false);
                                  }}
                                >
                                  {selectedIdentity.id === currentIdentity?.id ? '当前身份' : '设为当前身份'}
                                </Button>
                                <Button
                                  appearance="secondary"
                                  onClick={() => handleStartEditIdentity(selectedIdentity)}
                                >
                                  编辑身份
                                </Button>
                              </div>
                            </div>

                            <div className="identity-manager__metrics identity-manager__metrics--wide">
                              <MetricTile label="账号总数" value={selectedIdentityAccountCount.toString()} />
                              <MetricTile
                                label="余额合计"
                                value={selectedIdentityBalanceSum > 0 ? `${selectedIdentityBalanceSum.toFixed(2)} 元` : '未获取'}
                              />
                              <MetricTile
                                label="详情缓存"
                                value={`${selectedIdentityProfileCoverage}/${selectedIdentityAccountCount || 0}`}
                              />
                            </div>
                          </div>
                        </Card>

                        <div className="identity-manager__panes">
                          <Card
                            className="identity-manager__accounts-pane"
                          >
                            <div style={{ display: 'grid', gap: 14 }}>
                              <SectionEnterMotion>
                                <div style={{ display: 'flex', justifyContent: 'space-between', gap: 12, alignItems: 'center', flexWrap: 'wrap' }}>
                                  <div>
                                    <Text weight="semibold" size={400} block>
                                      账号与一卡通信息
                                    </Text>
                                    <Text size={200} style={{ color: 'var(--colorNeutralForeground3)' }}>
                                      在这里直接刷新余额与个人账户信息，或切换到账号编辑表单。
                                    </Text>
                                  </div>
                                  <Button
                                    appearance="subtle"
                                    icon={<PersonAdd24Regular />}
                                    onClick={() => {
                                      setSelectedAccount({ id: -1 } as Account);
                                      setAccountForm({
                                        account_name: '',
                                        account_id: '',
                                        password: '',
                                        enable: true,
                                        enable_update: true,
                                        admission_date: '',
                                        graduation_date: '',
                                        graduation_to_present: true,
                                      });
                                    }}
                                  >
                                    添加账号
                                  </Button>
                                </div>
                              </SectionEnterMotion>

                              <PageEnterMotion key={selectedIdentity.id}>
                                <div className="identity-manager__accounts-list">
                                  {accounts.length === 0 ? (
                                    <div
                                      style={{
                                        padding: 28,
                                        borderRadius: 16,
                                        border: '1px dashed var(--colorNeutralStroke2)',
                                        background: 'var(--colorNeutralBackground2)',
                                        textAlign: 'center',
                                      }}
                                    >
                                      <Text weight="semibold" block style={{ marginBottom: 6 }}>
                                        这个身份下还没有账号
                                      </Text>
                                      <Text size={200} style={{ color: 'var(--colorNeutralForeground3)' }}>
                                        先添加校园卡账号，之后才能拉取账单和余额信息。
                                      </Text>
                                    </div>
                                  ) : (
                                    accounts.map((account) => (
                                      <AccountManagementCard
                                        key={account.id}
                                        account={account}
                                        info={personAccounts[account.id] ?? null}
                                        isSelected={selectedAccount?.id === account.id}
                                        isLoading={!!personAccountLoading[account.id]}
                                        error={personAccountError[account.id] ?? null}
                                        onSelect={() => handleSelectAccount(account)}
                                        onRefresh={() => void handleRefreshPersonAccount(account.id)}
                                        onDelete={() => void handleDeleteAccount(account.id)}
                                      />
                                    ))
                                  )}
                                </div>
                              </PageEnterMotion>
                            </div>
                          </Card>

                          <Card
                            className="identity-manager__editor-pane"
                          >
                            <div style={{ display: 'grid', gap: 14 }}>
                              <div>
                                <Text weight="semibold" size={400} block>
                                  {selectedAccount
                                    ? selectedAccount.id === -1
                                      ? '添加账号'
                                      : `编辑 ${selectedAccount.account_name}`
                                    : '账号编辑区'}
                                </Text>
                                <Text size={200} style={{ color: 'var(--colorNeutralForeground3)' }}>
                                  {selectedAccount
                                    ? '修改账号基础信息、启用状态和同步有效时间。'
                                    : '从左侧选择一个账号，或新建一个账号。'}
                                </Text>
                              </div>

                              {selectedAccount ? (
                                <div style={{ display: 'grid', gap: 12 }}>
                                  <Field label="账号名称">
                                    <Input
                                      value={accountForm.account_name}
                                      onChange={(e) =>
                                        setAccountForm((s) => ({ ...s, account_name: e.currentTarget.value }))
                                      }
                                    />
                                  </Field>
                                  <Field label="学号（12 位数字）">
                                    <Input
                                      value={accountForm.account_id}
                                      onChange={(e) =>
                                        setAccountForm((s) => ({ ...s, account_id: e.currentTarget.value }))
                                      }
                                      placeholder="202012345678"
                                    />
                                  </Field>
                                  <Field label="密码">
                                    <Input
                                      type="password"
                                      value={accountForm.password}
                                      onChange={(e) =>
                                        setAccountForm((s) => ({ ...s, password: e.currentTarget.value }))
                                      }
                                      placeholder={selectedAccount.id !== -1 ? '留空则不修改' : '请输入密码'}
                                    />
                                  </Field>
                                  <Field label="开始时间">
                                    <Input
                                      type="date"
                                      value={accountForm.admission_date}
                                      onChange={(e) =>
                                        setAccountForm((s) => ({ ...s, admission_date: e.currentTarget.value }))
                                      }
                                    />
                                  </Field>
                                  <div style={{ display: 'grid', gap: 8 }}>
                                    <div style={{ display: 'flex', justifyContent: 'space-between', gap: 12, alignItems: 'center' }}>
                                      <Text weight="medium">结束时间</Text>
                                      <Switch
                                        label="至今"
                                        checked={accountForm.graduation_to_present}
                                        onChange={(_, data) =>
                                          setAccountForm((s) => ({
                                            ...s,
                                            graduation_to_present: data.checked,
                                            graduation_date: data.checked ? '' : s.graduation_date,
                                          }))
                                        }
                                      />
                                    </div>
                                    <Input
                                      type="date"
                                      value={accountForm.graduation_date}
                                      onChange={(e) =>
                                        setAccountForm((s) => ({
                                          ...s,
                                          graduation_date: e.currentTarget.value,
                                          graduation_to_present: false,
                                        }))
                                      }
                                      disabled={accountForm.graduation_to_present}
                                    />
                                  </div>
                                  <div style={{ display: 'flex', gap: 16, flexWrap: 'wrap' }}>
                                    <Switch
                                      label="启用账号"
                                      checked={accountForm.enable}
                                      onChange={(_, data) =>
                                        setAccountForm((s) => ({ ...s, enable: data.checked }))
                                      }
                                    />
                                    <Switch
                                      label="允许同步"
                                      checked={accountForm.enable_update}
                                      onChange={(_, data) =>
                                        setAccountForm((s) => ({ ...s, enable_update: data.checked }))
                                      }
                                    />
                                  </div>
                                  <Divider />
                                  <div style={{ display: 'flex', gap: 8, flexWrap: 'wrap' }}>
                                    <Button appearance="primary" onClick={() => void handleSaveAccount()}>
                                      保存
                                    </Button>
                                    {selectedAccount.id !== -1 && (
                                      <Button
                                        appearance="secondary"
                                        icon={<Delete24Regular />}
                                        onClick={() => void handleDeleteAccount(selectedAccount.id)}
                                      >
                                        删除账号
                                      </Button>
                                    )}
                                  </div>
                                </div>
                              ) : (
                                <div
                                  style={{
                                    padding: 24,
                                    borderRadius: 16,
                                    border: '1px dashed var(--colorNeutralStroke2)',
                                    background: 'var(--colorNeutralBackground2)',
                                  }}
                                >
                                  <Text size={200} style={{ color: 'var(--colorNeutralForeground3)' }}>
                                    左侧账号卡会展示余额、实名状态、有效期，以及接口实际返回的所有个人账户信息。
                                  </Text>
                                </div>
                              )}
                            </div>
                          </Card>
                        </div>
                      </>
                    ) : (
                      <Card
                        className="identity-manager__empty"
                      >
                        <div style={{ maxWidth: 420 }}>
                          <Text weight="semibold" size={500} block style={{ marginBottom: 8 }}>
                            请选择一个身份
                          </Text>
                          <Text size={200} style={{ color: 'var(--colorNeutralForeground3)' }}>
                            左侧会列出所有身份；选中后即可查看账号、刷新一卡通余额与个人账户信息，并设置当前身份。
                          </Text>
                        </div>
                      </Card>
                    )}
                  </div>
                </div>
              </div>
            </DialogContent>
            <DialogActions>
              <Button appearance="secondary" onClick={() => setShowIdentityManagerDialog(false)}>
                关闭
              </Button>
            </DialogActions>
          </DialogBody>
        </DialogSurface>
      </Dialog>

      <Dialog
        open={editingIdentityId !== null}
        onOpenChange={(_, data) => !data.open && handleCancelEditIdentity()}
      >
        <DialogSurface style={{ maxWidth: 440 }}>
          <DialogBody>
            <DialogTitle>编辑身份信息</DialogTitle>
            <DialogContent>
              <div style={{ display: 'grid', gap: 12 }}>
                <Field label="身份名称">
                  <Input
                    value={editingIdentityName}
                    onChange={(e) => setEditingIdentityName(e.currentTarget.value)}
                    onKeyDown={(e) => {
                      if (e.key === 'Enter') {
                        void handleSaveEditIdentity();
                      }
                      if (e.key === 'Escape') {
                        handleCancelEditIdentity();
                      }
                    }}
                    placeholder="输入身份名称"
                  />
                </Field>
                <Text size={200} style={{ color: 'var(--colorNeutralForeground3)' }}>
                  修改后只会更新界面展示名称，不影响该身份下已有账号和账单数据。
                </Text>
              </div>
            </DialogContent>
            <DialogActions>
              <Button appearance="secondary" onClick={handleCancelEditIdentity}>
                取消
              </Button>
              <Button appearance="primary" onClick={() => void handleSaveEditIdentity()}>
                保存
              </Button>
            </DialogActions>
          </DialogBody>
        </DialogSurface>
      </Dialog>

      <Dialog open={!!personAccountCaptcha}>
        <DialogSurface style={{ maxWidth: 460 }}>
          <DialogBody>
            <DialogTitle>
              <Shield24Regular style={{ marginRight: 8 }} />
              一卡通验证码
            </DialogTitle>
            <DialogContent>
              {personAccountCaptcha && (
                <div style={{ display: 'grid', gap: 12 }}>
                  <Text size={200} style={{ color: 'var(--colorNeutralForeground3)' }}>
                    {personAccountCaptcha.accountName} 的会话已过期，请输入验证码以继续刷新余额与个人账户信息。
                  </Text>
                  <div
                    style={{
                      display: 'flex',
                      justifyContent: 'center',
                      padding: 16,
                      borderRadius: 14,
                      background: 'var(--colorNeutralBackground2)',
                    }}
                  >
                    <img
                      src={`data:image/png;base64,${personAccountCaptcha.image}`}
                      alt="验证码"
                      style={{ maxWidth: '100%', height: 'auto', borderRadius: 8 }}
                    />
                  </div>
                  <Field label="验证码结果">
                    <Input
                      autoFocus
                      placeholder="请输入验证码结果"
                      value={personAccountCaptchaCode}
                      onChange={(e) => setPersonAccountCaptchaCode(e.currentTarget.value)}
                      onKeyDown={(e) => {
                        if (e.key === 'Enter') {
                          void handleSubmitPersonAccountCaptcha();
                        }
                      }}
                    />
                  </Field>
                </div>
              )}
            </DialogContent>
            <DialogActions>
              <Button
                appearance="secondary"
                onClick={() => {
                  setPersonAccountCaptcha(null);
                  setPersonAccountCaptchaCode('');
                }}
                disabled={submittingPersonAccountCaptcha}
              >
                取消
              </Button>
              <Button
                appearance="primary"
                onClick={() => void handleSubmitPersonAccountCaptcha()}
                disabled={!personAccountCaptchaCode.trim() || submittingPersonAccountCaptcha}
              >
                {submittingPersonAccountCaptcha ? '提交中...' : '确认'}
              </Button>
            </DialogActions>
          </DialogBody>
        </DialogSurface>
      </Dialog>
    </>
  );
};

function MetricTile({ label, value }: { label: string; value: string }) {
  return (
    <div
      className="identity-manager__metric-tile"
    >
      <Text size={200} className="identity-manager__metric-label">
        {label}
      </Text>
      <Text weight="semibold" size={500} block className="identity-manager__metric-value">
        {value}
      </Text>
    </div>
  );
}

interface AccountManagementCardProps {
  account: Account;
  info: PersonAccountInfo | null;
  isSelected: boolean;
  isLoading: boolean;
  error: string | null;
  onSelect: () => void;
  onRefresh: () => void;
  onDelete: () => void;
}

function AccountManagementCard({
  account,
  info,
  isSelected,
  isLoading,
  error,
  onSelect,
  onRefresh,
  onDelete,
}: AccountManagementCardProps) {
  return (
    <Card
      className={`motion-hover-lift motion-sheen identity-manager__account-card${
        isSelected ? ' is-selected' : ''
      }`}
      onClick={onSelect}
    >
      <div style={{ display: 'grid', gap: 12 }}>
        <div style={{ display: 'flex', justifyContent: 'space-between', gap: 12, alignItems: 'flex-start' }}>
          <div style={{ display: 'flex', gap: 12, minWidth: 0 }}>
            <div className="identity-manager__account-avatar">
              {account.account_name.slice(0, 1).toUpperCase()}
            </div>
            <div style={{ minWidth: 0 }}>
              <Text weight="semibold" size={400} block>
                {account.account_name}
              </Text>
              <Text size={200} style={{ color: 'var(--colorNeutralForeground3)' }}>
                学号 {account.account_id}
              </Text>
            </div>
          </div>
          <div style={{ display: 'flex', gap: 4 }}>
            <Button
              appearance="subtle"
              icon={isLoading ? <Spinner size="tiny" /> : <ArrowSync24Regular />}
              size="small"
              onClick={(e) => {
                e.stopPropagation();
                onRefresh();
              }}
              disabled={isLoading}
            />
            <Button
              appearance="subtle"
              icon={<Delete24Regular />}
              size="small"
              onClick={(e) => {
                e.stopPropagation();
                onDelete();
              }}
            />
          </div>
        </div>

        <div style={{ display: 'flex', gap: 6, flexWrap: 'wrap' }}>
          <Badge appearance={account.enable ? 'filled' : 'outline'} color={account.enable ? 'success' : 'danger'}>
            {account.enable ? '已启用' : '已禁用'}
          </Badge>
          <Badge appearance={account.enable_update ? 'filled' : 'outline'} color={account.enable_update ? 'brand' : 'warning'}>
            {account.enable_update ? '允许同步' : '停止同步'}
          </Badge>
          {info?.cash_balance_raw && (
            <Badge appearance="tint" color="informative" icon={<Money24Regular />}>
              {info.cash_balance_raw} 元
            </Badge>
          )}
        </div>

        <div
          style={{
            display: 'grid',
            gridTemplateColumns: 'repeat(2, minmax(0, 1fr))',
            gap: 10,
          }}
        >
          <InlineInfo label="实名" value={info?.real_name || '未获取'} />
          <InlineInfo label="实名状态" value={info?.real_name_auth_status || '未获取'} />
          <InlineInfo label="有效期" value={formatTimeline(account.admission_date, account.graduation_date)} />
          <InlineInfo label="更新时间" value={info ? formatDateTime(info.fetched_at) : '未获取'} />
        </div>

        {info && (
          <div
            className="identity-manager__detail-block"
          >
            {buildPersonAccountRows(info).map((row) => (
              <div key={row.label} style={{ display: 'flex', gap: 8, alignItems: 'baseline' }}>
                <Text
                  size={100}
                  style={{
                    color: 'var(--colorNeutralForeground3)',
                    minWidth: 78,
                    flexShrink: 0,
                  }}
                >
                  {row.label}
                </Text>
                <Text size={100}>{row.value}</Text>
              </div>
            ))}
          </div>
        )}

        {error && (
          <MessageBar intent="error">
            <MessageBarBody>{error}</MessageBarBody>
          </MessageBar>
        )}
      </div>
    </Card>
  );
}

function InlineInfo({ label, value }: { label: string; value: string }) {
  return (
    <div
      style={{
        padding: '10px 12px',
        borderRadius: 12,
        background: 'var(--colorNeutralBackground2)',
      }}
    >
      <Text size={100} style={{ color: 'var(--colorNeutralForeground3)' }}>
        {label}
      </Text>
      <Text size={200} block style={{ marginTop: 2 }}>
        {value}
      </Text>
    </div>
  );
}

function parsePersonAccountCaptchaMarker(error: unknown): { image: string; execution: string } | null {
  const message = typeof error === 'string'
    ? error
    : error instanceof Error
      ? error.message
      : '';
  if (!message.startsWith('MANUAL_CAPTCHA_REQUIRED|')) return null;
  const [, image, execution] = message.split('|');
  if (!image || !execution) return null;
  return { image, execution };
}

function extractErrorMessage(error: unknown, fallback: string): string {
  if (typeof error === 'string' && error.trim()) return error;
  if (error instanceof Error && error.message.trim()) return error.message;
  return fallback;
}

function formatTimeline(start: string | null, end: string | null): string {
  const left = start || '未设置';
  const right = end || '至今';
  return `${left} - ${right}`;
}

function formatDateTime(value: string): string {
  if (!value) return '未获取';
  const date = new Date(value);
  if (Number.isNaN(date.getTime())) return value;
  return date.toLocaleString('zh-CN', {
    year: 'numeric',
    month: '2-digit',
    day: '2-digit',
    hour: '2-digit',
    minute: '2-digit',
  });
}

function buildPersonAccountRows(info: PersonAccountInfo): Array<{ label: string; value: string }> {
  return [
    ['姓名', info.real_name],
    ['实名认证', info.real_name_auth_status],
    ['现金资金', info.cash_balance_raw ? `${info.cash_balance_raw} 元` : ''],
    ['安全保护问题', info.security_question_status],
    ['注册时间', info.register_date],
    ['学工号', info.student_id],
    ['电子邮箱', info.email],
    ['昵称', info.nickname],
    ['性别', info.gender],
    ['班级', info.class_name],
    ['手机/固话', info.phone_num],
    ['证件类型', info.id_type],
    ['证件号码', info.id_number],
    ['用户类型', info.user_type],
    ['备注', info.remark],
  ]
    .filter(([, value]) => value && value.trim() !== '')
    .map(([label, value]) => ({ label, value }));
}
