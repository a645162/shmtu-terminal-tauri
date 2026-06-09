import React, { useEffect, useRef, useState } from 'react';
import {
  Dialog,
  DialogSurface,
  DialogTitle,
  DialogBody,
  DialogContent,
  DialogActions,
  Button,
  Input,
  Switch,
  Text,
  MessageBar,
  MessageBarBody,
  Label,
  Divider,
  Spinner,
  Card,
  CardHeader,
} from '@fluentui/react-components';
import { PersonAdd24Regular, Delete24Regular, Edit24Regular, ArrowSync24Regular, Money24Regular } from '@fluentui/react-icons';
import { useAppStore } from '../../stores/appStore';
import type { Identity, Account, PersonAccountInfo } from '../../types';
import * as tauri from '../../services/tauri';
import {
  PageEnterMotion,
  SectionEnterMotion,
} from '../../components/Common/motion';

export const IdentityManagerDialog: React.FC = () => {
  const showIdentityManagerDialog = useAppStore((s) => s.showIdentityManagerDialog);
  const setShowIdentityManagerDialog = useAppStore((s) => s.setShowIdentityManagerDialog);
  const identities = useAppStore((s) => s.identities);
  const loadIdentities = useAppStore((s) => s.loadIdentities);
  const currentIdentity = useAppStore((s) => s.currentIdentity);
  const setCurrentIdentity = useAppStore((s) => s.setCurrentIdentity);

  const [selectedIdentity, setSelectedIdentity] = useState<Identity | null>(null);
  const [selectedAccount, setSelectedAccount] = useState<Account | null>(null);
  const [accounts, setAccounts] = useState<Account[]>([]);
  const [newIdentityName, setNewIdentityName] = useState('');
  const [isAddingIdentity, setIsAddingIdentity] = useState(false);
  const [editingIdentityId, setEditingIdentityId] = useState<number | null>(null);
  const [editingIdentityName, setEditingIdentityName] = useState('');

  // 打开 dialog 或 currentIdentity 变化时, 如果尚未选中身份, 自动选中 currentIdentity
  const initialSelected = useRef(false);
  useEffect(() => {
    if (!showIdentityManagerDialog) {
      initialSelected.current = false;
      return;
    }
    if (selectedIdentity) return;
    const identity = currentIdentity ?? identities[0] ?? null;
    if (identity && !initialSelected.current) {
      // 只在打开 dialog 时执行一次, 避免 currentIdentity.id 变化时(引用相同对象)也触发
      initialSelected.current = true;
      setSelectedIdentity(identity);
      // 同时拉取该身份的账号列表, 否则右边区会显示"暂无账号"
      tauri.list_accounts(identity.id).then(setAccounts).catch(() => setAccounts([]));
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [showIdentityManagerDialog, selectedIdentity?.id]);

  // Account form state
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

  // 一卡通个人账户详情：按 account_db_id 缓存
  const [personAccounts, setPersonAccounts] = useState<Record<number, PersonAccountInfo | null>>({});
  const [personAccountLoading, setPersonAccountLoading] = useState<Record<number, boolean>>({});
  const [personAccountError, setPersonAccountError] = useState<Record<number, string | null>>({});

  // 选中身份变化时，按账号列表批量加载缓存
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
      } catch {
        if (!cancelled) setPersonAccounts({});
      }
    })();
    return () => {
      cancelled = true;
    };
  }, [selectedIdentity, accounts]);

  const handleRefreshPersonAccount = async (accountDbId: number) => {
    setPersonAccountLoading((s) => ({ ...s, [accountDbId]: true }));
    setPersonAccountError((s) => ({ ...s, [accountDbId]: null }));
    try {
      const info = await tauri.fetch_person_account(accountDbId);
      setPersonAccounts((s) => ({ ...s, [accountDbId]: info }));
    } catch (e) {
      setPersonAccountError((s) => ({
        ...s,
        [accountDbId]: typeof e === 'string' ? e : (e instanceof Error ? e.message : '拉取失败'),
      }));
    } finally {
      setPersonAccountLoading((s) => ({ ...s, [accountDbId]: false }));
    }
  };

  const handleSelectIdentity = async (identity: Identity) => {
    setSelectedIdentity(identity);
    setSelectedAccount(null);
    try {
      const accList = await tauri.list_accounts(identity.id);
      setAccounts(accList);
    } catch {
      setAccounts([]);
    }
  };

  const handleAddIdentity = async () => {
    if (!newIdentityName.trim()) return;
    try {
      await tauri.create_identity(newIdentityName.trim());
      setNewIdentityName('');
      setIsAddingIdentity(false);
      loadIdentities();
    } catch (e) {
      console.error('Failed to create identity:', e);
    }
  };

  const handleDeleteIdentity = async (id: number) => {
    if (!confirm('确定要删除此身份吗？删除后数据无法恢复。')) return;
    try {
      await tauri.delete_identity(id);
      if (selectedIdentity?.id === id) {
        setSelectedIdentity(null);
        setAccounts([]);
      }
      loadIdentities();
    } catch (e) {
      console.error('Failed to delete identity:', e);
    }
  };

  const handleStartEditIdentity = (identity: Identity) => {
    setEditingIdentityId(identity.id);
    setEditingIdentityName(identity.name);
  };

  const handleSaveEditIdentity = async () => {
    if (!editingIdentityId || !editingIdentityName.trim()) return;
    try {
      const identity = identities.find((i) => i.id === editingIdentityId);
      if (identity) {
        const updatedIdentity = { ...identity, name: editingIdentityName.trim() };
        await tauri.update_identity(updatedIdentity);
        await loadIdentities();
        if (selectedIdentity?.id === editingIdentityId) {
          setSelectedIdentity(updatedIdentity);
        }
        if (currentIdentity?.id === editingIdentityId) {
          setCurrentIdentity(updatedIdentity);
        }
      }
    } catch (e) {
      console.error('Failed to update identity:', e);
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
      }
      const accList = await tauri.list_accounts(selectedIdentity.id);
      setAccounts(accList);
      setSelectedAccount(null);
    } catch (e) {
      console.error('Failed to save account:', e);
    }
  };

  const handleDeleteAccount = async (id: number) => {
    if (!confirm('确定要删除此账号吗？')) return;
    try {
      await tauri.delete_account(id);
      if (selectedIdentity) {
        const accList = await tauri.list_accounts(selectedIdentity.id);
        setAccounts(accList);
      }
      setSelectedAccount(null);
    } catch (e) {
      console.error('Failed to delete account:', e);
    }
  };

  return (
    <>
      <Dialog open={showIdentityManagerDialog} onOpenChange={(_, data) => !data.open && setShowIdentityManagerDialog(false)}>
        <DialogSurface style={{ maxWidth: 800 }}>
          <DialogBody>
            <DialogTitle>身份与账号管理</DialogTitle>
            <DialogContent>
            <div style={{ display: 'grid', gridTemplateColumns: '200px 1fr', gap: 16, minHeight: 400 }}>
              {/* Left: Identity List */}
              <div
                style={{
                  borderRight: '1px solid var(--colorNeutralStroke2)',
                  paddingRight: 12,
                }}
              >
                <Text weight="semibold" block style={{ marginBottom: 8 }}>
                  身份列表
                </Text>
                {identities.map((identity) => (
                  <div
                    key={identity.id}
                    onClick={() => handleSelectIdentity(identity)}
                    className="motion-hover-lift"
                    style={{
                      padding: '8px 12px',
                      borderRadius: 4,
                      cursor: 'pointer',
                      backgroundColor:
                        selectedIdentity?.id === identity.id
                          ? 'var(--colorBrandBackground2)'
                          : 'transparent',
                      marginBottom: 4,
                      display: 'flex',
                      justifyContent: 'space-between',
                      alignItems: 'center',
                    }}
                  >
                    <Text
                      size={200}
                      weight={selectedIdentity?.id === identity.id ? 'semibold' : 'regular'}
                    >
                      {identity.name}
                    </Text>
                    <div style={{ display: 'flex', gap: 2 }}>
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
                          handleDeleteIdentity(identity.id);
                        }}
                      />
                    </div>
                  </div>
                ))}
                {isAddingIdentity ? (
                  <div style={{ marginTop: 8 }}>
                    <Input
                      size="small"
                      placeholder="输入身份名称"
                      value={newIdentityName}
                      onChange={(e) => setNewIdentityName(e.currentTarget.value)}
                      onKeyDown={(e) => e.key === 'Enter' && handleAddIdentity()}
                    />
                    <div style={{ display: 'flex', gap: 4, marginTop: 4 }}>
                      <Button size="small" appearance="primary" onClick={handleAddIdentity}>
                        确定
                      </Button>
                      <Button size="small" onClick={() => setIsAddingIdentity(false)}>
                        取消
                      </Button>
                    </div>
                  </div>
                ) : (
                  <Button
                    appearance="subtle"
                    icon={<PersonAdd24Regular />}
                    size="small"
                    onClick={() => setIsAddingIdentity(true)}
                    style={{ marginTop: 8, width: '100%' }}
                  >
                    添加身份
                  </Button>
                )}
              </div>

              {/* Right: Account List + Form */}
              <div>
                {selectedIdentity ? (
                  <>
                    <SectionEnterMotion>
                      <Text weight="semibold" block style={{ marginBottom: 8 }}>
                        {selectedIdentity.name} 的账号列表
                      </Text>
                    </SectionEnterMotion>
                    <PageEnterMotion key={selectedIdentity.id}>
                      <div>
                        {accounts.length === 0 ? (
                          <Text size={200} style={{ color: 'var(--colorNeutralForeground3)' }}>
                            暂无账号
                          </Text>
                        ) : (
                          accounts.map((account) => (
                            <div
                              key={account.id}
                              onClick={() => handleSelectAccount(account)}
                              className="motion-hover-lift motion-sheen"
                              style={{
                                padding: 12,
                                border: '1px solid var(--colorNeutralStroke2)',
                                borderRadius: 4,
                                marginBottom: 8,
                                cursor: 'pointer',
                                backgroundColor:
                                  selectedAccount?.id === account.id
                                    ? 'var(--colorBrandBackground2)'
                                    : 'transparent',
                              }}
                            >
                              <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center' }}>
                                <Text size={200} weight="semibold">
                                  {account.account_name}
                                </Text>
                                <div style={{ display: 'flex', gap: 2 }}>
                                  <Button
                                    appearance="subtle"
                                    icon={<ArrowSync24Regular />}
                                    size="small"
                                    onClick={(e) => {
                                      e.stopPropagation();
                                      void handleRefreshPersonAccount(account.id);
                                    }}
                                    disabled={!!personAccountLoading[account.id]}
                                    title="刷新一卡通个人账户"
                                  />
                                  <Button
                                    appearance="subtle"
                                    icon={<Delete24Regular />}
                                    size="small"
                                    onClick={(e) => {
                                      e.stopPropagation();
                                      handleDeleteAccount(account.id);
                                    }}
                                  />
                                </div>
                              </div>
                              <Text size={100} style={{ color: 'var(--colorNeutralForeground3)' }}>
                                学号: {account.account_id} | {account.enable ? '已启用' : '已禁用'}
                              </Text>
                              <Text size={100} style={{ color: 'var(--colorNeutralForeground3)' }}>
                                学籍: {account.admission_date || '未设置'} 至 {account.graduation_date || '至今'}
                              </Text>
                              <PersonAccountSection
                                accountId={account.id}
                                info={personAccounts[account.id] ?? null}
                                loading={!!personAccountLoading[account.id]}
                                error={personAccountError[account.id] ?? null}
                                onRefresh={() => void handleRefreshPersonAccount(account.id)}
                              />
                            </div>
                          ))
                        )}
                      </div>
                    </PageEnterMotion>
                    <Button
                      appearance="subtle"
                      icon={<PersonAdd24Regular />}
                      size="small"
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
                      style={{ marginTop: 4 }}
                    >
                      添加账号
                    </Button>

                    {/* Account Edit Form */}
                    {selectedAccount && (
                      <PageEnterMotion key={selectedAccount.id}>
                        <div>
                        <Divider style={{ margin: '12px 0' }} />
                        <Text weight="semibold" block style={{ marginBottom: 8 }}>
                          {selectedAccount.id === -1 ? '添加账号' : '编辑账号'}
                        </Text>
                        <div style={{ display: 'grid', gap: 8 }}>
                          <div>
                            <Label>账号名称</Label>
                            <Input
                              value={accountForm.account_name}
                              onChange={(e) =>
                                setAccountForm({ ...accountForm, account_name: e.currentTarget.value })
                              }
                              style={{ width: '100%' }}
                            />
                          </div>
                          <div>
                            <Label>学号（12位数字）</Label>
                            <Input
                              value={accountForm.account_id}
                              onChange={(e) =>
                                setAccountForm({ ...accountForm, account_id: e.currentTarget.value })
                              }
                              style={{ width: '100%' }}
                              placeholder="202012345678"
                            />
                          </div>
                          <div>
                            <Label>密码</Label>
                            <Input
                              type="password"
                              value={accountForm.password}
                              onChange={(e) =>
                                setAccountForm({ ...accountForm, password: e.currentTarget.value })
                              }
                              style={{ width: '100%' }}
                              placeholder={selectedAccount.id !== -1 ? '留空则不修改' : ''}
                            />
                          </div>
                          <div>
                            <Label>入学时间</Label>
                            <Input
                              type="date"
                              value={accountForm.admission_date}
                              onChange={(e) =>
                                setAccountForm({ ...accountForm, admission_date: e.currentTarget.value })
                              }
                              style={{ width: '100%' }}
                            />
                          </div>
                          <div style={{ display: 'grid', gap: 8 }}>
                            <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center' }}>
                              <Label>毕业时间</Label>
                              <Switch
                                label="至今"
                                checked={accountForm.graduation_to_present}
                                onChange={(_, data) =>
                                  setAccountForm({
                                    ...accountForm,
                                    graduation_to_present: data.checked,
                                    graduation_date: data.checked ? '' : accountForm.graduation_date,
                                  })
                                }
                              />
                            </div>
                            <Input
                              type="date"
                              value={accountForm.graduation_date}
                              onChange={(e) =>
                                setAccountForm({
                                  ...accountForm,
                                  graduation_date: e.currentTarget.value,
                                  graduation_to_present: false,
                                })
                              }
                              disabled={accountForm.graduation_to_present}
                              style={{ width: '100%' }}
                            />
                          </div>
                          <div style={{ display: 'flex', gap: 16 }}>
                            <Switch
                              label="启用"
                              checked={accountForm.enable}
                              onChange={(_, data) =>
                                setAccountForm({ ...accountForm, enable: data.checked })
                              }
                            />
                            <Switch
                              label="允许同步"
                              checked={accountForm.enable_update}
                              onChange={(_, data) =>
                                setAccountForm({ ...accountForm, enable_update: data.checked })
                              }
                            />
                          </div>
                          <div style={{ display: 'flex', gap: 8, marginTop: 8 }}>
                            <Button appearance="primary" onClick={handleSaveAccount}>
                              保存
                            </Button>
                            {selectedAccount.id !== -1 && (
                              <Button
                                appearance="secondary"
                                onClick={() => handleDeleteAccount(selectedAccount.id)}
                              >
                                删除此账号
                              </Button>
                            )}
                          </div>
                        </div>
                        </div>
                      </PageEnterMotion>
                    )}
                  </>
                ) : (
                  <div
                    style={{
                      display: 'flex',
                      justifyContent: 'center',
                      alignItems: 'center',
                      height: '100%',
                      color: 'var(--colorNeutralForeground3)',
                    }}
                  >
                    <Text>请在左侧选择一个身份</Text>
                  </div>
                )}
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
      <Dialog open={editingIdentityId !== null} onOpenChange={(_, data) => !data.open && handleCancelEditIdentity()}>
        <DialogSurface style={{ maxWidth: 420 }}>
          <DialogBody>
            <DialogTitle>编辑身份信息</DialogTitle>
            <DialogContent>
              <div style={{ display: 'grid', gap: 12 }}>
                <div>
                  <Label>身份名称</Label>
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
                    style={{ width: '100%' }}
                  />
                </div>
                <Text size={200} style={{ color: 'var(--colorNeutralForeground3)' }}>
                  修改后会同步更新该身份在界面中的显示名称，不影响其下已有账号和账单数据。
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
    </>
  );
};

