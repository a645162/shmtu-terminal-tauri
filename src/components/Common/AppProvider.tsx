import React, { useEffect, useState } from 'react';
import {
  FluentProvider,
  webLightTheme,
  webDarkTheme,
  Toaster,
} from '@fluentui/react-components';
import { listen } from '@tauri-apps/api/event';
import { useAppStore } from '../../stores/appStore';
import { AppLayout } from '../Layout/AppLayout';
import { StartupPasswordDialog } from './StartupPasswordDialog';
import { IdentitySelectDialog } from './IdentitySelectDialog';
import { IdentityManagerDialog } from '../../pages/IdentityManager/IdentityManagerDialog';
import { SettingsDialog } from '../../pages/Settings/SettingsDialog';
import { AboutDialog } from '../../pages/About/AboutDialog';
import { CaptchaTestDialog } from '../../pages/CaptchaTest/CaptchaTestDialog';
import { DataTransferDialog } from '../../pages/DataTransfer/DataTransferDialog';
import { StatisticsDialog } from '../../pages/Statistics/StatisticsDialog';
import { ManualCaptchaDialog } from './ManualCaptchaDialog';
import { SyncRangeDialog } from './SyncRangeDialog';
import { ErrorDialog } from './ErrorDialog';
import { SyncStatusPanel } from './SyncStatusPanel';
import { GlobalContextMenuGuard } from './GlobalContextMenuGuard';
import type { SyncProgress } from '../../types';

