import React, { useEffect, useState } from 'react';
import {
  FluentProvider,
  webLightTheme,
  webDarkTheme,
  Toaster,
} from '@fluentui/react-components';
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
import { ErrorDialog } from './ErrorDialog';

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
  const showManualCaptchaDialog = useAppStore((s) => s.showManualCaptchaDialog);
  const captchaImage = useAppStore((s) => s.captchaImage);
  const captchaExecution = useAppStore((s) => s.captchaExecution);
  const setShowManualCaptchaDialog = useAppStore((s) => s.setShowManualCaptchaDialog);

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

  const fluentTheme = theme === 'dark' ? webDarkTheme : webLightTheme;

  return (
    <FluentProvider theme={fluentTheme}>
      <AppLayout />
      <Toaster />

      {/* Modal Dialogs */}
      {showStartupDialog && <StartupPasswordDialog />}
      {showIdentitySelectDialog && <IdentitySelectDialog />}
      {showIdentityManagerDialog && <IdentityManagerDialog />}
      {showSettingsDialog && <SettingsDialog />}
      {showAboutDialog && <AboutDialog />}
      {showCaptchaTestDialog && <CaptchaTestDialog />}
      {showDataTransferDialog && <DataTransferDialog />}
      {showStatisticsDialog && <StatisticsDialog />}
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