interface PersonAccountSectionProps {
  accountId: number;
  info: PersonAccountInfo | null;
  loading: boolean;
  error: string | null;
  onRefresh: () => void;
}

function PersonAccountSection({
  accountId,
  info,
  loading,
  error,
  onRefresh,
}: PersonAccountSectionProps) {
  return (
    <div
      onClick={(e) => e.stopPropagation()}
      style={{
        marginTop: 8,
        padding: 10,
        borderRadius: 4,
        backgroundColor: 'var(--colorNeutralBackground2)',
      }}
    >
      <div
        style={{
          display: 'flex',
          alignItems: 'center',
          justifyContent: 'space-between',
          marginBottom: 6,
        }}
      >
        <div style={{ display: 'flex', alignItems: 'center', gap: 6 }}>
          <Money24Regular style={{ color: 'var(--colorBrandForeground1)' }} />
          <Text size={200} weight="semibold">
            一卡通个人账户
          </Text>
        </div>
        {loading && <Spinner size="tiny" />}
      </div>
      {error ? (
        <MessageBar intent="error" style={{ marginBottom: 6 }}>
          <MessageBarBody>
            <Text size={200}>{error}</Text>
            <Button size="small" appearance="subtle" onClick={onRefresh}>
              点击重试
            </Button>
          </MessageBarBody>
        </MessageBar>
      ) : info ? (
        <PersonAccountFields info={info} />
      ) : (
        <div style={{ display: 'flex', alignItems: 'center', gap: 8 }}>
          <Text size={100} style={{ color: 'var(--colorNeutralForeground3)' }}>
            暂无数据
          </Text>
          <Button size="small" appearance="subtle" onClick={onRefresh} disabled={loading}>
            点击刷新
          </Button>
        </div>
      )}
    </div>
  );
}