export const AppProvider: React.FC = () => {
  const theme = useAppStore((s) => s.theme);
  const loadIdentities = useAppStore((s) => s.loadIdentities);
  const loadConfig = useAppStore((s) => s.loadConfig);
  const activateIdentity = useAppStore((s) => s.activateIdentity);
  const currentIdentity = useAppStore((s) => s.currentIdentity);
  const identities = useAppStore((s) => s.identities);
  const setShowStartupDialog = useAppStore((s) => s.setShowStartupDialog);
  const setShowIdentitySelectDialog = useAppStore((s) => s.setShowIdentitySelectDialog);
  const config = useAppStore((s) => s.config);
  const [isBootstrapped, setIsBootstrapped] = useState(false);
  const [isStartupUnlocked, setIsStartupUnlocked] = useState(false);
  const [hasRequestedStartupDialog, setHasRequestedStartupDialog] = useState(false);

  useEffect(() => {
    const init = async () => {
      await loadConfig();
      await loadIdentities();
      setIsBootstrapped(true);
    };
    init().catch(console.error);
  }, [loadConfig, loadIdentities]);

  // Enable startup protection if configured
  useEffect(() => {
    if (config?.security?.enable_startup_protection) {
      setIsStartupUnlocked(false);
      setHasRequestedStartupDialog(true);
      setShowStartupDialog(true);
    } else if (config) {
      setHasRequestedStartupDialog(false);
      setIsStartupUnlocked(true);
    }
  }, [config, setShowStartupDialog]);

  const showStartupDialog = useAppStore((s) => s.showStartupDialog);
  const showIdentitySelectDialog = useAppStore((s) => s.showIdentitySelectDialog);
  const showIdentityManagerDialog = useAppStore((s) => s.showIdentityManagerDialog);
  const showSettingsDialog = useAppStore((s) => s.showSettingsDialog);
  const showAboutDialog = useAppStore((s) => s.showAboutDialog);
  const showCaptchaTestDialog = useAppStore((s) => s.showCaptchaTestDialog);
  const showDataTransferDialog = useAppStore((s) => s.showDataTransferDialog);
  const showStatisticsDialog = useAppStore((s) => s.showStatisticsDialog);
  const showSyncRangeDialog = useAppStore((s) => s.showSyncRangeDialog);
  const showManualCaptchaDialog = useAppStore((s) => s.showManualCaptchaDialog);
  const captchaImage = useAppStore((s) => s.captchaImage);
  const captchaExecution = useAppStore((s) => s.captchaExecution);
  const setShowManualCaptchaDialog = useAppStore((s) => s.setShowManualCaptchaDialog);
  const setSyncProgress = useAppStore((s) => s.setSyncProgress);
  const clearSyncProgress = useAppStore((s) => s.clearSyncProgress);
  const syncProgress = useAppStore((s) => s.syncProgress);

  useEffect(() => {
    if (config?.security?.enable_startup_protection && hasRequestedStartupDialog && !showStartupDialog) {
      setIsStartupUnlocked(true);
    }
  }, [config, hasRequestedStartupDialog, showStartupDialog]);

  useEffect(() => {
    if (!isBootstrapped || !config || !isStartupUnlocked || currentIdentity) {
      return;
    }

    const enabledIdentities = identities.filter((identity) => identity.enable);
    if (enabledIdentities.length === 0) {
      setShowIdentitySelectDialog(true);
      return;
    }

    if (enabledIdentities.length === 1) {
      setShowIdentitySelectDialog(false);
      activateIdentity(enabledIdentities[0]).catch(console.error);
      return;
    }

    const startupIdentityId = config.identity.remember_default
      ? config.identity.default_identity_id
      : config.identity.last_identity_id;
    const startupIdentity = enabledIdentities.find((identity) => identity.id === startupIdentityId);

    if (startupIdentity) {
      setShowIdentitySelectDialog(false);
      activateIdentity(startupIdentity).catch(console.error);
    } else {
      setShowIdentitySelectDialog(true);
    }
  }, [
    activateIdentity,
    config,
    currentIdentity,
    identities,
    isBootstrapped,
    isStartupUnlocked,
    setShowIdentitySelectDialog,
  ]);

  useEffect(() => {
    let disposed = false;
    let unlisten: (() => void) | undefined;

    listen<SyncProgress>('sync-progress', (event) => {
      if (disposed) {
        return;
      }

      const progress = event.payload;
      setSyncProgress({
        ...progress,
        message:
          progress.message ??
          (progress.status === 'captcha_required'
              ? progress.error ?? '需要输入验证码以继续'
              : progress.status === 'completed'
                ? `同步完成，本次新增 ${progress.new_items} 条记录`
                : undefined),
      });
    })
      .then((fn) => {
        unlisten = fn;
      })
      .catch(console.error);

    return () => {
      disposed = true;
      if (unlisten) {
        void unlisten();
      }
    };
  }, [setSyncProgress]);

  useEffect(() => {
    if (syncProgress?.status !== 'completed') {
      return;
    }

    const timer = window.setTimeout(() => {
      clearSyncProgress();
    }, 3200);

    return () => window.clearTimeout(timer);
  }, [clearSyncProgress, syncProgress]);

  const fluentTheme = theme === 'dark' ? webDarkTheme : webLightTheme;

  return (
    <FluentProvider theme={fluentTheme}>
      <GlobalContextMenuGuard />
      <AppLayout />
      <div id="app-context-menu-portal" />
      <Toaster />
      <SyncStatusPanel />

      {/* Modal Dialogs */}
      {showStartupDialog && <StartupPasswordDialog />}
      {showIdentitySelectDialog && <IdentitySelectDialog />}
      {showIdentityManagerDialog && <IdentityManagerDialog />}
      {showSettingsDialog && <SettingsDialog />}
      {showAboutDialog && <AboutDialog />}
      {showCaptchaTestDialog && <CaptchaTestDialog />}
      {showDataTransferDialog && <DataTransferDialog />}
      {showStatisticsDialog && <StatisticsDialog />}
      {showSyncRangeDialog && <SyncRangeDialog />}
      {showManualCaptchaDialog && captchaImage && captchaExecution && (
        <ManualCaptchaDialog
          captchaImage={captchaImage}
          execution={captchaExecution}
          onSuccess={() => setShowManualCaptchaDialog(false)}
          onCancel={() => setShowManualCaptchaDialog(false)}
        />
      )}
      <ErrorDialog />
    </FluentProvider>
  );
};
