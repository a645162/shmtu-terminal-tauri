import React, { useState } from 'react';
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
} from '@fluentui/react-components';
import { PersonAdd24Regular, Delete24Regular, Edit24Regular } from '@fluentui/react-icons';
import { useAppStore } from '../../stores/appStore';
import type { Identity, Account } from '../../types';
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

  const [selectedIdentity, setSelectedIdentity] = useState<Identity | null>(null);
  const [selectedAccount, setSelectedAccount] = useState<Account | null>(null);
  const [accounts, setAccounts] = useState<Account[]>([]);
  const [newIdentityName, setNewIdentityName] = useState('');
  const [isAddingIdentity, setIsAddingIdentity] = useState(false);
  const [editingIdentityId, setEditingIdentityId] = useState<number | null>(null);
  const [editingIdentityName, setEditingIdentityName] = useState('');

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
        await tauri.update_identity({ ...identity, name: editingIdentityName.trim() });
        loadIdentities();
        if (selectedIdentity?.id === editingIdentityId) {
          setSelectedIdentity({ ...identity, name: editingIdentityName.trim() });
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
                              <Text size={100} style={{ color: 'var(--colorNeutralForeground3)' }}>
                                学号: {account.account_id} | {account.enable ? '已启用' : '已禁用'}
                              </Text>
                              <Text size={100} style={{ color: 'var(--colorNeutralForeground3)' }}>
                                学籍: {account.admission_date || '未设置'} 至 {account.graduation_date || '至今'}
                              </Text>
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