function PersonAccountFields({ info }: { info: PersonAccountInfo }) {
  const fieldRows: { label: string; value: string }[] = [
    { label: '姓名', value: info.real_name },
    { label: '实名认证', value: info.real_name_auth_status },
    { label: '现金资金', value: info.cash_balance_raw ? `${info.cash_balance_raw} 元` : '' },
    { label: '安全保护问题', value: info.security_question_status },
    { label: '注册时间', value: info.register_date },
    { label: '学工号', value: info.student_id },
    { label: '电子邮箱', value: info.email },
    { label: '昵称', value: info.nickname },
    { label: '性别', value: info.gender },
    { label: '班级', value: info.class_name },
    { label: '手机/固话', value: info.phone_num },
    { label: '证件类型', value: info.id_type },
    { label: '证件号码', value: info.id_number },
    { label: '用户类型', value: info.user_type },
    { label: '备注', value: info.remark },
  ];
  return (
    <div style={{ display: 'grid', gap: 4 }}>
      {fieldRows
        .filter((r) => r.value && r.value.trim() !== '')
        .map((r) => (
          <div
            key={r.label}
            style={{ display: 'flex', gap: 8, alignItems: 'baseline' }}
          >
            <Text
              size={100}
              style={{
                color: 'var(--colorNeutralForeground3)',
                minWidth: 80,
                flexShrink: 0,
              }}
            >
              {r.label}:
            </Text>
            <Text size={100}>{r.value}</Text>
          </div>
        ))}
    </div>
  );
}
